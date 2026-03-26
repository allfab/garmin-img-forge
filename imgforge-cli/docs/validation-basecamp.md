# Validation manuelle BaseCamp & GPS — Story 13.6

Guide de validation manuelle du fichier `.img` produit par `imgforge-cli` avec la fixture BDTOPO Isère.

> **Note :** Cette validation est hors-scope automatisation (AC3 & AC4).
> Les tests automatisés de la structure binaire sont dans `tests/integration_test.rs` (section "E2E Story 13.6").

---

## Étape 1 : Compiler la fixture BDTOPO

```bash
# Depuis la racine du projet imgforge-cli
cargo build --release
./target/release/imgforge-cli compile tests/fixtures/bdtopo_tile.mp -o /tmp/bdtopo_tile.img
```

Résultat attendu : sortie vide, code de retour 0, fichier `/tmp/bdtopo_tile.img` créé.

---

## Étape 2 : Ouvrir dans Garmin BaseCamp

1. Lancer **Garmin BaseCamp** (macOS ou Windows).
2. Menu **Maps → Install Map…**
3. Sélectionner `/tmp/bdtopo_tile.img`.
4. Attendre l'import (quelques secondes).

---

## Étape 3 : Naviguer sur la zone Isère

1. Dans le panneau de gauche, faire un clic droit sur la carte importée → **Go To Map**.
2. La carte devrait centrer sur l'Isère **(lat ≈ 45.2°, lon ≈ 5.8°)**.
3. Zoomer aux **niveaux 1 et 2** (résolutions Level0=24, Level1=18).

---

## Étape 4 : Checklist visuelle

| Élément | Attendu | Observé |
|---------|---------|---------|
| Route D1075 | Visible en rouge/orange aux niveaux 1–3 | ☐ |
| Autoroute ~[0x04]A480 (shield) | Pictogramme autoroute + "A480" | ☐ |
| Route D523 | Visible aux niveaux 1–3 | ☐ |
| Chemin de la Châtaigneraie | Visible uniquement au niveau 1 | ☐ |
| Mairie de Saint-Égrève (POI 0x2C00) | Icône mairie, cliquable, nom lisible | ☐ |
| Col du Glandon (POI 0x6400) | Icône sommet | ☐ |
| Forêt de Porte (polygone 0x50) | Zone verte | ☐ |
| Zone bâtie (polygone 0x01) | Zone grise/beige | ☐ |
| Lac de Montagnole (polygone 0x3C) | Zone bleue | ☐ |
| Labels accentués | "Châtaigneraie", "Saint-Égrève" lisibles | ☐ |

---

## Étape 5 (optionnel) : Copier sur GPS Garmin

### Via carte SD (eTrex, Edge, etc.)
```bash
# Monter la carte SD du GPS
cp /tmp/bdtopo_tile.img /media/<VOLUME>/Garmin/
# Éjecter proprement la carte, réinsérer dans le GPS
```

### Via USB Mass Storage
```bash
# Le GPS apparaît comme un volume USB
cp /tmp/bdtopo_tile.img /media/<GPS_VOLUME>/Garmin/
```

**Sur le GPS :** Menu Maps → vérifier que la carte est activée → naviguer vers lat 45.2°, lon 5.8°.

---

## Diagnostics connus

| Symptôme | Cause probable | Solution |
|----------|----------------|----------|
| Carte vide dans BaseCamp | Conflit de Map ID | Changer `ID=` dans la fixture, recompiler |
| Labels illisibles / caractères corrompus | Problème d'encodage CP1252 | Vérifier le LBL avec `xxd /tmp/bdtopo_tile.img | grep -A2 "Mairie"` |
| Carte non visible sur GPS | Mauvaise cible Garmin | Vérifier que le modèle GPS supporte les cartes custom |
| `gmaptool -info bdtopo_tile.img` (si disponible) | Validation binaire externe | `gmaptool` non installé par défaut ; utiliser les tests automatisés |

### Validation binaire rapide (sans BaseCamp)

```bash
# Vérifier le magic GARMIN
xxd /tmp/bdtopo_tile.img | head -2

# Vérifier que "63240038" est présent (Map ID dans le directory)
python3 -c "
data = open('/tmp/bdtopo_tile.img', 'rb').read()
print('Map ID found:', b'63240038' in data[:1024])
print('D1075 in LBL:', b'D1075' in data)
print('Mairie de Saint- in LBL:', b'Mairie de Saint-' in data)
"

# Vérifier la signature DOS 0x55/0xAA
python3 -c "
data = open('/tmp/bdtopo_tile.img', 'rb').read()
print(f'Signature: 0x{data[0x1FE]:02X} 0x{data[0x1FF]:02X} (attendu: 0x55 0xAA)')
import functools; print(f'XOR header: 0x{functools.reduce(lambda a, b: a ^ b, data[:512]):02X} (attendu: 0x00)')
"
```
