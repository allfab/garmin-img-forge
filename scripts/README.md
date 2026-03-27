# Scripts utilitaires MPForge

> Scripts pour faciliter la gestion des releases, tags Git et le pipeline BDTOPO → Garmin

## 📜 Scripts disponibles

### 📥 download-bdtopo.sh — Téléchargement BD TOPO® IGN

**Usage** :
```bash
# Département unique
./scripts/download-bdtopo.sh --zones D038 --format SHP

# Dry-run (simulation sans téléchargement)
./scripts/download-bdtopo.sh --zones D038 --dry-run

# Avec debug
./scripts/download-bdtopo.sh --zones D038 --debug
```

**Description** :
- Interroge l'API Géoplateforme (`data.geopf.fr`) pour découvrir les datasets BDTOPO disponibles
- Auto-détecte la dernière édition trimestrielle disponible
- Télécharge l'archive `.7z` avec reprise automatique (`curl -C -`)
- Vérifie le hash MD5 des fichiers téléchargés
- Extrait les dossiers thématiques Shapefile (ADMINISTRATIF, BATI, HYDROGRAPHIE, …)
- Organise les données dans `data/bdtopo/{YYYY}/v{YYYY.MM}/{DXXX}/`
- Idempotent : skip les fichiers déjà téléchargés et intacts (MD5 OK)

**Prérequis** :
```bash
sudo apt install curl p7zip-full
```

**Pipeline complet** :
```
download-bdtopo.sh → mpforge-cli build → imgforge-cli → gmapsupp.img
```

---


### 🏷️ retag.sh - Forcer un tag existant

**Usage** :
```bash
./scripts/retag.sh v0.1.0           # Retag current HEAD
./scripts/retag.sh v0.1.0 abc123    # Retag specific commit
```

**Description** :
- Supprime le tag local et distant
- Re-crée le tag sur le commit spécifié (ou HEAD)
- Push le nouveau tag
- Déclenche automatiquement le workflow Woodpecker

**Cas d'usage** :
- Corriger un workflow qui a échoué
- Mettre à jour une release avec un nouveau commit

---

### 🚀 release.sh - Créer une release complète

**Usage** :
```bash
./scripts/release.sh v0.1.0
```

**Description** :
- Vérifie que vous êtes sur `main`
- Vérifie qu'il n'y a pas de changements non commités
- Vérifie que le tag n'existe pas déjà
- Pull pour synchroniser
- Demande un message de release interactif
- Crée et push le tag

**Cas d'usage** :
- Créer une nouvelle release de façon sécurisée
- Workflow de release complet avec validation

---

## 📖 Documentation complète

Voir la section **[CI/CD : Woodpecker CI](../README.md#cicd--woodpecker-ci)** du README principal pour :
- Guide complet de gestion des tags et releases
- Bonnes pratiques
- Commandes de référence

---

## 🔧 Installation

Les scripts sont déjà exécutables. Si nécessaire :

```bash
chmod +x scripts/*.sh
```

---

## ⚠️ Important

Ces scripts modifient l'historique Git (tags). Utilisez-les avec précaution en production.

**Recommandation** : Testez d'abord sur une branche de développement ou un tag de test.
