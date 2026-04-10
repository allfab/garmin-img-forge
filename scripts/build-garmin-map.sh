#!/usr/bin/env bash
# =============================================================================
# build-garmin-map.sh — Pipeline mpforge → imgforge → gmapsupp.img
# =============================================================================
#
# Enchaîne mpforge build et imgforge build pour produire une carte Garmin :
#
#   1. Prépare la config YAML (fournie ou générée depuis DATA_ROOT)
#   2. Lance mpforge build (génère les tuiles .mp)
#   3. Vérifie le code de sortie et le rapport JSON
#   4. Lance imgforge build (compile .mp → gmapsupp.img)
#   5. Affiche le résumé final (tuiles, temps, taille)
#
# Pipeline : download-bdtopo.sh → build-garmin-map.sh → gmapsupp.img
#
# Prérequis : mpforge (ou cd tools/mpforge && cargo build --release), imgforge (idem tools/imgforge)
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration par défaut
# ---------------------------------------------------------------------------
SCRIPT_VERSION="2.0.0"
DATA_ROOT="./pipeline/data/bdtopo"
CONTOURS_DATA_ROOT="./pipeline/data/courbes"
RANDO_DATA_ROOT="./pipeline/data/randonnee"
OUTPUT_DIR="./pipeline/output"
REPORT_FILE=""              # calculé après parse_args (dépend de OUTPUT_DIR)
IMGFORGE_REPORT_FILE=""     # calculé après parse_args
JOBS=8
DRY_RUN=false
CONFIG_FILE=""              # si vide : génération automatique depuis DATA_ROOT
RULES_FILE="${RULES_FILE:-}" # si vide : auto-découverte de bdtopo-garmin-rules.yaml
FAMILY_ID=6324              # remplace MKGMAP_FAMILY_ID
DESCRIPTION="BDTOPO Garmin" # remplace MKGMAP_FAMILY_NAME
TYP_FILE=""                 # fichier TYP styles (optionnel)
SKIP_EXISTING=false
VERBOSE_COUNT=0             # 0=warn, 1=-v, 2=-vv

# Binaires résolus
_MPFORGE=""
_IMGFORGE=""            # binaire imgforge résolu

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
IMGFORGE_ROUTING_NODES=0
IMGFORGE_ROUTING_ARCS=0

# Fichier config temporaire généré automatiquement
_TMP_CONFIG=""

# État pipeline : tuiles en échec malgré exit 0 de mpforge (error_handling=continue)
PARTIAL_FAILURE=false

# ---------------------------------------------------------------------------
# Nettoyage — supprime le config temporaire si interruption (SIGINT/SIGTERM/EXIT)
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
log_step()  { echo -e "\n${BOLD}${CYAN}═══ $* ═══${NC}\n"; }

# ---------------------------------------------------------------------------
# Aide
# ---------------------------------------------------------------------------
show_help() {
    cat << 'EOF'
build-garmin-map.sh — Pipeline mpforge → imgforge → gmapsupp.img

USAGE :
    ./scripts/build-garmin-map.sh [OPTIONS]

OPTIONS :
    --config FILE           Config YAML mpforge (défaut: génération auto depuis --data-root)
    --rules FILE            Fichier de règles YAML (défaut: auto-découverte bdtopo-garmin-rules.yaml)
    --jobs N                Parallélisation (défaut: 8)
    --output DIR            Répertoire de sortie tiles/ + gmapsupp.img (défaut: ./pipeline/output)
    --imgforge FILE     Chemin binaire imgforge (défaut: auto-découverte)
    --family-id N           Family ID Garmin (défaut: 6324)
    --description STR       Description de la carte (défaut: "BDTOPO Garmin")
    --typ FILE              Fichier TYP styles personnalisés (optionnel)
    --data-root DIR         Racine des données BDTOPO (défaut: ./pipeline/data/bdtopo)
    --contours-root DIR     Racine des courbes de niveau (défaut: ./pipeline/data/courbes)
    --skip-existing         Passer les tuiles déjà présentes (idempotence)
    --dry-run               Simuler sans exécuter les commandes
    -v, --verbose           Mode verbeux (-vv pour très verbeux)
    --version               Version du script
    -h, --help              Aide

EXEMPLES :
    ./scripts/build-garmin-map.sh                                     # Auto-découverte de tout
    ./scripts/build-garmin-map.sh --data-root pipeline/data/bdtopo/2025/v2025.12/D038
    ./scripts/build-garmin-map.sh --config pipeline/configs/france-bdtopo.yaml --jobs 16
    ./scripts/build-garmin-map.sh --dry-run                          # Simuler le pipeline
    ./scripts/build-garmin-map.sh --skip-existing --jobs 4           # Reprise partielle

PRÉREQUIS :
    mpforge   (ou cargo build --release dans tools/mpforge/)
    imgforge  (ou cargo build --release dans tools/imgforge/)

STRUCTURE DE SORTIE :
    ./pipeline/output/
    ├── tiles/              ← tuiles .mp générées par mpforge
    ├── gmapsupp.img        ← carte Garmin finale générée par imgforge
    ├── mpforge-report.json ← rapport mpforge
    └── imgforge-report.json ← rapport imgforge
EOF
    exit 0
}

