#!/usr/bin/env bash
# =============================================================================
# build-garmin-map.sh — Pipeline mpforge-cli → mkgmap → gmapsupp.img
# =============================================================================
#
# Enchaîne mpforge-cli build et mkgmap pour produire une carte Garmin :
#
#   1. Prépare la config YAML (fournie ou générée depuis DATA_ROOT)
#   2. Lance mpforge-cli build (génère les tuiles .mp)
#   3. Vérifie le code de sortie et le rapport JSON
#   4. Lance mkgmap (compile .mp → gmapsupp.img)
#   5. Affiche le résumé final (tuiles, temps, taille)
#
# Pipeline : download-bdtopo.sh → build-garmin-map.sh → gmapsupp.img
#            (version mkgmap — sera remplacé par imgforge-cli — Epic 16)
#
# Prérequis : mpforge-cli (ou cargo), java, mkgmap.jar
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration par défaut (Task 1.2)
# ---------------------------------------------------------------------------
SCRIPT_VERSION="1.0.0"
DATA_ROOT="./data/bdtopo"
OUTPUT_DIR="./output"
REPORT_FILE=""              # calculé après parse_args (dépend de OUTPUT_DIR)
JOBS=8
DRY_RUN=false
CONFIG_FILE=""              # si vide : génération automatique depuis DATA_ROOT
RULES_FILE=""               # si vide : auto-découverte de bdtopo-garmin-rules.yaml
MKGMAP_JAR=""               # si vide : auto-découverte
MKGMAP_STYLE=""             # fichier TYP/style (optionnel)
MKGMAP_DEM=""               # répertoire MNT pour relief ombré (optionnel)
MKGMAP_FAMILY_ID=1
MKGMAP_FAMILY_NAME="BDTOPO Garmin"
MKGMAP_SERIES_NAME="France BDTOPO"
MKGMAP_LEVELS="0:24,1:22,2:21,3:19,4:17,5:15,6:13,7:11"
SKIP_EXISTING=false
VERBOSE_COUNT=0             # 0=warn, 1=-v, 2=-vv

# Binaire mpforge-cli résolu
_MPFORGE_CLI=""

# Métriques collectées depuis le rapport JSON
BUILD_START_TIME=0
TILES_TOTAL=0
TILES_SUCCESS=0
TILES_FAILED=0
MPFORGE_DURATION=0
FEATURES_PROCESSED=0

# Fichier config temporaire généré automatiquement
_TMP_CONFIG=""

# État pipeline : tuiles en échec malgré exit 0 de mpforge-cli (error_handling=continue)
PARTIAL_FAILURE=false

# ---------------------------------------------------------------------------
# Nettoyage — supprime le config temporaire si interruption (SIGINT/SIGTERM/EXIT)
# ---------------------------------------------------------------------------
cleanup_trap() {
    [[ -n "$_TMP_CONFIG" && -f "$_TMP_CONFIG" ]] && rm -f "$_TMP_CONFIG" || true
}
trap cleanup_trap INT TERM EXIT

