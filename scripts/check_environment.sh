#!/bin/bash
# Script de vérification de l'environnement de développement mpforge
# Vérifie la présence et les versions des outils requis pour C++, Rust et Python/QGIS

# Note: pas de 'set -e' car on veut continuer même si certains checks échouent

# Couleurs pour l'output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color
BOLD='\033[1m'

# Compteurs
CHECKS_PASSED=0
CHECKS_FAILED=0
CHECKS_WARNING=0

# Fonction pour afficher le header
print_header() {
    echo ""
    echo -e "${BOLD}=== Vérification Environnement mpforge ===${NC}"
    echo ""
}

# Fonction pour vérifier une commande
check_command() {
    local cmd=$1
    local name=$2
    local version_flag=${3:---version}
    local required=${4:-true}

    echo -n "Vérification $name... "

    if command -v "$cmd" &> /dev/null; then
        local version
        version=$($cmd $version_flag 2>&1 | head -n1)
        echo -e "${GREEN}✓${NC} $version"
        ((CHECKS_PASSED++))
        return 0
    else
        if [ "$required" = "true" ]; then
            echo -e "${RED}✗ NON TROUVÉ${NC}"
            ((CHECKS_FAILED++))
        else
            echo -e "${YELLOW}⚠ NON TROUVÉ (optionnel)${NC}"
            ((CHECKS_WARNING++))
        fi
        return 1
    fi
}

# Fonction pour vérifier une version minimale
check_version() {
    local cmd=$1
    local name=$2
    local min_version=$3
    local version_cmd=$4

    if command -v "$cmd" &> /dev/null; then
        local current_version
        current_version=$(eval "$version_cmd")
        echo -e "  ${name}: ${current_version} (minimum requis: ${min_version})"
    fi
}

# Fonction pour vérifier un package Python
check_python_package() {
    local package=$1
    local name=$2
    local required=${3:-true}

    echo -n "Vérification $name... "

    if python3 -c "import $package" 2>/dev/null; then
        local version
        version=$(python3 -c "import $package; print(getattr($package, '__version__', 'version inconnue'))" 2>/dev/null)
        echo -e "${GREEN}✓${NC} $version"
        ((CHECKS_PASSED++))
        return 0
    else
        if [ "$required" = "true" ]; then
            echo -e "${RED}✗ NON TROUVÉ${NC}"
            ((CHECKS_FAILED++))
        else
            echo -e "${YELLOW}⚠ NON TROUVÉ (optionnel)${NC}"
            ((CHECKS_WARNING++))
        fi
        return 1
    fi
}

# Fonction pour vérifier un fichier/répertoire
check_path() {
    local path=$1
    local name=$2
    local required=${3:-false}

    echo -n "Vérification $name... "

    if [ -e "$path" ]; then
        echo -e "${GREEN}✓${NC} $path"
        ((CHECKS_PASSED++))
        return 0
    else
        if [ "$required" = "true" ]; then
            echo -e "${RED}✗ NON TROUVÉ${NC}"
            ((CHECKS_FAILED++))
        else
            echo -e "${YELLOW}⚠ NON TROUVÉ${NC}"
            ((CHECKS_WARNING++))
        fi
        return 1
    fi
}

# Fonction pour vérifier les variables d'environnement
check_env_var() {
    local var=$1
    local name=$2
    local required=${3:-false}

    echo -n "Vérification $name... "

    if [ -n "${!var}" ]; then
        echo -e "${GREEN}✓${NC} ${!var}"
        ((CHECKS_PASSED++))
        return 0
    else
        if [ "$required" = "true" ]; then
            echo -e "${RED}✗ NON DÉFINI${NC}"
            ((CHECKS_FAILED++))
        else
            echo -e "${YELLOW}⚠ NON DÉFINI${NC}"
            ((CHECKS_WARNING++))
        fi
        return 1
    fi
}

# Afficher le header
print_header

# ============================================================================
# SECTION 1 : Outils de Build Basiques
# ============================================================================
echo -e "${BOLD}[1/7] Outils de Build Basiques${NC}"
check_command "gcc" "GCC" "--version"
if command -v gcc &> /dev/null; then
    check_version "gcc" "  Version GCC" "13.0" "gcc -dumpversion"
fi

check_command "g++" "G++" "--version"
check_command "clang" "Clang" "--version" false
check_command "cmake" "CMake" "--version"
if command -v cmake &> /dev/null; then
    check_version "cmake" "  Version CMake" "3.20" "cmake --version | grep -oP 'version \K[0-9.]+'"