# ---------------------------------------------------------------------------
# Parse args
# ---------------------------------------------------------------------------
parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --config)        CONFIG_FILE="$2"; shift 2 ;;
            --rules)         RULES_FILE="$2"; shift 2 ;;
            --jobs)          JOBS="$2"; shift 2 ;;
            --output)        OUTPUT_DIR="$2"; shift 2 ;;
            --imgforge)  _IMGFORGE="$2"; shift 2 ;;
            --family-id)     FAMILY_ID="$2"; shift 2 ;;
            --description)   DESCRIPTION="$2"; shift 2 ;;
            --typ)           TYP_FILE="$2"; shift 2 ;;
            --data-root)     DATA_ROOT="$2"; shift 2 ;;
            --contours-root) CONTOURS_DATA_ROOT="$2"; shift 2 ;;
            --skip-existing) SKIP_EXISTING=true; shift ;;
            --dry-run)       DRY_RUN=true; shift ;;
            -v|--verbose)    VERBOSE_COUNT=$(( VERBOSE_COUNT + 1 > 2 ? 2 : VERBOSE_COUNT + 1 )); shift ;;
            -vv)             VERBOSE_COUNT=2; shift ;;
            --version)       echo "build-garmin-map.sh v${SCRIPT_VERSION}"; exit 0 ;;
            -h|--help)       show_help ;;
            *)               log_error "Option inconnue : $1"; exit 1 ;;
        esac
    done

    REPORT_FILE="${OUTPUT_DIR}/mpforge-report.json"
    IMGFORGE_REPORT_FILE="${OUTPUT_DIR}/imgforge-report.json"
}

# ---------------------------------------------------------------------------
# Auto-découverte binaire mpforge
# ---------------------------------------------------------------------------
find_mpforge() {
    local candidates=(
        "./tools/mpforge/target/release/mpforge"
        "../tools/mpforge/target/release/mpforge"
    )
    for c in "${candidates[@]}"; do
        if [[ -x "$c" ]]; then
            echo "$c"
            return 0
        fi
    done
    if command -v mpforge &>/dev/null; then
        command -v mpforge
        return 0
    fi
    echo ""
}

# ---------------------------------------------------------------------------
# Auto-découverte binaire imgforge
# ---------------------------------------------------------------------------
find_imgforge() {
    local candidates=(
        "./tools/imgforge/target/release/imgforge"
        "../tools/imgforge/target/release/imgforge"
    )
    for c in "${candidates[@]}"; do
        if [[ -x "$c" ]]; then
            echo "$c"
            return 0
        fi
    done
    if command -v imgforge &>/dev/null; then
        command -v imgforge
        return 0
    fi
    echo ""
}