# ---------------------------------------------------------------------------
# Couleurs (Task 1.1)
# ---------------------------------------------------------------------------
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[0;33m'
BLUE='\033[0;34m'; CYAN='\033[0;36m'; BOLD='\033[1m'; NC='\033[0m'

log_info()  { echo -e "${BLUE}[INFO]${NC}  $*"; }
log_ok()    { echo -e "${GREEN}[OK]${NC}    $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }
log_step()  { echo -e "\n${BOLD}${CYAN}═══ $* ═══${NC}\n"; }

# ---------------------------------------------------------------------------
# Aide (Task 1.1)
# ---------------------------------------------------------------------------
show_help() {
    cat << 'EOF'
build-garmin-map.sh — Pipeline mpforge-cli → mkgmap → gmapsupp.img

USAGE :
    ./scripts/build-garmin-map.sh [OPTIONS]

OPTIONS :
    --config FILE       Config YAML mpforge-cli (défaut: génération auto depuis --data-root)
    --rules FILE        Fichier de règles YAML (défaut: auto-découverte bdtopo-garmin-rules.yaml)
    --jobs N            Parallélisation mpforge-cli (défaut: 8)
    --output DIR        Répertoire de sortie tiles/ + gmapsupp.img (défaut: ./output)
    --mkgmap-jar FILE   Chemin vers mkgmap.jar (défaut: auto-découverte)
    --mkgmap-levels L       Niveaux de zoom mkgmap (défaut: 0:24,1:22,2:21,3:19,4:17,5:15,6:13,7:11)
    --mkgmap-style FILE     Fichier TYP/style mkgmap (optionnel)
    --mkgmap-dem DIR        Répertoire MNT pour relief ombré mkgmap (optionnel)
    --mkgmap-family-id N    Family ID Garmin (défaut: 1)
    --mkgmap-family-name N  Family name Garmin (défaut: "BDTOPO Garmin")
    --mkgmap-series-name N  Series name Garmin (défaut: "France BDTOPO")
    --data-root DIR     Racine des données BDTOPO (défaut: ./data/bdtopo)
    --skip-existing     Passer les tuiles déjà présentes (idempotence)
    --dry-run           Simuler sans exécuter les commandes
    -v, --verbose       Mode verbeux (-vv pour très verbeux)
    --version           Version du script
    -h, --help          Aide

EXEMPLES :
    ./scripts/build-garmin-map.sh                                     # Auto-découverte de tout
    ./scripts/build-garmin-map.sh --data-root data/bdtopo/2025/v2025.12/D038
    ./scripts/build-garmin-map.sh --config configs/france-bdtopo.yaml --jobs 16
    ./scripts/build-garmin-map.sh --dry-run                          # Simuler le pipeline
    ./scripts/build-garmin-map.sh --skip-existing --jobs 4           # Reprise partielle

PRÉREQUIS :
    mpforge-cli (ou cargo build --release dans mpforge-cli/)
    java (apt install default-jre-headless)
    mkgmap.jar  (https://www.mkgmap.org.uk/download/mkgmap.html)

STRUCTURE DE SORTIE :
    ./output/
    ├── tiles/              ← tuiles .mp générées par mpforge-cli
    ├── gmapsupp.img        ← carte Garmin finale générée par mkgmap
    └── mpforge-report.json ← rapport d'exécution mpforge-cli
EOF
    exit 0
}

# ---------------------------------------------------------------------------
# Parse args (Task 1.3)
# ---------------------------------------------------------------------------
parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --config)        CONFIG_FILE="$2"; shift 2 ;;
            --rules)         RULES_FILE="$2"; shift 2 ;;
            --jobs)          JOBS="$2"; shift 2 ;;
            --output)        OUTPUT_DIR="$2"; shift 2 ;;
            --mkgmap-jar)    MKGMAP_JAR="$2"; shift 2 ;;
            --mkgmap-levels)      MKGMAP_LEVELS="$2"; shift 2 ;;
            --mkgmap-style)       MKGMAP_STYLE="$2"; shift 2 ;;
            --mkgmap-dem)         MKGMAP_DEM="$2"; shift 2 ;;
            --mkgmap-family-id)   MKGMAP_FAMILY_ID="$2"; shift 2 ;;
            --mkgmap-family-name) MKGMAP_FAMILY_NAME="$2"; shift 2 ;;
            --mkgmap-series-name) MKGMAP_SERIES_NAME="$2"; shift 2 ;;
            --data-root)          DATA_ROOT="$2"; shift 2 ;;
            --skip-existing)      SKIP_EXISTING=true; shift ;;
            --dry-run)            DRY_RUN=true; shift ;;
            -v|--verbose)         VERBOSE_COUNT=$(( VERBOSE_COUNT + 1 > 2 ? 2 : VERBOSE_COUNT + 1 )); shift ;;
            -vv)                  VERBOSE_COUNT=2; shift ;;
            --version)       echo "build-garmin-map.sh v${SCRIPT_VERSION}"; exit 0 ;;
            -h|--help)       show_help ;;
            *)               log_error "Option inconnue : $1"; exit 1 ;;
        esac
    done

    REPORT_FILE="${OUTPUT_DIR}/mpforge-report.json"
}

