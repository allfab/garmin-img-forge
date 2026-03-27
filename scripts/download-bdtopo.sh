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
# Pipeline : download-bdtopo.sh → mpforge-cli → imgforge-cli → gmapsupp.img
#            (actuellement : download-bdtopo.sh → mpforge-cli → mkgmap → gmapsupp.img)
#
# Prérequis : curl, 7z (p7zip-full)
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
API_BASE="https://data.geopf.fr/telechargement"
DATA_ROOT="./data/bdtopo"
PRODUCT="FULL"          # FULL | DIFF | EXPRESS
FORMAT="SHP"            # SHP | GPKG | SQL
THEMES="TOUSTHEMES"     # TOUSTHEMES | TRANSPORT | HYDROGRAPHIE | etc.
ZONES=()
REGION=""
EDITION_DATE=""          # YYYY-MM-DD — si vide, on prend le plus récent via l'API
DRY_RUN=false
SKIP_EXISTING=true
AUTO_EXTRACT=true
DEBUG=false
SCRIPT_VERSION="1.0.0"

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
log_step()  { echo -e "\n${BOLD}${CYAN}═══ $* ═══${NC}\n"; }
log_debug() { [[ "$DEBUG" == true ]] && echo -e "${YELLOW}[DEBUG]${NC} $*" || true; }

# ---------------------------------------------------------------------------
# Régions
# ---------------------------------------------------------------------------
declare -A REGIONS=(
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
    [FXX]="D001 D002 D003 D004 D005 D006 D007 D008 D009 D010 D011 D012 D013 D014 D015 D016 D017 D018 D019 D02A D02B D021 D022 D023 D024 D025 D026 D027 D028 D029 D030 D031 D032 D033 D034 D035 D036 D037 D038 D039 D040 D041 D042 D043 D044 D045 D046 D047 D048 D049 D050 D051 D052 D053 D054 D055 D056 D057 D058 D059 D060 D061 D062 D063 D064 D065 D066 D067 D068 D069 D070 D071 D072 D073 D074 D075 D076 D077 D078 D079 D080 D081 D082 D083 D084 D085 D086 D087 D088 D089 D090 D091 D092 D093 D094 D095"
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
    --zones ZONES       Codes département (ex: D038 ou D001,D007,D038)
    --region CODE       Raccourci région : ARA BFC BRE CVL COR GES HDF IDF
                        NOR NAQ OCC PDL PAC FXX (France métro complète)
    --format FORMAT     SHP (défaut) | GPKG | SQL
    --product PRODUCT   FULL (défaut) | DIFF | EXPRESS
    --themes THEMES     TOUSTHEMES (défaut) | TRANSPORT | HYDROGRAPHIE | etc.
    --date YYYY-MM-DD   Forcer une date d'édition (sinon la plus récente)
    --data-root DIR     Racine des données (défaut: ./data/bdtopo)
    --no-extract        Ne pas décompresser les .7z
    --no-skip           Re-télécharger même si déjà présent
    --dry-run           Simuler sans télécharger
    --debug             Afficher les requêtes API et réponses
    --version           Version du script
    -h, --help          Aide

EXEMPLES :
    ./download-bdtopo.sh --zones D038 --format SHP
    ./download-bdtopo.sh --region ARA --format GPKG
    ./download-bdtopo.sh --zones D038 --product DIFF --format GPKG
    ./download-bdtopo.sh --product EXPRESS
    ./download-bdtopo.sh --region FXX --dry-run
    ./download-bdtopo.sh --zones D038 --date 2025-12-15
    DEBUG=1 ./download-bdtopo.sh --zones D038
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
            --date)       EDITION_DATE="$2"; shift 2 ;;
            --data-root)  DATA_ROOT="$2"; shift 2 ;;
            --no-extract) AUTO_EXTRACT=false; shift ;;
            --no-skip)    SKIP_EXISTING=false; shift ;;
            --dry-run)    DRY_RUN=true; shift ;;
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
        log_info "Région $REGION → ${#ZONES[@]} départements"
    fi

    if [[ "$PRODUCT" == "EXPRESS" ]]; then
        ZONES=("FXX"); FORMAT="GPKG"
        log_info "BD TOPO Express : France entière en GPKG"
        return
    fi

    if [[ ${#ZONES[@]} -eq 0 ]]; then
        log_error "Aucune zone. Utilisez --zones D038 ou --region ARA"
        exit 1
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
xml_get_download_links() {
    local xml="$1"
    echo "$xml" | grep -oP '<link[^>]+href="\K[^"]*download[^"]*' 2>/dev/null || true
}

# Extraire l'attribut gpf_dl:length d'une balise <link>
xml_get_link_length() {
    local xml="$1"
    echo "$xml" | grep -oP 'gpf_dl:length="\K[0-9]+' 2>/dev/null | head -1 || true
}

# ---------------------------------------------------------------------------
# Nom de la ressource API selon le produit
# ---------------------------------------------------------------------------
get_resource_name() {
    case "$PRODUCT" in
        FULL)    echo "BDTOPO" ;;
        DIFF)    echo "BDTOPO_DIFF" ;;
        EXPRESS) echo "BDTOPO_EXPRESS" ;;
    esac
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

            # Extraire le hash MD5 (dans <content> de l'entry)
            local md5_hash
            md5_hash=$(echo "$detail_response" | grep -oP '<entry>[\s\S]*?<content>\K[a-f0-9]{32}' | head -1 || true)
            # Fallback
            if [[ -z "$md5_hash" ]]; then
                md5_hash=$(echo "$detail_response" | grep -oP '<content>[a-f0-9]{32}</content>' | grep -oP '>[a-f0-9]{32}<' | grep -oP '[a-f0-9]{32}' | head -1 || true)
            fi

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

    mkdir -p "$target_dir"

    if [[ "$DRY_RUN" == true ]]; then
        echo -e "    ${YELLOW}[DRY-RUN]${NC} curl -L -C - -o '$filepath' \\"
        echo -e "               '$url'"
        return 0
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
            return 0
        fi

        retry=$((retry + 1))
        if [[ $retry -lt $max_retries ]]; then log_warn "  Retry $((retry+1))/$max_retries..."; sleep 10; fi
    done

    log_error "  Échec après $max_retries tentatives"
    return 1
}

