#!/usr/bin/env bash
# =============================================================================
# download-bdtopo.sh — Téléchargement BD TOPO® IGN
# =============================================================================
#
# Télécharge les données BD TOPO depuis data.geopf.fr via l'API ATOM :
#
#   1. Interroge l'API pour découvrir les datasets disponibles
#   2. Récupère la page détail du dataset le plus récent
#   3. Extrait les URLs de download et hash MD5 depuis la réponse API
#
# Pipeline : download-bdtopo.sh → mpforge → imgforge → gmapsupp.img
#            (actuellement : download-bdtopo.sh → mpforge → mkgmap → gmapsupp.img)
#
# Prérequis : curl, 7z (p7zip-full)
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
API_BASE="https://data.geopf.fr/telechargement"
DATA_ROOT="./pipeline/data/bdtopo"
PRODUCT="FULL"          # FULL | DIFF | EXPRESS
FORMAT="SHP"            # SHP | GPKG | SQL
THEMES="TOUSTHEMES"     # TOUSTHEMES | TRANSPORT | HYDROGRAPHIE | etc.
ZONES=()
REGION=""
EDITION_DATE=""          # YYYY-MM-DD — si vide, on prend le plus récent via l'API
BDTOPO_VERSION=""        # vYYYY.MM — alias résolu dynamiquement vers EDITION_DATE
LIST_EDITIONS=false      # --list-editions : liste les millésimes et quitte
DRY_RUN=false
SKIP_EXISTING=true
AUTO_EXTRACT=true
DEBUG=false
JSON_OUTPUT=""          # chemin fichier pour résumé JSON (vide = stdout)
WITH_CONTOURS=false
CONTOURS_DATA_ROOT="./pipeline/data/contours"
WITH_OSM=false
OSM_DATA_ROOT="./pipeline/data/osm"
GEOFABRIK_BASE="https://download.geofabrik.de/europe/france"
WITH_DEM=false
DEM_DATA_ROOT="./pipeline/data/dem"
SCRIPT_VERSION="1.4.0"

# Métriques téléchargement — collecte pour résumé JSON (AC6)
_LAST_DOWNLOAD_SIZE=0
_LAST_DOWNLOAD_STATUS=""
DOWNLOAD_START_TIME=0
DOWNLOAD_STATUSES=()
DOWNLOAD_SIZES=()