# ---------------------------------------------------------------------------
# Auto-découverte binaire mpforge-cli (Task 1.4)
# ---------------------------------------------------------------------------
find_mpforge_cli() {
    local candidates=(
        "./mpforge-cli/target/release/mpforge-cli"
        "../mpforge-cli/target/release/mpforge-cli"
    )
    for c in "${candidates[@]}"; do
        if [[ -x "$c" ]]; then
            echo "$c"
            return 0
        fi
    done
    if command -v mpforge-cli &>/dev/null; then
        command -v mpforge-cli
        return 0
    fi
    echo ""
}

# ---------------------------------------------------------------------------
# Auto-découverte mkgmap.jar (Task 1.4)
# Pattern inspiré de ogr-polishmap/test/test_mkgmap_compilation.sh
# ---------------------------------------------------------------------------
find_mkgmap_jar() {
    local candidates=(
        "${HOME}/mkgmap/mkgmap.jar"
        "/opt/mkgmap/mkgmap.jar"
        "./mkgmap/mkgmap.jar"
    )
    for c in "${candidates[@]}"; do
        if [[ -f "$c" ]]; then
            echo "$c"
            return 0
        fi
    done
    # Chercher dans les sous-dossiers proches (max 3 niveaux)
    find . -maxdepth 3 -name "mkgmap.jar" -type f 2>/dev/null | head -1 || true
}

# ---------------------------------------------------------------------------
# Auto-découverte fichier de règles (Task 1.4)
# ---------------------------------------------------------------------------
find_rules_file() {
    local candidates=(
        "./mpforge-cli/rules/bdtopo-garmin-rules.yaml"
        "../mpforge-cli/rules/bdtopo-garmin-rules.yaml"
        "./rules/bdtopo-garmin-rules.yaml"
        "./bdtopo-garmin-rules.yaml"
    )
    for c in "${candidates[@]}"; do
        if [[ -f "$c" ]]; then
            echo "$c"
            return 0
        fi
    done
    echo ""
}

# ---------------------------------------------------------------------------
# Vérification prérequis (Task 1.4)
# ---------------------------------------------------------------------------
check_prerequisites() {
    log_step "Vérification des prérequis"

    # --- mpforge-cli ---
    if [[ -z "$_MPFORGE_CLI" ]]; then
        _MPFORGE_CLI=$(find_mpforge_cli)
    fi

    if [[ -z "$_MPFORGE_CLI" ]]; then
        if command -v cargo &>/dev/null; then
            log_warn "mpforge-cli non trouvé comme binaire — fallback 'cargo run --release'"
            _MPFORGE_CLI="__CARGO_RUN__"
        else
            log_error "mpforge-cli introuvable (ni binaire ni cargo)"
            log_error "  → Compilez avec : cd mpforge-cli && cargo build --release"
            exit 1
        fi
    else
        log_ok "mpforge-cli : $_MPFORGE_CLI"
    fi

    # --- java ---
    command -v java &>/dev/null || {
        log_error "java requis pour mkgmap (apt install default-jre-headless)"
        exit 1
    }
    local java_ver
    java_ver=$(java -version 2>&1 | head -1)
    log_ok "java : $java_ver"

    # --- mkgmap.jar ---
    if [[ -z "$MKGMAP_JAR" ]]; then
        MKGMAP_JAR=$(find_mkgmap_jar)
    fi

    if [[ -z "$MKGMAP_JAR" || ! -f "$MKGMAP_JAR" ]]; then
        log_error "mkgmap.jar introuvable"
        log_error "  → Téléchargez : https://www.mkgmap.org.uk/download/mkgmap.html"
        log_error "  → Placez dans  : \$HOME/mkgmap/mkgmap.jar ou /opt/mkgmap/mkgmap.jar"
        log_error "  → Ou utilisez  : --mkgmap-jar /chemin/vers/mkgmap.jar"
        exit 1
    fi
    log_ok "mkgmap.jar : $MKGMAP_JAR"

    # --- Validation fichier config (si fourni explicitement) ---
    if [[ -n "$CONFIG_FILE" && ! -f "$CONFIG_FILE" ]]; then
        log_error "Config mpforge-cli introuvable : $CONFIG_FILE"
        exit 1
    fi

    # --- Validation fichier rules (si fourni explicitement) ---
    if [[ -n "$RULES_FILE" && ! -f "$RULES_FILE" ]]; then
        log_error "Fichier de règles introuvable : $RULES_FILE"
        exit 1
    fi
}

