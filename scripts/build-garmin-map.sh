#!/usr/bin/env bash
# =============================================================================
# build-garmin-map.sh — Pipeline mpforge → imgforge → gmapsupp.img
# =============================================================================
#
# Enchaîne mpforge build et imgforge build pour produire une carte Garmin :
#
#   1. Résout les chemins data depuis les paramètres (zones, année, version)
#   2. Lance mpforge build (génère les tuiles .mp)
#   3. Vérifie le code de sortie et le rapport JSON
#   4. Lance imgforge build (compile .mp → gmapsupp.img)
#   5. Affiche le résumé final (tuiles, temps, taille)
#
# Pipeline : download-bdtopo.sh → build-garmin-map.sh → gmapsupp.img
#
# Prérequis : mpforge, imgforge (ou cargo build --release dans tools/*)
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration par défaut
# ---------------------------------------------------------------------------
SCRIPT_VERSION="3.0.0"

# Paramètres géographiques
ZONES=""                # D038 | D038,D069 | (obligatoire sauf si --region)
REGION=""               # Raccourci région (ARA, FRANCE-SE, etc.)
YEAR=""                 # 2025 (auto-détecté si vide)
VERSION=""              # v2025.12 (auto-détecté si vide)
BASE_ID=""              # Auto-calculé depuis le premier département

# Chemins racine
DATA_DIR="./pipeline/data"
OUTPUT_BASE="./pipeline/output"

# Chemins sources (si vide : dérivé de DATA_DIR)
CONTOURS_DIR=""         # défaut: ${DATA_DIR}/contours
DEM_DIR=""              # défaut: ${DATA_DIR}/dem
OSM_DIR=""              # défaut: ${DATA_DIR}/osm
HIKING_TRAILS_DIR=""    # défaut: ${DATA_DIR}/hiking-trails

# mpforge
CONFIG_FILE=""          # si vide : utilise sources-shp.yaml avec envsubst
JOBS=8

# imgforge
FAMILY_ID=1100
PRODUCT_ID=1
FAMILY_NAME=""          # Auto-calculé : IGN-BDTOPO-{ZONES}-{VERSION}
SERIES_NAME="IGN-BDTOPO-MAP"
CODE_PAGE=1252
LEVELS="24,22,20,18,16"
TYP_FILE="pipeline/resources/typfiles/I2023100.typ"
COPYRIGHT="©$(date +%Y) Allfab Studio - ©IGN BDTOPO - ©OpenStreetMap Les Contributeurs - Licence Ouverte Etalab 2.0"

# Contrôle
DRY_RUN=false
SKIP_EXISTING=false
VERBOSE_COUNT=0         # 0=warn, 1=-v, 2=-vv
WITH_ROUTE=true
WITH_DEM=true

# Binaires résolus
_MPFORGE=""
_IMGFORGE=""

# Métriques mpforge
BUILD_START_TIME=0
TILES_TOTAL=0
TILES_SUCCESS=0
TILES_FAILED=0
MPFORGE_DURATION=0
FEATURES_PROCESSED=0

# Métriques imgforge
IMGFORGE_TILES_COMPILED=0
IMGFORGE_TILES_FAILED=0
IMGFORGE_DURATION=0
IMGFORGE_IMG_SIZE=0

# Fichier config temporaire
_TMP_CONFIG=""

# État pipeline
PARTIAL_FAILURE=false

# Chemins résolus (calculés dans resolve_paths)
_DATA_ROOT=""
_CONTOURS_DATA_ROOT=""
_DEM_DATA_ROOT=""
_OSM_DATA_ROOT=""
_HIKING_TRAILS_DATA_ROOT=""
_OUTPUT_DIR=""
_REPORT_FILE=""
_IMGFORGE_REPORT_FILE=""

# ---------------------------------------------------------------------------
# Régions → zones de données BDTOPO (même mapping que REGIONS dans download-bdtopo.sh)
# Mix R-codes (régions complètes) + D-codes (départements isolés) = répertoires sur disque
# ---------------------------------------------------------------------------
declare -A REGIONS_TO_ZONES=(
    [ARA]="R84"
    [BFC]="R27"
    [BRE]="R53"
    [CVL]="R24"
    [COR]="R94"
    [GES]="R44"
    [HDF]="R32"
    [IDF]="R11"
    [NOR]="R28"
    [NAQ]="R75"
    [OCC]="R76"
    [PDL]="R52"
    [PAC]="R93"
    # Groupements multi-régions
    [FRANCE-SUD]="R75,R76,R84,R93,R94"
    [FRANCE-NORD]="R11,R24,R27,R28,R32,R44,R52,R53"
    [FXX]="R11,R24,R27,R28,R32,R44,R52,R53,R75,R76,R84,R93,R94"
    # Quadrants Garmin (couverture TOPO France v7 PRO — mix R-codes + D-codes)
    [FRANCE-SE]="R84,R93,R94,D011,D030,D034,D048,D066"
    [FRANCE-SO]="R75,D009,D012,D031,D032,D046,D065,D081,D082"
    [FRANCE-NE]="R27,R32,R44,R11,D027,D076"
    [FRANCE-NO]="R24,R52,R53,R11,D014,D050,D061"
)

