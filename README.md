<h1 align="center">
  <br>
  Garmin IMG Forge
  <br>
</h1>

<h4 align="center">Du SIG au GPS en une ligne de commande.<br/>Des cartes Garmin <code>.img</code> <strong>professionnelles</strong>, pilotées par de simples fichiers <strong>YAML</strong>.</h4>

<p align="center">
  <a href="https://imgforge.garmin.allfabox.fr/" target="_blank"><img src="https://img.shields.io/badge/Site-imgforge.garmin.allfabox.fr-526CFE?style=for-the-badge&logoColor=white" /></a>
  <a href="https://www.rust-lang.org/" target="_blank"><img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" /></a>
  <a href="https://gdal.org/" target="_blank"><img src="https://img.shields.io/badge/GDAL-5CAE58?style=for-the-badge&logo=osgeo&logoColor=white" /></a>
  <a href="https://woodpecker-ci.org/" target="_blank"><img src="https://img.shields.io/badge/Woodpecker_CI-4CAF50?style=for-the-badge&logo=woodpeckerci&logoColor=white" /></a>
  <a href="./LICENSE" target="_blank"><img src="https://img.shields.io/badge/Licence-GPL_v3_%2F_MIT-blue?style=for-the-badge" /></a>
</p>

<p align="center">
  <a href="#-pourquoi-garmin-img-forge-">Pourquoi</a> •
  <a href="#-ta-première-carte-en-5-minutes">Démarrage rapide</a> •
  <a href="#-monter-un-pipeline-de-production">Pipeline de production</a> •
  <a href="#-galerie">Galerie</a> •
  <a href="#-liens--ressources">Liens</a>
</p>

<p align="center">
  <img src="site/docs/assets/images/readme/hero-pipeline.svg" alt="Pipeline Garmin IMG Forge : SIG → mpforge → imgforge → GPS" width="100%"/>
</p>

> 🪞 **Miroir public en lecture.** Ce dépôt GitHub est un clone miroir filtré d'un dépôt Forgejo interne. Issues et PR GitHub sont bienvenues mais mergées côté source (voir [CONTRIBUTING.md](./CONTRIBUTING.md)).

---

## ✨ Pourquoi Garmin IMG Forge ?

Tu as des données SIG (BDTOPO, OpenStreetMap, un shapefile métier, une couche cadastre…) et tu veux les voir sur ton **Edge, eTrex, Oregon ou GPSMAP** — sans clic, sans usine à gaz, sans dépendre d'un outil propriétaire.

**Garmin IMG Forge** est une chaîne d'outils open source qui transforme **n'importe quelle source SIG** en **carte Garmin** prête à copier sur ton GPS, **via de simples fichiers YAML déclaratifs**.

| 🧬 **Déclaratif**       | Un YAML décrit ta carte. L'outil forge le reste. Versionne, diff, rejoue. |
|-------------------------|----------------------------------------------------------------------------|
| ⚡ **Zéro friction**    | `mpforge` embarque GDAL + PROJ + GEOS en statique. `imgforge` est **Pure Rust**. Un seul binaire, zéro dépendance système. |
| 🎯 **Multi-zoom natif** | Profils de simplification conditionnels (`CL_ADMIN`, `IMPORTANCE`…) — jusqu'à **10 géométries par feature** selon le zoom. Absent de `mkgmap`. |
| 🏗️ **Production-ready** | Pipeline CI complet, releases reproductibles, métadonnées signées SHA-256. |
| 🆓 **Libre**            | GPL v3 / MIT. Tes données, tes règles, ton infra. |

### Les briques

| Outil                                             | Rôle                                                   | Langage         | Licence |
|---------------------------------------------------|--------------------------------------------------------|-----------------|---------|
| [`ogr-polishmap`](./tools/ogr-polishmap/)         | Driver GDAL/OGR pour le format Polish Map (`.mp`)      | C++             | MIT     |
| [`mpforge`](./tools/mpforge/)                     | SIG → tuiles `.mp` (règles YAML, multi-zoom)           | Rust + GDAL     | GPL v3  |
| [`imgforge`](./tools/imgforge/)                   | `.mp` → Garmin `.img` (remplace `cGPSmapper`)          | Rust pur        | GPL v3  |
| [`ogr-garminimg`](./tools/ogr-garminimg/) *(WIP)* | Driver GDAL/OGR pour **lire** les `.img` (diagnostic)  | C++             | —       |