# ---------------------------------------------------------------------------
# Extraction valeur entière depuis rapport JSON (bash natif, sans jq)
# ---------------------------------------------------------------------------
json_extract_int() {
    local json_file="$1" key="$2" default="${3:-0}"
    local val
    val=$(grep -o "\"${key}\":[[:space:]]*[0-9]*" "$json_file" 2>/dev/null \
          | grep -o '[0-9]*$' | head -1) || true
    echo "${val:-$default}"
}

# ---------------------------------------------------------------------------
# Affichage des erreurs depuis le rapport JSON mpforge-cli (AC2)
# ---------------------------------------------------------------------------
show_report_errors() {
    local report="$1"
    [[ -f "$report" ]] || return 0

    log_error "── Erreurs du rapport JSON ──────────────────────────────"
    # Extraire les messages d'erreur (format : "message":"...")
    grep -o '"message":"[^"]*"' "$report" 2>/dev/null \
        | sed 's/"message":"//;s/"$//' \
        | while IFS= read -r msg; do
            [[ -n "$msg" ]] && log_error "  • $msg"
        done || true

    local failed
    failed=$(json_extract_int "$report" "tiles_failed" 0)
    [[ "$failed" -gt 0 ]] && log_error "  $failed tuile(s) en échec"
    log_error "─────────────────────────────────────────────────────────"
}