# ---------------------------------------------------------------------------
# Régions → départements (pour DEM et courbes de niveau, livrés par département)
# ---------------------------------------------------------------------------
declare -A REGIONS_TO_DEPARTMENTS=(
    [ARA]="D001,D003,D007,D015,D026,D038,D042,D043,D063,D069,D073,D074"
    [BFC]="D021,D025,D039,D058,D070,D071,D089,D090"
    [BRE]="D022,D029,D035,D056"
    [CVL]="D018,D028,D036,D037,D041,D045"
    [COR]="D02A,D02B"
    [GES]="D008,D010,D051,D052,D054,D055,D057,D067,D068,D088"
    [HDF]="D002,D059,D060,D062,D080"
    [IDF]="D075,D077,D078,D091,D092,D093,D094,D095"
    [NOR]="D014,D027,D050,D061,D076"
    [NAQ]="D016,D017,D019,D023,D024,D033,D040,D047,D064,D079,D086,D087"
    [OCC]="D009,D011,D012,D030,D031,D032,D034,D046,D048,D065,D066,D081,D082"
    [PDL]="D044,D049,D053,D072,D085"
    [PAC]="D004,D005,D006,D013,D083,D084"
    # Groupements multi-régions
    [FRANCE-SUD]="D001,D003,D004,D005,D006,D007,D009,D011,D012,D013,D015,D016,D017,D019,D023,D024,D026,D02A,D02B,D030,D031,D032,D033,D034,D038,D040,D042,D043,D046,D047,D048,D063,D064,D065,D066,D069,D073,D074,D079,D081,D082,D083,D084,D086,D087"
    [FRANCE-NORD]="D002,D008,D010,D014,D018,D021,D022,D025,D027,D028,D029,D035,D036,D037,D039,D041,D044,D045,D049,D050,D051,D052,D053,D054,D055,D056,D057,D058,D059,D060,D061,D062,D067,D068,D070,D071,D072,D075,D076,D077,D078,D080,D085,D088,D089,D090,D091,D092,D093,D094,D095"
    [FXX]="D001,D002,D003,D004,D005,D006,D007,D008,D009,D010,D011,D012,D013,D014,D015,D016,D017,D018,D019,D02A,D02B,D021,D022,D023,D024,D025,D026,D027,D028,D029,D030,D031,D032,D033,D034,D035,D036,D037,D038,D039,D040,D041,D042,D043,D044,D045,D046,D047,D048,D049,D050,D051,D052,D053,D054,D055,D056,D057,D058,D059,D060,D061,D062,D063,D064,D065,D066,D067,D068,D069,D070,D071,D072,D073,D074,D075,D076,D077,D078,D079,D080,D081,D082,D083,D084,D085,D086,D087,D088,D089,D090,D091,D092,D093,D094,D095"
    # Quadrants Garmin (couverture TOPO France v7 PRO)
    [FRANCE-SE]="D001,D003,D004,D005,D006,D007,D011,D013,D015,D02A,D02B,D026,D030,D034,D038,D042,D043,D048,D063,D066,D069,D073,D074,D083,D084"
    [FRANCE-SO]="D009,D012,D016,D017,D019,D023,D024,D031,D032,D033,D040,D046,D047,D064,D065,D079,D081,D082,D086,D087"
    [FRANCE-NE]="D002,D008,D010,D021,D025,D027,D039,D051,D052,D054,D055,D057,D058,D059,D060,D062,D067,D068,D070,D071,D075,D076,D077,D078,D080,D088,D089,D090,D091,D092,D093,D094,D095"
    [FRANCE-NO]="D014,D018,D022,D028,D029,D035,D036,D037,D041,D044,D045,D049,D050,D053,D056,D061,D072,D075,D077,D078,D085,D091,D092,D093,D094,D095"
)

# ---------------------------------------------------------------------------
# Nettoyage
# ---------------------------------------------------------------------------
cleanup_trap() {
    if [[ -n "$_TMP_CONFIG" && -f "$_TMP_CONFIG" ]]; then
        rm -f "$_TMP_CONFIG"
    fi
}
trap cleanup_trap INT TERM EXIT

