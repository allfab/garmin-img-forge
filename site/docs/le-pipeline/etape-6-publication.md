# Étape 6 — Publication

Une fois le `gmapsupp.img` compilé (étape 4), deux cibles de publication sont possibles via l'option `--publish` de `scripts/build-garmin-map.sh` :

| Cible | Variable | Destination | Usage |
|---|---|---|---|
| **local** *(défaut)* | `PUBLISH_TARGET=local` | `site/docs/telechargements/files/` | Intégration directe dans le site MkDocs |
| **s3** | `PUBLISH_TARGET=s3` | Bucket Garage S3 via `rclone` | Hébergement externalisé (volumineux) |

Dans les deux cas, le script met à jour `site/docs/telechargements/manifest.json` (catalogue des versions publiées) et patche les pages Markdown concernées (liens de téléchargement + SHA256).

---

## Cible `local`

```bash
./scripts/build-garmin-map.sh --region ARA --publish
```

Comportement par défaut si `PUBLISH_TARGET` n'est pas défini. Copie l'`.img` directement dans `site/docs/telechargements/files/<coverage>/<version>/`.

---

## Publier sans rebuilder

Quand une carte a déjà été validée (tests sur GPS, inspection QmapShack...) et qu'on veut seulement la republier ou la pousser vers une autre cible, il suffit de relancer le script avec `--skip-existing` et la cible voulue :

```bash
./scripts/build-garmin-map.sh \
    --region FRANCE-SE \
    --version v2026.03 \
    ... \
    --skip-existing \
    --publish \
    --publish-target s3
```

Grâce à `--skip-existing` :

- **Phase 1 mpforge** : les `.mp` existants sont conservés (un tour rapide sur le scan d'extents, ~1-4 min).
- **Phase 2 imgforge** : si le `.img` cible existe déjà dans `pipeline/output/<...>/img/`, **le rebuild est entièrement skippé** (pas de suppression du fichier, pas de recompilation).
- **Publication** : s'exécute normalement avec l'`.img` déjà présent.

C'est particulièrement utile pour :

- Basculer d'une publication `local` à `s3` (ou inverse) après validation.
- Re-déclencher `update_manifest` et le patch de pages MkDocs après une modification éditoriale.
- Pousser le même build vers plusieurs buckets S3 (en variant `PUBLISH_TARGET` / `S3_BUCKET`).

!!! warning "Cohérence des paramètres"
    `--skip-existing` ne vérifie que la présence du fichier cible, pas ses paramètres de build.
    Si vous avez changé `--family-name`, `--base-id`, les options géométrie ou la config YAML
    depuis le dernier build, **supprimez le `.img` et les `.mp` concernés** pour forcer le rebuild.

---

## Cible `s3` (Garage)

### Configuration `.env`

```dotenv
PUBLISH_TARGET=s3

# rclone remote "garage:" via variables d'env (pas de rclone.conf)
RCLONE_CONFIG_GARAGE_TYPE=s3
RCLONE_CONFIG_GARAGE_PROVIDER=Other
RCLONE_CONFIG_GARAGE_ACCESS_KEY_ID=<access-key>
RCLONE_CONFIG_GARAGE_SECRET_ACCESS_KEY=<secret-key>
RCLONE_CONFIG_GARAGE_ENDPOINT=https://garage-api.example.com
RCLONE_CONFIG_GARAGE_REGION=garage
RCLONE_CONFIG_GARAGE_ACL=public-read

S3_BUCKET=<nom-du-bucket>
PUBLIC_URL_BASE=https://download-maps.example.com
```

Les 5 variables requises (`RCLONE_CONFIG_GARAGE_ACCESS_KEY_ID`, `_SECRET_ACCESS_KEY`, `_ENDPOINT`, `S3_BUCKET`, `PUBLIC_URL_BASE`) sont validées au démarrage du script ; absence = erreur avant build.

### Prérequis infrastructure

Le serveur Garage expose **deux endpoints** distincts :

| Port | Rôle | Exposé à |
|---|---|---|
| **3900** | API S3 (signée AWS v4) | Uploads authentifiés via `rclone` |
| **3902** | Website (lecture HTTP anonyme) | Clients finaux téléchargeant les `.img` |

Le bucket doit avoir **Website Access** activé et un alias cohérent avec `root_domain` :

```bash
garage bucket website --allow <nom-du-bucket> --index-document index.html
garage bucket alias <bucket-id> <nom-du-bucket>
```

### Reverse proxy (exemple Caddy)

```caddy
# API S3 — upload
garage-api.example.com {
    reverse_proxy http://<garage-host>:3900
}

# Website — download public
download-maps.example.com {
    reverse_proxy http://<garage-host>:3902 {
        header_up Host "<nom-du-bucket>.<root_domain>"
    }
}
```

Le `Host` réécrit est crucial : Garage route par virtual-host `<bucket>.<root_domain>` tel que défini dans `garage.toml` (`[s3_web] root_domain`).

### Firewall

Le port **3902** doit être ouvert entre le reverse proxy et Garage (en plus de 3900 pour l'API). Sur Proxmox LXC, éditer `/etc/pve/firewall/<VMID>.fw` :

```
[RULES]
IN ACCEPT -source +dc/<ipset> -p tcp -dport 3900 -log nolog # GARAGE S3 API
IN ACCEPT -source +dc/<ipset> -p tcp -dport 3902 -log nolog # GARAGE S3 Website
IN ACCEPT -source +dc/<ipset> -p tcp -dport 3909 -log nolog # GARAGE Dashboard
```

### Test de connexion

Avant le premier `--publish`, valider la chaîne complète avec :

```bash
./scripts/test-s3-connection.sh
```

5 tests séquentiels : variables d'env, `rclone lsd garage:`, listing du bucket, aller-retour upload/lecture/SHA256/delete, et requête HTTP publique vers `PUBLIC_URL_BASE`. Le passage `-v` active `rclone -vv` pour le dump des headers signés (utile pour diagnostiquer 403).

Code HTTP attendu sur `PUBLIC_URL_BASE/` : **404** (bucket vide, pas d'`index.html`) → c'est un succès fonctionnel, ça prouve que Caddy et Garage website communiquent.

---

## Rétention

Script `scripts/prune-s3.sh` — supprime les anciennes versions du bucket tout en gardant les N plus récentes par coverage :

```bash
./scripts/prune-s3.sh --dry-run             # simulation (tout afficher)
./scripts/prune-s3.sh --keep 3              # garder 3 versions/coverage
./scripts/prune-s3.sh --coverage departement/d038 --keep 2
```

Met à jour `manifest.json` en conséquence (le commit git reste manuel).

---

## Manifest

Le fichier `site/docs/telechargements/manifest.json` centralise toutes les versions publiées (toutes cibles confondues). Il est consommé côté site par `site/docs/javascripts/downloads-manifest.js` pour afficher les cartes disponibles et leurs checksums.

Structure :

```json
{
  "coverages": {
    "departement/d038": {
      "latest": "v2025.12",
      "latest_url": "https://download-maps.example.com/departement/d038/v2025.12/IGN-BDTOPO-D038-v2025.12.img",
      "versions": [
        { "version": "v2025.12", "path": "...", "sha256": "...", "size": 123456 }
      ]
    }
  }
}
```