# ---------------------------------------------------------------------------
# Génération dynamique du config YAML mpforge-cli depuis DATA_ROOT
# Option 1 recommandée (Dev Notes) : découverte auto des .shp (21 couches BDTOPO)
# Les règles sont injectées dans le YAML (pas de --rules CLI dans mpforge-cli)
# ---------------------------------------------------------------------------
generate_config() {
    local data_dir="$1"
    local out_dir="$2"
    local rules_path="$3"
    local tmp_config
    tmp_config=$(mktemp /tmp/mpforge-config-XXXXXX.yaml)
    _TMP_CONFIG="$tmp_config"

    # 21 couches BDTOPO dans l'ordre FME
    local -a LAYERS=(
        "TRANSPORT/TRONCON_DE_ROUTE"
        "TRANSPORT/TRONCON_DE_VOIE_FERREE"
        "TRANSPORT/PISTE_D_AERODROME"
        "TRANSPORT/TRANSPORT_PAR_CABLE"
        "ADMINISTRATIF/COMMUNE"
        "LIEUX_NOMMES/ZONE_D_HABITATION"
        "HYDROGRAPHIE/TRONCON_HYDROGRAPHIQUE"
        "HYDROGRAPHIE/SURFACE_HYDROGRAPHIQUE"
        "HYDROGRAPHIE/DETAIL_HYDROGRAPHIQUE"
        "BATI/BATIMENT"
        "BATI/CIMETIERE"
        "BATI/CONSTRUCTION_LINEAIRE"
        "BATI/CONSTRUCTION_PONCTUELLE"
        "BATI/PYLONE"
        "BATI/TERRAIN_DE_SPORT"
        "BATI/LIGNE_OROGRAPHIQUE"
        "OCCUPATION_DU_SOL/ZONE_DE_VEGETATION"
        "SERVICES_ET_ACTIVITES/ZONE_D_ACTIVITE_OU_D_INTERET"
        "SERVICES_ET_ACTIVITES/LIGNE_ELECTRIQUE"
        "ZONES_REGLEMENTEES/FORET_PUBLIQUE"
        "LIEUX_NOMMES/TOPONYMIE"
    )

    {
        echo "# Config générée automatiquement par build-garmin-map.sh v${SCRIPT_VERSION}"
        echo "# Source des données : $data_dir"
        echo "# Généré le : $(date '+%Y-%m-%d %H:%M:%S')"
        echo "version: 1"
        echo ""
        echo "grid:"
        echo "  cell_size: 0.15       # ~16.5 km par tuile"
        echo "  overlap: 0.005        # Léger chevauchement"
        echo ""
        echo "inputs:"

        for layer in "${LAYERS[@]}"; do
            local layer_name
            layer_name=$(basename "$layer")
            while IFS= read -r shp; do
                echo "  - path: \"${shp}\""
                echo "    source_srs: \"EPSG:2154\""
                echo "    target_srs: \"EPSG:4326\""
            done < <(find "$data_dir" -name "${layer_name}.shp" -type f 2>/dev/null | sort)
        done

        echo ""
        echo "output:"
        echo "  directory: \"${out_dir}/tiles/\""
        echo "  filename_pattern: \"tile_{col}_{row}.mp\""
        echo "  overwrite: true"
        echo ""
        echo "rules: \"${rules_path}\""
        echo ""
        echo "error_handling: \"continue\""
    } > "$tmp_config"

    echo "$tmp_config"
}