# ---------------------------------------------------------------------------
# Couleurs
# ---------------------------------------------------------------------------
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[0;33m'
BLUE='\033[0;34m'; CYAN='\033[0;36m'; BOLD='\033[1m'; NC='\033[0m'

log_info()  { echo -e "${BLUE}[INFO]${NC}  $*"; }
log_ok()    { echo -e "${GREEN}[OK]${NC}    $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }
log_step() {
    local title="$*"
    local width=60
    local prefix="── ${title} "
    local prefix_len=${#prefix}
    local pad=$((width - prefix_len))
    [[ $pad -lt 2 ]] && pad=2
    local trail
    trail=$(printf '─%.0s' $(seq 1 "$pad"))
    echo ""
    echo -e "${BOLD}${CYAN}${prefix}${trail}${NC}"
}

# ---------------------------------------------------------------------------
# Aide
# ---------------------------------------------------------------------------
show_help() {
    cat << 'EOF'
build-garmin-map.sh — Pipeline mpforge → imgforge → gmapsupp.img

USAGE :
    ./scripts/build-garmin-map.sh --zones D038 [OPTIONS]
    ./scripts/build-garmin-map.sh --region FRANCE-SE [OPTIONS]
    ./scripts/build-garmin-map.sh --zones D038,D069 --year 2025 --version v2025.12

OPTIONS GÉOGRAPHIQUES :
    --zones ZONES           Départements : D038 | D038,D069 | D001,D038,D039
    --region CODE           Raccourci région (alternatif à --zones) :
                              Régions  : ARA BFC BRE CVL COR GES HDF IDF NOR NAQ OCC PDL PAC
                              Groupes  : FRANCE-SUD FRANCE-NORD FXX
                              Quadrants Garmin (couverture TOPO France v7 PRO) :
                                         FRANCE-SE FRANCE-SO FRANCE-NE FRANCE-NO
                                         Note : IDF partagée entre NE et NO (conforme Garmin)
    --year YYYY             Année BDTOPO (défaut: auto-détecté)
    --version vYYYY.MM      Version BDTOPO (défaut: auto-détecté)
    --base-id N             Base ID Garmin (défaut: premier code département)

CHEMINS :
    --data-dir DIR          Racine des données (défaut: ./pipeline/data)
    --contours-dir DIR      Racine courbes de niveau (défaut: ${data-dir}/contours)
    --dem-dir DIR           Racine MNT BD ALTI (défaut: ${data-dir}/dem)
    --osm-dir DIR           Racine données OSM (défaut: ${data-dir}/osm)
    --hiking-trails-dir DIR Racine sentiers GR (défaut: ${data-dir}/hiking-trails)
    --output-base DIR       Base des sorties (défaut: ./pipeline/output)
    --config FILE           Config YAML mpforge custom (défaut: sources-shp.yaml)

MPFORGE :
    --jobs N                Parallélisation (défaut: 8)
    --skip-existing         Passer les tuiles .mp déjà présentes

IMGFORGE :
    --family-id N           Family ID Garmin (défaut: 1100)
    --product-id N          Product ID Garmin (défaut: 1)
    --family-name STR       Nom de la carte (défaut: auto IGN-BDTOPO-{ZONES}-{VERSION})
    --series-name STR       Nom de la série (défaut: IGN-BDTOPO-MAP)
    --code-page N           Code page encodage (défaut: 1252)
    --levels STR            Niveaux de zoom (défaut: 24,22,20,18,16)
    --typ FILE              Fichier TYP styles (défaut: pipeline/resources/typfiles/I2023100.typ)
    --copyright STR         Message copyright
    --no-route              Désactiver le routage
    --no-dem                Désactiver le DEM (relief ombré)

CONTRÔLE :
    --dry-run               Simuler sans exécuter
    -v, --verbose           Mode verbeux (-vv pour très verbeux)
    --version-info          Version du script
    -h, --help              Aide

EXEMPLES :
    # Un département
    ./scripts/build-garmin-map.sh --zones D038

    # Multi-départements
    ./scripts/build-garmin-map.sh --zones D038,D069 --jobs 4

    # Forcer année/version
    ./scripts/build-garmin-map.sh --zones D038 --year 2025 --version v2025.12

    # Dry-run pour vérifier les chemins
    ./scripts/build-garmin-map.sh --zones D038,D069 --dry-run

    # Quadrant Garmin Sud-Est (25 départements)
    ./scripts/build-garmin-map.sh --region FRANCE-SE

    # Région Auvergne-Rhône-Alpes
    ./scripts/build-garmin-map.sh --region ARA --year 2025 --version v2025.12
EOF
    exit 0
}

# ---------------------------------------------------------------------------
# Parse args
# ---------------------------------------------------------------------------
parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --zones)         ZONES="$2"; shift 2 ;;
            --region)        REGION="${2^^}"; shift 2 ;;
            --year)          YEAR="$2"; shift 2 ;;
            --version)       VERSION="$2"; shift 2 ;;
            --base-id)       BASE_ID="$2"; shift 2 ;;
            --data-dir)      DATA_DIR="$2"; shift 2 ;;
            --contours-dir)  CONTOURS_DIR="$2"; shift 2 ;;
            --dem-dir)       DEM_DIR="$2"; shift 2 ;;
            --osm-dir)       OSM_DIR="$2"; shift 2 ;;
            --hiking-trails-dir) HIKING_TRAILS_DIR="$2"; shift 2 ;;
            --output-base)   OUTPUT_BASE="$2"; shift 2 ;;
            --config)        CONFIG_FILE="$2"; shift 2 ;;
            --jobs)          JOBS="$2"; shift 2 ;;
            --skip-existing) SKIP_EXISTING=true; shift ;;
            --family-id)     FAMILY_ID="$2"; shift 2 ;;
            --product-id)    PRODUCT_ID="$2"; shift 2 ;;
            --family-name)   FAMILY_NAME="$2"; shift 2 ;;
            --series-name)   SERIES_NAME="$2"; shift 2 ;;
            --code-page)     CODE_PAGE="$2"; shift 2 ;;
            --levels)        LEVELS="$2"; shift 2 ;;
            --typ)           TYP_FILE="$2"; shift 2 ;;
            --copyright)     COPYRIGHT="$2"; shift 2 ;;
            --no-route)      WITH_ROUTE=false; shift ;;
            --no-dem)        WITH_DEM=false; shift ;;
            --dry-run)       DRY_RUN=true; shift ;;
            -v|--verbose)    VERBOSE_COUNT=$(( VERBOSE_COUNT + 1 > 2 ? 2 : VERBOSE_COUNT + 1 )); shift ;;
            -vv)             VERBOSE_COUNT=2; shift ;;
            --version-info)  echo "build-garmin-map.sh v${SCRIPT_VERSION}"; exit 0 ;;
            -h|--help)       show_help ;;
            *)               log_error "Option inconnue : $1"; exit 1 ;;
        esac
    done

    # Résolution --region → --zones (zones de données = répertoires sur disque)
    if [[ -n "$REGION" ]]; then
        if [[ -n "$ZONES" ]]; then
            log_error "--region et --zones sont mutuellement exclusifs"
            exit 1
        fi
        if [[ -z "${REGIONS_TO_ZONES[$REGION]+x}" ]]; then
            log_error "Région inconnue : $REGION"
            log_error "  Disponibles : ${!REGIONS_TO_ZONES[*]}"
            exit 1
        fi
        ZONES="${REGIONS_TO_ZONES[$REGION]}"
        log_info "Région $REGION → zones de données : $ZONES"
    fi

    if [[ -z "$ZONES" ]]; then
        log_error "Le paramètre --zones ou --region est obligatoire"
        log_error "  Exemple : --zones D038 ou --region FRANCE-SE"
        exit 1
    fi

    if [[ "$ZONES" == *" "* ]]; then
        log_error "--zones : les zones doivent être séparées par des virgules, pas des espaces"
        log_error "  Reçu   : --zones '$ZONES'"
        log_error "  Attendu : --zones $(echo "$ZONES" | tr ' ' ',')"
        exit 1
    fi
}

