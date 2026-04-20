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
# Auto-chargement pipeline/.env (credentials S3, etc.) — sans écraser l'env existant
# ---------------------------------------------------------------------------
_SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
_ENV_FILE="${_SCRIPT_DIR}/../pipeline/.env"
if [[ -f "$_ENV_FILE" ]]; then
    set -a
    # shellcheck disable=SC1090
    source "$_ENV_FILE"
    set +a
fi

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
CONFIG_FILE=""          # si vide : utilise sources.yaml avec envsubst
JOBS=8
MPFORGE_JOBS=""         # si vide : fallback sur $JOBS
IMGFORGE_JOBS=""        # si vide : fallback sur $JOBS (imgforge consomme bien plus de RAM que mpforge)

# imgforge
FAMILY_ID=1100
PRODUCT_ID=1
FAMILY_NAME=""          # Auto-calculé : IGN-BDTOPO-{ZONES}-{VERSION}
SERIES_NAME="IGN-BDTOPO-MAP"
CODE_PAGE=1252
LEVELS="24,22,20,18,16"
TYP_FILE="pipeline/resources/typfiles/I2023100.typ"
COPYRIGHT="©$(date +%Y) Allfab Studio - ©IGN BDTOPO - ©OpenStreetMap Les Contributeurs - Licence Ouverte Etalab 2.0"

# imgforge — optimisation taille IMG (opt-in, off par défaut)
REDUCE_POINT_DENSITY=""    # imgforge --reduce-point-density (réf. mkgmap : 4.0)
SIMPLIFY_POLYGONS=""       # imgforge --simplify-polygons (format "24:12,18:10,16:8")
MIN_SIZE_POLYGON=""        # imgforge --min-size-polygon (réf. mkgmap : 8)
MERGE_LINES=false          # imgforge --merge-lines
PACKAGING="legacy"         # imgforge --packaging (legacy | gmp)

# Contrôle
DRY_RUN=false
PUBLISH=false
PUBLISH_TARGET="${PUBLISH_TARGET:-local}"  # local | s3
SKIP_EXISTING=false
VERBOSE_COUNT=0         # 0=warn, 1=-v, 2=-vv
WITH_ROUTE=true
WITH_DEM=true

# Tech-spec #2 : profils multi-Data
DISABLE_PROFILES=false  # si true, passe --disable-profiles à mpforge (revalidation golden AC1)

# Tech-spec #2 : résolution driver GDAL ogr-polishmap.
# Le binaire mpforge est compilé contre GDAL système (pas de statique embed)
# et charge le driver ogr-polishmap dynamiquement. Si l'utilisateur a laissé
# le plugin système (/usr/lib/gdalplugins) non mis à jour, le chemin local
# est exposé via GDAL_DRIVER_PATH pour garantir les features multi-Data.
# Vide = pas d'override (utilise plugin système tel quel).
# Priorité de résolution auto (si GDAL_DRIVER_PATH non défini) :
#   1. ~/.gdal/plugins/ogr_PolishMap.so (install user)
#   2. ./tools/ogr-polishmap/build/ogr_PolishMap.so (build local release)
GDAL_DRIVER_PATH_OVERRIDE="${GDAL_DRIVER_PATH:-}"

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
# Tech-spec #2 AC17 : features skipées sur échec bucket additionnel (FFI/WKT).
SKIPPED_ADDITIONAL_GEOM=0

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
    # Quadrants Garmin (couverture TOPO France v7 PRO — départements uniquement, millésimes alignés)
    [FRANCE-SE]="D001,D003,D004,D005,D006,D007,D011,D013,D015,D02A,D02B,D026,D030,D034,D038,D042,D043,D048,D063,D066,D069,D073,D074,D083,D084"
    [FRANCE-SO]="D009,D012,D016,D017,D019,D023,D024,D031,D032,D033,D040,D046,D047,D064,D065,D079,D081,D082,D086,D087"
    [FRANCE-NE]="D002,D008,D010,D021,D025,D027,D039,D051,D052,D054,D055,D057,D058,D059,D060,D062,D067,D068,D070,D071,D075,D076,D077,D078,D080,D088,D089,D090,D091,D092,D093,D094,D095"
    [FRANCE-NO]="D014,D018,D022,D028,D029,D035,D036,D037,D041,D044,D045,D049,D050,D053,D056,D061,D072,D075,D077,D078,D085,D091,D092,D093,D094,D095"
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
# Publication : mapping R-code → trigramme, labels humains, résolution type/slug
# ---------------------------------------------------------------------------
declare -A R_CODE_TO_TRIGRAM=(
    [R11]=IDF [R24]=CVL [R27]=BFC [R28]=NOR [R32]=HDF [R44]=GES
    [R52]=PDL [R53]=BRE [R75]=NAQ [R76]=OCC [R84]=ARA [R93]=PAC [R94]=COR
)

declare -A REGION_LABELS=(
    [IDF]="Île-de-France" [CVL]="Centre-Val de Loire" [BFC]="Bourgogne-Franche-Comté"
    [NOR]="Normandie" [HDF]="Hauts-de-France" [GES]="Grand Est"
    [PDL]="Pays de la Loire" [BRE]="Bretagne" [NAQ]="Nouvelle-Aquitaine"
    [OCC]="Occitanie" [ARA]="Auvergne-Rhône-Alpes"
    [PAC]="Provence-Alpes-Côte d'Azur" [COR]="Corse"
    [FRANCE-SE]="France Sud-Est" [FRANCE-SO]="France Sud-Ouest"
    [FRANCE-NE]="France Nord-Est" [FRANCE-NO]="France Nord-Ouest"
    [FRANCE-SUD]="France Sud" [FRANCE-NORD]="France Nord"
    [FXX]="France métropolitaine"
)

# Globals peuplés par resolve_coverage_info
_PUB_TYPE=""
_PUB_SLUG=""
_PUB_LABEL=""

# Noms de fichier .img (peuplés par resolve_paths après calcul de FAMILY_NAME)
_IMG_FILENAME=""
_IMG_LATEST_NAME=""

