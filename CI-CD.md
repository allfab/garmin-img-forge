<h1 align="center">CI/CD — Garmin IMG Forge</h1>

<h4 align="center">Plomberie d'intégration et de release : Woodpecker côté infra interne, GitHub Actions en appoint sur le miroir public.</h4>

<p align="center">
  <a href="./README.md">← Retour au README</a>
</p>

---

Le projet utilise **Woodpecker CI** comme plateforme principale (légère, intégration Docker native, YAML simple) sur l'infra interne. GitHub Actions joue un rôle d'appoint sur le miroir public.

> **Note miroir GitHub** : les fichiers `.woodpecker/*.yml` ne sont pas miroirés côté GitHub (dossier exclu du filtrage `git filter-repo`). Les descriptions ci-dessous documentent le système à titre informatif.

## Sommaire

- [Pipelines Woodpecker](#pipelines-woodpecker-canoniques)
- [Workflows GitHub Actions](#workflows-github-actions-appoint-côté-miroir)
- [Architecture du build statique `mpforge`](#architecture-du-build-statique-mpforge)
- [Configuration initiale Woodpecker](#configuration-initiale-woodpecker)
- [Versioning automatique](#versioning-automatique)
- [Créer une release](#créer-une-release)
- [Remplacer / supprimer un tag](#remplacer-un-tag-re-déclencher-un-build)
- [Référence rapide](#référence-rapide-des-commandes)
- [Semantic Versioning](#semantic-versioning)

---

## Pipelines Woodpecker (canoniques)

| Pipeline                        | Déclencheur                        | Description |
|---------------------------------|------------------------------------|-------------|
| `.woodpecker/mpforge.yml`       | Tag `mpforge-v*`                   | Build statique Linux x64 (GDAL + GEOS + PROJ + driver PolishMap intégrés) |
| `.woodpecker/imgforge.yml`      | Tag `imgforge-v*`                  | Build standard Linux x64 (Pure Rust, zéro dépendance native) |
| `.woodpecker/site.yml`          | Push sur `main` (dans `site/`)     | Build et déploiement LXC du site Zensical |
| `.woodpecker/mirror-github.yml` | Push sur `main`                    | Miroir filtré Forgejo → GitHub (`git filter-repo`) |

Les pipelines `mpforge` et `imgforge` produisent automatiquement une **release Forgejo** avec binaire, checksums SHA-256 et métadonnées JSON.

## Workflows GitHub Actions (appoint, côté miroir)

| Workflow                                   | Déclencheur             | Description |
|--------------------------------------------|-------------------------|-------------|
| `.github/workflows/pages.yml`              | Push sur `main`         | Build Zensical + déploiement GitHub Pages |
| `.github/workflows/release-republish.yml`  | Tag `mpforge-v*` / `imgforge-v*` | Téléchargement des binaires depuis la release Forgejo et republication en release GitHub |

Le workflow `release-republish` attend que Forgejo ait fini son build (poll API toutes les 2 min, timeout 25 min), télécharge les assets, vérifie les SHA-256, puis crée la release GitHub équivalente. **Aucune recompilation** côté GitHub — le build serveur (~20 min pour `mpforge` avec GDAL statique) n'est exécuté qu'une seule fois, sur l'infra interne.

## Architecture du build statique `mpforge`

```
Tag mpforge-v* poussé --> Woodpecker CI déclenche mpforge.yml
  Phase 1  : Installation dépendances (cmake, pkg-config, sqlite3)
  Phase 2  : Compilation PROJ 9.3.1 statique
  Phase 3  : Compilation GEOS 3.13.0 statique
  Phase 4  : Téléchargement GDAL 3.10.1
  Phase 5  : Intégration driver PolishMap dans l'arborescence GDAL
  Phase 6  : Configuration GDAL statique (avec PROJ + GEOS)
  Phase 7  : Compilation et installation GDAL
  Phase 8  : Configuration Rust (GDAL_STATIC=1, pkg-config)
  Phase 9  : Copie proj.db dans resources/
  Phase 10 : Compilation mpforge (proj.db embarqué via include_bytes!)
  Phase 11 : Vérification (ldd, taille, test --version)
  Phase 12 : Package + checksums + upload release Forgejo
```

Le binaire produit est **100% autonome** : aucune dépendance externe, `proj.db` embarqué.

> **Troubleshooting `proj.db`** : Si `proj_create_from_database: Cannot find proj.db` apparaît, c'est que PROJ ne trouve pas sa base de données. En production, ce problème est résolu par l'embarquement de `proj.db` directement dans le binaire (extraction automatique dans un tempdir au démarrage). En développement local, positionner `PROJ_DATA` vers le répertoire contenant `proj.db` (typiquement `/usr/share/proj`).

## Configuration initiale Woodpecker

Pour activer le CI sur un nouveau dépôt :

1. Se connecter à l'instance Woodpecker interne
2. Activer le dépôt dans **Settings > Repositories**
3. Créer un secret `forgejo_token` dans **Settings > Secrets** (token API Forgejo avec droits `write:package`)
4. Le webhook Forgejo → Woodpecker est créé automatiquement

## Versioning automatique

La version affichée par `--version` est dérivée du tag Git via `build.rs` dans chaque crate. Les préfixes de tag (`mpforge-`, `imgforge-`) sont automatiquement strippés :

```
Sur un tag       : mpforge v1.0.0    (tag mpforge-v1.0.0)
                   imgforge v0.1.0   (tag imgforge-v0.1.0)
Entre deux tags  : mpforge v1.0.0-3-g1a2b3c4
Dirty            : mpforge v1.0.0-dirty
```

Fallbacks : `CI_COMMIT_TAG` (strip préfixe) > `git describe --tags` > `git rev-parse --short HEAD` > `CARGO_PKG_VERSION`.

## Créer une release

Les tags sont préfixés par le nom de l'outil pour permettre des cycles de release indépendants :

```bash
# 1. Vérifier que tout est propre
git status
git push

# 2. Release mpforge (~15-20 min de build)
git tag -a mpforge-v1.0.0 -m "Release mpforge v1.0.0"
git push origin mpforge-v1.0.0

# 3. Release imgforge (~2-3 min de build)
git tag -a imgforge-v0.1.0 -m "Release imgforge v0.1.0"
git push origin imgforge-v0.1.0

# 4. Surveiller le build sur l'instance Woodpecker interne
```

## Remplacer un tag (re-déclencher un build)

```bash
# Méthode propre : supprimer puis recréer
git tag -d mpforge-v1.0.0
git push --delete origin mpforge-v1.0.0
# Supprimer aussi la release dans Forgejo UI si elle existe

git tag -a mpforge-v1.0.0 -m "Release mpforge v1.0.0 (corrected)"
git push origin mpforge-v1.0.0
```

Ou plus simplement, créer un patch : `git tag -a mpforge-v1.0.1 -m "Fix for v1.0.0"`.

## Supprimer un tag

```bash
# Local + remote
git tag -d mpforge-v1.0.0
git push --delete origin mpforge-v1.0.0
```

> ⚠ Supprimer un tag ne supprime **pas** la release Forgejo ni la release GitHub. Il faut les supprimer manuellement via l'UI ou l'API.

## Référence rapide des commandes

| Action                | Commande                                                       |
|-----------------------|----------------------------------------------------------------|
| Release mpforge       | `git tag -a mpforge-v1.0.0 -m "Release mpforge v1.0.0"`        |
| Release imgforge      | `git tag -a imgforge-v0.1.0 -m "Release imgforge v0.1.0"`      |
| Pousser tag           | `git push origin mpforge-v1.0.0`                               |
| Lister tags par outil | `git tag -l 'mpforge-v*'` / `git tag -l 'imgforge-v*'`         |
| Voir détails tag      | `git show mpforge-v1.0.0`                                      |
| Supprimer tag local   | `git tag -d mpforge-v1.0.0`                                    |
| Supprimer tag remote  | `git push --delete origin mpforge-v1.0.0`                      |
| Fetch tags forcés     | `git fetch --tags --force`                                     |

## Semantic Versioning

```
vMAJOR.MINOR.PATCH

v0.1.0 -> v0.1.1  : Bug fix
v0.1.1 -> v0.2.0  : Nouvelle feature (compatible)
v0.2.0 -> v1.0.0  : Breaking change
```

Un helper `scripts/release-tool.sh` automatise bump + tag + push (voir son `--help`).
