# Exemples de configuration mpforge

Ce répertoire contient des exemples de configuration pour différents cas d'usage.

## 📁 Fichiers disponibles

| Fichier | Description | Cas d'usage |
|---------|-------------|-------------|
| **simple.yaml** | Configuration minimale | Démarrage rapide, tests |
| **bdtopo.yaml** | BDTOPO générique avec GeoPackage | Production avec GeoPackage multi-couches |
| **france-nord-bdtopo.yaml** | BDTOPO France Nord (46 shapefiles) | Production avec Shapefiles organisés |
| **france-nord-simple.yaml** | BDTOPO France Nord avec wildcards | Version compacte de france-nord-bdtopo.yaml |
| **run-france-nord.sh** | Script d'exécution automatisé | Pipeline de production |

## 🚀 Utilisation rapide

### Exemple 1 : Configuration simple

```bash
# Depuis le répertoire mpforge
mpforge build --config examples/simple.yaml
```

**Prérequis** : Adapter le chemin `path: "data/input.shp"` dans `simple.yaml` vers vos données réelles.

---

### Exemple 2 : BDTOPO France Nord (version détaillée)

```bash
# 1. Se positionner dans le répertoire des données
cd /mnt/e/GARMIN/GARMIN-IGN-BDTOPO-MAP/04-DATA-OUTPUT/FRANCE-NORD/v2025.12/01-SHP/

# 2. Exécuter le tuilage (4 threads, mode production)
mpforge build \
  --config /home/allfab/code/forgejo/mpforge/mpforge/examples/france-nord-bdtopo.yaml \
  --jobs 4 \
  --report rapport-$(date +%Y%m%d).json \
  -v
```

**Configuration** : `france-nord-bdtopo.yaml`
- 46 shapefiles listés explicitement
- Organisation par thématique (Administratif, Hydrographie, Bâti, Transport, OSM, Contours)
- Contrôle total sur l'ordre de traitement

---

### Exemple 3 : BDTOPO France Nord (version wildcards)

```bash
# Depuis le répertoire des données
cd /mnt/e/GARMIN/GARMIN-IGN-BDTOPO-MAP/04-DATA-OUTPUT/FRANCE-NORD/v2025.12/01-SHP/

# Exécution avec la version simplifiée
mpforge build \
  --config /home/allfab/code/forgejo/mpforge/mpforge/examples/france-nord-simple.yaml \
  --jobs 8
```

**Configuration** : `france-nord-simple.yaml`
- Utilise des wildcards (`01-BDTOPO/*.shp`)
- Plus compact (5 lignes vs 46 lignes)
- Même résultat mais ordre de traitement indéterminé

---

### Exemple 4 : Script automatisé

```bash
# Depuis le répertoire examples/
cd /home/allfab/code/forgejo/mpforge/mpforge/examples/

# Exécuter le script (avec confirmation interactive)
./run-france-nord.sh

# Exécution non-interactive (CI/CD)
export JOBS=8
yes o | ./run-france-nord.sh
```

**Fonctionnalités du script** :
- ✅ Vérification de l'environnement (mpforge, données)
- ✅ Détection automatique du nombre de CPUs
- ✅ Génération de rapport JSON horodaté
- ✅ Résumé avec jq (si disponible)
- ✅ Codes de sortie pour intégration CI/CD

---

## 📊 Structure des données France Nord

```
01-SHP/
├── 01-BDTOPO/          # 31 fichiers (Administratif, Hydrographie, Bâti, Transport, Toponymie)
├── 02-OSM/             #  5 fichiers (POI naturels, urbains, tourisme)
├── 03-CONTOUR-LINES/   #  8 fichiers (Courbes SRTM 10m par département)
├── 04-BOUNDS/          #  1 fichier  (Limites océan/mer)
└── 05-GR/              #  1 fichier  (Sentiers de Grande Randonnée)
                        # = 46 shapefiles au total
```

---

## 🎯 Choisir la bonne configuration

### Utiliser **france-nord-bdtopo.yaml** si :
- ✅ Vous voulez un **contrôle total** sur l'ordre de traitement
- ✅ Vous avez besoin de **documentation** (commentaires sur chaque fichier)
- ✅ Vous voulez **exclure** certains fichiers facilement (commentez la ligne)
- ✅ Vous préparez une **production** avec validation stricte

