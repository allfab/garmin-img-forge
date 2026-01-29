# MPForge

> On forge des cartes Garmin à partir de données SIG massives


🎯 Prochaines Étapes Recommandées

  1. Revue de Code : Exécuter /bmad-bmm-code-review avec un LLM différent pour une revue adversariale
  2. Story 1.2 : Implémentation du Polish Map Header Parser et IMG ID section
  3. Tests d'Intégration : Ajouter des tests avec de vrais fichiers .mp (corpus de test)

  ❓ Questions ou Clarifications ?

  Est-ce que tu souhaites que je t'explique :
  - Comment fonctionne le système d'enregistrement GDAL ?
  - Pourquoi certaines décisions architecturales ont été prises ?
  - Comment étendre le driver pour les prochaines stories ?
  - Autre chose sur l'implémentation


---
Étape 1 : Installer Rust (obligatoire)

# Installation via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Charger l'environnement (ou redémarrer le terminal)
source $HOME/.cargo/env

# Ajouter les composants recommandés
rustup component add clippy rustfmt rust-src

---
Étape 2 : Installer QGIS + PyQGIS (obligatoire)

Sur Debian :
# Ajouter le dépôt QGIS officiel
sudo apt install gnupg software-properties-common
sudo wget -O /etc/apt/keyrings/qgis-archive-keyring.gpg https://download.qgis.org/downloads/qgis-archive-keyring.gpg

# Ajouter le dépôt (adapter pour votre version Debian)
echo "deb [signed-by=/etc/apt/keyrings/qgis-archive-keyring.gpg] https://qgis.org/debian bookworm main" | sudo tee /etc/apt/sources.list.d/qgis.list

# Installer QGIS et PyQGIS
sudo apt update
sudo apt install qgis python3-qgis

---
Étape 3 : Configurer les variables d'environnement

Ajoutez ces lignes à votre ~/.bashrc ou ~/.zshrc :

# GDAL
export GDAL_DATA=/usr/share/gdal
export GDAL_DRIVER_PATH=$HOME/.gdal/plugins
export GDAL_HOME=/usr

# Rust
export RUST_BACKTRACE=1
export RUST_LOG=info

# PyQGIS
export PYTHONPATH=/usr/share/qgis/python:$PYTHONPATH
export QGIS_PREFIX_PATH=/usr

Puis rechargez :
source ~/.bashrc  # ou ~/.zshrc

---
Étape 4 : Créer le répertoire plugins GDAL

mkdir -p ~/.gdal/plugins
mkdir -p ~/.local/share/QGIS/QGIS3/profiles/default/python/plugins

---
Étape 5 (optionnel) : Java + mkgmap

Pour la génération de cartes Garmin (Phase 4) :
# Java
sudo apt install openjdk-11-jre

# mkgmap
wget https://www.mkgmap.org.uk/download/mkgmap-latest.tar.gz
tar xzf mkgmap-latest.tar.gz
sudo mv mkgmap-* /opt/mkgmap
echo 'export PATH=/opt/mkgmap:$PATH' >> ~/.bashrc