fi

check_command "make" "Make" "--version"
check_command "pkg-config" "pkg-config" "--version"
echo ""

# ============================================================================
# SECTION 2 : GDAL et Dépendances Géospatiales
# ============================================================================
echo -e "${BOLD}[2/7] GDAL et Dépendances Géospatiales${NC}"
check_command "gdalinfo" "GDAL" "--version"
if command -v gdalinfo &> /dev/null; then
    check_version "gdalinfo" "  Version GDAL" "3.8.0" "gdalinfo --version | grep -oP 'GDAL \K[0-9.]+'"
fi

check_command "ogrinfo" "OGR" "--version"

# Vérifier headers GDAL
if [ -f "/usr/include/gdal/gdal.h" ] || [ -f "/usr/include/gdal.h" ]; then
    echo -e "Vérification Headers GDAL... ${GREEN}✓${NC} Trouvés"
    ((CHECKS_PASSED++))
else
    echo -e "Vérification Headers GDAL... ${YELLOW}⚠ NON TROUVÉS${NC}"
    echo "  Installer: sudo dnf install gdal-devel (Fedora) ou sudo apt install libgdal-dev (Ubuntu)"
    ((CHECKS_WARNING++))
fi

# Vérifier PROJ
if command -v proj &> /dev/null; then
    echo -n "Vérification PROJ... "
    proj_version=$(proj 2>&1 | grep -oP 'Rel. \K[0-9.]+' | head -n1)
    echo -e "${GREEN}✓${NC} $proj_version"
    ((CHECKS_PASSED++))
else
    echo -e "Vérification PROJ... ${YELLOW}⚠ NON TROUVÉ${NC}"
    ((CHECKS_WARNING++))
fi

# Vérifier le plugin directory GDAL
check_env_var "GDAL_DRIVER_PATH" "GDAL_DRIVER_PATH" false
if [ -z "$GDAL_DRIVER_PATH" ]; then
    echo "  Suggéré: export GDAL_DRIVER_PATH=\$HOME/.gdal/plugins"
fi

echo ""

# ============================================================================
# SECTION 3 : Rust Toolchain
# ============================================================================
echo -e "${BOLD}[3/7] Rust Toolchain${NC}"
check_command "rustc" "Rust" "--version"
if command -v rustc &> /dev/null; then
    check_version "rustc" "  Version Rust" "1.75.0" "rustc --version | grep -oP 'rustc \K[0-9.]+'"
fi

check_command "cargo" "Cargo" "--version"
check_command "rustup" "Rustup" "--version"

# Vérifier les composants Rust
if command -v rustup &> /dev/null; then
    echo -n "Vérification Clippy... "
    if rustup component list | grep -q "clippy.*(installed)"; then
        echo -e "${GREEN}✓${NC} Installé"
        ((CHECKS_PASSED++))
    else
        echo -e "${YELLOW}⚠ NON INSTALLÉ${NC}"
        echo "  Installer: rustup component add clippy"
        ((CHECKS_WARNING++))
    fi

    echo -n "Vérification rustfmt... "
    if rustup component list | grep -q "rustfmt.*(installed)"; then
        echo -e "${GREEN}✓${NC} Installé"
        ((CHECKS_PASSED++))
    else
        echo -e "${YELLOW}⚠ NON INSTALLÉ${NC}"
        echo "  Installer: rustup component add rustfmt"
        ((CHECKS_WARNING++))
    fi
fi

echo ""

# ============================================================================
# SECTION 4 : Python et QGIS
# ============================================================================
echo -e "${BOLD}[4/7] Python et QGIS${NC}"
check_command "python3" "Python" "--version"
if command -v python3 &> /dev/null; then
    check_version "python3" "  Version Python" "3.10.0" "python3 --version | grep -oP 'Python \K[0-9.]+'"
fi

check_command "qgis" "QGIS" "--version"
if command -v qgis &> /dev/null; then
    qgis_version=$(qgis --version 2>&1 | head -n1)
    echo "  $qgis_version"
fi

# Vérifier PyQGIS
check_python_package "qgis.core" "PyQGIS" true
if python3 -c "from qgis.core import QgsApplication" 2>/dev/null; then
    pyqgis_version=$(python3 -c "from qgis.core import QgsApplication; print(QgsApplication.version())" 2>/dev/null)
    echo "  Version PyQGIS: $pyqgis_version"