download_all() {
    log_step "Téléchargement"

    local total=${#DOWNLOAD_URLS[@]} success=0 failed=0

    for i in "${!DOWNLOAD_URLS[@]}"; do
        echo -e "\n${BOLD}[$((i+1))/$total]${NC} ${DOWNLOAD_NAMES[$i]}"
        if download_file "${DOWNLOAD_URLS[$i]}" "${DOWNLOAD_DIRS[$i]}" "${DOWNLOAD_NAMES[$i]}" "${DOWNLOAD_MD5S[$i]:-}"; then
            success=$((success + 1))
        else
            failed=$((failed + 1))
        fi
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
        local bn dir
        bn=$(basename "$archive")
        dir=$(dirname "$archive")

        # Pour les splits, ne traiter que .7z.001
        if [[ "$bn" =~ \.7z\.[0-9]+$ && ! "$bn" =~ \.7z\.001$ ]]; then continue; fi

        log_info "Extraction : $bn"
        local tmp_extract="${dir}/_extract_tmp"
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
                        rm -rf "${dir}/${theme_name}"
                        mv "$theme_folder" "${dir}/${theme_name}"
                        count=$((count + 1))
                    done < <(find "$themes_dir" -mindepth 1 -maxdepth 1 -type d 2>/dev/null)

                    log_ok "  → ${dir}/ ($count dossiers thématiques)"
                    rm -rf "$tmp_extract"
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
        echo -e "  ${BOLD}Région :${NC}   $REGION (${#ZONES[@]} départements)"
    else
        echo -e "  ${BOLD}Zones :${NC}    ${ZONES[*]}"
    fi
    echo -e "  ${BOLD}Sortie :${NC}   $DATA_ROOT"
    echo -e "  ${BOLD}Fichiers :${NC} ${#DOWNLOAD_URLS[@]}"
    if [[ "$DRY_RUN" == true ]]; then echo -e "\n  ${YELLOW}${BOLD}MODE DRY-RUN${NC}"; fi
    echo ""
}

show_next_steps() {
    if [[ "$DRY_RUN" == true ]]; then return 0; fi
    log_step "Pipeline Garmin — prochaines étapes"
    echo -e "  ${CYAN}mpforge-cli build --config france-garmin.yaml --rules bdtopo-garmin-rules.yaml --jobs 8${NC}"
    echo -e "  ${CYAN}java -Xmx10000m -jar mkgmap.jar --verbose --keep-going --index tiles/*.mp${NC}"
    if [[ "$PRODUCT" == "DIFF" ]]; then echo -e "\n  ${GREEN}Différentiel → ne régénérer que les tuiles impactées${NC}"; fi
    echo ""
}

# =============================================================================
# MAIN
# =============================================================================
main() {
    echo -e "${BOLD}${CYAN}"
    echo "  ┌─────────────────────────────────────────────────────┐"
    echo "  │  download-bdtopo.sh — Téléchargement BD TOPO® IGN  │"
    echo "  │  API Géoplateforme · data.geopf.fr                 │"
    echo "  └─────────────────────────────────────────────────────┘"
    echo -e "${NC}"

    parse_args "$@"
    check_prerequisites
    resolve_zones
    discover_downloads
    show_summary
    download_all
    extract_archives
    show_next_steps
    log_ok "Terminé — données dans : $DATA_ROOT"
}

main "$@"
