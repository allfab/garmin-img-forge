#!/usr/bin/env bash
# Build complet du site bilingue FR + EN
# Usage : ./build-site.sh [--serve]
set -euo pipefail

SITE_DIR="$(cd "$(dirname "$0")" && pwd)"

# --- Build FR (→ _site/) ---
echo "==> Build FR..."
cd "$SITE_DIR"
zensical build

# --- Build EN (→ _site-en/) ---
echo "==> Build EN..."
zensical build --config-file zensical-en.toml

# --- Fusion : copie _site-en/ → _site/en/ ---
echo "==> Fusion EN → _site/en/..."
rm -rf "$SITE_DIR/_site/en"
mkdir -p "$SITE_DIR/_site/en"
cp -r "$SITE_DIR/_site-en/." "$SITE_DIR/_site/en/"

# --- Assets partagés (CSS, JS, images) : copie depuis le build FR ---
echo "==> Copie assets partagés FR → _site/en/..."
for dir in stylesheets javascripts assets; do
    if [[ -d "$SITE_DIR/_site/$dir" ]]; then
        rm -rf "$SITE_DIR/_site/en/$dir"
        cp -r "$SITE_DIR/_site/$dir" "$SITE_DIR/_site/en/$dir"
    fi
done

# --- Re-application des assets EN spécifiques par-dessus les assets FR ---
echo "==> Re-application assets EN spécifiques → _site/en/assets/..."
cp -r "$SITE_DIR/_site-en/assets/." "$SITE_DIR/_site/en/assets/"

echo ""
echo "Site bilingue disponible dans _site/"
echo "  FR : _site/          → http://localhost:8000/"
echo "  EN : _site/en/       → http://localhost:8000/en/"
echo ""

# Serveur local optionnel
if [[ "${1:-}" == "--serve" ]]; then
    echo "==> Démarrage du serveur local sur http://localhost:8000"
    cd "$SITE_DIR/_site"
    python3 -m http.server 8000
fi