to_lower() { printf '%s' "$1" | tr '[:upper:]' '[:lower:]'; }

resolve_coverage_info() {
    _PUB_TYPE=""
    _PUB_SLUG=""
    _PUB_LABEL=""

    if [[ -n "$REGION" ]]; then
        case "$REGION" in
            FRANCE-SE|FRANCE-SO|FRANCE-NE|FRANCE-NO)
                _PUB_TYPE="quadrant"
                ;;
            FXX|FRANCE-SUD|FRANCE-NORD)
                _PUB_TYPE="national"
                ;;
            *)
                _PUB_TYPE="region"
                ;;
        esac
        _PUB_SLUG=$(to_lower "$REGION")
        _PUB_LABEL="${REGION_LABELS[$REGION]:-$REGION}"
    else
        _PUB_TYPE="departement"
        local sorted
        sorted=$(echo "$ZONES" | tr ',' '\n' | sort -u | paste -sd, -)
        _PUB_SLUG=$(to_lower "$sorted" | tr ',' '-')
        _PUB_LABEL="Départements: $sorted"
    fi
}

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
    --config FILE           Config YAML mpforge custom (défaut auto-résolu :
                              - Quadrant Garmin (FRANCE-SE/SO/NE/NO) → france-quadrant/sources.yaml
                              - DOM (D971/D972/D973/D974/D976)       → outre-mer/<slug>/sources.yaml
                              - Sinon (département, région, FXX)     → departement/sources.yaml)