# ---------------------------------------------------------------------------
# Auto-découverte fichier de règles
# ---------------------------------------------------------------------------
find_rules_file() {
    local candidates=(
        "./tools/mpforge/rules/bdtopo-garmin-rules.yaml"
        "../tools/mpforge/rules/bdtopo-garmin-rules.yaml"
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
# Vérification prérequis
# ---------------------------------------------------------------------------
check_prerequisites() {
    log_step "Vérification des prérequis"

    # --- mpforge ---
    if [[ -z "$_MPFORGE" ]]; then
        _MPFORGE=$(find_mpforge)
    fi

    if [[ -z "$_MPFORGE" ]]; then
        if command -v cargo &>/dev/null; then
            log_warn "mpforge non trouvé comme binaire — fallback 'cargo run --release'"
            _MPFORGE="__CARGO_RUN__"
        else
            log_error "mpforge introuvable (ni binaire ni cargo)"
            log_error "  → Compilez avec : cd tools/mpforge && cargo build --release"
            exit 1
        fi
    else
        log_ok "mpforge : $_MPFORGE"
    fi

    # --- imgforge ---
    if [[ -z "$_IMGFORGE" ]]; then
        _IMGFORGE=$(find_imgforge)
    fi

    if [[ -z "$_IMGFORGE" ]]; then
        if command -v cargo &>/dev/null; then
            log_warn "imgforge non trouvé comme binaire — fallback 'cargo run --release'"
            _IMGFORGE="__CARGO_RUN_IMGFORGE__"
        else
            log_error "imgforge introuvable"
            log_error "  → Compilez avec : cd tools/imgforge && cargo build --release"
            exit 1
        fi
    else
        log_ok "imgforge : $_IMGFORGE"
    fi

    # --- Validation fichier config (si fourni explicitement) ---
    if [[ -n "$CONFIG_FILE" && ! -f "$CONFIG_FILE" ]]; then
        log_error "Config mpforge introuvable : $CONFIG_FILE"
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
# Affichage des erreurs depuis le rapport JSON (AC2)
# ---------------------------------------------------------------------------
show_report_errors() {
    local report="$1"
    [[ -f "$report" ]] || return 0

    log_error "── Erreurs du rapport JSON ──────────────────────────────"
    # Extraire les messages d'erreur avec contexte tuile si disponible
    if grep -q '"tile":' "$report" 2>/dev/null; then
        # Format imgforge : errors[{tile, error}] — afficher avec contexte tuile
        grep -o '"tile":"[^"]*","error":"[^"]*"' "$report" 2>/dev/null \
            | sed 's/"tile":"//;s/","error":"/ : /;s/"$//' \
            | while IFS= read -r msg; do
                [[ -n "$msg" ]] && log_error "  • tuile $msg"
            done || true
    else
        # Format mpforge : "message":"..."
        grep -o '"message":"[^"]*"\|"error":"[^"]*"' "$report" 2>/dev/null \
            | sed 's/"message":"//;s/"error":"//;s/"$//' \
            | while IFS= read -r msg; do
                [[ -n "$msg" ]] && log_error "  • $msg"
            done || true
    fi

    local failed
    failed=$(json_extract_int "$report" "tiles_failed" 0)
    [[ "$failed" -gt 0 ]] && log_error "  $failed tuile(s) en échec"
    log_error "─────────────────────────────────────────────────────────"
}

# ---------------------------------------------------------------------------
# Génération dynamique du config YAML mpforge depuis DATA_ROOT
# Option 1 recommandée (Dev Notes) : découverte auto des .shp (21 couches BDTOPO)
# Les règles sont injectées dans le YAML (pas de --rules CLI dans mpforge)
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
# Préparation de la configuration mpforge (résolution règles + config)
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
            export DATA_ROOT CONTOURS_DATA_ROOT RANDO_DATA_ROOT OUTPUT_DIR RULES_FILE
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

        # L3 : mkdir déplacé dans run_mpforge() — valide aussi pour --config explicite
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
# Étape 1/2 — Lancement mpforge build
# ---------------------------------------------------------------------------
run_mpforge() {
    log_step "Étape 1/2 — mpforge build"

    mkdir -p "$OUTPUT_DIR"

    # Construction de la commande
    local -a cmd=()

    if [[ "$_MPFORGE" == "__CARGO_RUN__" ]]; then
        # Fallback cargo run (mode dev)
        local mpforge_dir
        mpforge_dir=$(find . -maxdepth 3 -name "Cargo.toml" \
                      -exec grep -l 'name.*=.*"mpforge"' {} \; 2>/dev/null \
                      | head -1 | xargs dirname 2>/dev/null || echo "")

        if [[ -z "$mpforge_dir" ]]; then
            # Essai d'un répertoire standard
            mpforge_dir="./tools/mpforge"
        fi
        cmd=(env PROJ_DATA=/usr/share/proj
             cargo run --manifest-path "${mpforge_dir}/Cargo.toml" --release --)
    else
        cmd=("$_MPFORGE")
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
        log_ok "Dry-run : commande mpforge affichée (non exécutée)"
        return 0
    fi

    local exit_code=0
    "${cmd[@]}" || exit_code=$?

    # Arrêt immédiat sur échec mpforge
    if [[ "$exit_code" -ne 0 ]]; then
        log_error "mpforge a échoué (exit code : $exit_code)"
        show_report_errors "$REPORT_FILE"
        log_error "Pipeline arrêté — imgforge NON lancé"
        exit "$exit_code"
    fi

    log_ok "mpforge terminé avec succès"

    # Lecture des métriques depuis le rapport JSON
    if [[ -f "$REPORT_FILE" ]]; then
        TILES_TOTAL=$(json_extract_int "$REPORT_FILE" "tiles_generated" 0)
        TILES_FAILED=$(json_extract_int "$REPORT_FILE" "tiles_failed" 0)
        TILES_SUCCESS=$(( TILES_TOTAL - TILES_FAILED ))
        MPFORGE_DURATION=$(json_extract_int "$REPORT_FILE" "duration_seconds" 0)
        FEATURES_PROCESSED=$(json_extract_int "$REPORT_FILE" "features_processed" 0)
        log_info "  Tuiles   : ${TILES_SUCCESS}/${TILES_TOTAL} (${TILES_FAILED} échec(s))"
        [[ "$FEATURES_PROCESSED" -gt 0 ]] && log_info "  Features : ${FEATURES_PROCESSED}"
        # Tuiles en échec avec error_handling=continue → carte incomplète sans exit non-zéro
        if [[ "$TILES_FAILED" -gt 0 ]]; then
            log_warn "${TILES_FAILED} tuile(s) en échec — le gmapsupp.img sera incomplet"
            PARTIAL_FAILURE=true
        fi
    fi
}

# ---------------------------------------------------------------------------
# Étape 2/2 — Lancement imgforge build
# ---------------------------------------------------------------------------
run_imgforge_build() {
    log_step "Étape 2/2 — imgforge build"

    local tiles_dir="${OUTPUT_DIR}/tiles"

    # Construction de la commande
    local -a cmd=()

    if [[ "$_IMGFORGE" == "__CARGO_RUN_IMGFORGE__" ]]; then
        # Fallback cargo run (mode dev)
        local imgforge_dir
        imgforge_dir=$(find . -maxdepth 3 -name "Cargo.toml" \
                       -exec grep -l 'name.*=.*"imgforge"' {} \; 2>/dev/null \
                       | head -1 | xargs dirname 2>/dev/null || echo "")
        if [[ -z "$imgforge_dir" ]]; then
            imgforge_dir="./tools/imgforge"
        fi
        cmd=(cargo run --manifest-path "${imgforge_dir}/Cargo.toml" --release --)
    else
        cmd=("$_IMGFORGE")
    fi

    cmd+=(build
          "${tiles_dir}"
          -o "${OUTPUT_DIR}/gmapsupp.img"
          --family-id "${FAMILY_ID}"
          --family-name "${DESCRIPTION}"
          -j "${JOBS}")

    [[ -n "$TYP_FILE" ]] && cmd+=(--typ "${TYP_FILE}")
    [[ "$VERBOSE_COUNT" -ge 1 ]] && cmd+=(-v)
    [[ "$VERBOSE_COUNT" -ge 2 ]] && cmd+=(-v)

    log_info "Commande : ${cmd[*]}"

    if [[ "$DRY_RUN" == true ]]; then
        echo -e "  ${YELLOW}[DRY-RUN]${NC} ${cmd[*]}"
        log_ok "Dry-run : commande imgforge affichée (non exécutée)"
        return 0
    fi

    # Vérifier la présence de tuiles .mp (après dry-run check)
    local mp_count
    mp_count=$(find "$tiles_dir" -name "*.mp" -type f 2>/dev/null | wc -l)
    if [[ "$mp_count" -eq 0 ]]; then
        log_error "Aucune tuile .mp trouvée dans : $tiles_dir"
        exit 1
    fi
    log_info "  $mp_count tuile(s) .mp à compiler"

    local exit_code=0
    "${cmd[@]}" || exit_code=$?

    if [[ "$exit_code" -ne 0 ]]; then
        log_error "imgforge a échoué (exit code : $exit_code)"
        show_report_errors "$IMGFORGE_REPORT_FILE"
        log_error "Pipeline arrêté"
        exit "$exit_code"
    fi

    [[ ! -f "${OUTPUT_DIR}/gmapsupp.img" ]] && {
        log_error "gmapsupp.img non produit dans : $OUTPUT_DIR"
        exit 1
    }

    log_ok "gmapsupp.img produit : ${OUTPUT_DIR}/gmapsupp.img"

    # Lecture métriques rapport imgforge
    if [[ -f "$IMGFORGE_REPORT_FILE" ]]; then
        IMGFORGE_TILES_COMPILED=$(json_extract_int "$IMGFORGE_REPORT_FILE" "tiles_compiled" 0)
        IMGFORGE_TILES_FAILED=$(json_extract_int "$IMGFORGE_REPORT_FILE" "tiles_failed" 0)
        IMGFORGE_DURATION=$(json_extract_int "$IMGFORGE_REPORT_FILE" "duration_seconds" 0)
        IMGFORGE_IMG_SIZE=$(json_extract_int "$IMGFORGE_REPORT_FILE" "img_size_bytes" 0)
        IMGFORGE_ROUTING_NODES=$(json_extract_int "$IMGFORGE_REPORT_FILE" "routing_nodes" 0)
        IMGFORGE_ROUTING_ARCS=$(json_extract_int "$IMGFORGE_REPORT_FILE" "routing_arcs" 0)
        log_info "  Tuiles compilées : ${IMGFORGE_TILES_COMPILED} (${IMGFORGE_TILES_FAILED} échec(s))"
        [[ "$IMGFORGE_TILES_FAILED" -gt 0 ]] && {
            log_warn "${IMGFORGE_TILES_FAILED} tuile(s) en échec — carte incomplète"
            PARTIAL_FAILURE=true
        }
    fi
}

# ---------------------------------------------------------------------------
# Résumé final
# ---------------------------------------------------------------------------
show_summary() {
    log_step "Résumé"

    local total_duration=$(( SECONDS - BUILD_START_TIME ))

    if [[ "$DRY_RUN" == false ]]; then
        echo -e "  ${BOLD}[Phase 1 — mpforge]${NC}"
        echo -e "  Tuiles générées    : ${TILES_SUCCESS}/${TILES_TOTAL}"
        if [[ "$TILES_FAILED" -gt 0 ]]; then
            echo -e "  ${YELLOW}${BOLD}Tuiles en échec  :${NC}  ${TILES_FAILED}"
        fi
        [[ "$FEATURES_PROCESSED" -gt 0 ]] && \
            echo -e "  Features           : ${FEATURES_PROCESSED}"
        if [[ "$MPFORGE_DURATION" -gt 0 ]]; then
            local m=$(( MPFORGE_DURATION / 60 )) s=$(( MPFORGE_DURATION % 60 ))
            echo -e "  mpforge        : ${m}m${s}s"
        fi

        echo ""
        echo -e "  ${BOLD}[Phase 2 — imgforge]${NC}"
        echo -e "  Tuiles compilées   : ${IMGFORGE_TILES_COMPILED}"
        [[ "$IMGFORGE_TILES_FAILED" -gt 0 ]] && \
            echo -e "  ${YELLOW}Tuiles en échec  : ${IMGFORGE_TILES_FAILED}${NC}"
        [[ "$IMGFORGE_ROUTING_NODES" -gt 0 ]] && \
            echo -e "  Nœuds routage      : ${IMGFORGE_ROUTING_NODES}"
        [[ "$IMGFORGE_ROUTING_ARCS" -gt 0 ]] && \
            echo -e "  Arcs routage       : ${IMGFORGE_ROUTING_ARCS}"
        if [[ "$IMGFORGE_DURATION" -gt 0 ]]; then
            local im=$(( IMGFORGE_DURATION / 60 )) is=$(( IMGFORGE_DURATION % 60 ))
            echo -e "  imgforge       : ${im}m${is}s"
        fi
        echo ""
    fi

    local total_m=$(( total_duration / 60 )) total_s=$(( total_duration % 60 ))
    echo -e "  ${BOLD}Temps total     :${NC}  ${total_m}m${total_s}s"

    if [[ -f "${OUTPUT_DIR}/gmapsupp.img" ]]; then
        local size_bytes
        if [[ "$IMGFORGE_IMG_SIZE" -gt 0 ]]; then
            size_bytes="$IMGFORGE_IMG_SIZE"
        else
            size_bytes=$(stat -c%s "${OUTPUT_DIR}/gmapsupp.img" 2>/dev/null || echo 0)
        fi
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
    echo "  │  build-garmin-map.sh — Pipeline mpforge → imgforge      │"
    echo "  │  BDTOPO → tuiles .mp → gmapsupp.img · v${SCRIPT_VERSION}                │"
    echo "  └─────────────────────────────────────────────────────────────────┘"
    echo -e "${NC}"

    parse_args "$@"
    BUILD_START_TIME=$SECONDS

    check_prerequisites
    prepare_config
    run_mpforge
    run_imgforge_build
    show_summary

    if [[ "$PARTIAL_FAILURE" == true ]]; then
        log_warn "Pipeline terminé avec avertissements — carte partielle dans : ${OUTPUT_DIR}/"
        exit 2
    fi
    log_ok "Pipeline terminé — carte disponible dans : ${OUTPUT_DIR}/"
}

main "$@"
