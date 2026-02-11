#!/bin/bash
# Script d'exécution pour le tuilage BDTOPO France Nord
# Usage: ./run-france-nord.sh [options]

set -e  # Arrêt en cas d'erreur

# Configuration
CONFIG="france-nord-bdtopo.yaml"
DATA_DIR="/mnt/e/GARMIN/GARMIN-IGN-BDTOPO-MAP/04-DATA-OUTPUT/FRANCE-NORD/v2025.12/01-SHP"
MPFORGE_CLI="../target/release/mpforge-cli"

# Vérifier que mpforge-cli existe
if [ ! -f "$MPFORGE_CLI" ]; then
    echo "❌ mpforge-cli non trouvé. Compilation en cours..."
    cd ..
    cargo build --release
    cd examples
fi

# Vérifier que le répertoire de données existe
if [ ! -d "$DATA_DIR" ]; then
    echo "❌ Répertoire de données non trouvé: $DATA_DIR"
    echo "Veuillez monter le disque E: ou adapter DATA_DIR dans le script"
    exit 1
fi

# Créer un lien symbolique si nécessaire (pour chemins relatifs)
if [ ! -L "data" ]; then
    echo "📂 Création du lien symbolique vers les données..."
    ln -s "$DATA_DIR" data
fi

# Détecter le nombre de CPUs
NCPU=$(nproc 2>/dev/null || echo 4)
JOBS=${JOBS:-$((NCPU / 2))}  # Par défaut : moitié des CPUs

echo "========================================="
echo "🗺️  MPForge - Tuilage France Nord"
echo "========================================="
echo "Config      : $CONFIG"
echo "Données     : $DATA_DIR"
echo "Threads     : $JOBS/$NCPU"
echo "========================================="
echo ""

# Demander confirmation
read -p "Lancer le tuilage ? (o/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Oo]$ ]]; then
    echo "❌ Annulé"
    exit 0
fi

# Changer de répertoire de travail vers 01-SHP (pour chemins relatifs)
echo "📂 Changement de répertoire vers: $DATA_DIR"
cd "$DATA_DIR"

# Date de début
START_TIME=$(date +%s)
REPORT_FILE="report-$(date +%Y%m%d-%H%M%S).json"

echo ""
echo "🚀 Démarrage du tuilage..."
echo ""

# Exécution avec options par défaut
# Adapter selon vos besoins (-v, -vv, --fail-fast, etc.)
"$MPFORGE_CLI" build \
    --config "$(dirname "$0")/$CONFIG" \
    --jobs "$JOBS" \
    --report "$REPORT_FILE" \
    -v

# Codes de sortie
EXIT_CODE=$?

# Date de fin
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

echo ""
echo "========================================="
if [ $EXIT_CODE -eq 0 ]; then
    echo "✅ Tuilage terminé avec succès"
else
    echo "❌ Tuilage échoué (code: $EXIT_CODE)"
fi
echo "========================================="
echo "Durée       : ${DURATION}s ($(date -u -d @${DURATION} +%T))"
echo "Rapport     : $REPORT_FILE"
echo "========================================="
echo ""

# Afficher un résumé du rapport si disponible
if [ -f "$REPORT_FILE" ] && command -v jq &> /dev/null; then
    echo "📊 Résumé du rapport:"
    echo "----------------------------------------"
    jq -r '"  Tuiles générées  : \(.tiles_generated)\n  Tuiles échouées  : \(.tiles_failed)\n  Tuiles vides     : \(.tiles_skipped)\n  Features traitées: \(.features_processed)\n  Durée (s)        : \(.duration_seconds | floor)"' "$REPORT_FILE"
    echo "----------------------------------------"

    # Afficher les erreurs si présentes
    ERRORS=$(jq '.errors | length' "$REPORT_FILE")
    if [ "$ERRORS" -gt 0 ]; then
        echo ""
        echo "⚠️  Erreurs détectées ($ERRORS):"
        jq -r '.errors[] | "  - Tuile \(.tile): \(.error)"' "$REPORT_FILE" | head -10
        if [ "$ERRORS" -gt 10 ]; then
            echo "  ... et $((ERRORS - 10)) autres erreurs (voir $REPORT_FILE)"
        fi
    fi
fi

echo ""
exit $EXIT_CODE