# ---------------------------------------------------------------------------
# Validation des paramètres
# ---------------------------------------------------------------------------
validate_params() {
    local errors=0

    # --- zones : format DXXX (département) ou RXX (région) ---
    IFS=',' read -ra zone_array <<< "$ZONES"
    for zone in "${zone_array[@]}"; do
        if [[ ! "$zone" =~ ^D[0-9]{2}[0-9A-B]?$ ]] && [[ ! "$zone" =~ ^R[0-9]{2}$ ]]; then
            log_error "--zones : format invalide '${zone}'"
            log_error "  Format attendu : D-code (D038, D02A) ou R-code (R84, R11)"
            errors=$(( errors + 1 ))
        fi
    done

    # --- base-id : validé séparément dans validate_base_id() après resolve_paths ---
    # (peut être auto-calculé depuis le premier département)

    # --- jobs : entier positif ---
    if ! [[ "$JOBS" =~ ^[0-9]+$ ]] || [[ "$JOBS" -lt 1 ]]; then
        log_error "--jobs : doit être un entier positif, reçu '${JOBS}'"
        errors=$(( errors + 1 ))
    elif [[ "$JOBS" -gt 64 ]]; then
        log_warn "--jobs ${JOBS} : valeur élevée, ${JOBS} workers GDAL en parallèle consommeront beaucoup de RAM"
    fi

    # --- family-id : u16 (0..65535) ---
    # Raison : le champ Family ID dans le format TDB Garmin est encodé sur 16 bits.
    # BaseCamp/MapInstall utilisent ce champ pour regrouper les cartes d'une même famille.
    if ! [[ "$FAMILY_ID" =~ ^[0-9]+$ ]] || [[ "$FAMILY_ID" -gt 65535 ]]; then
        log_error "--family-id : doit être un entier 0-65535, reçu '${FAMILY_ID}'"
        log_error "  Raison : encodé sur 16 bits (u16) dans le format TDB Garmin"
        errors=$(( errors + 1 ))
    fi

    # --- product-id : u16 (0..65535) ---
    if ! [[ "$PRODUCT_ID" =~ ^[0-9]+$ ]] || [[ "$PRODUCT_ID" -gt 65535 ]]; then
        log_error "--product-id : doit être un entier 0-65535, reçu '${PRODUCT_ID}'"
        log_error "  Raison : encodé sur 16 bits (u16) dans le format TDB Garmin"
        errors=$(( errors + 1 ))
    fi

    # --- code-page : valeurs connues ---
    # Raison : le code page détermine l'encodage des labels dans le fichier IMG.
    # Les GPS Garmin ne supportent qu'un nombre limité d'encodages.
    case "$CODE_PAGE" in
        1252|1250|1251|1253|1254|1255|1256|1257|1258|65001|0)
            ;; # Valeurs connues et supportées
        *)
            log_warn "--code-page ${CODE_PAGE} : valeur inhabituelle"
            log_warn "  Valeurs courantes : 1252 (Latin-1, Europe occidentale), 65001 (UTF-8)"
            ;;
    esac

    # --- levels : format "N,N,N" avec valeurs 1-24, décroissantes ---
    if [[ -n "$LEVELS" ]]; then
        if ! [[ "$LEVELS" =~ ^[0-9]+(,[0-9]+)*$ ]]; then
            log_error "--levels : format invalide '${LEVELS}'"
            log_error "  Format attendu : liste de nombres séparés par des virgules"
            log_error "  Exemple : 24,22,20,18,16"
            errors=$(( errors + 1 ))
        else
            IFS=',' read -ra level_array <<< "$LEVELS"
            local prev=99
            for lvl in "${level_array[@]}"; do
                if [[ "$lvl" -lt 1 || "$lvl" -gt 24 ]]; then
                    log_error "--levels : niveau ${lvl} hors intervalle 1-24"
                    log_error "  Raison : les niveaux de zoom Garmin vont de 1 (le plus large) à 24 (le plus détaillé)"
                    errors=$(( errors + 1 ))
                    break
                fi
                if [[ "$lvl" -ge "$prev" ]]; then
                    log_error "--levels : les niveaux doivent être décroissants (${lvl} >= ${prev})"
                    log_error "  Raison : le premier niveau est le plus détaillé (zoom max), le dernier le plus large"
                    log_error "  Exemple : 24,22,20,18,16 (détaillé → large)"
                    errors=$(( errors + 1 ))
                    break
                fi
                prev=$lvl
            done
        fi
    fi

    # --- year : format YYYY ---
    if [[ -n "$YEAR" && ! "$YEAR" =~ ^[0-9]{4}$ ]]; then
        log_error "--year : format invalide '${YEAR}', attendu YYYY (ex: 2025)"
        errors=$(( errors + 1 ))
    fi

    # --- version : format vYYYY.MM ---
    if [[ -n "$VERSION" && ! "$VERSION" =~ ^v[0-9]{4}\.[0-9]{1,2}$ ]]; then
        log_error "--version : format invalide '${VERSION}', attendu vYYYY.MM (ex: v2025.12)"
        errors=$(( errors + 1 ))
    fi

    if [[ "$errors" -gt 0 ]]; then
        log_error "${errors} erreur(s) de validation — abandon"
        exit 1
    fi
}

