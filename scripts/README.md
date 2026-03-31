# Scripts utilitaires MPForge

> Scripts pour faciliter la gestion des releases, tags Git et le pipeline BDTOPO → Garmin

## Pipeline complet

```
download-bdtopo.sh → build-garmin-map.sh → gmapsupp.img
       ↓                     ↓                    ↓
  Télécharge les       mpforge build          Carte Garmin
  données BDTOPO       (tuiles .mp)           prête à copier
  depuis l'IGN         + imgforge build       sur le GPS
                       (gmapsupp.img)
```

---

## Scripts disponibles

### download-bdtopo.sh — Téléchargement BD TOPO® IGN

**Usage** :
```bash
# Département unique
./scripts/download-bdtopo.sh --zones D038 --format SHP

# Région entière (fichier agrégé)
./scripts/download-bdtopo.sh --region ARA --format SHP

# Dry-run (simulation sans téléchargement)
./scripts/download-bdtopo.sh --zones D038 --dry-run

# Avec debug
./scripts/download-bdtopo.sh --zones D038 --debug
```

**Description** :
- Interroge l'API Géoplateforme (`data.geopf.fr`) pour découvrir les datasets BDTOPO disponibles
- Auto-détecte la dernière édition trimestrielle disponible
- Télécharge l'archive `.7z` avec reprise automatique (`curl -C -`)
- Vérifie le hash MD5 des fichiers téléchargés
- Extrait les dossiers thématiques Shapefile (ADMINISTRATIF, BATI, HYDROGRAPHIE, …)
- Organise les données dans `pipeline/data/bdtopo/{YYYY}/v{YYYY.MM}/{DXXX}/`
- Idempotent : skip les fichiers déjà téléchargés et intacts (MD5 OK)
- Supporte les régions pré-agrégées (`--region ARA`, `FXX` pour la France métro)

**Prérequis** :
```bash
sudo apt install curl p7zip-full
```

---

### build-garmin-map.sh — Pipeline mpforge → imgforge → gmapsupp.img

**Usage** :
```bash
# Auto-découverte de tout (données dans ./pipeline/data/bdtopo/)
./scripts/build-garmin-map.sh

# Spécifier la racine des données (ex: un seul département)
./scripts/build-garmin-map.sh --data-root pipeline/data/bdtopo/2025/v2025.12/D038

# Avec un fichier config YAML explicite
./scripts/build-garmin-map.sh --config pipeline/configs/france-bdtopo.yaml --jobs 16

# Dry-run (affiche les commandes sans les exécuter)
./scripts/build-garmin-map.sh --dry-run

# Reprise partielle (skip les tuiles déjà générées)
./scripts/build-garmin-map.sh --skip-existing --jobs 4
```

**Description** :
- Enchaîne automatiquement les deux étapes du build :
  1. **mpforge build** — Lit les Shapefiles BDTOPO, applique les règles de conversion (`bdtopo-garmin-rules.yaml`) et génère des tuiles `.mp` (Polish Map Format)
  2. **imgforge build** — Compile les tuiles `.mp` en un fichier `gmapsupp.img` prêt pour le GPS Garmin
- Auto-découverte des binaires `mpforge` et `imgforge` (binaires compilés ou fallback `cargo run`)
- Auto-découverte du fichier de règles `bdtopo-garmin-rules.yaml`
- Génération dynamique de la config YAML mpforge depuis `DATA_ROOT` (21 couches BDTOPO)
- Supporte aussi une config YAML explicite avec placeholders `${DATA_ROOT}` (substitution via `envsubst`)
- Produit un rapport JSON par étape (`mpforge-report.json`, `imgforge-report.json`)
- Affiche un résumé final avec métriques (tuiles, temps, taille, routage)
- Gestion des échecs partiels (`error_handling: continue`) — exit code 2 si carte incomplète

**Options principales** :

| Option | Description | Défaut |
|---|---|---|
| `--data-root DIR` | Racine des données BDTOPO | `./pipeline/data/bdtopo` |
| `--config FILE` | Config YAML mpforge explicite | génération auto |
| `--rules FILE` | Fichier de règles YAML | auto-découverte |
| `--jobs N` | Parallélisation | `8` |
| `--output DIR` | Répertoire de sortie | `./pipeline/output` |
| `--family-id N` | Family ID Garmin | `6324` |
| `--description STR` | Description de la carte | `"BDTOPO Garmin"` |
| `--typ FILE` | Fichier TYP styles personnalisés | *(aucun)* |
| `--skip-existing` | Passer les tuiles déjà générées | `false` |
| `--dry-run` | Simuler sans exécuter | `false` |
| `-v`, `-vv` | Mode verbeux | off |