# ---------------------------------------------------------------------------
# Préparation de la configuration mpforge-cli (résolution règles + config)
# ---------------------------------------------------------------------------
prepare_config() {
    log_step "Préparation de la configuration"

    # Config : fournie explicitement ou générée dynamiquement
    if [[ -n "$CONFIG_FILE" ]]; then
        # M1 : détecter les placeholders envsubst (${DATA_ROOT}, ${OUTPUT_DIR}…)
        if grep -qE '\$\{[A-Z_]+\}' "$CONFIG_FILE" 2>/dev/null; then
            if ! command -v envsubst &>/dev/null; then
                log_error "Config '$CONFIG_FILE' contient des placeholders \${...} mais 'envsubst' est introuvable"
                log_error "  → Installez     : apt install gettext-base"
                log_error "  → Ou substituez : envsubst < $CONFIG_FILE > /tmp/config.yaml"
                log_error "                    puis relancez avec --config /tmp/config.yaml"
                exit 1
            fi
            local tmp_expanded
            tmp_expanded=$(mktemp /tmp/mpforge-config-expanded-XXXXXX.yaml)
            _TMP_CONFIG="$tmp_expanded"
            envsubst < "$CONFIG_FILE" > "$tmp_expanded"
            if grep -qE '\$\{[A-Z_]+\}' "$tmp_expanded" 2>/dev/null; then
                log_warn "Des placeholders non résolus subsistent — vérifiez vos variables d'environnement (DATA_ROOT, OUTPUT_DIR…)"
            fi
            CONFIG_FILE="$tmp_expanded"
            log_info "Config    : $CONFIG_FILE (placeholders substitués via envsubst)"
        else
            log_info "Config    : $CONFIG_FILE (fournie explicitement)"
        fi
    else
        # M2 : résolution des règles uniquement pour la génération dynamique de config
        if [[ -z "$RULES_FILE" ]]; then
            RULES_FILE=$(find_rules_file)
            if [[ -z "$RULES_FILE" ]]; then
                log_error "Fichier de règles bdtopo-garmin-rules.yaml introuvable"
                log_error "  → Utilisez : --rules /chemin/vers/bdtopo-garmin-rules.yaml"
                exit 1
            fi
            log_info "Règles    : $RULES_FILE (auto-découverte)"
        else
            log_info "Règles    : $RULES_FILE"
        fi

        log_info "Config    : génération automatique depuis $DATA_ROOT"

        if [[ ! -d "$DATA_ROOT" ]]; then
            log_error "DATA_ROOT introuvable : $DATA_ROOT"
            log_error "  → Téléchargez d'abord avec : ./scripts/download-bdtopo.sh"
            log_error "  → Ou spécifiez : --data-root /chemin/vers/bdtopo"
            exit 1
        fi

        local shp_count
        shp_count=$(find "$DATA_ROOT" -name "*.shp" -type f 2>/dev/null | wc -l)
        if [[ "$shp_count" -eq 0 ]]; then
            log_error "Aucun fichier .shp trouvé dans : $DATA_ROOT"
            log_error "  → Vérifiez que les archives BDTOPO sont extraites"
            exit 1
        fi
        log_info "  → $shp_count fichier(s) .shp disponible(s)"

        # L3 : mkdir déplacé dans run_mpforge_cli() — valide aussi pour --config explicite
        CONFIG_FILE=$(generate_config "$DATA_ROOT" "$OUTPUT_DIR" "$RULES_FILE")

        local layers_in_config
        layers_in_config=$(grep -c "^  - path:" "$CONFIG_FILE" 2>/dev/null || echo 0)
        log_ok "Config générée : $CONFIG_FILE ($layers_in_config couche(s) BDTOPO)"

        if [[ "$layers_in_config" -eq 0 ]]; then
            log_error "Aucune couche BDTOPO reconnue dans : $DATA_ROOT"
            log_error "  → Vérifiez la structure : $DATA_ROOT/TRANSPORT/TRONCON_DE_ROUTE.shp"
            exit 1
        fi
    fi

    log_info "Jobs      : $JOBS"
    log_info "Sortie    : $OUTPUT_DIR"
}