# ---------------------------------------------------------------------------
# Validation base_id (après resolve_paths qui peut l'auto-calculer)
# ---------------------------------------------------------------------------
validate_base_id() {
    if ! [[ "$BASE_ID" =~ ^[0-9]+$ ]]; then
        log_error "base-id : doit être un entier, reçu '${BASE_ID}'"
        log_error "  Raison : mpforge génère des IDs Garmin = base_id × 10000 + seq"
        exit 1
    fi
    if [[ "$BASE_ID" -lt 1 || "$BASE_ID" -gt 9999 ]]; then
        log_error "base-id : doit être dans l'intervalle 1-9999, reçu ${BASE_ID}"
        log_error "  Raison : mpforge génère des IDs Garmin = base_id × 10000 + seq"
        log_error "  L'ID résultant doit tenir sur 8 chiffres (format IMG Garmin)"
        log_error "  Exemple : --base-id 38 → IDs 00380001, 00380002, etc."
        exit 1
    fi
}

# ---------------------------------------------------------------------------
# Auto-détection année/version depuis l'arborescence data
# ---------------------------------------------------------------------------
auto_detect_year_version() {
    local bdtopo_dir="${DATA_DIR}/bdtopo"

    if [[ ! -d "$bdtopo_dir" ]]; then
        log_error "Répertoire BDTOPO introuvable : $bdtopo_dir"
        if [[ -n "$REGION" ]]; then
            log_error "  → Téléchargez d'abord avec : ./scripts/download-bdtopo.sh --region ${REGION}"
        else
            log_error "  → Téléchargez d'abord avec : ./scripts/download-bdtopo.sh --zones ${ZONES}"
        fi
        exit 1
    fi

    # Auto-détection année (prend la plus récente)
    if [[ -z "$YEAR" ]]; then
        YEAR=$(ls -1d "${bdtopo_dir}"/[0-9]* 2>/dev/null | sort -r | head -1 | xargs basename 2>/dev/null || echo "")
        if [[ -z "$YEAR" ]]; then
            log_error "Aucune année détectée dans : $bdtopo_dir"
            log_error "  → Spécifiez --year YYYY"
            exit 1
        fi
        log_info "Année auto-détectée : $YEAR"
    fi

    # Auto-détection version (prend la plus récente)
    if [[ -z "$VERSION" ]]; then
        VERSION=$(ls -1d "${bdtopo_dir}/${YEAR}"/v* 2>/dev/null | sort -r | head -1 | xargs basename 2>/dev/null || echo "")
        if [[ -z "$VERSION" ]]; then
            log_error "Aucune version détectée dans : ${bdtopo_dir}/${YEAR}"
            log_error "  → Spécifiez --version vYYYY.MM"
            exit 1
        fi
        log_info "Version auto-détectée : $VERSION"
    fi
}

# ---------------------------------------------------------------------------
# Résolution des chemins
# ---------------------------------------------------------------------------
resolve_paths() {
    _DATA_ROOT="${DATA_DIR}/bdtopo/${YEAR}/${VERSION}"
    _CONTOURS_DATA_ROOT="${CONTOURS_DIR:-${DATA_DIR}/contours}"
    _DEM_DATA_ROOT="${DEM_DIR:-${DATA_DIR}/dem}"
    _OSM_DATA_ROOT="${OSM_DIR:-${DATA_DIR}/osm}"
    _HIKING_TRAILS_DATA_ROOT="${HIKING_TRAILS_DIR:-${DATA_DIR}/hiking-trails}"

    # Nom de la carte pour l'output
    local zones_label
    if [[ -n "$REGION" ]]; then
        zones_label="$REGION"
    else
        zones_label=$(echo "$ZONES" | tr ',' '-')
    fi
    local map_name="${zones_label}-${VERSION}"

    _OUTPUT_DIR="${OUTPUT_BASE}/${YEAR}/${VERSION}/${zones_label}"
    _REPORT_FILE="${_OUTPUT_DIR}/mpforge-report.json"
    _IMGFORGE_REPORT_FILE="${_OUTPUT_DIR}/imgforge-report.json"

    # Auto-calcul base_id depuis le premier département
    if [[ -z "$BASE_ID" ]]; then
        local first_zone
        first_zone=$(echo "$ZONES" | cut -d',' -f1)
        # Extraire le numéro : D038 → 38, D02A → 2 (cas Corse simplifié)
        BASE_ID=$(echo "$first_zone" | sed 's/^D0*//' | sed 's/[A-Za-z]//g')
        if [[ -z "$BASE_ID" ]]; then
            BASE_ID=1
        fi
    fi

    # Auto-calcul family-name
    if [[ -z "$FAMILY_NAME" ]]; then
        FAMILY_NAME="IGN-BDTOPO-${zones_label}-${VERSION}"
    fi

    # Validation : vérifier que les données existent pour chaque zone
    local missing=false
    IFS=',' read -ra zone_array <<< "$ZONES"
    for zone in "${zone_array[@]}"; do
        if [[ ! -d "${_DATA_ROOT}/${zone}" ]]; then
            log_error "Données BDTOPO manquantes pour ${zone} : ${_DATA_ROOT}/${zone}"
            missing=true
        fi
    done

    if [[ "$missing" == true ]]; then
        if [[ -n "$REGION" ]]; then
            log_error "  → Téléchargez avec : ./scripts/download-bdtopo.sh --region ${REGION}"
        else
            log_error "  → Téléchargez avec : ./scripts/download-bdtopo.sh --zones ${ZONES}"
        fi
        exit 1
    fi
}

# ---------------------------------------------------------------------------
# Auto-découverte binaire mpforge / imgforge
# ---------------------------------------------------------------------------
find_binary() {
    local name="$1"
    local candidates=(
        "./tools/${name}/target/release/${name}"
        "../tools/${name}/target/release/${name}"
    )
    for c in "${candidates[@]}"; do
        if [[ -x "$c" ]]; then
            echo "$c"
            return 0
        fi
    done
    if command -v "$name" &>/dev/null; then
        command -v "$name"
        return 0
    fi
    echo ""
}