MPFORGE / IMGFORGE :
    --jobs N                Parallélisation commune par défaut (défaut: 8)
    --mpforge-jobs N        Parallélisation mpforge (surcharge --jobs)
    --imgforge-jobs N       Parallélisation imgforge (surcharge --jobs ;
                            pensez à réduire : imgforge consomme beaucoup de RAM)
    --skip-existing         Passer les tuiles .mp déjà présentes
    --disable-profiles      Tech-spec #2 : bypasse le catalogue externe
                            generalize_profiles_path (l'inline reste actif).
                            Utilisé pour regénérer le golden baseline
                            mono-Data (AC1). Accepte aussi l'env var
                            MPFORGE_PROFILES=off.
    --gdal-driver-path PATH Override GDAL_DRIVER_PATH pour charger le driver
                            ogr-polishmap frais (tech-spec #2). Résolution
                            auto si vide :
                              1. ~/.gdal/plugins/
                              2. ./tools/ogr-polishmap/build/
                            Sinon utilise le plugin GDAL système.

IMGFORGE :
    --family-id N           Family ID Garmin (défaut: 1100)
    --product-id N          Product ID Garmin (défaut: 1)
    --family-name STR       Nom de la carte ET du fichier .img produit
                            (défaut: auto IGN-BDTOPO-{ZONES}-{VERSION} →
                            IGN-BDTOPO-FRANCE-SE-v2026.03.img)
    --series-name STR       Nom de la série (défaut: IGN-BDTOPO-MAP)
    --code-page N           Code page encodage (défaut: 1252)
    --levels STR            Niveaux de zoom (défaut: 24,22,20,18,16)
    --typ FILE              Fichier TYP styles (défaut: pipeline/resources/typfiles/I2023100.typ)
    --copyright STR         Message copyright
    --no-route              Désactiver le routage
    --no-dem                Désactiver le DEM (relief ombré)
    --reduce-point-density F   Douglas-Peucker épsilon lignes (réf. mkgmap : 4.0)
    --simplify-polygons SPEC   Épsilon DP polygones par résolution (ex. "24:12,18:10,16:8")
    --min-size-polygon N       Filtre polygones < N unités carte (réf. mkgmap : 8)
    --merge-lines              Fusionne polylignes adjacentes même type/label
    --packaging MODE           legacy (6 FAT par tuile, défaut) | gmp (1 .GMP consolidé)

CONTRÔLE :
    --dry-run               Simuler sans exécuter
    --publish               Publier le .img (cible: --publish-target, défaut local)
                            Local: copie vers site/docs/telechargements/
                            S3   : upload vers bucket Garage (voir pipeline/.env)
    --publish-target TGT    local | s3 (défaut: local ; env: PUBLISH_TARGET)
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

    # Build + publication dans le site
    ./scripts/build-garmin-map.sh --region ARA --publish

    # Quadrant FRANCE-SE optimisé taille (config auto-résolue vers france-quadrant/sources.yaml)
    ./scripts/build-garmin-map.sh --region FRANCE-SE \
        --reduce-point-density 4.0 \
        --simplify-polygons "24:12,18:10,16:8" \
        --min-size-polygon 8

    # Carte DOM (config auto-résolue vers outre-mer/<slug>/sources.yaml)
    ./scripts/build-garmin-map.sh --zones D971   # Guadeloupe
    ./scripts/build-garmin-map.sh --zones D974   # La Réunion
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
            --mpforge-jobs)  MPFORGE_JOBS="$2"; shift 2 ;;
            --imgforge-jobs) IMGFORGE_JOBS="$2"; shift 2 ;;
            --skip-existing) SKIP_EXISTING=true; shift ;;
            --disable-profiles) DISABLE_PROFILES=true; shift ;;
            --gdal-driver-path) GDAL_DRIVER_PATH_OVERRIDE="$2"; shift 2 ;;
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
            --reduce-point-density) REDUCE_POINT_DENSITY="$2"; shift 2 ;;
            --simplify-polygons)    SIMPLIFY_POLYGONS="$2";    shift 2 ;;
            --min-size-polygon)     MIN_SIZE_POLYGON="$2";     shift 2 ;;
            --merge-lines)          MERGE_LINES=true;          shift   ;;
            --packaging)            PACKAGING="$2";            shift 2 ;;
            --dry-run)       DRY_RUN=true; shift ;;
            --publish)       PUBLISH=true; shift ;;
            --publish-target=*) PUBLISH_TARGET="${1#*=}"; shift ;;
            --publish-target)   PUBLISH_TARGET="$2"; shift 2 ;;
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

    # --- packaging : legacy | gmp ---
    if [[ "$PACKAGING" != "legacy" && "$PACKAGING" != "gmp" ]]; then
        log_error "--packaging : valeur attendue 'legacy' ou 'gmp', reçu '${PACKAGING}'"
        errors=$(( errors + 1 ))
    fi

    # Fallback des jobs par étape sur $JOBS si non spécifiés
    MPFORGE_JOBS="${MPFORGE_JOBS:-$JOBS}"
    IMGFORGE_JOBS="${IMGFORGE_JOBS:-$JOBS}"

    # --- mpforge-jobs / imgforge-jobs : entiers positifs ---
    for _pair in "mpforge-jobs:${MPFORGE_JOBS}" "imgforge-jobs:${IMGFORGE_JOBS}"; do
        _flag="${_pair%%:*}"
        _val="${_pair#*:}"
        if ! [[ "$_val" =~ ^[0-9]+$ ]] || [[ "$_val" -lt 1 ]]; then
            log_error "--${_flag} : doit être un entier positif, reçu '${_val}'"
            errors=$(( errors + 1 ))
        elif [[ "$_val" -gt 64 ]]; then
            log_warn "--${_flag} ${_val} : valeur élevée, consommation RAM importante à prévoir"
        fi
    done
    unset _pair _flag _val

    # --- imgforge opt-in : numériques strictement positifs si définis ---
    if [[ -n "$REDUCE_POINT_DENSITY" ]]; then
        if ! [[ "$REDUCE_POINT_DENSITY" =~ ^[0-9]+(\.[0-9]+)?$|^[0-9]*\.[0-9]+$ ]] \
           || ! awk -v v="$REDUCE_POINT_DENSITY" 'BEGIN{exit !(v+0 > 0)}'; then
            log_error "--reduce-point-density : doit être un nombre strictement positif, reçu '${REDUCE_POINT_DENSITY}'"
            errors=$(( errors + 1 ))
        fi
    fi
    if [[ -n "$MIN_SIZE_POLYGON" ]]; then
        if ! [[ "$MIN_SIZE_POLYGON" =~ ^[0-9]+$ ]] || [[ "$MIN_SIZE_POLYGON" -lt 1 ]]; then
            log_error "--min-size-polygon : doit être un entier strictement positif, reçu '${MIN_SIZE_POLYGON}'"
            errors=$(( errors + 1 ))
        fi
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
        # Cas spéciaux Corse : 2A et 2B partagent le même numéro → mapping INSEE
        # statistique 201/202 pour garantir l'unicité (indispensable Garmin).
        case "$first_zone" in
            D02A) BASE_ID=201 ;;
            D02B) BASE_ID=202 ;;
            *)
                # Extraire le numéro : D038 → 38, D971 → 971
                BASE_ID=$(echo "$first_zone" | sed 's/^D0*//' | sed 's/[A-Za-z]//g')
                ;;
        esac
        if [[ -z "$BASE_ID" ]]; then
            BASE_ID=1
        fi
    fi

    # Auto-calcul family-name
    if [[ -z "$FAMILY_NAME" ]]; then
        FAMILY_NAME="IGN-BDTOPO-${zones_label}-${VERSION}"
    fi

    # Nom du fichier .img produit et nom stable pour l'alias latest/ :
    # - _IMG_FILENAME     : fichier versionné (ex: IGN-BDTOPO-FRANCE-SE-v2026.03.img)
    # - _IMG_LATEST_NAME  : alias stable sans suffixe de version (URL durable dans les pages MD)
    #   Si FAMILY_NAME se termine par -v{YYYY.MM}, on strip ce suffixe ; sinon identique.
    _IMG_FILENAME="${FAMILY_NAME}.img"
    if [[ "$FAMILY_NAME" =~ ^(.+)-v[0-9]{4}\.(0[1-9]|1[0-2])$ ]]; then
        _IMG_LATEST_NAME="${BASH_REMATCH[1]}.img"
    else
        _IMG_LATEST_NAME="${FAMILY_NAME}.img"
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
    local found=""
    # Priorité 1 : binaire installé dans le $PATH (ex: /usr/local/bin)
    if command -v "$name" &>/dev/null; then
        found=$(command -v "$name")
    else
        # Fallback : build locale cargo release
        local candidates=(
            "./tools/${name}/target/release/${name}"
            "../tools/${name}/target/release/${name}"
        )
        local c
        for c in "${candidates[@]}"; do
            if [[ -x "$c" ]]; then
                found="$c"
                break
            fi
        done
    fi
    # Log sur stderr pour que stdout reste capturable par l'appelant ($(find_binary …))
    if [[ -n "$found" && "${VERBOSE_COUNT:-0}" -ge 1 ]]; then
        echo "[INFO]  find_binary: ${name} → ${found}" >&2
    fi
    echo "$found"
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

    # --- Driver ogr-polishmap (tech-spec #2 multi-Data) ---
    # mpforge charge le driver dynamiquement via libgdal ; si le plugin
    # système /usr/lib/gdalplugins/ogr_PolishMap.so n'a pas été mis à jour
    # pour supporter MULTI_GEOM_FIELDS, il faut pointer vers une version
    # locale à jour. Résolution par ordre de priorité :
    #   1. --gdal-driver-path ou env GDAL_DRIVER_PATH (utilisateur)
    #   2. ~/.gdal/plugins/ogr_PolishMap.so (install user local)
    #   3. tools/ogr-polishmap/build/ogr_PolishMap.so (build cargo/cmake)
    #   4. Aucun override → plugin système utilisé tel quel (warn si l'user
    #      a construit multi-Data mais n'a pas installé le plugin frais).
    if [[ -z "$GDAL_DRIVER_PATH_OVERRIDE" ]]; then
        local user_plugin="${HOME}/.gdal/plugins/ogr_PolishMap.so"
        local local_build="$(pwd)/tools/ogr-polishmap/build/ogr_PolishMap.so"
        if [[ -f "$user_plugin" ]]; then
            GDAL_DRIVER_PATH_OVERRIDE="${HOME}/.gdal/plugins"
        elif [[ -f "$local_build" ]]; then
            GDAL_DRIVER_PATH_OVERRIDE="$(pwd)/tools/ogr-polishmap/build"
        fi
    fi
    if [[ -n "$GDAL_DRIVER_PATH_OVERRIDE" ]]; then
        export GDAL_DRIVER_PATH="$GDAL_DRIVER_PATH_OVERRIDE"
        log_ok "GDAL_DRIVER_PATH : $GDAL_DRIVER_PATH"
    else
        log_warn "Aucun plugin ogr-polishmap local détecté — mpforge utilisera"
        log_warn "  le plugin système. Si celui-ci n'a pas été recompilé"
        log_warn "  depuis tech-spec #2, le writer multi-Data sera silencieusement"
        log_warn "  ignoré (sortie mono-Data). Rebuild : cmake --build tools/ogr-polishmap/build"
    fi

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

    # --- Outils requis uniquement si --publish (GNU coreutils, Linux uniquement) ---
    if [[ "$PUBLISH" == true ]]; then
        if ! command -v jq &>/dev/null; then
            log_error "jq requis pour --publish (manipulation manifest.json)"
            log_error "  → Installez : sudo dnf install jq  (ou apt install jq)"
            exit 1
        fi
        if ! command -v sha256sum &>/dev/null; then
            log_error "sha256sum requis pour --publish"
            exit 1
        fi
        if ! command -v python3 &>/dev/null; then
            log_error "python3 requis pour --publish (patch idempotent des pages MD)"
            exit 1
        fi
        log_ok "jq / sha256sum / python3 : disponibles"

        case "$PUBLISH_TARGET" in
            local) : ;;
            s3)
                if ! command -v rclone &>/dev/null; then
                    log_error "rclone requis pour --publish-target=s3 (dnf install rclone)"
                    exit 1
                fi
                local _v
                for _v in RCLONE_CONFIG_GARAGE_ACCESS_KEY_ID RCLONE_CONFIG_GARAGE_SECRET_ACCESS_KEY \
                          RCLONE_CONFIG_GARAGE_ENDPOINT S3_BUCKET PUBLIC_URL_BASE; do
                    if [[ -z "${!_v:-}" ]]; then
                        log_error "Variable $_v manquante (voir pipeline/.env.example)"
                        exit 1
                    fi
                done
                log_ok "rclone + vars S3 : OK (bucket ${S3_BUCKET})"
                ;;
            *)
                log_error "--publish-target invalide : '${PUBLISH_TARGET}' (attendu: local|s3)"
                exit 1
                ;;
        esac
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

    # Auto-résolution CONFIG_FILE selon le scope :
    #   - Quadrants Garmin (FRANCE-SE, …) → france-quadrant/sources.yaml
    #   - DOM (D971/D972/D973/D974/D976)  → outre-mer/<slug>/sources.yaml
    #   - Sinon (département métro, région R-code, FXX…) → departement/sources.yaml
    if [[ -z "$CONFIG_FILE" ]]; then
        case "$REGION" in
            FRANCE-SE|FRANCE-SO|FRANCE-NE|FRANCE-NO)
                CONFIG_FILE="pipeline/configs/ign-bdtopo/france-quadrant/sources.yaml"
                ;;
            *)
                case "$ZONES" in
                    D971) CONFIG_FILE="pipeline/configs/ign-bdtopo/outre-mer/la-guadeloupe/sources.yaml" ;;
                    D972) CONFIG_FILE="pipeline/configs/ign-bdtopo/outre-mer/la-martinique/sources.yaml" ;;
                    D973) CONFIG_FILE="pipeline/configs/ign-bdtopo/outre-mer/la-guyane/sources.yaml" ;;
                    D974) CONFIG_FILE="pipeline/configs/ign-bdtopo/outre-mer/la-reunion/sources.yaml" ;;
                    D976) CONFIG_FILE="pipeline/configs/ign-bdtopo/outre-mer/mayotte/sources.yaml" ;;
                    *)    CONFIG_FILE="pipeline/configs/ign-bdtopo/departement/sources.yaml" ;;
                esac
                ;;
        esac
    fi

    log_info "Config source : $CONFIG_FILE"
    log_info "Zones         : $ZONES"
    log_info "Données       : $_DATA_ROOT"
    log_info "Sortie        : $_OUTPUT_DIR"
    log_info "Base ID       : $BASE_ID"
    log_info "Jobs          : mpforge=${MPFORGE_JOBS} / imgforge=${IMGFORGE_JOBS}"

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
        --jobs "$MPFORGE_JOBS"
    )

    [[ "$SKIP_EXISTING" == true ]] && cmd+=(--skip-existing)
    [[ "$DISABLE_PROFILES" == true ]] && cmd+=(--disable-profiles)
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
        SKIPPED_ADDITIONAL_GEOM=$(json_extract_int "$_REPORT_FILE" "skipped_additional_geom" 0)
        log_info "  Tuiles   : ${TILES_SUCCESS}/${TILES_TOTAL} (${TILES_FAILED} échec(s))"
        [[ "$FEATURES_PROCESSED" -gt 0 ]] && log_info "  Features : ${FEATURES_PROCESSED}"
        if [[ "$TILES_FAILED" -gt 0 ]]; then
            log_warn "${TILES_FAILED} tuile(s) en échec — le gmapsupp.img sera incomplet"
            PARTIAL_FAILURE=true
        fi
        # Tech-spec #2 AC17 : alerte si des features ont été skipées à cause
        # d'un échec bucket additionnel (FFI OGR_F_SetGeomField ou WKT).
        if [[ "$SKIPPED_ADDITIONAL_GEOM" -gt 0 ]]; then
            log_warn "${SKIPPED_ADDITIONAL_GEOM} feature(s) skipée(s) suite à un échec sur"
            log_warn "  bucket multi-Data additionnel (AC17). Cherche 'FEATURE SKIPPED'"
            log_warn "  dans les logs mpforge pour le détail."
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

    # Mode publish-only : si --skip-existing actif et .img cible déjà présent,
    # on skippe le rebuild complet. Permet un cycle publication sans 12-15 min
    # d'imgforge quand la carte est déjà validée.
    local target_img="${_OUTPUT_DIR}/img/${_IMG_FILENAME}"
    if [[ "$SKIP_EXISTING" == true && -f "$target_img" ]]; then
        log_info "IMG cible déjà présent + --skip-existing : skip phase 2 imgforge"
        log_info "  → $target_img"
        return 0
    fi

    # Nettoyage des .img existants
    local existing_img
    existing_img=$(find "${_OUTPUT_DIR}/img" -type f 2>/dev/null | wc -l)
    if [[ "$existing_img" -gt 0 ]]; then
        log_info "Nettoyage de $existing_img fichier(s) existant(s) dans img/"
        rm -f "${_OUTPUT_DIR}"/img/*.*
    fi

    local -a cmd=(
        "$_IMGFORGE" build "$mp_dir"
        --output "${_OUTPUT_DIR}/img/${_IMG_FILENAME}"
        --jobs "$IMGFORGE_JOBS"
        --family-id "$FAMILY_ID"
        --product-id "$PRODUCT_ID"
        --family-name "$FAMILY_NAME"
        --series-name "$SERIES_NAME"
        --code-page "$CODE_PAGE"
        --lower-case
        --levels "$LEVELS"
        --copyright-message "$COPYRIGHT"
    )

    if [[ "$WITH_ROUTE" == true ]]; then
        cmd+=(--route)
    else
        cmd+=(--no-route)
    fi
    [[ -n "$TYP_FILE" ]] && cmd+=(--typ-file "$TYP_FILE")

    cmd+=(--packaging "$PACKAGING")

    if [[ -n "$REDUCE_POINT_DENSITY" || -n "$SIMPLIFY_POLYGONS" || -n "$MIN_SIZE_POLYGON" || "$MERGE_LINES" == true ]]; then
        log_info "Optimisations imgforge actives :"
        [[ -n "$REDUCE_POINT_DENSITY" ]] && { cmd+=(--reduce-point-density "$REDUCE_POINT_DENSITY"); log_info "  --reduce-point-density ${REDUCE_POINT_DENSITY}"; }
        [[ -n "$SIMPLIFY_POLYGONS" ]]    && { cmd+=(--simplify-polygons "$SIMPLIFY_POLYGONS");       log_info "  --simplify-polygons ${SIMPLIFY_POLYGONS}"; }
        [[ -n "$MIN_SIZE_POLYGON" ]]     && { cmd+=(--min-size-polygon "$MIN_SIZE_POLYGON");         log_info "  --min-size-polygon ${MIN_SIZE_POLYGON}"; }
        [[ "$MERGE_LINES" == true ]]     && { cmd+=(--merge-lines);                                  log_info "  --merge-lines"; }
    fi

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

    if [[ ! -f "${_OUTPUT_DIR}/img/${_IMG_FILENAME}" ]]; then
        log_error "${_IMG_FILENAME} non produit dans : ${_OUTPUT_DIR}/img/"
        exit 1
    fi

    log_ok "${_IMG_FILENAME} produit : ${_OUTPUT_DIR}/img/${_IMG_FILENAME}"

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
# Publication : met à jour manifest.json (upsert version, recalcule latest)
# ---------------------------------------------------------------------------
update_manifest() {
    local type="$1" slug="$2" label="$3" version="$4" size="$5" sha256="$6"
    local img_file="$7" latest_file="$8"
    local public_url_base="${9:-}"
    local manifest="site/docs/telechargements/manifest.json"
    local now
    now=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    mkdir -p "$(dirname "$manifest")"
    # Init si absent, vide, ou JSON invalide (jq sur fichier vide → sortie vide + rc=0
    # qui écraserait manifest.json avec du vide).
    if [[ ! -s "$manifest" ]] || ! jq empty "$manifest" >/dev/null 2>&1; then
        echo '{"generated_at":"","coverages":{}}' > "$manifest"
    fi

    local key="${type}/${slug}"
    local rel_path latest_rel_path storage_type storage_endpoint
    if [[ -n "$public_url_base" ]]; then
        rel_path="${public_url_base}/${type}/${slug}/${version}/${img_file}"
        latest_rel_path=""  # latest_url calculé par jq depuis la version latest
        storage_type="s3"
        storage_endpoint="$public_url_base"
    else
        rel_path="files/${type}/${slug}/${version}/${img_file}"
        latest_rel_path="files/${type}/${slug}/latest/${latest_file}"
        storage_type="local"
        storage_endpoint=""
    fi
    local tmp="${manifest}.tmp"

    # build_params : valeurs dynamiques nécessaires pour régénérer les commandes
    # download-bdtopo.sh et build-garmin-map.sh de cette publication. Le front
    # (downloads-manifest.js) les injecte dans un template statique.
    local bp_zones="${ZONES}"
    local bp_base_id="${BASE_ID}"
    local bp_year="${YEAR}"
    local bp_version="${VERSION}"
    local bp_family_id="${FAMILY_ID}"
    local bp_family_name="${FAMILY_NAME}"
    local bp_copyright="${COPYRIGHT}"

    jq \
        --arg key "$key" \
        --arg type "$type" \
        --arg slug "$slug" \
        --arg label "$label" \
        --arg version "$version" \
        --arg now "$now" \
        --arg path "$rel_path" \
        --arg sha256 "$sha256" \
        --arg file "$img_file" \
        --arg latest_file "$latest_file" \
        --arg latest_path "$latest_rel_path" \
        --arg storage_type "$storage_type" \
        --arg storage_endpoint "$storage_endpoint" \
        --argjson size "$size" \
        --arg bp_zones "$bp_zones" \
        --arg bp_base_id "$bp_base_id" \
        --arg bp_year "$bp_year" \
        --arg bp_version "$bp_version" \
        --arg bp_family_id "$bp_family_id" \
        --arg bp_family_name "$bp_family_name" \
        --arg bp_copyright "$bp_copyright" \
        '
        .generated_at = $now
        | .storage = (
            if $storage_type == "s3"
            then {type:"s3", endpoint_public:$storage_endpoint}
            else {type:"local"}
            end
          )
        | .coverages[$key] = (
            (.coverages[$key] // {type:$type, slug:$slug, label:$label, latest:"", versions:[]})
            | .type = $type
            | .slug = $slug
            | .label = $label
            | .latest_file = $latest_file
            | (if $latest_path != "" then .latest_path = $latest_path else del(.latest_path) end)
            | .versions = (
                (.versions // [] | map(select(.version != $version)))
                + [{
                    version: $version,
                    published_at: $now,
                    size_bytes: $size,
                    sha256: $sha256,
                    file: $file,
                    path: $path,
                    build_params: {
                        zones: $bp_zones,
                        base_id: $bp_base_id,
                        year: $bp_year,
                        version: $bp_version,
                        family_id: $bp_family_id,
                        family_name: $bp_family_name,
                        copyright: $bp_copyright
                    }
                }]
              )
            | .versions |= sort_by(.version)
            | .latest = (.versions | map(.version) | max // "")
            | (.latest) as $lv
            | .latest_url = ((.versions | map(select(.version == $lv)) | first // {path:""}).path)
          )
        ' "$manifest" > "$tmp" && mv "$tmp" "$manifest"

    log_ok "manifest.json mis à jour (${key} → ${version})"
}

# ---------------------------------------------------------------------------
# Publication : patch idempotent des pages MD (remplace (#) par le lien latest/)
# ---------------------------------------------------------------------------
patch_download_page() {
    local type="$1" slug="$2" latest_file="$3"
    local public_url_base="${4:-}" version="${5:-}" img_filename="${6:-}"
    local page=""
    local anchor=""

    case "$type" in
        region)
            page="site/docs/telechargements/regions.md"
            local upper r_code found=""
            upper=$(echo "$slug" | tr '[:lower:]' '[:upper:]')
            for r_code in "${!R_CODE_TO_TRIGRAM[@]}"; do
                if [[ "${R_CODE_TO_TRIGRAM[$r_code]}" == "$upper" ]]; then
                    found="$r_code"
                    break
                fi
            done
            if [[ -z "$found" ]]; then
                log_warn "Pas de R-code connu pour slug '$slug' — pas de patch MD"
                return 0
            fi
            anchor=" - ${found}"
            ;;
        quadrant|national)
            page="site/docs/telechargements/france.md"
            # Ancres terminées par `$'\n'` pour éviter les collisions de préfixe
            # (ex: "France NORD" ⊂ "France NORD-EST").
            case "$slug" in
                fxx)         anchor=$'} France métropolitaine\n' ;;
                france-nord) anchor=$'} France NORD\n' ;;
                france-sud)  anchor=$'} France SUD\n' ;;
                france-ne)   anchor=$'} France NORD-EST\n' ;;
                france-no)   anchor=$'} France NORD-OUEST\n' ;;
                france-se)   anchor=$'} France SUD-EST\n' ;;
                france-so)   anchor=$'} France SUD-OUEST\n' ;;
                *)
                    log_warn "france.md ne contient pas encore de section pour '${slug}' — pas de patch MD"
                    return 0
                    ;;
            esac
            ;;
        departement)
            # Les slugs multi-zones (ex: d038-d069) n'ont pas de ligne dédiée
            # dans departement.md → skip volontaire.
            if [[ "$slug" == *-* ]]; then
                log_info "Slug département multi-zones (${slug}) — pas de patch MD"
                return 0
            fi
            # DROM (D971/D972/D973/D974/D976) → outre-mer.md, sinon metropolitan.
            case "$slug" in
                d971|d972|d973|d974|d976)
                    page="site/docs/telechargements/outre-mer.md" ;;
                *)
                    page="site/docs/telechargements/departement.md" ;;
            esac
            # Ancre = code uppercase (ex: D038) en début de cellule du tableau
            anchor=$(printf '%s' "$slug" | tr '[:lower:]' '[:upper:]')
            ;;
        *)
            log_warn "Type de couverture inconnu '${type}' — pas de patch MD"
            return 0
            ;;
    esac

    local url url_prefix
    if [[ -n "$public_url_base" ]]; then
        url="${public_url_base}/${type}/${slug}/${version}/${img_filename}"
        url_prefix="${public_url_base}/${type}/${slug}/"
    else
        url="files/${type}/${slug}/latest/${latest_file}"
        url_prefix="files/${type}/${slug}/latest/"
    fi

    if [[ ! -f "$page" ]]; then
        log_warn "Page MD introuvable : $page"
        return 0
    fi

    if grep -qF "${url}" "$page"; then
        log_info "Lien déjà patché : ${url}"
        return 0
    fi

    # Migration cross-mode : en mode S3, remplacer un éventuel lien local
    # "files/${type}/${slug}/latest/..." par la nouvelle URL absolue S3.
    if [[ -n "$public_url_base" ]]; then
        local legacy_prefix="files/${type}/${slug}/latest/"
        if grep -qF "${legacy_prefix}" "$page"; then
            local mig_rc=0
            python3 - "$page" "$legacy_prefix" "$url" <<'PY' || mig_rc=$?
import sys, re
page, prefix, new_url = sys.argv[1], sys.argv[2], sys.argv[3]
with open(page, 'r', encoding='utf-8') as f:
    src = f.read()
pat = re.compile(r'\(' + re.escape(prefix) + r'[^)]+\)')
new_src, n = pat.subn('(' + new_url + ')', src, count=1)
if n == 1:
    with open(page, 'w', encoding='utf-8') as f:
        f.write(new_src)
    sys.exit(0)
sys.exit(2)
PY
            if [[ "$mig_rc" -eq 0 ]]; then
                log_ok "Page MD migrée local→S3 : ${page} → ${url}"
                return 0
            fi
        fi
    fi

    # Migration : si la page contient un ancien lien latest/ pour cette
    # couverture (ex: latest/gmapsupp.img ou nom de fichier précédent),
    # on le remplace par la nouvelle URL plutôt que de tenter un re-patch du (#).
    if grep -qF "${url_prefix}" "$page"; then
        local mig_rc=0
        python3 - "$page" "$url_prefix" "$url" <<'PY' || mig_rc=$?
import sys, re
page, prefix, new_url = sys.argv[1], sys.argv[2], sys.argv[3]
with open(page, 'r', encoding='utf-8') as f:
    src = f.read()
pat = re.compile(r'\(' + re.escape(prefix) + r'[^)]+\)')
new_src, n = pat.subn('(' + new_url + ')', src, count=1)
if n == 1:
    with open(page, 'w', encoding='utf-8') as f:
        f.write(new_src)
    sys.exit(0)
sys.exit(2)
PY
        if [[ "$mig_rc" -eq 0 ]]; then
            log_ok "Page MD migrée : ${page} → ${url}"
            return 0
        fi
    fi

    local rc=0
    python3 - "$page" "$anchor" "$url" <<'PY' || rc=$?
import sys, re
page, anchor, url = sys.argv[1], sys.argv[2], sys.argv[3]
with open(page, 'r', encoding='utf-8') as f:
    src = f.read()

patched = False

# Stratégie 1 : ligne de tableau Markdown (ex: "| D038 | Isère | ...(#)... |")
# L'ancre doit apparaître dans une cellule encadrée par `|` pour éviter les
# faux positifs dans le préambule ou un bloc de code.
if not patched:
    lines = src.splitlines(keepends=True)
    anchor_cell = re.compile(r'\|\s*' + re.escape(anchor) + r'\s*\|')
    for i, line in enumerate(lines):
        if not line.lstrip().startswith('|'):
            continue
        if not anchor_cell.search(line):
            continue
        new_line, n = re.subn(r'\]\(\#\)', '](' + url + ')', line, count=1)
        if n == 1:
            lines[i] = new_line
            src = ''.join(lines)
            patched = True
            break

# Stratégie 2 : carte grid "-   …" — patche UNIQUEMENT le bloc contenant
# l'ancre. Évite la collision "France SUD" ⊂ "France SUD-EST" et les patches
# croisés entre cartes adjacentes.
if not patched:
    parts = re.split(r'(\n-   )', src)
    # parts = [preamble, sep1, block1, sep2, block2, ...]
    for i in range(2, len(parts), 2):
        block = parts[i]
        if anchor not in block:
            continue
        new_block, n = re.subn(r'\]\(\#\)', '](' + url + ')', block, count=1)
        if n == 1:
            parts[i] = new_block
            src = ''.join(parts)
            patched = True
            break

if patched:
    with open(page, 'w', encoding='utf-8') as f:
        f.write(src)
    sys.exit(0)
sys.exit(2)
PY
    if [[ "$rc" -eq 0 ]]; then
        log_ok "Page MD patchée : ${page} → ${url}"
    else
        log_warn "Aucun placeholder (#) trouvé pour l'ancre '${anchor}' dans ${page}"
    fi
}

# ---------------------------------------------------------------------------
# Publication : copie gmapsupp.img dans site/docs/telechargements/files/
# ---------------------------------------------------------------------------
publish_coverage() {
    log_step "Publication vers le site"

    local src="${_OUTPUT_DIR}/img/${_IMG_FILENAME}"
    if [[ ! -f "$src" ]]; then
        log_error "Source introuvable pour publication : $src"
        return 1
    fi

    if [[ ! "$VERSION" =~ ^v[0-9]{4}\.(0[1-9]|1[0-2])$ ]]; then
        log_error "Format --version invalide pour publication : '$VERSION' (attendu vYYYY.MM)"
        return 1
    fi

    resolve_coverage_info

    if [[ -z "$_PUB_SLUG" ]]; then
        log_error "Slug de publication vide — zones/region invalides"
        return 1
    fi

    local dest_root="site/docs/telechargements/files/${_PUB_TYPE}/${_PUB_SLUG}"
    local dest_version="${dest_root}/${VERSION}"
    local dest_latest="${dest_root}/latest"
    local lockfile="site/docs/telechargements/.publish.lock"

    mkdir -p "$dest_version" "$dest_latest" "$(dirname "$lockfile")"

    # Verrou pour éviter les races entre 2 publications concurrentes
    exec 200>"$lockfile"
    if ! flock -n 200; then
        log_info "Attente du verrou de publication…"
        flock 200
    fi

    local dest_version_file="${dest_version}/${_IMG_FILENAME}"
    local dest_latest_file="${dest_latest}/${_IMG_LATEST_NAME}"

    # Nettoyage d'un éventuel alias latest/ d'un nom différent (migration ou
    # changement de --family-name) pour éviter d'accumuler des fichiers morts.
    # Log explicite de chaque fichier supprimé + respect de --dry-run.
    local stale
    while IFS= read -r -d '' stale; do
        if [[ "$DRY_RUN" == true ]]; then
            log_info "  [dry-run] suppression alias obsolète : ${stale}"
        else
            log_info "  Suppression alias obsolète : ${stale}"
            rm -f -- "$stale"
        fi
    done < <(find "$dest_latest" -maxdepth 1 -type f -name '*.img' \
                 ! -name "${_IMG_LATEST_NAME}" -print0 2>/dev/null)

    # Copie atomique : écrit dans tmp puis mv (même FS)
    local tmp_version="${dest_version}/.${_IMG_FILENAME}.tmp"
    cp "$src" "$tmp_version"
    mv "$tmp_version" "$dest_version_file"

    # latest/ = hardlink vers la version courante (atomique, zéro coût disque)
    local tmp_latest="${dest_latest}/.${_IMG_LATEST_NAME}.tmp"
    rm -f "$tmp_latest"
    if ! ln -f "$dest_version_file" "$tmp_latest" 2>/dev/null; then
        cp "$dest_version_file" "$tmp_latest"
    fi
    mv "$tmp_latest" "$dest_latest_file"

    local sha256 size
    sha256=$(sha256sum "$dest_version_file" | awk '{print $1}')
    size=$(stat -c%s "$dest_version_file")

    local size_hr
    size_hr=$(numfmt --to=iec-i --suffix=B "$size" 2>/dev/null || echo "${size} octets")

    log_info "  Type   : ${_PUB_TYPE}"
    log_info "  Slug   : ${_PUB_SLUG}"
    log_info "  Label  : ${_PUB_LABEL}"
    log_info "  Taille : ${size_hr}"
    log_info "  sha256 : ${sha256:0:16}…"

    update_manifest "$_PUB_TYPE" "$_PUB_SLUG" "$_PUB_LABEL" "$VERSION" \
                    "$size" "$sha256" "$_IMG_FILENAME" "$_IMG_LATEST_NAME"
    patch_download_page "$_PUB_TYPE" "$_PUB_SLUG" "$_IMG_LATEST_NAME"

    log_ok "Publié : ${dest_version_file}"
    log_ok "Alias  : ${dest_latest_file}"
}

# ---------------------------------------------------------------------------
# Publication S3 : upload gmapsupp.img vers bucket Garage via rclone
# ---------------------------------------------------------------------------
publish_coverage_s3() {
    log_step "Publication S3 (Garage → ${S3_BUCKET})"

    local src="${_OUTPUT_DIR}/img/${_IMG_FILENAME}"
    if [[ ! -f "$src" ]]; then
        log_error "Source introuvable pour publication S3 : $src"
        return 1
    fi

    if [[ ! "$VERSION" =~ ^v[0-9]{4}\.(0[1-9]|1[0-2])$ ]]; then
        log_error "Format --version invalide pour publication : '$VERSION' (attendu vYYYY.MM)"
        return 1
    fi

    resolve_coverage_info

    if [[ -z "$_PUB_SLUG" ]]; then
        log_error "Slug de publication vide — zones/region invalides"
        return 1
    fi

    local key="${_PUB_TYPE}/${_PUB_SLUG}/${VERSION}/${_IMG_FILENAME}"
    local remote="garage:${S3_BUCKET}/${key}"
    local public_url="${PUBLIC_URL_BASE}/${key}"

    local sha256 size
    sha256=$(sha256sum "$src" | awk '{print $1}')
    size=$(stat -c%s "$src")
    local size_hr
    size_hr=$(numfmt --to=iec-i --suffix=B "$size" 2>/dev/null || echo "${size} octets")

    log_info "  Type   : ${_PUB_TYPE}"
    log_info "  Slug   : ${_PUB_SLUG}"
    log_info "  Label  : ${_PUB_LABEL}"
    log_info "  Taille : ${size_hr}"
    log_info "  sha256 : ${sha256:0:16}…"
    log_info "  Remote : ${remote}"
    log_info "  URL    : ${public_url}"

    if [[ "$DRY_RUN" == true ]]; then
        log_info "  [dry-run] rclone copyto --checksum --s3-no-check-bucket \"$src\" \"$remote\""
        log_info "  [dry-run] manifest.json non modifié"
        return 0
    fi

    rclone copyto --checksum --s3-no-check-bucket "$src" "$remote"

    # Verification post-upload : on re-calcule le sha256 en streamant l'objet
    # distant (rclone cat) pour attraper une corruption post-upload. Plus fiable
    # que `rclone hashsum` (Garage ne stocke pas sha256 en metadata native).
    local remote_sha256
    remote_sha256=$(rclone cat "$remote" | sha256sum | awk '{print $1}')
    if [[ "$remote_sha256" != "$sha256" ]]; then
        log_error "sha256 mismatch post-upload (local=${sha256:0:16}… remote=${remote_sha256:0:16}…)"
        return 1
    fi
    log_ok "sha256 remote vérifié"

    update_manifest "$_PUB_TYPE" "$_PUB_SLUG" "$_PUB_LABEL" "$VERSION" \
                    "$size" "$sha256" "$_IMG_FILENAME" "$_IMG_LATEST_NAME" \
                    "$PUBLIC_URL_BASE"
    patch_download_page "$_PUB_TYPE" "$_PUB_SLUG" "$_IMG_LATEST_NAME" \
                        "$PUBLIC_URL_BASE" "$VERSION" "$_IMG_FILENAME"

    log_ok "Publié S3 : ${public_url}"
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

    if [[ -f "${_OUTPUT_DIR}/img/${_IMG_FILENAME}" ]]; then
        local size_bytes
        if [[ "$IMGFORGE_IMG_SIZE" -gt 0 ]]; then
            size_bytes="$IMGFORGE_IMG_SIZE"
        else
            size_bytes=$(stat -c%s "${_OUTPUT_DIR}/img/${_IMG_FILENAME}" 2>/dev/null || echo 0)
        fi
        local size_hr
        size_hr=$(numfmt --to=iec-i --suffix=B "$size_bytes" 2>/dev/null \
                  || echo "${size_bytes} octets")
        echo -e "  ${BOLD}Taille img      :${NC}  ${size_hr}"
        echo -e "  ${BOLD}Emplacement     :${NC}  ${_OUTPUT_DIR}/img/${_IMG_FILENAME}"
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

    if [[ "$PUBLISH" == true ]]; then
        if [[ "$DRY_RUN" == true && "$PUBLISH_TARGET" != "s3" ]]; then
            log_warn "--publish ignoré en mode --dry-run"
        elif [[ "$PARTIAL_FAILURE" == true ]]; then
            log_warn "--publish ignoré : build partiel (PARTIAL_FAILURE=true)"
        elif [[ ! -f "${_OUTPUT_DIR}/img/${_IMG_FILENAME}" ]]; then
            log_warn "--publish ignoré : ${_IMG_FILENAME} manquant"
        else
            case "$PUBLISH_TARGET" in
                local) publish_coverage ;;
                s3)    publish_coverage_s3 ;;
                *)     log_error "PUBLISH_TARGET invalide: ${PUBLISH_TARGET}"; exit 1 ;;
            esac
        fi
    fi

    if [[ "$PARTIAL_FAILURE" == true ]]; then
        log_warn "Pipeline terminé avec avertissements — carte partielle dans : ${_OUTPUT_DIR}/"
        exit 2
    fi
    log_ok "Pipeline terminé — carte disponible dans : ${_OUTPUT_DIR}/img/"
}

main "$@"
