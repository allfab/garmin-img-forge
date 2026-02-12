# Scripts utilitaires MPForge

> Scripts pour faciliter la gestion des releases et tags Git

## 📜 Scripts disponibles

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

Voir **[docs/ci-cd/GIT-TAGS-RELEASES.md](../docs/ci-cd/GIT-TAGS-RELEASES.md)** pour :
- Guide complet de gestion des tags
- Bonnes pratiques
- Exemples détaillés
- Dépannage

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