# ---------------------------------------------------------------------------
# Vérification prérequis
# ---------------------------------------------------------------------------
check_prerequisites() {
    log_step "Vérification des prérequis"

    # --- mpforge ---
    if [[ -z "$_MPFORGE" ]]; then
        _MPFORGE=$(find_binary mpforge)
    fi
    if [[ -z "$_MPFORGE" ]]; then
        log_error "mpforge introuvable"
        log_error "  → Compilez avec : cd tools/mpforge && cargo build --release"
        exit 1
    fi
    log_ok "mpforge : $_MPFORGE"

    # --- imgforge ---
    if [[ -z "$_IMGFORGE" ]]; then
        _IMGFORGE=$(find_binary imgforge)
    fi
    if [[ -z "$_IMGFORGE" ]]; then
        log_error "imgforge introuvable"
        log_error "  → Compilez avec : cd tools/imgforge && cargo build --release"
        exit 1
    fi
    log_ok "imgforge : $_IMGFORGE"

    # --- TYP file ---
    if [[ -n "$TYP_FILE" && ! -f "$TYP_FILE" ]]; then
        log_error "Fichier TYP introuvable : $TYP_FILE"
        exit 1
    fi

    # --- Config file (si custom) ---
    if [[ -n "$CONFIG_FILE" && ! -f "$CONFIG_FILE" ]]; then
        log_error "Config mpforge introuvable : $CONFIG_FILE"
        exit 1
    fi
}

# ---------------------------------------------------------------------------
# Extraction valeur entière depuis rapport JSON (sans jq)
# ---------------------------------------------------------------------------
json_extract_int() {
    local json_file="$1" key="$2" default="${3:-0}"
    local val
    val=$(grep -o "\"${key}\":[[:space:]]*[0-9]*" "$json_file" 2>/dev/null \
          | grep -o '[0-9]*$' | head -1) || true
    echo "${val:-$default}"
}

# ---------------------------------------------------------------------------
# Affichage des erreurs depuis le rapport JSON
# ---------------------------------------------------------------------------
show_report_errors() {
    local report="$1"
    [[ -f "$report" ]] || return 0

    log_error "── Erreurs du rapport JSON ──────────────────────────────"
    if grep -q '"tile":' "$report" 2>/dev/null; then
        grep -o '"tile":"[^"]*","error":"[^"]*"' "$report" 2>/dev/null \
            | sed 's/"tile":"//;s/","error":"/ : /;s/"$//' \
            | while IFS= read -r msg; do
                [[ -n "$msg" ]] && log_error "  • tuile $msg"
            done || true
    else
        grep -o '"message":"[^"]*"\|"error":"[^"]*"' "$report" 2>/dev/null \
            | sed 's/"message":"//;s/"error":"//;s/"$//' \
            | while IFS= read -r msg; do
                [[ -n "$msg" ]] && log_error "  • $msg"
            done || true
    fi
    log_error "─────────────────────────────────────────────────────────"
}

# ---------------------------------------------------------------------------
# Préparation de la configuration mpforge
# ---------------------------------------------------------------------------
prepare_config() {
    log_step "Préparation de la configuration"

    if [[ -z "$CONFIG_FILE" ]]; then
        CONFIG_FILE="pipeline/configs/ign-bdtopo/sources-shp.yaml"
    fi

    log_info "Config source : $CONFIG_FILE"
    log_info "Zones         : $ZONES"
    log_info "Données       : $_DATA_ROOT"
    log_info "Sortie        : $_OUTPUT_DIR"
    log_info "Base ID       : $BASE_ID"
    log_info "Jobs          : $JOBS"

    # Exporter les variables pour la substitution interne de mpforge
    export DATA_ROOT="$_DATA_ROOT"
    export CONTOURS_DATA_ROOT="$_CONTOURS_DATA_ROOT"
    export OSM_DATA_ROOT="$_OSM_DATA_ROOT"
    export HIKING_TRAILS_DATA_ROOT="$_HIKING_TRAILS_DATA_ROOT"
    export OUTPUT_DIR="$_OUTPUT_DIR"
    export BASE_ID
    export ZONES

    # Compter les .shp disponibles
    IFS=',' read -ra zone_array <<< "$ZONES"
    local shp_count=0
    for zone in "${zone_array[@]}"; do
        local count
        count=$(find "${_DATA_ROOT}/${zone}" -name "*.shp" -type f 2>/dev/null | wc -l)
        shp_count=$(( shp_count + count ))
    done

    if [[ "$shp_count" -eq 0 ]]; then
        log_error "Aucun fichier .shp trouvé dans les zones : $ZONES"
        exit 1
    fi
    log_ok "$shp_count fichier(s) .shp disponible(s) dans ${#zone_array[@]} zone(s)"
}