---

## 🚀 Ta première carte en 5 minutes

> **Objectif :** produire un `gmapsupp.img` depuis un exemple YAML fourni, et le copier sur ton GPS.

### 1. Récupère les binaires *(ou compile depuis les sources — voir [pré-requis](#-pré-requis-détaillés))*

```bash
# Binaires statiques Linux x64 publiés à chaque release
curl -LO https://github.com/allfab/garmin-img-forge/releases/latest/download/mpforge
curl -LO https://github.com/allfab/garmin-img-forge/releases/latest/download/imgforge
chmod +x mpforge imgforge
```

### 2. Prends un exemple YAML livré avec le dépôt

```bash
git clone https://github.com/allfab/garmin-img-forge.git
cd garmin-img-forge/tools/mpforge/examples
ls *.yaml
# simple.yaml • simple-with-mapping.yaml • bdtopo.yaml • france-nord-bdtopo.yaml ...
```

### 3. Forge les tuiles `.mp`

```bash
mpforge --config simple.yaml --output /tmp/tuiles/
```

### 4. Compile en `.img` pour ton GPS

```bash
imgforge /tmp/tuiles/*.mp --output gmapsupp.img
```

### 5. Copie sur ton GPS

Branche ton GPS en USB, dépose `gmapsupp.img` dans le dossier `Garmin/` de la carte SD (ou de la mémoire interne). Débranche. **C'est tout.** La carte apparaît dans ton gestionnaire de cartes Garmin.

<p align="center">
  <img src="site/docs/assets/images/readme/gps-preview.svg" alt="Carte chargée sur un GPS Garmin" width="80%"/>
</p>

---

## 🏭 Monter un pipeline de production

Quand tu veux passer de « un exemple qui tourne » à « je publie des cartes régionales à jour toutes les semaines », le dépôt fournit un **squelette de pipeline** prêt à personnaliser.

### Anatomie d'un pipeline

```
pipeline/
├── configs/           # TES fichiers YAML (par région, par thématique)
│   ├── ign-bdtopo/    #   ├── generalize-profiles.yaml  (profils multi-zoom)
│   │   └── departement/  #   └── <dept>.yaml            (une carte = un YAML)
│   └── osm/
├── data/              # Sources SIG téléchargées (BDTOPO, extraits OSM…)
├── output/<année>/    # Tuiles .mp + gmapsupp.img versionnés
└── resources/         # TYP files, icônes, ressources partagées
```

### Ton premier pipeline personnalisé, en 4 étapes

**1. Déclare ta carte** — crée `pipeline/configs/ign-bdtopo/departement/mon-departement.yaml` :

```yaml
map:
  name: "Mon Département — Randonnée"
  id: 61050000                  # ID Garmin unique
  copyright: "© IGN BDTOPO 2026"

sources:
  - path: data/BDTOPO/TRONCON_DE_ROUTE.shp
  - path: data/BDTOPO/COURS_D_EAU.shp

mapping:
  - where: "CL_ADMIN = 'Autoroute'"
    polish_type: 0x01
    profile: route_major        # référence un profil de generalize-profiles.yaml
  - where: "NATURE = 'Rivière'"
    polish_type: 0x1F
    profile: hydro_main

output:
  tile_size: 0.5                # degrés
```

> 👉 Exemples complets et documentés : [`tools/mpforge/examples/bdtopo-d038-config.yaml`](./tools/mpforge/examples/bdtopo-d038-config.yaml)

**2. Rejoue le pipeline localement** :

```bash
mpforge --config pipeline/configs/ign-bdtopo/departement/mon-departement.yaml \
        --output pipeline/output/2026/v2026.04/

imgforge pipeline/output/2026/v2026.04/*.mp \
         --output pipeline/output/2026/v2026.04/gmapsupp.img
```

**3. Intègre en CI** — le dépôt fournit des pipelines Woodpecker pour l'infra interne et des workflows GitHub Actions pour le miroir public. Détails complets dans **[CI-CD.md](./CI-CD.md)**.

**4. Publie** — les releases sont pilotées par tag (`mpforge-v*`, `imgforge-v*`) via `scripts/release-tool.sh`. Chaque release produit binaire + `SHA256SUMS` + métadonnées JSON, automatiquement republiés sur le miroir GitHub.