# ---------------------------------------------------------------------------
# Étape 1/2 — Lancement mpforge-cli build (AC1, AC2 — Task 1.5)
# ---------------------------------------------------------------------------
run_mpforge_cli() {
    log_step "Étape 1/2 — mpforge-cli build"

    mkdir -p "$OUTPUT_DIR"

    # Construction de la commande
    local -a cmd=()

    if [[ "$_MPFORGE_CLI" == "__CARGO_RUN__" ]]; then
        # Fallback cargo run (mode dev)
        local mpforge_dir
        mpforge_dir=$(find . -maxdepth 2 -name "Cargo.toml" \
                      -exec grep -l 'name.*=.*"mpforge-cli"' {} \; 2>/dev/null \
                      | head -1 | xargs dirname 2>/dev/null || echo "")

        if [[ -z "$mpforge_dir" ]]; then
            # Essai d'un répertoire standard
            mpforge_dir="./mpforge-cli"
        fi
        cmd=(env PROJ_DATA=/usr/share/proj
             cargo run --manifest-path "${mpforge_dir}/Cargo.toml" --release --)
    else
        cmd=("$_MPFORGE_CLI")
    fi

    cmd+=(build
          --config "${CONFIG_FILE}"
          --report "${REPORT_FILE}"
          --jobs   "${JOBS}")

    [[ "$SKIP_EXISTING"  == true ]] && cmd+=(--skip-existing)
    [[ "$VERBOSE_COUNT"  -ge 1   ]] && cmd+=(-v)
    [[ "$VERBOSE_COUNT"  -ge 2   ]] && cmd+=(-v)

    log_info "Commande : ${cmd[*]}"

    if [[ "$DRY_RUN" == true ]]; then
        echo -e "  ${YELLOW}[DRY-RUN]${NC} ${cmd[*]}"
        log_ok "Dry-run : commande mpforge-cli affichée (non exécutée)"
        return 0
    fi

    local exit_code=0
    "${cmd[@]}" || exit_code=$?

    # AC2 : arrêt immédiat sur échec mpforge-cli
    if [[ "$exit_code" -ne 0 ]]; then
        log_error "mpforge-cli a échoué (exit code : $exit_code)"
        show_report_errors "$REPORT_FILE"
        log_error "Pipeline arrêté — mkgmap NON lancé"
        exit "$exit_code"
    fi

    log_ok "mpforge-cli terminé avec succès"

    # Lecture des métriques depuis le rapport JSON (pour AC3)
    if [[ -f "$REPORT_FILE" ]]; then
        TILES_TOTAL=$(json_extract_int "$REPORT_FILE" "tiles_total" 0)
        TILES_SUCCESS=$(json_extract_int "$REPORT_FILE" "tiles_success" 0)
        TILES_FAILED=$(json_extract_int "$REPORT_FILE" "tiles_failed" 0)
        MPFORGE_DURATION=$(json_extract_int "$REPORT_FILE" "duration_seconds" 0)
        FEATURES_PROCESSED=$(json_extract_int "$REPORT_FILE" "features_processed" 0)
        log_info "  Tuiles   : ${TILES_SUCCESS}/${TILES_TOTAL} (${TILES_FAILED} échec(s))"
        [[ "$FEATURES_PROCESSED" -gt 0 ]] && log_info "  Features : ${FEATURES_PROCESSED}"
        # H2 : tuiles en échec avec error_handling=continue → carte incomplète sans exit non-zéro
        if [[ "$TILES_FAILED" -gt 0 ]]; then
            log_warn "${TILES_FAILED} tuile(s) en échec — le gmapsupp.img sera incomplet"
            PARTIAL_FAILURE=true
        fi
    fi
}

# ---------------------------------------------------------------------------
# Étape 2/2 — Lancement mkgmap (AC1 — Task 1.6)
# ---------------------------------------------------------------------------
run_mkgmap() {
    log_step "Étape 2/2 — mkgmap compilation"

    local tiles_dir="${OUTPUT_DIR}/tiles"

    # Construction de la commande mkgmap
    local -a cmd=(java -jar "${MKGMAP_JAR}"
        "--family-id=${MKGMAP_FAMILY_ID}"
        "--product-id=1"
        "--family-name=${MKGMAP_FAMILY_NAME}"
        "--series-name=${MKGMAP_SERIES_NAME}"
        "--levels=${MKGMAP_LEVELS}"
        "--code-page=1252"
        "--latin1"
        "--gmapsupp"
        "--output-dir=${OUTPUT_DIR}")

    [[ -n "$MKGMAP_STYLE" ]] && cmd+=("--style-file=${MKGMAP_STYLE}")
    [[ -n "$MKGMAP_DEM"   ]] && cmd+=("--dem=${MKGMAP_DEM}")

    if [[ "$DRY_RUN" == true ]]; then
        echo -e "  ${YELLOW}[DRY-RUN]${NC} java -jar ${MKGMAP_JAR} \\"
        echo -e "               --family-id=${MKGMAP_FAMILY_ID} --gmapsupp \\"
        echo -e "               --output-dir=${OUTPUT_DIR} \\"
        echo -e "               ${tiles_dir}/*.mp"
        log_ok "Dry-run : commande mkgmap affichée (non exécutée)"
        return 0
    fi

    # Vérifier la présence de tuiles .mp avant de lancer mkgmap
    local mp_count
    mp_count=$(find "$tiles_dir" -name "*.mp" -type f 2>/dev/null | wc -l)
    if [[ "$mp_count" -eq 0 ]]; then
        log_error "Aucune tuile .mp trouvée dans : $tiles_dir"
        exit 1
    fi
    log_info "  $mp_count tuile(s) .mp à compiler"

    # Ajouter les .mp à la commande (expansion glob sécurisée)
    while IFS= read -r mp_file; do
        cmd+=("$mp_file")
    done < <(find "$tiles_dir" -name "*.mp" -type f 2>/dev/null | sort)

    local exit_code=0
    "${cmd[@]}" || exit_code=$?

    if [[ "$exit_code" -ne 0 ]]; then
        log_error "mkgmap a échoué (exit code : $exit_code)"
        exit "$exit_code"
    fi

    # Vérification que gmapsupp.img a bien été produit (AC1)
    if [[ ! -f "${OUTPUT_DIR}/gmapsupp.img" ]]; then
        log_error "gmapsupp.img non produit dans : $OUTPUT_DIR"
        exit 1
    fi

    log_ok "gmapsupp.img produit : ${OUTPUT_DIR}/gmapsupp.img"
}