# ---------------------------------------------------------------------------
# Étape 1/2 — Lancement mpforge build
# ---------------------------------------------------------------------------
run_mpforge() {
    log_step "Étape 1/2 — mpforge build"

    mkdir -p "${_OUTPUT_DIR}/mp"

    # Nettoyage des .mp existants (sauf si --skip-existing)
    if [[ "$SKIP_EXISTING" == false ]]; then
        local existing_mp
        existing_mp=$(find "${_OUTPUT_DIR}/mp" -name "*.mp" -type f 2>/dev/null | wc -l)
        if [[ "$existing_mp" -gt 0 ]]; then
            log_info "Nettoyage de $existing_mp tuile(s) .mp existante(s)"
            rm -f "${_OUTPUT_DIR}"/mp/*.mp
        fi
    fi

    local -a cmd=(
        "$_MPFORGE" build
        --config "$CONFIG_FILE"
        --report "$_REPORT_FILE"
        --jobs "$JOBS"
    )

    [[ "$SKIP_EXISTING" == true ]] && cmd+=(--skip-existing)
    [[ "$VERBOSE_COUNT" -ge 1 ]] && cmd+=(-v)
    [[ "$VERBOSE_COUNT" -ge 2 ]] && cmd+=(-v)

    log_info "Commande : ${cmd[*]}"

    if [[ "$DRY_RUN" == true ]]; then
        echo -e "  ${YELLOW}[DRY-RUN]${NC} ${cmd[*]}"
        log_ok "Dry-run : commande mpforge affichée (non exécutée)"
        return 0
    fi

    local exit_code=0
    "${cmd[@]}" || exit_code=$?

    if [[ "$exit_code" -ne 0 ]]; then
        log_error "mpforge a échoué (exit code : $exit_code)"
        show_report_errors "$_REPORT_FILE"
        log_error "Pipeline arrêté — imgforge NON lancé"
        exit "$exit_code"
    fi

    log_ok "mpforge terminé avec succès"

    # Métriques
    if [[ -f "$_REPORT_FILE" ]]; then
        TILES_TOTAL=$(json_extract_int "$_REPORT_FILE" "tiles_generated" 0)
        TILES_FAILED=$(json_extract_int "$_REPORT_FILE" "tiles_failed" 0)
        TILES_SUCCESS=$(( TILES_TOTAL - TILES_FAILED ))
        MPFORGE_DURATION=$(json_extract_int "$_REPORT_FILE" "duration_seconds" 0)
        FEATURES_PROCESSED=$(json_extract_int "$_REPORT_FILE" "features_processed" 0)
        log_info "  Tuiles   : ${TILES_SUCCESS}/${TILES_TOTAL} (${TILES_FAILED} échec(s))"
        [[ "$FEATURES_PROCESSED" -gt 0 ]] && log_info "  Features : ${FEATURES_PROCESSED}"
        if [[ "$TILES_FAILED" -gt 0 ]]; then
            log_warn "${TILES_FAILED} tuile(s) en échec — le gmapsupp.img sera incomplet"
            PARTIAL_FAILURE=true
        fi
    fi
}

# ---------------------------------------------------------------------------
# Étape 2/2 — Lancement imgforge build
# ---------------------------------------------------------------------------
run_imgforge() {
    log_step "Étape 2/2 — imgforge build"

    local mp_dir="${_OUTPUT_DIR}/mp"
    mkdir -p "${_OUTPUT_DIR}/img"

    # Nettoyage des .img existants
    local existing_img
    existing_img=$(find "${_OUTPUT_DIR}/img" -type f 2>/dev/null | wc -l)
    if [[ "$existing_img" -gt 0 ]]; then
        log_info "Nettoyage de $existing_img fichier(s) existant(s) dans img/"
        rm -f "${_OUTPUT_DIR}"/img/*.*
    fi

    local -a cmd=(
        "$_IMGFORGE" build "$mp_dir"
        --output "${_OUTPUT_DIR}/img/gmapsupp.img"
        --jobs "$JOBS"
        --family-id "$FAMILY_ID"
        --product-id "$PRODUCT_ID"
        --family-name "$FAMILY_NAME"
        --series-name "$SERIES_NAME"
        --code-page "$CODE_PAGE"
        --lower-case
        --levels "$LEVELS"
        --copyright-message "$COPYRIGHT"
    )

    [[ "$WITH_ROUTE" == true ]] && cmd+=(--route)
    [[ -n "$TYP_FILE" ]] && cmd+=(--typ-file "$TYP_FILE")

    # DEM : ajouter les répertoires DEM pour chaque département
    # Le DEM est livré par département (D-codes), pas par région (R-codes)
    if [[ "$WITH_DEM" == true ]]; then
        local -a dem_departments
        if [[ -n "$REGION" && -n "${REGIONS_TO_DEPARTMENTS[$REGION]+x}" ]]; then
            IFS=',' read -ra dem_departments <<< "${REGIONS_TO_DEPARTMENTS[$REGION]}"
        else
            IFS=',' read -ra dem_departments <<< "$ZONES"
        fi
        for dept in "${dem_departments[@]}"; do
            local dem_dir="${_DEM_DATA_ROOT}/${dept}"
            if [[ -d "$dem_dir" ]]; then
                cmd+=(--dem "$dem_dir")
            else
                log_warn "Données DEM manquantes pour ${dept} : $dem_dir (ignoré)"
            fi
        done
        cmd+=(--dem-source-srs "EPSG:2154")
    fi

    [[ "$VERBOSE_COUNT" -ge 1 ]] && cmd+=(-v)
    [[ "$VERBOSE_COUNT" -ge 2 ]] && cmd+=(-v)

    log_info "Commande : ${cmd[*]}"

    if [[ "$DRY_RUN" == true ]]; then
        echo -e "  ${YELLOW}[DRY-RUN]${NC} ${cmd[*]}"
        log_ok "Dry-run : commande imgforge affichée (non exécutée)"
        return 0
    fi

    # Vérifier la présence de tuiles .mp
    local mp_count
    mp_count=$(find "$mp_dir" -name "*.mp" -type f 2>/dev/null | wc -l)
    if [[ "$mp_count" -eq 0 ]]; then
        log_error "Aucune tuile .mp trouvée dans : $mp_dir"
        exit 1
    fi
    log_info "  $mp_count tuile(s) .mp à compiler"

    local exit_code=0
    "${cmd[@]}" || exit_code=$?

    if [[ "$exit_code" -ne 0 ]]; then
        log_error "imgforge a échoué (exit code : $exit_code)"
        show_report_errors "$_IMGFORGE_REPORT_FILE"
        exit "$exit_code"
    fi

    if [[ ! -f "${_OUTPUT_DIR}/img/gmapsupp.img" ]]; then
        log_error "gmapsupp.img non produit dans : ${_OUTPUT_DIR}/img/"
        exit 1
    fi

    log_ok "gmapsupp.img produit : ${_OUTPUT_DIR}/img/gmapsupp.img"

    # Métriques
    if [[ -f "$_IMGFORGE_REPORT_FILE" ]]; then
        IMGFORGE_TILES_COMPILED=$(json_extract_int "$_IMGFORGE_REPORT_FILE" "tiles_compiled" 0)
        IMGFORGE_TILES_FAILED=$(json_extract_int "$_IMGFORGE_REPORT_FILE" "tiles_failed" 0)
        IMGFORGE_DURATION=$(json_extract_int "$_IMGFORGE_REPORT_FILE" "duration_seconds" 0)
        IMGFORGE_IMG_SIZE=$(json_extract_int "$_IMGFORGE_REPORT_FILE" "img_size_bytes" 0)
        log_info "  Tuiles compilées : ${IMGFORGE_TILES_COMPILED} (${IMGFORGE_TILES_FAILED} échec(s))"
        if [[ "$IMGFORGE_TILES_FAILED" -gt 0 ]]; then
            log_warn "${IMGFORGE_TILES_FAILED} tuile(s) en échec — carte incomplète"
            PARTIAL_FAILURE=true
        fi
    fi
}

# ---------------------------------------------------------------------------
# Résumé final
# ---------------------------------------------------------------------------
show_summary() {
    log_step "Résumé"

    local total_duration=$(( SECONDS - BUILD_START_TIME ))

    echo -e "  ${BOLD}Zones          :${NC}  $ZONES"
    echo -e "  ${BOLD}Millésime      :${NC}  $YEAR / $VERSION"
    echo ""

    if [[ "$DRY_RUN" == false ]]; then
        echo -e "  ${BOLD}[Phase 1 — mpforge]${NC}"
        echo -e "  Tuiles générées    : ${TILES_SUCCESS}/${TILES_TOTAL}"
        [[ "$TILES_FAILED" -gt 0 ]] && \
            echo -e "  ${YELLOW}${BOLD}Tuiles en échec  :${NC}  ${TILES_FAILED}"
        [[ "$FEATURES_PROCESSED" -gt 0 ]] && \
            echo -e "  Features           : ${FEATURES_PROCESSED}"
        if [[ "$MPFORGE_DURATION" -gt 0 ]]; then
            local m=$(( MPFORGE_DURATION / 60 )) s=$(( MPFORGE_DURATION % 60 ))
            echo -e "  Durée mpforge      : ${m}m${s}s"
        fi

        echo ""
        echo -e "  ${BOLD}[Phase 2 — imgforge]${NC}"
        echo -e "  Tuiles compilées   : ${IMGFORGE_TILES_COMPILED}"
        [[ "$IMGFORGE_TILES_FAILED" -gt 0 ]] && \
            echo -e "  ${YELLOW}Tuiles en échec  : ${IMGFORGE_TILES_FAILED}${NC}"
        if [[ "$IMGFORGE_DURATION" -gt 0 ]]; then
            local im=$(( IMGFORGE_DURATION / 60 )) is=$(( IMGFORGE_DURATION % 60 ))
            echo -e "  Durée imgforge     : ${im}m${is}s"
        fi
        echo ""
    fi

    local total_m=$(( total_duration / 60 )) total_s=$(( total_duration % 60 ))
    echo -e "  ${BOLD}Temps total     :${NC}  ${total_m}m${total_s}s"

    if [[ -f "${_OUTPUT_DIR}/img/gmapsupp.img" ]]; then
        local size_bytes
        if [[ "$IMGFORGE_IMG_SIZE" -gt 0 ]]; then
            size_bytes="$IMGFORGE_IMG_SIZE"
        else
            size_bytes=$(stat -c%s "${_OUTPUT_DIR}/img/gmapsupp.img" 2>/dev/null || echo 0)
        fi
        local size_hr
        size_hr=$(numfmt --to=iec-i --suffix=B "$size_bytes" 2>/dev/null \
                  || echo "${size_bytes} octets")
        echo -e "  ${BOLD}Taille img      :${NC}  ${size_hr}"
        echo -e "  ${BOLD}Emplacement     :${NC}  ${_OUTPUT_DIR}/img/gmapsupp.img"
    fi

    if [[ "$DRY_RUN" == true ]]; then
        echo -e "\n  ${YELLOW}${BOLD}MODE DRY-RUN — aucune commande exécutée${NC}"
    fi
    echo ""
}

# =============================================================================
# MAIN
# =============================================================================
main() {
    echo -e "${BOLD}${CYAN}"
    echo "  ┌─────────────────────────────────────────────────────────────────┐"
    echo "  │  build-garmin-map.sh — Pipeline mpforge → imgforge             │"
    echo "  │  BDTOPO → tuiles .mp → gmapsupp.img · v${SCRIPT_VERSION}                │"
    echo "  └─────────────────────────────────────────────────────────────────┘"
    echo -e "${NC}"

    parse_args "$@"
    BUILD_START_TIME=$SECONDS

    validate_params
    auto_detect_year_version
    resolve_paths
    validate_base_id
    check_prerequisites
    prepare_config
    run_mpforge
    run_imgforge
    show_summary

    if [[ "$PARTIAL_FAILURE" == true ]]; then
        log_warn "Pipeline terminé avec avertissements — carte partielle dans : ${_OUTPUT_DIR}/"
        exit 2
    fi
    log_ok "Pipeline terminé — carte disponible dans : ${_OUTPUT_DIR}/img/"
}

main "$@"