---

## 🖼️ Galerie

<p align="center">
  <img src="site/docs/assets/images/readme/yaml-to-map.svg" alt="Un YAML déclaratif devient une carte Garmin" width="100%"/>
</p>

---

## 🧰 Pré-requis détaillés

Pour **utiliser** les binaires publiés : aucune dépendance. Pour **compiler** depuis les sources :

| Composant                                           | Requis pour                          |
|-----------------------------------------------------|--------------------------------------|
| **Rust** (via [rustup](https://rustup.rs/))         | `mpforge`, `imgforge`                |
| **GCC/Clang + CMake ≥ 3.20**                        | `ogr-polishmap` (driver C++)         |
| **GDAL ≥ 3.6** (3.10+ recommandé)                   | `ogr-polishmap`, dev local `mpforge` |
| **Python 3.10+ + PyQGIS** *(option)*                | Plugin QGIS                          |
| **Java 11+ + mkgmap** *(option)*                    | Génération avancée / comparaison     |

<details>
<summary><strong>Installation des dépendances (Debian/Ubuntu) + variables d'environnement</strong></summary>

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh && source $HOME/.cargo/env

# C++ / GDAL
sudo apt install build-essential cmake pkg-config libgdal-dev

# Optionnel : QGIS, mkgmap
sudo apt install qgis python3-qgis openjdk-11-jre
```

```bash
# ~/.bashrc ou ~/.zshrc
export GDAL_DATA=/usr/share/gdal
export GDAL_DRIVER_PATH=$HOME/.gdal/plugins
export GDAL_HOME=/usr
export RUST_BACKTRACE=1
export RUST_LOG=info
mkdir -p ~/.gdal/plugins
```

```bash
# Build
cd tools/ogr-polishmap && cmake -B build -DCMAKE_BUILD_TYPE=Debug && cmake --build build
cd tools/mpforge       && cargo build --release
cd tools/imgforge      && cargo build --release
```

</details>

---

## 🔗 Liens & ressources

| Ressource                          | URL |
|------------------------------------|-----|
| 🌐 Site & documentation complète   | [imgforge.garmin.allfabox.fr](https://imgforge.garmin.allfabox.fr) |
| 📥 Cartes Garmin téléchargeables   | [download-maps.garmin.allfabox.fr](https://download-maps.garmin.allfabox.fr/) |
| 📦 Releases (binaires)             | [github.com/allfab/garmin-img-forge/releases](https://github.com/allfab/garmin-img-forge/releases) |
| 🔧 CI/CD, tags, releases           | [CI-CD.md](./CI-CD.md) |
| 🤝 Contribuer                      | [CONTRIBUTING.md](./CONTRIBUTING.md) |

### Structure du dépôt

```
garmin-img-forge/
├── tools/          # Code source (ogr-polishmap, mpforge, imgforge, ogr-garminimg)
├── pipeline/       # Squelette de production (configs YAML, data, output)
├── scripts/        # Orchestration — doc: scripts/README.md
├── site/           # Site Zensical (sources)
├── docs/           # Documentation projet (specs, planning, images README)
├── .woodpecker/    # CI Woodpecker (interne, non miroiré sur GitHub)
└── .github/        # Workflows + templates GitHub (miroir public)
```

---

## 🙏 Crédits

Sur les épaules de géants :

- **[GDAL](https://gdal.org/)** — la bibliothèque qui rend tout ça possible
- **[cGPSmapper](https://www.cgpsmapper.com/)** — pour la spec historique du format Polish Map
- **[mkgmap](https://www.mkgmap.org.uk/)** — inspiration et référence
- **[IGN](https://www.ign.fr/)** — BDTOPO en licence ouverte
- **[OpenStreetMap](https://www.openstreetmap.org/)** — données cartographiques libres
- **[Rust](https://www.rust-lang.org/) + [Zensical](https://zensical.org/) + [Woodpecker](https://woodpecker-ci.org/)** — une stack moderne, sobre, auto-hébergeable

---

## 📜 Licences

- `ogr-polishmap` : **MIT**
- `mpforge`, `imgforge` : **GPL v3**
- Documentation du site : **[CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/deed.fr)**

Voir [`LICENSE`](./LICENSE) pour les détails.