# ---------------------------------------------------------------------------
# Résumé final (AC3 — Task 1.7)
# ---------------------------------------------------------------------------
show_summary() {
    log_step "Résumé"

    local total_duration=$(( SECONDS - BUILD_START_TIME ))

    if [[ "$DRY_RUN" == false ]]; then
        echo -e "  ${BOLD}Tuiles générées :${NC}  ${TILES_SUCCESS}/${TILES_TOTAL}"
        if [[ "$TILES_FAILED" -gt 0 ]]; then
            echo -e "  ${YELLOW}${BOLD}Tuiles en échec :${NC}  ${TILES_FAILED}"
        fi
        [[ "$FEATURES_PROCESSED" -gt 0 ]] && \
            echo -e "  ${BOLD}Features        :${NC}  ${FEATURES_PROCESSED}"
        if [[ "$MPFORGE_DURATION" -gt 0 ]]; then
            local m=$(( MPFORGE_DURATION / 60 )) s=$(( MPFORGE_DURATION % 60 ))
            echo -e "  ${BOLD}mpforge-cli     :${NC}  ${m}m${s}s"
        fi
    fi

    local total_m=$(( total_duration / 60 )) total_s=$(( total_duration % 60 ))
    echo -e "  ${BOLD}Temps total     :${NC}  ${total_m}m${total_s}s"

    if [[ -f "${OUTPUT_DIR}/gmapsupp.img" ]]; then
        local size_bytes
        size_bytes=$(stat -c%s "${OUTPUT_DIR}/gmapsupp.img" 2>/dev/null || echo 0)
        local size_hr
        size_hr=$(numfmt --to=iec-i --suffix=B "$size_bytes" 2>/dev/null \
                  || echo "${size_bytes} octets")
        echo -e "  ${BOLD}Taille img      :${NC}  ${size_hr}"
        echo -e "  ${BOLD}Emplacement     :${NC}  ${OUTPUT_DIR}/gmapsupp.img"
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
    echo "  │  build-garmin-map.sh — Pipeline mpforge-cli → mkgmap           │"
    echo "  │  BDTOPO → tuiles .mp → gmapsupp.img · v${SCRIPT_VERSION}               │"
    echo "  └─────────────────────────────────────────────────────────────────┘"
    echo -e "${NC}"

    parse_args "$@"
    BUILD_START_TIME=$SECONDS

    check_prerequisites
    prepare_config
    run_mpforge_cli
    run_mkgmap
    show_summary

    if [[ "$PARTIAL_FAILURE" == true ]]; then
        log_warn "Pipeline terminé avec avertissements — carte partielle dans : ${OUTPUT_DIR}/"
        exit 2
    fi
    log_ok "Pipeline terminé — carte disponible dans : ${OUTPUT_DIR}/"
}

main "$@"