### Utiliser **france-nord-simple.yaml** si :
- ✅ Vous voulez une **configuration concise** (5 lignes vs 46)
- ✅ L'ordre de traitement **n'a pas d'importance**
- ✅ Vous voulez **tester rapidement** (moins de maintenance)
- ✅ Vous faites confiance aux **wildcards** pour tout charger

---

## 🔧 Personnalisation

### Adapter les chemins

Si vos données sont ailleurs, modifiez le chemin dans la configuration :

```yaml
# Chemin absolu
inputs:
  - path: "/mnt/d/mes-donnees/routes.shp"

# Chemin relatif depuis le répertoire courant
inputs:
  - path: "../data/routes.shp"
```

### Filtrer une zone géographique

Décommentez et adaptez la section `filters` :

```yaml
filters:
  # France Nord : Nord de la Loire
  bbox: [-5.5, 47.0, 10.0, 51.5]

  # Île-de-France seulement
  bbox: [1.5, 48.1, 3.5, 49.2]

  # Bretagne
  bbox: [-5.5, 47.0, -1.0, 49.0]
```

### Ajuster la grille

```yaml
grid:
  # Grille fine (petites tuiles ~11 km)
  cell_size: 0.10
  overlap: 0.005

  # Grille standard (tuiles ~16.5 km) - RECOMMANDÉ
  cell_size: 0.15
  overlap: 0.01

  # Grille grossière (grandes tuiles ~22 km)
  cell_size: 0.20
  overlap: 0.02
```

### Parallélisation optimale

```bash
# Petit dataset (<100 tuiles) : mode séquentiel
mpforge build --config config.yaml

# Dataset moyen (100-500 tuiles) : 4 threads
mpforge build --config config.yaml --jobs 4

# Large dataset (>500 tuiles) : 8 threads
mpforge build --config config.yaml --jobs 8

# Utiliser tous les CPUs (attention à la RAM)
mpforge build --config config.yaml --jobs $(nproc)
```

---

## 🐛 Dépannage

### Erreur : "No such file or directory"

```bash
# Vérifier que vous êtes dans le bon répertoire
pwd
# Doit afficher : /mnt/e/GARMIN/.../01-SHP/

# Ou utiliser des chemins absolus dans le YAML
```

### Erreur : "Failed to open shapefile"

```bash
# Vérifier que le fichier existe
ls -lh 01-BDTOPO/01-IGNBDTOPO-FRANCE-NORD-ADMINISTRATIF-COMMUNE.shp

# Vérifier les permissions
chmod 644 01-BDTOPO/*.shp
```

### Wildcard ne matche aucun fichier

```
WARN No files matched wildcard pattern pattern="data/*.xyz"
```

```bash
# Tester le pattern avec ls
ls -1 01-BDTOPO/*.shp

# Vérifier l'extension (.shp vs .SHP)
ls -1 01-BDTOPO/*.[Ss][Hh][Pp]
```

### Mode debug complet

```bash
# Logs détaillés GDAL + pas de barre de progression
mpforge build --config config.yaml -vv

# Logs ultra-verbeux (trace)
mpforge build --config config.yaml -vvv
```

---

## 📚 Ressources

- **Documentation complète** : [`../doc/config-schema.md`](../doc/config-schema.md)
- **README principal** : [`../README.md`](../README.md)
- **Code source** : [`../src/`](../src/)

---

## 💡 Astuces

### Vérifier la configuration sans traiter

```bash
# Validation syntaxique + affichage des sources (Story future)
mpforge validate --config config.yaml
```

### Estimer le nombre de tuiles

```bash
# Formule approximative
# Tuiles = (bbox_width / cell_size) × (bbox_height / cell_size)
# Exemple : France métropolitaine ~15° × 10° avec cell_size=0.15
# Tuiles ≈ (15/0.15) × (10/0.15) = 100 × 67 ≈ 6700 tuiles
```

### Traiter uniquement certaines catégories

Créez une configuration personnalisée en commentant les sections inutiles :

```yaml
inputs:
  # Seulement hydrographie + transport
  - path: "01-BDTOPO/07-IGNBDTOPO-*-HYDROGRAPHIE-*.shp"
  - path: "01-BDTOPO/18-IGNBDTOPO-*-TRANSPORT-*.shp"

  # Exclure végétation et bâti (commentés)
  # - path: "01-BDTOPO/10-IGNBDTOPO-*-VEGETATION-*.shp"
  # - path: "01-BDTOPO/11-IGNBDTOPO-*-BATI-*.shp"
```

---

**Besoin d'aide ?** Consultez la documentation complète ou ouvrez une issue sur le dépôt.