# ---------------------------------------------------------------------------
# Nettoyage — supprime _extract_tmp si interruption (SIGINT/SIGTERM/EXIT)
# ---------------------------------------------------------------------------
_CURRENT_TMP_EXTRACT=""
cleanup_trap() {
    [[ -n "$_CURRENT_TMP_EXTRACT" && -d "$_CURRENT_TMP_EXTRACT" ]] && rm -rf "$_CURRENT_TMP_EXTRACT" || true
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
log_debug() { [[ "$DEBUG" == true ]] && echo -e "${YELLOW}[DEBUG]${NC} $*" || true; }

# ---------------------------------------------------------------------------
# Régions → codes zone INSEE (R-codes) — un fichier agrégé par région
# L'API Géoplateforme fournit des fichiers régionaux pré-agrégés pour BDTOPO FULL
# Format : --region ARA télécharge R84 (1 fichier ~2.5G vs 12 fichiers dept ~2.4G)
# ---------------------------------------------------------------------------
declare -A REGIONS=(
    # Régions métropolitaines (code INSEE → zone API)
    [ARA]="R84"   # Auvergne-Rhône-Alpes
    [BFC]="R27"   # Bourgogne-Franche-Comté
    [BRE]="R53"   # Bretagne
    [CVL]="R24"   # Centre-Val de Loire
    [COR]="R94"   # Corse
    [GES]="R44"   # Grand Est
    [HDF]="R32"   # Hauts-de-France
    [IDF]="R11"   # Île-de-France
    [NOR]="R28"   # Normandie
    [NAQ]="R75"   # Nouvelle-Aquitaine
    [OCC]="R76"   # Occitanie
    [PDL]="R52"   # Pays de la Loire
    [PAC]="R93"   # Provence-Alpes-Côte d'Azur
    # Groupements multi-régions
    [FRANCE-SUD]="R75 R76 R84 R93 R94"              # 5 régions Sud
    [FRANCE-NORD]="R11 R24 R27 R28 R32 R44 R52 R53" # 8 régions Nord
    [FXX]="R11 R24 R27 R28 R32 R44 R52 R53 R75 R76 R84 R93 R94" # France métro (13 fichiers)
    # Quadrants Garmin (couverture TOPO France v7 PRO — départements uniquement, millésimes alignés)
    [FRANCE-SE]="D001 D003 D004 D005 D006 D007 D011 D013 D015 D02A D02B D026 D030 D034 D038 D042 D043 D048 D063 D066 D069 D073 D074 D083 D084"
    [FRANCE-SO]="D009 D012 D016 D017 D019 D023 D024 D031 D032 D033 D040 D046 D047 D064 D065 D079 D081 D082 D086 D087"
    [FRANCE-NE]="D002 D008 D010 D021 D025 D027 D039 D051 D052 D054 D055 D057 D058 D059 D060 D062 D067 D068 D070 D071 D075 D076 D077 D078 D080 D088 D089 D090 D091 D092 D093 D094 D095"
    [FRANCE-NO]="D014 D018 D022 D028 D029 D035 D036 D037 D041 D044 D045 D049 D050 D053 D056 D061 D072 D075 D077 D078 D085 D091 D092 D093 D094 D095"
)

# Mapping région → départements INSEE (pour les courbes de niveau, livrées par département)
declare -A REGIONS_TO_DEPARTMENTS=(
    [ARA]="D001 D003 D007 D015 D026 D038 D042 D043 D063 D069 D073 D074"
    [BFC]="D021 D025 D039 D058 D070 D071 D089 D090"
    [BRE]="D022 D029 D035 D056"
    [CVL]="D018 D028 D036 D037 D041 D045"
    [COR]="D02A D02B"
    [GES]="D008 D010 D051 D052 D054 D055 D057 D067 D068 D088"
    [HDF]="D002 D059 D060 D062 D080"
    [IDF]="D075 D077 D078 D091 D092 D093 D094 D095"
    [NOR]="D014 D027 D050 D061 D076"
    [NAQ]="D016 D017 D019 D023 D024 D033 D040 D047 D064 D079 D086 D087"
    [OCC]="D009 D011 D012 D030 D031 D032 D034 D046 D048 D065 D066 D081 D082"
    [PDL]="D044 D049 D053 D072 D085"
    [PAC]="D004 D005 D006 D013 D083 D084"
    # Groupements multi-régions
    [FRANCE-SUD]="D009 D011 D012 D016 D017 D019 D023 D024 D030 D031 D032 D033 D034 D040 D046 D047 D048 D064 D065 D066 D079 D081 D082 D086 D087 D004 D005 D006 D013 D083 D084 D001 D003 D007 D015 D026 D038 D042 D043 D063 D069 D073 D074 D02A D02B"
    [FRANCE-NORD]="D075 D077 D078 D091 D092 D093 D094 D095 D018 D028 D036 D037 D041 D045 D021 D025 D039 D058 D070 D071 D089 D090 D014 D027 D050 D061 D076 D002 D059 D060 D062 D080 D044 D049 D053 D072 D085 D022 D029 D035 D056 D008 D010 D051 D052 D054 D055 D057 D067 D068 D088"
    [FXX]="D001 D002 D003 D004 D005 D006 D007 D008 D009 D010 D011 D012 D013 D014 D015 D016 D017 D018 D019 D02A D02B D021 D022 D023 D024 D025 D026 D027 D028 D029 D030 D031 D032 D033 D034 D035 D036 D037 D038 D039 D040 D041 D042 D043 D044 D045 D046 D047 D048 D049 D050 D051 D052 D053 D054 D055 D056 D057 D058 D059 D060 D061 D062 D063 D064 D065 D066 D067 D068 D069 D070 D071 D072 D073 D074 D075 D076 D077 D078 D079 D080 D081 D082 D083 D084 D085 D086 D087 D088 D089 D090 D091 D092 D093 D094 D095"
    # Quadrants Garmin (couverture TOPO France v7 PRO)
    [FRANCE-SE]="D001 D003 D004 D005 D006 D007 D011 D013 D015 D02A D02B D026 D030 D034 D038 D042 D043 D048 D063 D066 D069 D073 D074 D083 D084"
    [FRANCE-SO]="D009 D012 D016 D017 D019 D023 D024 D031 D032 D033 D040 D046 D047 D064 D065 D079 D081 D082 D086 D087"
    [FRANCE-NE]="D002 D008 D010 D021 D025 D027 D039 D051 D052 D054 D055 D057 D058 D059 D060 D062 D067 D068 D070 D071 D075 D076 D077 D078 D080 D088 D089 D090 D091 D092 D093 D094 D095"
    [FRANCE-NO]="D014 D018 D022 D028 D029 D035 D036 D037 D041 D044 D045 D049 D050 D053 D056 D061 D072 D075 D077 D078 D085 D091 D092 D093 D094 D095"
)

# Mapping régions modernes → anciennes régions Geofabrik (pour OSM PBF)
# Geofabrik utilise les régions françaises pré-2016
declare -A REGIONS_TO_GEOFABRIK=(
    [ARA]="auvergne rhone-alpes"
    [BFC]="bourgogne franche-comte"
    [BRE]="bretagne"
    [CVL]="centre"
    [COR]="corse"
    [GES]="alsace champagne-ardenne lorraine"
    [HDF]="nord-pas-de-calais picardie"
    [IDF]="ile-de-france"
    [NOR]="basse-normandie haute-normandie"
    [NAQ]="aquitaine limousin poitou-charentes"
    [OCC]="languedoc-roussillon midi-pyrenees"
    [PDL]="pays-de-la-loire"
    [PAC]="provence-alpes-cote-d-azur"
    [FRANCE-SUD]="aquitaine limousin poitou-charentes languedoc-roussillon midi-pyrenees auvergne rhone-alpes provence-alpes-cote-d-azur corse"
    [FRANCE-NORD]="ile-de-france centre bourgogne franche-comte basse-normandie haute-normandie nord-pas-de-calais picardie pays-de-la-loire bretagne alsace champagne-ardenne lorraine"
    [FXX]="france"
    # Quadrants Garmin (couverture TOPO France v7 PRO)
    [FRANCE-SE]="auvergne rhone-alpes provence-alpes-cote-d-azur corse languedoc-roussillon"
    [FRANCE-SO]="aquitaine limousin poitou-charentes midi-pyrenees"
    [FRANCE-NE]="bourgogne franche-comte nord-pas-de-calais picardie alsace champagne-ardenne lorraine ile-de-france haute-normandie"
    [FRANCE-NO]="centre bretagne pays-de-la-loire ile-de-france basse-normandie"
    # DOM — Geofabrik publie les DOM sous europe/france/<nom>-latest.osm.pbf
    [GUA]="guadeloupe"
    [MAR]="martinique"
    [GUY]="guyane"
    [REU]="reunion"
    [MAY]="mayotte"
)

# Mapping département → code région (pour résoudre D038 → ARA → Geofabrik)
declare -A DEPT_TO_REGION=(
    [D001]="ARA" [D003]="ARA" [D007]="ARA" [D015]="ARA" [D026]="ARA"
    [D038]="ARA" [D042]="ARA" [D043]="ARA" [D063]="ARA" [D069]="ARA"
    [D073]="ARA" [D074]="ARA"
    [D021]="BFC" [D025]="BFC" [D039]="BFC" [D058]="BFC" [D070]="BFC"
    [D071]="BFC" [D089]="BFC" [D090]="BFC"
    [D022]="BRE" [D029]="BRE" [D035]="BRE" [D056]="BRE"
    [D018]="CVL" [D028]="CVL" [D036]="CVL" [D037]="CVL" [D041]="CVL"
    [D045]="CVL"
    [D02A]="COR" [D02B]="COR"
    [D008]="GES" [D010]="GES" [D051]="GES" [D052]="GES" [D054]="GES"
    [D055]="GES" [D057]="GES" [D067]="GES" [D068]="GES" [D088]="GES"
    [D002]="HDF" [D059]="HDF" [D060]="HDF" [D062]="HDF" [D080]="HDF"
    [D075]="IDF" [D077]="IDF" [D078]="IDF" [D091]="IDF" [D092]="IDF"
    [D093]="IDF" [D094]="IDF" [D095]="IDF"
    [D014]="NOR" [D027]="NOR" [D050]="NOR" [D061]="NOR" [D076]="NOR"
    [D016]="NAQ" [D017]="NAQ" [D019]="NAQ" [D023]="NAQ" [D024]="NAQ"
    [D033]="NAQ" [D040]="NAQ" [D047]="NAQ" [D064]="NAQ" [D079]="NAQ"
    [D086]="NAQ" [D087]="NAQ"
    [D009]="OCC" [D011]="OCC" [D012]="OCC" [D030]="OCC" [D031]="OCC"
    [D032]="OCC" [D034]="OCC" [D046]="OCC" [D048]="OCC" [D065]="OCC"
    [D066]="OCC" [D081]="OCC" [D082]="OCC"
    [D044]="PDL" [D049]="PDL" [D053]="PDL" [D072]="PDL" [D085]="PDL"
    [D004]="PAC" [D005]="PAC" [D006]="PAC" [D013]="PAC" [D083]="PAC"
    [D084]="PAC"
    # DOM — Départements d'Outre-Mer
    [D971]="GUA" [D972]="MAR" [D973]="GUY" [D974]="REU" [D976]="MAY"
)

# ---------------------------------------------------------------------------
# Aide
# ---------------------------------------------------------------------------
show_help() {
    cat << 'EOF'
download-bdtopo.sh — Téléchargement BD TOPO® IGN

USAGE :
    ./download-bdtopo.sh [OPTIONS]

OPTIONS :
    --zones ZONES       Codes zone (département D038, région R84, ou liste D038,D073,R84)
    --region CODE       Raccourci région :
                          Régions  : ARA BFC BRE CVL COR GES HDF IDF NOR NAQ OCC PDL PAC
                          Groupes  : FRANCE-SUD (R75,R76,R84,R93,R94)
                                     FRANCE-NORD (R11,R24,R27,R28,R32,R44,R52,R53)
                                     FXX (France métro complète — 13 fichiers)
                          Quadrants Garmin (couverture TOPO France v7 PRO, départements uniquement) :
                                     FRANCE-SE (25 départements du sud-est)
                                     FRANCE-SO (20 départements du sud-ouest)
                                     FRANCE-NE (33 départements du nord-est)
                                     FRANCE-NO (26 départements du nord-ouest)
                                     Note : IDF (75,77,78,91,92,93,94,95) partagée entre NE et NO (conforme Garmin)
    --format FORMAT     SHP (défaut) | GPKG | SQL
    --product PRODUCT   FULL (défaut) | DIFF | EXPRESS
                          FULL    → par département (D038) ou région (R84)
                          DIFF    → par région uniquement (R84, FXX, etc.)
                          EXPRESS → France entière en GPKG (zone=FXX, automatique)
    --themes THEMES     TOUSTHEMES (défaut) | TRANSPORT | HYDROGRAPHIE | etc.
    --date YYYY-MM-DD   Forcer une date d'édition (sinon la plus récente)
    --bdtopo-version vYYYY.MM
                        Alias de --date : résout dynamiquement via l'API
                        vers la dernière édition publiée ce mois-là
                        (ex: v2025.09 → 2025-09-15 si publiée à cette date)
    --list-editions     Liste les millésimes BDTOPO disponibles pour les
                        zones demandées puis quitte (ne télécharge rien)
    --data-root DIR     Racine des données (défaut: ./pipeline/data/bdtopo)
    --no-extract        Ne pas décompresser les .7z
    --no-skip           Re-télécharger même si déjà présent
    --dry-run           Simuler sans télécharger
    --json-output FILE  Écrire le résumé JSON dans un fichier (défaut: stdout)
    --with-contours     Télécharger aussi les courbes de niveau IGN (par département)
    --contours-root DIR Racine données courbes (défaut: ./pipeline/data/contours)
    --with-osm          Télécharger aussi les données OSM depuis Geofabrik (.osm.pbf)
    --osm-root DIR      Racine données OSM (défaut: ./pipeline/data/osm)
    --with-dem          Télécharger aussi le MNT BD ALTI v2 (ASC 25m, par département)
    --dem-root DIR      Racine données DEM (défaut: ./pipeline/data/dem)
    --debug             Afficher les requêtes API et réponses
    --version           Version du script
    -h, --help          Aide

EXEMPLES :
    ./download-bdtopo.sh --zones D038                       # Département Isère (SHP)
    ./download-bdtopo.sh --zones D038,D073,D074             # Multi-départements
    ./download-bdtopo.sh --region ARA                       # Auvergne-Rhône-Alpes (1 fichier R84 ~2.5G)
    ./download-bdtopo.sh --region FRANCE-SUD --format GPKG  # 5 régions Sud (R75,R76,R84,R93,R94)
    ./download-bdtopo.sh --region FRANCE-NORD               # 8 régions Nord
    ./download-bdtopo.sh --region FXX --dry-run             # France entière (13 fichiers régionaux)
    ./download-bdtopo.sh --zones R84 --product DIFF         # Différentiel ARA (région uniquement)
    ./download-bdtopo.sh --product EXPRESS                  # Express France entière (GPKG)
    ./download-bdtopo.sh --zones D038 --date 2025-12-15     # Date forcée
    ./download-bdtopo.sh --zones D038 --bdtopo-version v2025.09  # Dernière édition de sept 2025
    ./download-bdtopo.sh --zones D038 --list-editions       # Lister les millésimes dispo
    DEBUG=1 ./download-bdtopo.sh --zones D038               # Mode debug
    ./download-bdtopo.sh --zones D038 --with-contours      # BDTOPO + courbes de niveau D038
    ./download-bdtopo.sh --region ARA --with-contours      # BDTOPO R84 + courbes 12 départements ARA
    ./download-bdtopo.sh --region ARA --with-osm           # BDTOPO R84 + OSM auvergne + rhone-alpes
    ./download-bdtopo.sh --region FXX --with-osm            # France entière BDTOPO + OSM
    ./download-bdtopo.sh --with-osm --region ARA --dry-run  # Simuler téléchargement OSM
    ./download-bdtopo.sh --zones D038 --with-dem           # BDTOPO + MNT BD ALTI v2 D038
    ./download-bdtopo.sh --region ARA --with-dem           # BDTOPO + MNT 12 départements ARA
    ./download-bdtopo.sh --region FRANCE-SE --dry-run      # Quadrant Garmin Sud-Est (25 départements)
    ./download-bdtopo.sh --region FRANCE-NO --with-contours --with-dem  # Quadrant Garmin Nord-Ouest (26 départements)
EOF
    exit 0
}

# ---------------------------------------------------------------------------
# Parse args
# ---------------------------------------------------------------------------
parse_args() {
    if [[ "${DEBUG:-}" == "1" ]]; then DEBUG=true; fi

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --zones)      IFS=',' read -ra ZONES <<< "$2"; shift 2 ;;
            --region)     REGION="${2^^}"; shift 2 ;;
            --format)     FORMAT="${2^^}"; shift 2 ;;
            --product)    PRODUCT="${2^^}"; shift 2 ;;
            --themes)     THEMES="$2"; shift 2 ;;
            --date)
                if [[ ! "$2" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]]; then
                    log_error "Format de date invalide : $2 (attendu YYYY-MM-DD)"
                    exit 1
                fi
                EDITION_DATE="$2"; shift 2 ;;
            --bdtopo-version)
                if [[ ! "$2" =~ ^v[0-9]{4}\.[0-9]{2}$ ]]; then
                    log_error "Format de version invalide : $2 (attendu vYYYY.MM, ex v2025.09)"
                    exit 1
                fi
                BDTOPO_VERSION="$2"; shift 2 ;;
            --list-editions) LIST_EDITIONS=true; shift ;;
            --data-root)  DATA_ROOT="$2"; shift 2 ;;
            --no-extract) AUTO_EXTRACT=false; shift ;;
            --no-skip)    SKIP_EXISTING=false; shift ;;
            --dry-run)    DRY_RUN=true; shift ;;
            --json-output) JSON_OUTPUT="$2"; shift 2 ;;
            --with-contours) WITH_CONTOURS=true; shift ;;
            --contours-root) CONTOURS_DATA_ROOT="$2"; shift 2 ;;
            --with-osm)   WITH_OSM=true; shift ;;
            --osm-root)   OSM_DATA_ROOT="$2"; shift 2 ;;
            --with-dem)   WITH_DEM=true; shift ;;
            --dem-root)   DEM_DATA_ROOT="$2"; shift 2 ;;
            --debug)      DEBUG=true; shift ;;
            --version)    echo "download-bdtopo.sh v${SCRIPT_VERSION}"; exit 0 ;;
            -h|--help)    show_help ;;
            *)            log_error "Option inconnue : $1"; exit 1 ;;
        esac
    done
}

# ---------------------------------------------------------------------------
# Prérequis
# ---------------------------------------------------------------------------
check_prerequisites() {
    log_step "Vérification des prérequis"

    command -v curl &>/dev/null || { log_error "curl requis (apt install curl)"; exit 1; }

    if ! command -v 7z &>/dev/null; then
        if [[ "$AUTO_EXTRACT" == true ]]; then
            log_error "7z est requis pour l'extraction des archives (apt install p7zip-full)"
            exit 1
        fi
    fi

    command -v md5sum &>/dev/null || { log_error "md5sum requis (coreutils)"; exit 1; }
    command -v numfmt &>/dev/null || { log_error "numfmt requis (coreutils)"; exit 1; }

    log_ok "curl $(curl --version | head -1 | awk '{print $2}')"
    if [[ "$AUTO_EXTRACT" == true ]]; then log_ok "7z disponible"; fi
}

