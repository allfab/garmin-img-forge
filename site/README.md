# Site de documentation — GARMIN IGN BDTOPO MAP

Site de documentation du projet, accessible sur [maps.garmin.allfabox.fr](https://maps.garmin.allfabox.fr).

## Stack technique

- **Generateur** : [Zensical](https://pypi.org/project/zensical/) v0.0.30 (compatible MkDocs, config TOML)
- **Theme** : `classic` avec surcharges dans `overrides/`
- **CI** : `.woodpecker/site.yml`
- **Deploiement** : Docker + Nginx + Traefik (voir `deploy/`)

## Arborescence

```
site/
├── zensical.toml          # Configuration du site (navigation, theme, extensions)
├── requirements.txt       # Dependances Python (zensical pinne)
├── docs/                  # Pages source en Markdown
│   ├── index.md           # Page d'accueil
│   ├── le-projet/         # Presentation des outils (ogr-polishmap, mpforge, imgforge)
│   ├── le-pipeline/       # Etapes du pipeline de generation
│   ├── reference/         # Documentation de reference technique
│   ├── prerequis-installation/
│   ├── telechargements/   # Pages de telechargement (France, regions, outre-mer)
│   ├── soutenir/
│   ├── a-propos/
│   ├── assets/            # Images et ressources statiques
│   ├── stylesheets/       # CSS custom (hero, extra, donate)
│   └── javascripts/       # JS custom (lemonsqueezy, umami)
├── overrides/             # Surcharges de templates (home.html, main.html, partials/)
├── deploy/                # Config de deploiement (docker-compose, nginx, traefik)
└── _site/                 # Sortie du build (gitignore)
```

## Developpement local

```bash
# Installation
pip install -r site/requirements.txt

# Serveur de dev avec rechargement automatique
cd site && zensical serve

# Build statique
cd site && zensical build
```

Le site est genere dans `_site/`.

## Pages generees automatiquement

### Styles TYP (`reference/styles-typ.md`)

Cette page est generee par un script Python qui parse le fichier TYP texte et produit un catalogue visuel de tous les styles avec rendus SVG inline.

```bash
# Generer/regenerer la page de reference des styles TYP
python3 scripts/generate-typ-reference.py
```

**Source** : `pipeline/resources/typfiles/I2023100.txt`
**Script** : `scripts/generate-typ-reference.py`
**Sortie** : `site/docs/reference/styles-typ.md`

Le script :

1. Parse le fichier TYP texte (format Garmin) et extrait les 553 styles (130 polygones, 93 lignes, 330 points)
2. Convertit chaque motif XPM en SVG inline
3. Genere une page Markdown avec tableaux HTML contenant : rendu visuel, code type, description GRMN, couleurs

A relancer apres toute modification du fichier TYP pour garder la documentation a jour.

## Navigation

La navigation est configuree dans `zensical.toml` (section `[[nav]]`). Pour ajouter une page :

1. Creer le fichier `.md` dans le dossier `docs/` correspondant
2. Ajouter l'entree dans la section `[[nav]]` de `zensical.toml`