fi

# Vérifier le répertoire plugins QGIS
QGIS_PLUGINS_DIR="$HOME/.local/share/QGIS/QGIS3/profiles/default/python/plugins"
check_path "$QGIS_PLUGINS_DIR" "Répertoire plugins QGIS" false

echo ""

# ============================================================================
# SECTION 5 : Outils Optionnels
# ============================================================================
echo -e "${BOLD}[5/7] Outils Optionnels${NC}"
check_command "java" "Java" "-version" false
if command -v java &> /dev/null; then
    java_version=$(java -version 2>&1 | head -n1 | grep -oP 'version "\K[0-9.]+')
    echo "  Version Java: $java_version (minimum: 11)"
fi

# Vérifier mkgmap
if [ -d "/opt/mkgmap" ] || command -v mkgmap &> /dev/null; then
    echo -e "Vérification mkgmap... ${GREEN}✓${NC} Trouvé"
    ((CHECKS_PASSED++))
else
    echo -e "Vérification mkgmap... ${YELLOW}⚠ NON TROUVÉ (optionnel)${NC}"
    echo "  Télécharger depuis: https://www.mkgmap.org.uk/download/"
    ((CHECKS_WARNING++))
fi

# Vérifier splitter (optionnel, Phase 4)
if [ -d "/opt/splitter" ] || command -v splitter &> /dev/null; then
    echo -e "Vérification splitter... ${GREEN}✓${NC} Trouvé"
    ((CHECKS_PASSED++))
else
    echo -e "Vérification splitter... ${YELLOW}⚠ NON TROUVÉ (optionnel, Phase 4)${NC}"
    ((CHECKS_WARNING++))
fi

check_command "git" "Git" "--version" false
check_command "doxygen" "Doxygen" "--version" false

echo ""

# ============================================================================
# SECTION 6 : Variables d'Environnement
# ============================================================================
echo -e "${BOLD}[6/7] Variables d'Environnement${NC}"
check_env_var "GDAL_DATA" "GDAL_DATA" false
check_env_var "GDAL_HOME" "GDAL_HOME (pour Rust gdal crate)" false
check_env_var "PYTHONPATH" "PYTHONPATH (pour PyQGIS)" false
check_env_var "QGIS_PREFIX_PATH" "QGIS_PREFIX_PATH" false

echo ""

# ============================================================================
# SECTION 7 : Configuration Projet
# ============================================================================
echo -e "${BOLD}[7/7] Configuration Projet${NC}"

# Vérifier si on est dans le bon répertoire
if [ -f "Cargo.toml" ] || [ -d "gdal-driver-mp" ] || [ -d "docs" ]; then
    echo -e "Vérification Répertoire projet... ${GREEN}✓${NC} Dans le projet mpforge"
    ((CHECKS_PASSED++))
else
    echo -e "Vérification Répertoire projet... ${YELLOW}⚠${NC} Pas dans le répertoire mpforge"
    ((CHECKS_WARNING++))
fi

# Vérifier structure de base
check_path "docs" "docs/" false
check_path "scripts" "scripts/" false

echo ""

# ============================================================================
# RÉSUMÉ
# ============================================================================
echo -e "${BOLD}=== Résumé ===${NC}"
echo ""
echo -e "${GREEN}✓ Vérifications réussies:${NC} $CHECKS_PASSED"
if [ $CHECKS_WARNING -gt 0 ]; then
    echo -e "${YELLOW}⚠ Avertissements:${NC} $CHECKS_WARNING"
fi
if [ $CHECKS_FAILED -gt 0 ]; then
    echo -e "${RED}✗ Vérifications échouées:${NC} $CHECKS_FAILED"
fi

echo ""

# Statut final
if [ $CHECKS_FAILED -eq 0 ]; then
    echo -e "${GREEN}${BOLD}✅ Environnement prêt pour le développement mpforge !${NC}"
    echo ""
    echo "Prochaines étapes:"
    echo "  1. Lire la documentation: docs/development-environment.md"
    echo "  2. Configurer les variables d'environnement (voir doc)"
    echo "  3. Commencer le développement !"
    exit 0
else
    echo -e "${RED}${BOLD}❌ Environnement incomplet${NC}"
    echo ""
    echo "Actions requises:"
    echo "  1. Installer les composants manquants (voir messages ci-dessus)"
    echo "  2. Consulter: docs/development-environment.md"
    echo "  3. Relancer ce script après installation"
    exit 1
fi