# ---------------------------------------------------------------------------
# Résolution zones
# ---------------------------------------------------------------------------
resolve_zones() {
    if [[ -n "$REGION" ]]; then
        if [[ -z "${REGIONS[$REGION]+x}" ]]; then
            log_error "Région inconnue : $REGION (dispo: ${!REGIONS[*]})"
            exit 1
        fi
        IFS=' ' read -ra ZONES <<< "${REGIONS[$REGION]}"
        log_info "Région $REGION → ${#ZONES[@]} zone(s) : ${ZONES[*]}"
    fi

    if [[ "$PRODUCT" == "EXPRESS" ]]; then
        if [[ -n "$REGION" || ${#ZONES[@]} -gt 0 ]]; then
            log_warn "EXPRESS : --region / --zones ignorés (EXPRESS = France entière FXX en GPKG automatiquement)"
        fi
        ZONES=("FXX"); FORMAT="GPKG"
        log_info "BD TOPO Express : France entière en GPKG"
        return
    fi

    if [[ ${#ZONES[@]} -eq 0 ]]; then
        log_error "Aucune zone. Utilisez --zones D038, --zones R84, ou --region ARA"
        exit 1
    fi

    # DIFF ne supporte que les R-codes — avertir si des D-codes sont présents
    if [[ "$PRODUCT" == "DIFF" ]]; then
        local has_dcodes=false
        for z in "${ZONES[@]}"; do
            [[ "$z" =~ ^D ]] && has_dcodes=true && break
        done
        if [[ "$has_dcodes" == true ]]; then
            log_warn "DIFF : certaines zones sont des D-codes (départements) — le produit DIFF n'est disponible que par région (R-codes)"
            log_warn "  Les D-codes seront ignorés par l'API. Utilisez --product FULL pour un téléchargement par département."
        fi
    fi
}

# ---------------------------------------------------------------------------
# Appel API avec gestion d'erreur
# ---------------------------------------------------------------------------
api_fetch() {
    local url="$1"
    local response

    log_debug "API → $url"

    response=$(curl -s --connect-timeout 15 --max-time 60 -f "$url" 2>/dev/null) || {
        log_debug "API erreur pour : $url"
        echo ""
        return 1
    }

    log_debug "API ← ${#response} octets"
    echo "$response"
}

# ---------------------------------------------------------------------------
# Extraction XML simple (sans dépendance externe)
# Utilise grep/sed pour extraire les valeurs des balises ATOM
# ---------------------------------------------------------------------------

# Construire le chemin versionné à partir d'une date d'édition
# Ex: 2025-12-15 → data/bdtopo/2025/v2025.12/
edition_date_to_path() {
    local date="$1"
    local year="${date%%-*}"                      # 2025
    local month="${date#*-}"; month="${month%%-*}" # 12
    echo "${DATA_ROOT}/${year}/v${year}.${month}"
}

# Extraire toutes les occurrences d'une balise (contenu texte)
xml_get_all() {
    local xml="$1" tag="$2"
    # Gère les balises avec namespace (ex: gpf_dl:editionDate)
    echo "$xml" | grep -oP "<${tag}[^>]*>\K[^<]+" 2>/dev/null || true
}

# Extraire l'attribut href des balises <link> qui contiennent "download" dans le href
# Exclut les fichiers de métadonnées (md5, sha256) car l'API expose parfois à la fois
# l'archive et sa somme de contrôle en tant que liens "download" (cas des éditions
# BDTOPO antérieures à la dernière publiée).
xml_get_download_links() {
    local xml="$1"
    echo "$xml" | grep -oP '<link[^>]+href="\K[^"]*download[^"]*' 2>/dev/null \
        | grep -vE '\.(md5|sha256|sha1|sha512)$' || true
}

# Extraire le hash MD5 du fichier de données (pas d'une somme de contrôle) depuis
# un feed détail Géoplateforme. On apparie les <link href="..."/> et les <content>
# dans l'ordre d'apparition, on écarte les paires dont l'href pointe vers un
# fichier de checksum (.md5/.sha*), et on retourne le hash de la dernière paire
# data restante (robuste si l'API ajoute d'autres types de checksums).
xml_get_data_md5() {
    local xml="$1"
    printf '%s' "$xml" | python3 - <<'PY' 2>/dev/null || true
import sys, re
xml = sys.stdin.read()
links = re.findall(r'<link[^>]*href="([^"]*download[^"]*)"', xml)
contents = re.findall(r'<content[^>]*>\s*([a-f0-9]{32})\s*</content>', xml)
pairs = list(zip(links, contents))
data = [c for href, c in pairs
        if not re.search(r'\.(md5|sha1|sha256|sha512)(?:$|\?)', href, re.I)]
if data:
    print(data[-1])
PY
}

# Extraire l'attribut gpf_dl:length d'une balise <link> pointant vers un fichier
# de données (pas une somme de contrôle). On parse les <link> complets, on exclut
# ceux pointant vers .md5/.sha256/etc., puis on extrait la length du premier restant.
xml_get_link_length() {
    local xml="$1"
    echo "$xml" \
        | grep -oP '<link[^>]*href="[^"]*download[^"]*"[^/]*/>' 2>/dev/null \
        | grep -vE 'href="[^"]*\.(md5|sha256|sha1|sha512)"' \
        | grep -oP 'gpf_dl:length="\K[0-9]+' \
        | head -1 || true
}

# ---------------------------------------------------------------------------
# Nom de la ressource API selon le produit
# ---------------------------------------------------------------------------
get_resource_name() {
    case "$PRODUCT" in
        FULL)    echo "BDTOPO" ;;
        DIFF)    echo "BDTOPO-DIFF" ;;
        EXPRESS) echo "BDTOPO_EXPRESS" ;;
    esac
}

# ---------------------------------------------------------------------------
# Liste TOUTES les éditions disponibles pour une zone+format donnés.
# Sortie sur stdout, une ligne par édition : "YYYY-MM-DD <dataset_name>".
# ---------------------------------------------------------------------------
list_editions_for_zone() {
    local zone="$1" format="$2"
    local resource_name
    resource_name=$(get_resource_name)

    # Pagination par blocs de 50 (plafond raisonnable API Géoplateforme) pour éviter
    # 1 requête par édition (N sequential calls → rate-limit + latence).
    local page_size=50
    local probe_url="${API_BASE}/resource/${resource_name}?zone=${zone}&format=${format}&page=1&limit=1"
    local probe_response
    probe_response=$(api_fetch "$probe_url") || return 1

    local total_entries
    total_entries=$(echo "$probe_response" | grep -oP 'gpf_dl:totalentries="\K[0-9]+' | head -1 || echo "0")
    [[ "$total_entries" == "0" ]] && return 0

    local total_pages=$(( (total_entries + page_size - 1) / page_size ))
    local p
    for ((p=1; p<=total_pages; p++)); do
        local page_url="${API_BASE}/resource/${resource_name}?zone=${zone}&format=${format}&page=${p}&limit=${page_size}"
        local page_resp
        page_resp=$(api_fetch "$page_url") || continue
        # Un <title> par entrée ; on garde ceux contenant "_${zone}_" (exclut
        # le <title> du feed lui-même), et on extrait la date terminale YYYY-MM-DD.
        echo "$page_resp" \
            | grep -oP '<title>\K[^<]+' \
            | grep -F "_${zone}_" \
            | while IFS= read -r title; do
                local date
                date=$(printf '%s' "$title" | grep -oP '[0-9]{4}-[0-9]{2}-[0-9]{2}$' || true)
                [[ -n "$date" ]] && echo "${date} ${title}"
            done
    done | sort -u -r
}

# ---------------------------------------------------------------------------
# Affiche les millésimes disponibles pour toutes les zones demandées, puis exit.
# Appelé uniquement si --list-editions est passé.
# ---------------------------------------------------------------------------
run_list_editions() {
    log_step "Millésimes BDTOPO disponibles"

    local resource_name
    resource_name=$(get_resource_name)
    log_info "Ressource API : ${resource_name} · format : ${FORMAT}"

    for zone in "${ZONES[@]}"; do
        echo ""
        log_info "Zone ${zone} :"
        local editions
        editions=$(list_editions_for_zone "$zone" "$FORMAT") || {
            log_warn "  Impossible d'interroger l'API pour ${zone}"
            continue
        }
        if [[ -z "$editions" ]]; then
            log_warn "  Aucune édition trouvée"
            continue
        fi
        while IFS= read -r line; do
            local d="${line%% *}"
            local year="${d%%-*}"; local month="${d#*-}"; month="${month%%-*}"
            printf "  • v%s.%s  (date: %s)\n" "$year" "$month" "$d"
        done <<< "$editions"
    done

    echo ""
    log_info "Pour télécharger une édition précise :"
    log_info "  --date YYYY-MM-DD            (date exacte)"
    log_info "  --bdtopo-version vYYYY.MM    (dernière édition du mois)"
    exit 0
}

# ---------------------------------------------------------------------------
# Résout --bdtopo-version vYYYY.MM → EDITION_DATE via l'API.
# Stratégie : interroge la 1ère zone, filtre les éditions du mois demandé,
# prend la plus récente. Si la même date n'existe pas pour les autres zones,
# l'API renverra simplement "non trouvé" lors de discover_downloads.
# ---------------------------------------------------------------------------
resolve_bdtopo_version() {
    local version="$1"  # vYYYY.MM déjà validé par parse_args
    local year="${version:1:4}"
    local month="${version:6:2}"

    local reference_zone="${ZONES[0]}"
    log_info "Résolution ${version} via API (zone de référence : ${reference_zone})..."

    local editions
    editions=$(list_editions_for_zone "$reference_zone" "$FORMAT") || {
        log_error "Impossible d'interroger l'API pour résoudre ${version}"
        exit 1
    }

    local resolved
    resolved=$(echo "$editions" | awk '{print $1}' | grep "^${year}-${month}-" | sort -r | head -1 || true)

    if [[ -z "$resolved" ]]; then
        log_error "Aucune édition trouvée pour ${version} sur ${reference_zone}"
        log_info "Utilisez --list-editions pour voir les millésimes disponibles."
        exit 1
    fi

    EDITION_DATE="$resolved"
    log_ok "${version} → ${EDITION_DATE}"

    # Vérification cross-zones : IGN publie parfois à des dates différentes selon
    # le département. Avertir explicitement si une zone demandée n'a pas la même
    # date résolue (plutôt que de laisser discover_downloads échouer en silence).
    if [[ "${#ZONES[@]}" -gt 1 ]]; then
        local z zone_editions zone_date divergent=0
        for z in "${ZONES[@]:1}"; do
            zone_editions=$(list_editions_for_zone "$z" "$FORMAT") || {
                log_warn "  ${z} : impossible de vérifier la disponibilité de ${EDITION_DATE}"
                divergent=1
                continue
            }
            zone_date=$(echo "$zone_editions" | awk '{print $1}' | grep "^${year}-${month}-" | sort -r | head -1 || true)
            if [[ -z "$zone_date" ]]; then
                log_warn "  ${z} : aucune édition pour ${version} (le téléchargement échouera pour cette zone)"
                divergent=1
            elif [[ "$zone_date" != "$EDITION_DATE" ]]; then
                log_warn "  ${z} : édition ${version} publiée le ${zone_date} (≠ ${EDITION_DATE} de ${reference_zone})"
                divergent=1
            fi
        done
        if [[ "$divergent" -eq 1 ]]; then
            log_warn "Dates de publication divergentes entre zones. Le téléchargement sera partiel pour les zones concernées."
            log_warn "Utilisez --list-editions pour voir les millésimes par zone."
        fi
    fi
}

# ---------------------------------------------------------------------------
# Découverte des datasets via l'API
# Pour chaque zone, on interroge l'API pour trouver le dataset le plus récent,
# puis on récupère la page détail pour obtenir l'URL de download exacte.
# ---------------------------------------------------------------------------
discover_downloads() {
    log_step "Découverte des datasets via l'API"

    declare -ga DOWNLOAD_URLS=()
    declare -ga DOWNLOAD_DIRS=()
    declare -ga DOWNLOAD_NAMES=()
    declare -ga DOWNLOAD_MD5S=()

    local resource_name
    resource_name=$(get_resource_name)

    local themes_list
    IFS=',' read -ra themes_list <<< "$THEMES"

    for zone in "${ZONES[@]}"; do
        for theme in "${themes_list[@]}"; do
            log_info "Recherche : $zone / $theme / $FORMAT ..."

            # --- Étape 1 : Trouver le dataset le plus récent ---
            local dataset_name=""

            if [[ -n "$EDITION_DATE" ]]; then
                # Date forcée : construire le nom directement
                case "$PRODUCT" in
                    FULL)    dataset_name="BDTOPO_3-5_${theme}_${FORMAT}_LAMB93_${zone}_${EDITION_DATE}" ;;
                    DIFF)    dataset_name="BDTOPO-DIFF_3-5_${theme}_${FORMAT}_LAMB93_${zone}_${EDITION_DATE}" ;;
                    EXPRESS) dataset_name="BDTOPO-EXPRESS_3-5__${FORMAT}_LAMB93_${zone}_${EDITION_DATE}" ;;
                esac
                log_debug "Date forcée → $dataset_name"
            else
                # Interroger l'API pour trouver le plus récent
                # D'abord on récupère le nombre total d'entrées (page 1, limit 1)
                local probe_url="${API_BASE}/resource/${resource_name}?zone=${zone}&format=${FORMAT}&page=1&limit=1"
                local probe_response
                probe_response=$(api_fetch "$probe_url") || {
                    log_warn "  API indisponible pour $zone — ignoré"
                    continue
                }

                if [[ -z "$probe_response" ]]; then
                    log_warn "  Réponse vide pour $zone — ignoré"
                    continue
                fi

                # Extraire le nombre total d'entrées et le nombre de pages
                local total_entries
                total_entries=$(echo "$probe_response" | grep -oP 'gpf_dl:totalentries="\K[0-9]+' | head -1 || echo "0")

                if [[ "$total_entries" == "0" ]]; then
                    log_warn "  Aucun dataset trouvé pour $zone / $FORMAT"
                    continue
                fi

                log_debug "  $total_entries datasets disponibles pour $zone"

                # Récupérer la dernière page pour avoir le dataset le plus récent.
                # HYPOTHÈSE : l'API Géoplateforme trie les résultats du plus ancien
                # au plus récent (confirmé empiriquement, page N = édition la plus récente).
                # Si ce tri change, le script téléchargerait silencieusement une ancienne édition.
                local last_page_url="${API_BASE}/resource/${resource_name}?zone=${zone}&format=${FORMAT}&page=${total_entries}&limit=1"
                local last_page_response
                last_page_response=$(api_fetch "$last_page_url") || {
                    log_warn "  Impossible de récupérer la dernière page pour $zone"
                    continue
                }

                # Extraire le titre du dernier entry (= dataset le plus récent)
                dataset_name=$(echo "$last_page_response" | grep -oP '<entry>[\s\S]*?</entry>' | grep -oP '<title>\K[^<]+' | tail -1 || true)

                # Fallback : chercher dans tout le XML les titres qui correspondent au format demandé
                if [[ -z "$dataset_name" ]]; then
                    log_debug "  Fallback: extraction titre depuis la réponse complète"
                    dataset_name=$(echo "$last_page_response" | grep -oP '<title>[^<]*'"${zone}"'[^<]*</title>' | grep -oP '>\K[^<]+' | tail -1 || true)
                fi

                if [[ -z "$dataset_name" ]]; then
                    log_warn "  Impossible d'extraire le nom du dataset pour $zone"
                    log_debug "  Réponse reçue : ${last_page_response:0:500}"
                    continue
                fi

                log_debug "  Dataset le plus récent : $dataset_name"
            fi

            # --- Étape 2 : Récupérer la page détail du dataset ---
            local detail_url="${API_BASE}/resource/${resource_name}/${dataset_name}"
            local detail_response
            detail_response=$(api_fetch "$detail_url") || {
                log_warn "  Dataset introuvable : $dataset_name"
                continue
            }

            if [[ -z "$detail_response" ]]; then
                log_warn "  Réponse vide pour le détail de $dataset_name"
                continue
            fi

            # --- Étape 3 : Extraire l'URL de download depuis la page détail ---
            local download_url
            download_url=$(xml_get_download_links "$detail_response" | head -1)

            if [[ -z "$download_url" ]]; then
                log_warn "  Aucune URL de download dans la page détail de $dataset_name"
                log_debug "  Détail reçu : ${detail_response:0:500}"
                continue
            fi

            # Extraire le hash MD5 du fichier data (PAS de sa somme de contrôle).
            # Pour les éditions antérieures, l'API liste 2 <content> : d'abord le MD5
            # du .md5 lui-même, puis le MD5 du .7z. On prend donc le DERNIER.
            local md5_hash
            md5_hash=$(xml_get_data_md5 "$detail_response")

            # Extraire la taille
            local file_size
            file_size=$(xml_get_link_length "$detail_response")

            local filename
            filename=$(basename "$download_url")

            # Extraire la date d'édition depuis le nom du dataset ou la réponse API
            local ds_edition_date
            ds_edition_date=$(echo "$detail_response" | grep -oP '<gpf_dl:editionDate>\K[0-9]{4}-[0-9]{2}-[0-9]{2}' | head -1 || true)
            if [[ -z "$ds_edition_date" ]]; then
                # Fallback : extraire la date du nom du dataset (dernier YYYY-MM-DD)
                ds_edition_date=$(echo "$dataset_name" | grep -oP '[0-9]{4}-[0-9]{2}-[0-9]{2}$' || true)
            fi

            local version_path
            version_path=$(edition_date_to_path "$ds_edition_date")
            local target_dir="${version_path}/${zone}"

            DOWNLOAD_URLS+=("$download_url")
            DOWNLOAD_DIRS+=("$target_dir")
            DOWNLOAD_NAMES+=("$filename")
            DOWNLOAD_MD5S+=("$md5_hash")

            local size_info=""
            if [[ -n "$file_size" && "$file_size" -gt 0 ]]; then
                size_info=" ($(numfmt --to=iec "$file_size" 2>/dev/null || echo "${file_size} o"))"
            fi

            log_ok "  $zone → $filename${size_info}"
            if [[ -n "$md5_hash" ]]; then log_debug "  MD5 attendu : $md5_hash"; fi
        done
    done

    echo ""
    if [[ ${#DOWNLOAD_URLS[@]} -eq 0 ]]; then
        log_error "Aucun dataset trouvé."
        log_error "Vérifiez vos paramètres (--zones, --format, --product)"
        exit 1
    fi

    log_ok "${#DOWNLOAD_URLS[@]} fichier(s) à télécharger"
}

# ---------------------------------------------------------------------------
# Téléchargement d'un fichier
# ---------------------------------------------------------------------------
download_file() {
    local url="$1" target_dir="$2" filename="$3" expected_md5="${4:-}"
    local filepath="${target_dir}/${filename}"

    if [[ "$DRY_RUN" == true ]]; then
        echo -e "    ${YELLOW}[DRY-RUN]${NC} curl -L -C - -o '$filepath' \\"
        echo -e "               '$url'"
        return 0
    fi

    mkdir -p "$target_dir"

    # Skip si données déjà extraites (archive supprimée après extraction)
    if [[ "$SKIP_EXISTING" == true && ! -f "$filepath" ]]; then
        local subdir_count data_count
        subdir_count=$(find "$target_dir" -mindepth 1 -maxdepth 1 -type d ! -name '_extract_tmp' 2>/dev/null | wc -l)
        data_count=$(find "$target_dir" -maxdepth 1 \( -name '*.shp' -o -name '*.asc' \) -type f 2>/dev/null | wc -l)
        if [[ "$subdir_count" -gt 0 || "$data_count" -gt 0 ]]; then
            log_ok "  Données déjà extraites dans $target_dir — skip"
            _LAST_DOWNLOAD_SIZE=0
            _LAST_DOWNLOAD_STATUS="skipped"
            return 0
        fi
    fi

    # Skip si déjà complet
    if [[ "$SKIP_EXISTING" == true && -f "$filepath" ]]; then
        local local_size
        local_size=$(stat -c%s "$filepath" 2>/dev/null || echo "0")

        # Vérifier le MD5 si on l'a
        if [[ -n "$expected_md5" && "$local_size" -gt 0 ]]; then
            local actual_md5
            actual_md5=$(md5sum "$filepath" 2>/dev/null | awk '{print $1}' || echo "")
            if [[ "$actual_md5" == "$expected_md5" ]]; then
                local human_size
                human_size=$(numfmt --to=iec "$local_size" 2>/dev/null || echo "${local_size} o")
                log_ok "  Déjà complet et vérifié ($human_size, MD5 OK)"
                _LAST_DOWNLOAD_SIZE=$local_size
                _LAST_DOWNLOAD_STATUS="skipped"
                return 0
            else
                log_warn "  Fichier existant avec MD5 différent — re-téléchargement"
            fi
        elif [[ "$local_size" -gt 0 ]]; then
            # Sans MD5, comparer avec la taille distante
            local remote_size
            remote_size=$(curl -sI -L "$url" 2>/dev/null \
                | grep -i 'content-length' | tail -1 | awk '{print $2}' | tr -d '\r' || echo "")

            if [[ -n "$remote_size" && "$remote_size" -gt 0 && "$local_size" == "$remote_size" ]]; then
                local human_size
                human_size=$(numfmt --to=iec "$local_size" 2>/dev/null || echo "${local_size} o")
                log_ok "  Déjà complet ($human_size)"
                _LAST_DOWNLOAD_SIZE=$local_size
                _LAST_DOWNLOAD_STATUS="skipped"
                return 0
            fi
        fi
    fi

    log_info "  Téléchargement en cours..."

    local max_retries=3 retry=0
    while [[ $retry -lt $max_retries ]]; do
        if curl -L -C - \
            --connect-timeout 30 \
            --max-time 7200 \
            --retry 3 \
            --retry-delay 5 \
            -o "$filepath" \
            "$url"; then

            local fsize
            fsize=$(stat -c%s "$filepath" 2>/dev/null || echo "0")

            # Vérifier que ce n'est pas une page d'erreur
            if [[ "$fsize" -lt 500 ]]; then
                if head -c 200 "$filepath" 2>/dev/null | grep -qi "error\|not.found\|404\|<!doctype"; then
                    log_error "  Erreur serveur retournée au lieu du fichier"
                    rm -f "$filepath"
                    retry=$((retry + 1))
                    if [[ $retry -lt $max_retries ]]; then log_warn "  Retry $((retry+1))/$max_retries..."; sleep 10; fi
                    continue
                fi
            fi

            # Vérifier MD5 si disponible — mismatch = fichier corrompu, retry
            if [[ -n "$expected_md5" ]]; then
                local actual_md5
                actual_md5=$(md5sum "$filepath" 2>/dev/null | awk '{print $1}' || echo "")
                if [[ "$actual_md5" != "$expected_md5" ]]; then
                    log_warn "  MD5 différent (attendu: $expected_md5, obtenu: $actual_md5) — suppression et retry"
                    rm -f "$filepath"
                    retry=$((retry + 1))
                    if [[ $retry -lt $max_retries ]]; then log_warn "  Retry $((retry+1))/$max_retries..."; sleep 10; fi
                    continue
                fi
                log_debug "  MD5 vérifié OK"
            fi

            local human_size
            human_size=$(numfmt --to=iec "$fsize" 2>/dev/null || echo "${fsize} o")
            log_ok "  Téléchargé ($human_size)"
            _LAST_DOWNLOAD_SIZE=$fsize
            _LAST_DOWNLOAD_STATUS="ok"
            return 0
        fi

        retry=$((retry + 1))
        if [[ $retry -lt $max_retries ]]; then log_warn "  Retry $((retry+1))/$max_retries..."; sleep 10; fi
    done

    log_error "  Échec après $max_retries tentatives"
    _LAST_DOWNLOAD_STATUS="failed"
    return 1
}

download_all() {
    log_step "Téléchargement"

    DOWNLOAD_START_TIME=$SECONDS
    DOWNLOAD_STATUSES=()
    DOWNLOAD_SIZES=()

    local total=${#DOWNLOAD_URLS[@]} success=0 failed=0

    for i in "${!DOWNLOAD_URLS[@]}"; do
        _LAST_DOWNLOAD_SIZE=0
        _LAST_DOWNLOAD_STATUS="failed"
        echo -e "${BOLD}[$((i+1))/$total]${NC} ${DOWNLOAD_NAMES[$i]}"
        if download_file "${DOWNLOAD_URLS[$i]}" "${DOWNLOAD_DIRS[$i]}" "${DOWNLOAD_NAMES[$i]}" "${DOWNLOAD_MD5S[$i]:-}"; then
            success=$((success + 1))
        else
            failed=$((failed + 1))
        fi
        DOWNLOAD_STATUSES+=("$_LAST_DOWNLOAD_STATUS")
        DOWNLOAD_SIZES+=("$_LAST_DOWNLOAD_SIZE")
    done

    echo ""
    log_ok "$success/$total fichiers téléchargés"
    if [[ $failed -gt 0 ]]; then log_warn "$failed en échec"; fi
}

# ---------------------------------------------------------------------------
# Extraction
# ---------------------------------------------------------------------------
extract_archives() {
    if [[ "$AUTO_EXTRACT" != true || "$DRY_RUN" == true ]]; then return 0; fi

    log_step "Extraction des archives"
    local extracted=0

    while IFS= read -r archive; do
        local bn archive_dir
        bn=$(basename "$archive")
        archive_dir=$(dirname "$archive")

        # Pour les splits, ne traiter que .7z.001
        if [[ "$bn" =~ \.7z\.[0-9]+$ && ! "$bn" =~ \.7z\.001$ ]]; then continue; fi

        log_info "Extraction : $bn"
        local tmp_extract="${archive_dir}/_extract_tmp"
        rm -rf "$tmp_extract"
        mkdir -p "$tmp_extract"
        _CURRENT_TMP_EXTRACT="$tmp_extract"

        if 7z x -o"$tmp_extract" -y "$archive" &>/dev/null; then
            # Chercher le dossier 1_DONNEES_LIVRAISON qui contient les shapefiles
            local data_dir
            data_dir=$(find "$tmp_extract" -type d -name "1_DONNEES_LIVRAISON_*" 2>/dev/null | head -1)

            if [[ -n "$data_dir" ]]; then
                # Le dossier contenant les thèmes (ADMINISTRATIF, BATI, etc.)
                # est un niveau en dessous : 1_DONNEES_LIVRAISON_.../BDT_3-5_SHP_...
                local themes_dir
                themes_dir=$(find "$data_dir" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | head -1)

                if [[ -n "$themes_dir" ]]; then
                    # Déplacer chaque dossier thème directement dans le répertoire cible
                    # Suppression préalable si déjà présent (idempotence — évite les dossiers imbriqués)
                    local count=0
                    while IFS= read -r theme_folder; do
                        local theme_name
                        theme_name=$(basename "$theme_folder")
                        rm -rf "${archive_dir}/${theme_name}"
                        mv "$theme_folder" "${archive_dir}/${theme_name}"
                        count=$((count + 1))
                    done < <(find "$themes_dir" -mindepth 1 -maxdepth 1 -type d 2>/dev/null)

                    log_ok "  → ${archive_dir}/ ($count dossiers thématiques)"
                    rm -rf "$tmp_extract"

                    # Supprimer l'archive après extraction réussie
                    rm -f "$archive"
                    # Supprimer les éventuels fichiers split associés (.7z.002, .7z.003, ...)
                    local archive_base="${archive%.001}"
                    if [[ "$archive_base" != "$archive" ]]; then
                        rm -f "${archive_base}."[0-9][0-9][0-9]
                    fi
                    log_ok "  Archive supprimée : $(basename "$archive")"

                    extracted=$((extracted + 1))
                else
                    log_error "  Structure inattendue dans 1_DONNEES_LIVRAISON — structure thématique introuvable"
                    rm -rf "$tmp_extract"
                fi
            else
                log_error "  Pas de dossier 1_DONNEES_LIVRAISON trouvé — archive non conforme"
                rm -rf "$tmp_extract"
            fi
        else
            log_error "  Échec : $bn"
            rm -rf "$tmp_extract"
        fi
        _CURRENT_TMP_EXTRACT=""
    done < <(find "$DATA_ROOT" \( -name "*.7z" -o -name "*.7z.001" \) -type f 2>/dev/null | sort -u)

    log_ok "$extracted archive(s) extraite(s)"
}

# ---------------------------------------------------------------------------
# Résumé
# ---------------------------------------------------------------------------
show_summary() {
    log_step "Résumé"
    echo -e "  ${BOLD}Produit :${NC}  $PRODUCT"
    echo -e "  ${BOLD}Format :${NC}   $FORMAT"
    echo -e "  ${BOLD}Thèmes :${NC}   $THEMES"
    if [[ -n "$REGION" ]]; then
        echo -e "  ${BOLD}Région :${NC}   $REGION (${#ZONES[@]} zone(s) : ${ZONES[*]})"
    else
        echo -e "  ${BOLD}Zones :${NC}    ${ZONES[*]}"
    fi
    echo -e "  ${BOLD}Sortie :${NC}   $DATA_ROOT"
    echo -e "  ${BOLD}Fichiers :${NC} ${#DOWNLOAD_URLS[@]}"
    if [[ "$DRY_RUN" == true ]]; then echo -e "  ${YELLOW}${BOLD}MODE DRY-RUN${NC}"; fi
}

# ---------------------------------------------------------------------------
# Échappement JSON minimal (backslash, guillemets, caractères de contrôle)
# Évite les injections JSON via noms de fichiers retournés par l'API IGN
# ---------------------------------------------------------------------------
json_escape() {
    local s="$1"
    s="${s//\\/\\\\}"        # \ → \\
    s="${s//\"/\\\"}"        # " → \"
    s="${s//$'\n'/\\n}"      # newline → \n
    s="${s//$'\r'/\\r}"      # CR → \r
    s="${s//$'\t'/\\t}"      # tab → \t
    printf '%s' "$s"
}

# ---------------------------------------------------------------------------
# Résumé JSON (AC6) — sortie parsable par jq, sans dépendance externe
# Option --json-output FILE pour écrire dans un fichier (défaut: stdout)
# ---------------------------------------------------------------------------
show_json_summary() {
    local total=${#DOWNLOAD_URLS[@]}
    local success=0 failed=0 total_bytes=0
    local duration=$(( SECONDS - DOWNLOAD_START_TIME ))

    # Construire le tableau zones
    local zones_json=""
    for z in "${ZONES[@]}"; do
        [[ -n "$zones_json" ]] && zones_json+=","
        zones_json+='"'"$z"'"'
    done

    # Construire le tableau files et calculer les totaux
    local files_json=""
    for i in "${!DOWNLOAD_URLS[@]}"; do
        local zone
        zone=$(basename "${DOWNLOAD_DIRS[$i]}")
        local file="${DOWNLOAD_NAMES[$i]}"
        local status="${DOWNLOAD_STATUSES[$i]:-failed}"
        local size="${DOWNLOAD_SIZES[$i]:-0}"

        case "$status" in
            ok|skipped) success=$(( success + 1 )) ;;
            failed)     failed=$(( failed + 1 )) ;;
        esac
        total_bytes=$(( total_bytes + size ))

        local zone_esc file_esc status_esc
        zone_esc=$(json_escape "$zone")
        file_esc=$(json_escape "$file")
        status_esc=$(json_escape "$status")
        [[ -n "$files_json" ]] && files_json+=","
        files_json+='{"zone":"'"$zone_esc"'","file":"'"$file_esc"'","size_bytes":'"$size"',"status":"'"$status_esc"'"}'
    done

    local json_output
    json_output=$(printf '{\n'
    printf '  "product": "%s",\n' "$(json_escape "$PRODUCT")"
    printf '  "format": "%s",\n' "$(json_escape "$FORMAT")"
    printf '  "zones": [%s],\n' "$zones_json"
    printf '  "total": %d,\n' "$total"
    printf '  "success": %d,\n' "$success"
    printf '  "failed": %d,\n' "$failed"
    printf '  "total_bytes": %d,\n' "$total_bytes"
    printf '  "duration_seconds": %d,\n' "$duration"
    printf '  "files": [%s]\n' "$files_json"
    printf '}\n')

    if [[ -n "$JSON_OUTPUT" ]]; then
        echo "$json_output" > "$JSON_OUTPUT"
        log_ok "Résumé JSON écrit dans : $JSON_OUTPUT"
    else
        echo "$json_output"
    fi
}

show_next_steps() {
    if [[ "$DRY_RUN" == true ]]; then return 0; fi
    log_step "Pipeline Garmin — prochaines étapes"
    echo -e "  ${CYAN}mpforge build --config france-garmin.yaml --rules bdtopo-garmin-rules.yaml --jobs 8${NC}"
    echo -e "  ${CYAN}java -Xmx10000m -jar mkgmap.jar --verbose --keep-going --index tiles/*.mp${NC}"
    if [[ "$PRODUCT" == "DIFF" ]]; then echo -e "\n  ${GREEN}Différentiel → ne régénérer que les tuiles impactées${NC}"; fi
    echo ""
}

# =============================================================================
# COURBES DE NIVEAU
# =============================================================================

# Résoudre les zones BDTOPO (régions/départements) en D-codes pour les courbes
resolve_contour_zones() {
    declare -ga CONTOUR_ZONES=()

    # Si --region est défini, utiliser REGIONS_TO_DEPARTMENTS
    if [[ -n "$REGION" ]]; then
        if [[ -z "${REGIONS_TO_DEPARTMENTS[$REGION]+x}" ]]; then
            log_error "Région inconnue pour courbes : $REGION"
            exit 1
        fi
        IFS=' ' read -ra CONTOUR_ZONES <<< "${REGIONS_TO_DEPARTMENTS[$REGION]}"
        log_info "Courbes — Région $REGION → ${#CONTOUR_ZONES[@]} département(s)"
        return
    fi

    # Si --zones est défini, résoudre chaque zone
    for zone in "${ZONES[@]}"; do
        if [[ "$zone" =~ ^D ]]; then
            # Déjà un D-code
            CONTOUR_ZONES+=("$zone")
        elif [[ -n "${REGIONS_TO_DEPARTMENTS[$zone]+x}" ]]; then
            # Zone multi-régions (FRANCE-NORD, FRANCE-SUD, FXX) ou raccourci région (ARA, etc.)
            IFS=' ' read -ra dept_list <<< "${REGIONS_TO_DEPARTMENTS[$zone]}"
            CONTOUR_ZONES+=("${dept_list[@]}")
            log_debug "Courbes — $zone → ${#dept_list[@]} département(s)"
        elif [[ "$zone" =~ ^R ]]; then
            # Code région → trouver la clé REGIONS correspondante et résoudre en départements
            # Word-boundary match pour éviter que R84 matche aussi FRANCE-SUD contenant "R84"
            local found=false
            for key in "${!REGIONS[@]}"; do
                if [[ " ${REGIONS[$key]} " == *" $zone "* ]]; then
                    if [[ -n "${REGIONS_TO_DEPARTMENTS[$key]+x}" ]]; then
                        IFS=' ' read -ra dept_list <<< "${REGIONS_TO_DEPARTMENTS[$key]}"
                        CONTOUR_ZONES+=("${dept_list[@]}")
                        log_debug "Courbes — $zone (via $key) → ${#dept_list[@]} département(s)"
                        found=true
                        break
                    fi
                fi
            done
            if [[ "$found" == false ]]; then
                log_warn "Courbes — impossible de résoudre $zone en départements, ignoré"
            fi
        else
            log_warn "Courbes — zone $zone non supportée (attendu D-code ou R-code)"
        fi
    done

    if [[ ${#CONTOUR_ZONES[@]} -eq 0 ]]; then
        log_error "Courbes — aucune zone à télécharger. Utilisez --zones D038 ou --region ARA avec --with-contours"
        exit 1
    fi

    # Dédupliquer
    local -A seen
    local unique=()
    for z in "${CONTOUR_ZONES[@]}"; do
        if [[ -z "${seen[$z]+x}" ]]; then
            seen[$z]=1
            unique+=("$z")
        fi
    done
    CONTOUR_ZONES=("${unique[@]}")

    log_info "Courbes — ${#CONTOUR_ZONES[@]} département(s) : ${CONTOUR_ZONES[*]}"
}

# Découverte des datasets COURBES via l'API
discover_contour_downloads() {
    log_step "Découverte des courbes de niveau via l'API"

    declare -ga CONTOUR_DOWNLOAD_URLS=()
    declare -ga CONTOUR_DOWNLOAD_DIRS=()
    declare -ga CONTOUR_DOWNLOAD_NAMES=()
    declare -ga CONTOUR_DOWNLOAD_MD5S=()

    for zone in "${CONTOUR_ZONES[@]}"; do
        log_info "Recherche courbes : $zone / SHP ..."

        local dataset_name=""

        # Les courbes ont une date fixe 2021-01-01 et un seul entry par département
        local probe_url="${API_BASE}/resource/COURBES?zone=${zone}&format=SHP&page=1&limit=1"
        local probe_response
        probe_response=$(api_fetch "$probe_url") || {
            log_warn "  API indisponible pour courbes $zone — ignoré"
            continue
        }

        if [[ -z "$probe_response" ]]; then
            log_warn "  Réponse vide pour courbes $zone — ignoré"
            continue
        fi

        # Extraire le titre du dataset depuis le bloc <entry> (pas le <title> du feed)
        dataset_name=$(echo "$probe_response" | grep -oP '<entry>[\s\S]*?</entry>' | grep -oP '<title>\K[^<]+' | head -1 || true)

        # Fallback : chercher un titre contenant la zone
        if [[ -z "$dataset_name" ]]; then
            dataset_name=$(echo "$probe_response" | grep -oP '<title>[^<]*'"${zone}"'[^<]*</title>' | grep -oP '>\K[^<]+' | head -1 || true)
        fi

        if [[ -z "$dataset_name" ]]; then
            log_warn "  Aucun dataset courbes trouvé pour $zone"
            continue
        fi

        log_debug "  Dataset courbes : $dataset_name"

        # Récupérer la page détail
        local detail_url="${API_BASE}/resource/COURBES/${dataset_name}"
        local detail_response
        detail_response=$(api_fetch "$detail_url") || {
            log_warn "  Dataset courbes introuvable : $dataset_name"
            continue
        }

        if [[ -z "$detail_response" ]]; then
            log_warn "  Réponse vide pour le détail de $dataset_name"
            continue
        fi

        local download_url
        download_url=$(xml_get_download_links "$detail_response" | head -1)

        if [[ -z "$download_url" ]]; then
            log_warn "  Aucune URL de download pour courbes $dataset_name"
            continue
        fi

        # MD5 du fichier data (dernier <content>, cf. note discover_downloads)
        local md5_hash
        md5_hash=$(xml_get_data_md5 "$detail_response")

        local file_size
        file_size=$(xml_get_link_length "$detail_response")

        local filename
        filename=$(basename "$download_url")

        local target_dir="${CONTOURS_DATA_ROOT}/${zone}"

        CONTOUR_DOWNLOAD_URLS+=("$download_url")
        CONTOUR_DOWNLOAD_DIRS+=("$target_dir")
        CONTOUR_DOWNLOAD_NAMES+=("$filename")
        CONTOUR_DOWNLOAD_MD5S+=("$md5_hash")

        local size_info=""
        if [[ -n "$file_size" && "$file_size" -gt 0 ]]; then
            size_info=" ($(numfmt --to=iec "$file_size" 2>/dev/null || echo "${file_size} o"))"
        fi

        log_ok "  $zone → $filename${size_info}"
    done

    echo ""
    if [[ ${#CONTOUR_DOWNLOAD_URLS[@]} -eq 0 ]]; then
        log_warn "Aucun dataset courbes trouvé."
    else
        log_ok "${#CONTOUR_DOWNLOAD_URLS[@]} fichier(s) courbes à télécharger"
    fi
}

# Téléchargement des courbes de niveau
download_contours() {
    if [[ ${#CONTOUR_DOWNLOAD_URLS[@]} -eq 0 ]]; then return 0; fi

    log_step "Téléchargement des courbes de niveau"
    local total=${#CONTOUR_DOWNLOAD_URLS[@]} success=0 failed=0

    for i in "${!CONTOUR_DOWNLOAD_URLS[@]}"; do
        _LAST_DOWNLOAD_SIZE=0
        _LAST_DOWNLOAD_STATUS="failed"
        echo -e "\n${BOLD}[$((i+1))/$total]${NC} ${CONTOUR_DOWNLOAD_NAMES[$i]}"
        if download_file "${CONTOUR_DOWNLOAD_URLS[$i]}" "${CONTOUR_DOWNLOAD_DIRS[$i]}" "${CONTOUR_DOWNLOAD_NAMES[$i]}" "${CONTOUR_DOWNLOAD_MD5S[$i]:-}"; then
            success=$((success + 1))
        else
            failed=$((failed + 1))
        fi
    done

    echo ""
    log_ok "Courbes : $success/$total fichiers téléchargés"
    if [[ $failed -gt 0 ]]; then log_warn "Courbes : $failed en échec"; fi
}

# Extraction des archives courbes (structure différente de BDTOPO — pas de 1_DONNEES_LIVRAISON_*)
extract_contour_archives() {
    if [[ "$AUTO_EXTRACT" != true || "$DRY_RUN" == true ]]; then return 0; fi
    if [[ ${#CONTOUR_DOWNLOAD_URLS[@]} -eq 0 ]]; then return 0; fi

    log_step "Extraction des archives courbes de niveau"
    local extracted=0

    # Itérer seulement sur les répertoires téléchargés dans ce run (pas tout CONTOURS_DATA_ROOT)
    for dir in "${CONTOUR_DOWNLOAD_DIRS[@]}"; do
    while IFS= read -r archive; do
        local bn archive_dir
        bn=$(basename "$archive")
        archive_dir=$(dirname "$archive")

        # Pour les splits, ne traiter que .7z.001
        if [[ "$bn" =~ \.7z\.[0-9]+$ && ! "$bn" =~ \.7z\.001$ ]]; then continue; fi

        log_info "Extraction courbes : $bn"
        local tmp_extract="${archive_dir}/_extract_tmp"
        rm -rf "$tmp_extract"
        mkdir -p "$tmp_extract"
        _CURRENT_TMP_EXTRACT="$tmp_extract"

        if 7z x -o"$tmp_extract" -y "$archive" &>/dev/null; then
            # Les courbes n'ont pas de 1_DONNEES_LIVRAISON — déplacer tous les SHP
            # vers le répertoire cible directement
            local shp_count=0
            while IFS= read -r shp_file; do
                local shp_name
                shp_name=$(basename "$shp_file")
                local shp_base="${shp_name%.*}"
                # Déplacer tous les fichiers associés (.shp, .dbf, .shx, .prj, .cpg)
                local src_dir
                src_dir=$(dirname "$shp_file")
                for ext in shp dbf shx prj cpg; do
                    if [[ -f "${src_dir}/${shp_base}.${ext}" ]]; then
                        mv "${src_dir}/${shp_base}.${ext}" "${archive_dir}/"
                    fi
                done
                shp_count=$((shp_count + 1))
            done < <(find "$tmp_extract" -name "*.shp" -type f 2>/dev/null)

            if [[ $shp_count -gt 0 ]]; then
                # Vérifier que l'extraction est complète avant de supprimer l'archive
                local expected_shp
                expected_shp=$(7z l "$archive" 2>/dev/null | grep -c '\.shp$' || echo "0")
                if [[ "$expected_shp" -gt 0 && "$shp_count" -lt "$expected_shp" ]]; then
                    log_warn "  Extraction partielle : $shp_count/$expected_shp fichiers SHP — archive conservée"
                else
                    log_ok "  → ${archive_dir}/ ($shp_count fichiers SHP)"
                    rm -f "$archive"
                    local archive_base="${archive%.001}"
                    if [[ "$archive_base" != "$archive" ]]; then
                        rm -f "${archive_base}."[0-9][0-9][0-9]
                    fi
                    log_ok "  Archive supprimée : $(basename "$archive")"
                fi

                extracted=$((extracted + 1))
            else
                log_warn "  Aucun SHP trouvé dans l'archive courbes $bn"
            fi
            rm -rf "$tmp_extract"
        else
            log_error "  Échec extraction : $bn"
            rm -rf "$tmp_extract"
        fi
        _CURRENT_TMP_EXTRACT=""
    done < <(find "$dir" \( -name "*.7z" -o -name "*.7z.001" \) -type f 2>/dev/null | sort -u)
    done

    log_ok "$extracted archive(s) courbes extraite(s)"
}

# =============================================================================
# OSM PBF (Geofabrik)
# =============================================================================

# Résoudre les zones en noms de régions Geofabrik
resolve_osm_regions() {
    declare -ga OSM_REGIONS=()

    # Si --region est défini, utiliser REGIONS_TO_GEOFABRIK
    if [[ -n "$REGION" ]]; then
        if [[ -z "${REGIONS_TO_GEOFABRIK[$REGION]+x}" ]]; then
            log_error "Région inconnue pour OSM : $REGION"
            exit 1
        fi
        IFS=' ' read -ra OSM_REGIONS <<< "${REGIONS_TO_GEOFABRIK[$REGION]}"
        log_info "OSM — Région $REGION → ${#OSM_REGIONS[@]} fichier(s) Geofabrik : ${OSM_REGIONS[*]}"
    else
        # Résoudre depuis --zones
        for zone in "${ZONES[@]}"; do
            if [[ "$zone" =~ ^D ]]; then
                # Département → trouver la région parente → Geofabrik
                local region_code="${DEPT_TO_REGION[$zone]:-}"
                if [[ -z "$region_code" ]]; then
                    log_warn "OSM — département $zone inconnu, ignoré"
                    continue
                fi
                IFS=' ' read -ra geofabrik_names <<< "${REGIONS_TO_GEOFABRIK[$region_code]}"
                OSM_REGIONS+=("${geofabrik_names[@]}")
                log_debug "OSM — $zone → $region_code → ${geofabrik_names[*]}"
            elif [[ -n "${REGIONS_TO_GEOFABRIK[$zone]+x}" ]]; then
                # Code région ou groupement connu directement
                IFS=' ' read -ra geofabrik_names <<< "${REGIONS_TO_GEOFABRIK[$zone]}"
                OSM_REGIONS+=("${geofabrik_names[@]}")
                log_debug "OSM — $zone → ${geofabrik_names[*]}"
            else
                log_warn "OSM — zone $zone non supportée pour Geofabrik"
            fi
        done
    fi

    if [[ ${#OSM_REGIONS[@]} -eq 0 ]]; then
        log_error "OSM — aucune région Geofabrik à télécharger"
        exit 1
    fi

    # Dédupliquer
    local -A seen
    local unique=()
    for r in "${OSM_REGIONS[@]}"; do
        if [[ -z "${seen[$r]+x}" ]]; then
            seen[$r]=1
            unique+=("$r")
        fi
    done
    OSM_REGIONS=("${unique[@]}")

    log_info "OSM — ${#OSM_REGIONS[@]} fichier(s) PBF Geofabrik : ${OSM_REGIONS[*]}"
}

# Télécharger les fichiers PBF depuis Geofabrik
download_osm_pbf() {
    if [[ ${#OSM_REGIONS[@]} -eq 0 ]]; then return 0; fi

    log_step "Téléchargement des données OSM (Geofabrik PBF)"

    declare -ga OSM_DOWNLOAD_FILES=()
    local total=${#OSM_REGIONS[@]} success=0 failed=0

    for i in "${!OSM_REGIONS[@]}"; do
        local region="${OSM_REGIONS[$i]}"
        local filename="${region}-latest.osm.pbf"
        local url

        # France entière = URL racine, régions = sous-dossier
        if [[ "$region" == "france" ]]; then
            url="https://download.geofabrik.de/europe/france-latest.osm.pbf"
        else
            url="${GEOFABRIK_BASE}/${filename}"
        fi

        echo -e "\n${BOLD}[$((i+1))/$total]${NC} ${filename}"

        if [[ "$DRY_RUN" == true ]]; then
            echo -e "    ${YELLOW}[DRY-RUN]${NC} curl -L -C - -o '${OSM_DATA_ROOT}/${filename}' \\"
            echo -e "               '${url}'"
            OSM_DOWNLOAD_FILES+=("$filename")
            success=$((success + 1))
            continue
        fi

        local filepath="${OSM_DATA_ROOT}/${filename}"

        # Skip si existant et même taille
        if [[ "$SKIP_EXISTING" == true && -f "$filepath" ]]; then
            local local_size
            local_size=$(stat -c%s "$filepath" 2>/dev/null || echo "0")

            if [[ "$local_size" -gt 0 ]]; then
                local remote_size
                remote_size=$(curl -sI -L "$url" 2>/dev/null \
                    | grep -i 'content-length' | tail -1 | awk '{print $2}' | tr -d '\r' || echo "")

                if [[ -n "$remote_size" && "$remote_size" -gt 0 && "$local_size" == "$remote_size" ]]; then
                    local human_size
                    human_size=$(numfmt --to=iec "$local_size" 2>/dev/null || echo "${local_size} o")
                    log_ok "  Déjà présent ($human_size) — skip"
                    OSM_DOWNLOAD_FILES+=("$filename")
                    success=$((success + 1))
                    continue
                fi
            fi
        fi

        mkdir -p "$OSM_DATA_ROOT"
        log_info "  Téléchargement en cours..."
        if curl -L -C - --connect-timeout 30 --max-time 7200 --retry 3 --retry-delay 5 \
            -o "$filepath" "$url" 2>/dev/null; then
            local dl_size
            dl_size=$(stat -c%s "$filepath" 2>/dev/null || echo "0")
            local human_size
            human_size=$(numfmt --to=iec "$dl_size" 2>/dev/null || echo "${dl_size} o")
            log_ok "  Téléchargé ($human_size)"
            OSM_DOWNLOAD_FILES+=("$filename")
            success=$((success + 1))
        else
            log_error "  Échec téléchargement : $filename"
            failed=$((failed + 1))
        fi
    done

    echo ""
    log_ok "OSM : $success/$total fichiers PBF téléchargés dans $OSM_DATA_ROOT"
    if [[ $failed -gt 0 ]]; then log_warn "OSM : $failed en échec"; fi
}

# Convertir les PBF en GPKG par catégorie (élimine les problèmes du driver OSM)
# Le driver OSM lit séquentiellement et bufferise les couches non demandées,
# provoquant "Too many features accumulated" sur les gros PBF.
# Les GPKG ont un accès aléatoire → pas de problème de buffer.
prepare_osm_gpkg() {
    if [[ ${#OSM_DOWNLOAD_FILES[@]} -eq 0 && "$DRY_RUN" == true ]]; then
        log_info "OSM — [DRY-RUN] ogr2ogr convertirait les PBF en GPKG"
        return 0
    fi

    command -v ogr2ogr &>/dev/null || {
        log_error "ogr2ogr requis pour la conversion PBF → GPKG (apt install gdal-bin)"
        exit 1
    }

    log_step "Conversion OSM PBF → GPKG"

    local osmconf="${OSM_CONFIG_FILE:-pipeline/configs/osm/osmconf.ini}"
    if [[ ! -f "$osmconf" ]]; then
        log_error "osmconf.ini introuvable : $osmconf"
        exit 1
    fi

    local gpkg_dir="${OSM_DATA_ROOT}/gpkg"
    mkdir -p "$gpkg_dir"

    # Catégories à extraire : nom, couche source, filtre SQL
    local -a categories=(
        "amenity-points|points|amenity IS NOT NULL"
        "shop-points|points|shop IS NOT NULL"
        "natural-lines|lines|natural IN ('ridge','arete','cliff')"
        "natural-points|points|natural IN ('cave_entrance','cave','rock','sinkhole')"
        "tourism-points|points|tourism = 'viewpoint'"
    )

    for pbf in "${OSM_DATA_ROOT}"/*.osm.pbf; do
        [[ -f "$pbf" ]] || continue
        local base
        base=$(basename "$pbf" .osm.pbf)

        for cat_def in "${categories[@]}"; do
            IFS='|' read -r cat_name layer_name where_clause <<< "$cat_def"
            local gpkg_file="${gpkg_dir}/${base}-${cat_name}.gpkg"

            if [[ "$SKIP_EXISTING" == true && -f "$gpkg_file" ]]; then
                local gpkg_size
                gpkg_size=$(stat -c%s "$gpkg_file" 2>/dev/null || echo "0")
                if [[ "$gpkg_size" -gt 0 ]]; then
                    log_ok "  ${base}-${cat_name}.gpkg — déjà présent, skip"
                    continue
                fi
            fi

            log_info "  ${base} → ${cat_name} ..."
            rm -f "$gpkg_file"
            if OSM_CONFIG_FILE="$osmconf" OGR_GEOMETRY_ACCEPT_UNCLOSED_RING=YES \
                OSM_MAX_TMPFILE_SIZE=1024 \
                ogr2ogr -f GPKG "$gpkg_file" "$pbf" "$layer_name" \
                -where "$where_clause" -progress 2>/dev/null; then
                local gpkg_size
                gpkg_size=$(stat -c%s "$gpkg_file" 2>/dev/null || echo "0")
                local human_size
                human_size=$(numfmt --to=iec "$gpkg_size" 2>/dev/null || echo "${gpkg_size} o")
                log_ok "  ${base}-${cat_name}.gpkg ($human_size)"
            else
                log_error "  Échec conversion : ${base}-${cat_name}"
                rm -f "$gpkg_file"
            fi
        done
    done

    log_ok "GPKG OSM dans : $gpkg_dir"
}

# =============================================================================
# DEM — BD ALTI v2 (MNT 25m, ASC, par département)
# =============================================================================

# Résoudre les zones en D-codes pour le DEM (même logique que les courbes)
resolve_dem_zones() {
    declare -ga DEM_ZONES=()

    if [[ -n "$REGION" ]]; then
        if [[ -z "${REGIONS_TO_DEPARTMENTS[$REGION]+x}" ]]; then
            log_error "Région inconnue pour DEM : $REGION"
            exit 1
        fi
        IFS=' ' read -ra DEM_ZONES <<< "${REGIONS_TO_DEPARTMENTS[$REGION]}"
        log_info "DEM — Région $REGION → ${#DEM_ZONES[@]} département(s)"
        return
    fi

    for zone in "${ZONES[@]}"; do
        if [[ "$zone" =~ ^D ]]; then
            DEM_ZONES+=("$zone")
        elif [[ -n "${REGIONS_TO_DEPARTMENTS[$zone]+x}" ]]; then
            IFS=' ' read -ra dept_list <<< "${REGIONS_TO_DEPARTMENTS[$zone]}"
            DEM_ZONES+=("${dept_list[@]}")
            log_debug "DEM — $zone → ${#dept_list[@]} département(s)"
        elif [[ "$zone" =~ ^R ]]; then
            local found=false
            for key in "${!REGIONS[@]}"; do
                if [[ " ${REGIONS[$key]} " == *" $zone "* ]]; then
                    if [[ -n "${REGIONS_TO_DEPARTMENTS[$key]+x}" ]]; then
                        IFS=' ' read -ra dept_list <<< "${REGIONS_TO_DEPARTMENTS[$key]}"
                        DEM_ZONES+=("${dept_list[@]}")
                        log_debug "DEM — $zone (via $key) → ${#dept_list[@]} département(s)"
                        found=true
                        break
                    fi
                fi
            done
            if [[ "$found" == false ]]; then
                log_warn "DEM — impossible de résoudre $zone en départements, ignoré"
            fi
        else
            log_warn "DEM — zone $zone non supportée (attendu D-code ou R-code)"
        fi
    done

    if [[ ${#DEM_ZONES[@]} -eq 0 ]]; then
        log_error "DEM — aucune zone à télécharger"
        exit 1
    fi

    # Dédupliquer
    local -A seen
    local unique=()
    for z in "${DEM_ZONES[@]}"; do
        if [[ -z "${seen[$z]+x}" ]]; then
            seen[$z]=1
            unique+=("$z")
        fi
    done
    DEM_ZONES=("${unique[@]}")

    log_info "DEM — ${#DEM_ZONES[@]} département(s) : ${DEM_ZONES[*]}"
}

# Découverte des datasets BD ALTI v2 via l'API
discover_dem_downloads() {
    log_step "Découverte des MNT BD ALTI v2 via l'API"

    declare -ga DEM_DOWNLOAD_URLS=()
    declare -ga DEM_DOWNLOAD_DIRS=()
    declare -ga DEM_DOWNLOAD_NAMES=()
    declare -ga DEM_DOWNLOAD_MD5S=()

    for zone in "${DEM_ZONES[@]}"; do
        log_info "Recherche DEM : $zone / ASC ..."

        local probe_url="${API_BASE}/resource/BDALTI?zone=${zone}&format=ASC&page=1&limit=1"
        local probe_response
        probe_response=$(api_fetch "$probe_url") || {
            log_warn "  API indisponible pour DEM $zone — ignoré"
            continue
        }

        if [[ -z "$probe_response" ]]; then
            log_warn "  Réponse vide pour DEM $zone — ignoré"
            continue
        fi

        local total_entries
        total_entries=$(echo "$probe_response" | grep -oP 'gpf_dl:totalentries="\K[0-9]+' | head -1 || echo "0")

        if [[ "$total_entries" == "0" ]]; then
            log_warn "  Aucun dataset DEM trouvé pour $zone"
            continue
        fi

        # Récupérer le dataset le plus récent (dernière page)
        local last_page_url="${API_BASE}/resource/BDALTI?zone=${zone}&format=ASC&page=${total_entries}&limit=1"
        local last_page_response
        last_page_response=$(api_fetch "$last_page_url") || {
            log_warn "  Impossible de récupérer le dernier dataset DEM pour $zone"
            continue
        }

        local dataset_name
        dataset_name=$(echo "$last_page_response" | grep -oP '<entry>[\s\S]*?</entry>' | grep -oP '<title>\K[^<]+' | tail -1 || true)

        if [[ -z "$dataset_name" ]]; then
            dataset_name=$(echo "$last_page_response" | grep -oP '<title>[^<]*'"${zone}"'[^<]*</title>' | grep -oP '>\K[^<]+' | tail -1 || true)
        fi

        if [[ -z "$dataset_name" ]]; then
            log_warn "  Aucun dataset DEM trouvé pour $zone"
            continue
        fi

        log_debug "  Dataset DEM : $dataset_name"

        local detail_url="${API_BASE}/resource/BDALTI/${dataset_name}"
        local detail_response
        detail_response=$(api_fetch "$detail_url") || {
            log_warn "  Dataset DEM introuvable : $dataset_name"
            continue
        }

        if [[ -z "$detail_response" ]]; then
            log_warn "  Réponse vide pour le détail de $dataset_name"
            continue
        fi

        local download_url
        download_url=$(xml_get_download_links "$detail_response" | head -1)

        if [[ -z "$download_url" ]]; then
            log_warn "  Aucune URL de download pour DEM $dataset_name"
            continue
        fi

        # MD5 du fichier data (dernier <content>, cf. note discover_downloads)
        local md5_hash
        md5_hash=$(xml_get_data_md5 "$detail_response")

        local file_size
        file_size=$(xml_get_link_length "$detail_response")

        local filename
        filename=$(basename "$download_url")

        local target_dir="${DEM_DATA_ROOT}/${zone}"

        DEM_DOWNLOAD_URLS+=("$download_url")
        DEM_DOWNLOAD_DIRS+=("$target_dir")
        DEM_DOWNLOAD_NAMES+=("$filename")
        DEM_DOWNLOAD_MD5S+=("$md5_hash")

        local size_info=""
        if [[ -n "$file_size" && "$file_size" -gt 0 ]]; then
            size_info=" ($(numfmt --to=iec "$file_size" 2>/dev/null || echo "${file_size} o"))"
        fi

        log_ok "  $zone → $filename${size_info}"
    done

    echo ""
    if [[ ${#DEM_DOWNLOAD_URLS[@]} -eq 0 ]]; then
        log_warn "Aucun dataset DEM trouvé."
    else
        log_ok "${#DEM_DOWNLOAD_URLS[@]} fichier(s) DEM à télécharger"
    fi
}

# Téléchargement des fichiers DEM
download_dem() {
    if [[ ${#DEM_DOWNLOAD_URLS[@]} -eq 0 ]]; then return 0; fi

    log_step "Téléchargement des MNT BD ALTI v2"
    local total=${#DEM_DOWNLOAD_URLS[@]} success=0 failed=0

    for i in "${!DEM_DOWNLOAD_URLS[@]}"; do
        _LAST_DOWNLOAD_SIZE=0
        _LAST_DOWNLOAD_STATUS="failed"
        echo -e "${BOLD}[$((i+1))/$total]${NC} ${DEM_DOWNLOAD_NAMES[$i]}"
        if download_file "${DEM_DOWNLOAD_URLS[$i]}" "${DEM_DOWNLOAD_DIRS[$i]}" "${DEM_DOWNLOAD_NAMES[$i]}" "${DEM_DOWNLOAD_MD5S[$i]:-}"; then
            success=$((success + 1))
        else
            failed=$((failed + 1))
        fi
    done

    echo ""
    log_ok "DEM : $success/$total fichiers téléchargés"
    if [[ $failed -gt 0 ]]; then log_warn "DEM : $failed en échec"; fi
}

# Extraction des archives DEM (structure : dossier racine contenant des .asc)
extract_dem_archives() {
    if [[ "$AUTO_EXTRACT" != true || "$DRY_RUN" == true ]]; then return 0; fi
    if [[ ${#DEM_DOWNLOAD_URLS[@]} -eq 0 ]]; then return 0; fi

    log_step "Extraction des archives DEM"
    local extracted=0

    for dir in "${DEM_DOWNLOAD_DIRS[@]}"; do
    while IFS= read -r archive; do
        local bn archive_dir
        bn=$(basename "$archive")
        archive_dir=$(dirname "$archive")

        if [[ "$bn" =~ \.7z\.[0-9]+$ && ! "$bn" =~ \.7z\.001$ ]]; then continue; fi

        log_info "Extraction DEM : $bn"
        local tmp_extract="${archive_dir}/_extract_tmp"
        rm -rf "$tmp_extract"
        mkdir -p "$tmp_extract"
        _CURRENT_TMP_EXTRACT="$tmp_extract"

        if 7z x -o"$tmp_extract" -y "$archive" &>/dev/null; then
            # Déplacer le contenu extrait dans le répertoire cible
            local asc_count=0
            while IFS= read -r asc_file; do
                local asc_name
                asc_name=$(basename "$asc_file")
                mv "$asc_file" "${archive_dir}/"
                asc_count=$((asc_count + 1))
            done < <(find "$tmp_extract" -name "*.asc" -type f 2>/dev/null)

            if [[ $asc_count -gt 0 ]]; then
                # Vérifier que l'extraction est complète avant de supprimer l'archive
                local expected_asc
                expected_asc=$(7z l "$archive" 2>/dev/null | grep -c '\.asc$' || echo "0")
                if [[ "$expected_asc" -gt 0 && "$asc_count" -lt "$expected_asc" ]]; then
                    log_warn "  Extraction partielle : $asc_count/$expected_asc fichiers ASC — archive conservée"
                else
                    log_ok "  → ${archive_dir}/ ($asc_count fichiers ASC)"
                    rm -f "$archive"
                    local archive_base="${archive%.001}"
                    if [[ "$archive_base" != "$archive" ]]; then
                        rm -f "${archive_base}."[0-9][0-9][0-9]
                    fi
                    log_ok "  Archive supprimée : $(basename "$archive")"
                fi

                extracted=$((extracted + 1))
            else
                log_warn "  Aucun ASC trouvé dans l'archive DEM $bn"
            fi
            rm -rf "$tmp_extract"
        else
            log_error "  Échec extraction : $bn"
            rm -rf "$tmp_extract"
        fi
        _CURRENT_TMP_EXTRACT=""
    done < <(find "$dir" \( -name "*.7z" -o -name "*.7z.001" \) -type f 2>/dev/null | sort -u)
    done

    log_ok "$extracted archive(s) DEM extraite(s)"
}

# =============================================================================
# MAIN
# =============================================================================
main() {
    echo ""
    echo -e "${BOLD}${CYAN}  ┌──────────────────────────────────────────────┐${NC}"
    echo -e "${BOLD}${CYAN}  │  download-bdtopo.sh v${SCRIPT_VERSION}                   │${NC}"
    echo -e "${BOLD}${CYAN}  │  Téléchargement BD TOPO IGN - data.geopf.fr  │${NC}"
    echo -e "${BOLD}${CYAN}  └──────────────────────────────────────────────┘${NC}"

    parse_args "$@"
    check_prerequisites
    resolve_zones

    # --list-editions : lister les millésimes puis quitter, avant tout téléchargement
    if [[ "$LIST_EDITIONS" == true ]]; then
        run_list_editions
    fi

    # --bdtopo-version vYYYY.MM → résolution en EDITION_DATE via API
    if [[ -n "$BDTOPO_VERSION" ]]; then
        if [[ -n "$EDITION_DATE" ]]; then
            log_error "--bdtopo-version et --date sont mutuellement exclusifs"
            exit 1
        fi
        resolve_bdtopo_version "$BDTOPO_VERSION"
    fi

    discover_downloads
    show_summary
    download_all
    extract_archives

    # Passe 2 : Courbes de niveau (si --with-contours)
    if [[ "$WITH_CONTOURS" == true ]]; then
        resolve_contour_zones
        discover_contour_downloads
        download_contours
        extract_contour_archives
        log_ok "Courbes de niveau dans : $CONTOURS_DATA_ROOT"
    fi

    # Passe 3 : OSM PBF (si --with-osm)
    if [[ "$WITH_OSM" == true ]]; then
        resolve_osm_regions
        download_osm_pbf
        prepare_osm_gpkg
        log_ok "Données OSM dans : $OSM_DATA_ROOT"
    fi

    # Passe 4 : DEM BD ALTI v2 (si --with-dem)
    if [[ "$WITH_DEM" == true ]]; then
        resolve_dem_zones
        discover_dem_downloads
        download_dem
        extract_dem_archives
        log_ok "Données DEM dans : $DEM_DATA_ROOT"
    fi

    show_next_steps
    if [[ "$DRY_RUN" == false ]]; then show_json_summary; fi
    log_ok "Terminé — données dans : $DATA_ROOT"
}

main "$@"