**Structure de sortie** :
```
./pipeline/output/
├── tiles/               ← tuiles .mp générées par mpforge
├── gmapsupp.img         ← carte Garmin finale
├── mpforge-report.json  ← rapport mpforge (métriques, erreurs)
└── imgforge-report.json ← rapport imgforge (métriques, routage)
```

**Prérequis** :
```bash
# Compiler les outils Rust
cd tools/mpforge && cargo build --release
cd tools/imgforge && cargo build --release
```

**Exemple complet** (après téléchargement BDTOPO Isère) :
```bash
# Avec variables d'environnement explicites
PROJ_DATA=/usr/share/proj \
  ./scripts/build-garmin-map.sh \
  --data-root pipeline/data/bdtopo/2025/v2025.12/D038 \
  --skip-existing \
  --jobs 4
```

---

### check_environment.sh — Vérification de l'environnement de développement

**Usage** :
```bash
./scripts/check_environment.sh
```

**Description** :
- Vérifie la présence et les versions de tous les outils requis :
  - Outils de build (GCC, CMake, Make, pkg-config)
  - GDAL et dépendances géospatiales (GDAL, OGR, PROJ)
  - Rust toolchain (rustc, cargo, clippy, rustfmt)
  - Python et QGIS (Python 3, PyQGIS)
  - Outils optionnels (Java, mkgmap, splitter, Git, Doxygen)
  - Variables d'environnement (GDAL_DATA, PROJ_DATA, etc.)
  - Structure du projet
- Affiche un résumé coloré avec compteurs de succès/échecs/avertissements

**Cas d'usage** :
- Valider un nouvel environnement de développement
- Diagnostiquer des problèmes de build

---

### test-static-build.sh — Validation du build statique mpforge

**Usage** :
```bash
./scripts/test-static-build.sh <mpforge-linux-x64-static.tar.gz> [test-config.yaml]
```

**Description** :
- Valide qu'une archive de build statique mpforge est correctement empaquetée :
  1. Extraction et vérification de la structure (binaire, wrapper, proj.db)
  2. Test du binaire sans `PROJ_DATA` (doit échouer correctement)
  3. Test avec `PROJ_DATA` manuel
  4. Test du wrapper `mpforge.sh` (auto-configure `PROJ_DATA`)
  5. Test fonctionnel optionnel avec un fichier config
  6. Statistiques de taille

**Cas d'usage** :
- Valider une release CI/CD avant publication
- Vérifier la portabilité du binaire statique

---

### release.sh — Créer une release complète

**Usage** :
```bash
./scripts/release.sh v0.1.0
```

**Description** :
- Vérifie que vous êtes sur `main`
- Vérifie qu'il n'y a pas de changements non commités
- Vérifie que le tag n'existe pas déjà
- Pull pour synchroniser
- Demande un message de release interactif
- Crée et push le tag

**Cas d'usage** :
- Créer une nouvelle release de façon sécurisée
- Workflow de release complet avec validation

---

### retag.sh — Forcer un tag existant

**Usage** :
```bash
./scripts/retag.sh v0.1.0           # Retag current HEAD
./scripts/retag.sh v0.1.0 abc123    # Retag specific commit
```

**Description** :
- Supprime le tag local et distant
- Re-crée le tag sur le commit spécifié (ou HEAD)
- Push le nouveau tag
- Déclenche automatiquement le workflow Woodpecker

**Cas d'usage** :
- Corriger un workflow qui a échoué
- Mettre à jour une release avec un nouveau commit

---

## Documentation complète

Voir la section **[CI/CD : Woodpecker CI](../README.md#cicd--woodpecker-ci)** du README principal pour :
- Guide complet de gestion des tags et releases
- Bonnes pratiques
- Commandes de référence

---

## Installation

Les scripts sont déjà exécutables. Si nécessaire :

```bash
chmod +x scripts/*.sh
```

---

## Important

- `retag.sh` et `release.sh` modifient l'historique Git (tags). Utilisez-les avec précaution en production.
- `build-garmin-map.sh` peut consommer beaucoup de CPU/RAM selon le nombre de départements. Ajustez `--jobs` en fonction de votre machine.

**Recommandation** : Testez d'abord sur une branche de développement ou un tag de test.
