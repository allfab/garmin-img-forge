# typforge

> Éditeur graphique natif de fichiers TYP Garmin — Linux / Windows / macOS

**typforge** est une application desktop Rust + Slint qui remplace TYPViewer sur Linux. Il permet d'éditer visuellement les fichiers TYP Garmin (symboles de polygones, polylignes, POIs, draworder, métadonnées), avec galerie de thumbnails, éditeur pixel-grid pour les icônes, gestion jour/nuit, encodage CP1252 natif, et compilation/décompilation `.typ` binaire intégrée.

## Caractéristiques

- **Éditeur visuel complet** : polygones, polylignes, POIs, Extra POIs, DrawOrder
- **Galerie de thumbnails** : rendu immédiat de tous les types avec onglets Text / Icons
- **Preview Day + Night** : modes Single, Mosaic, Superposition avec fond configurable
- **Panneau métadonnées** : FID, ProductCode, CodePage, en-tête `[_id]`
- **Éditeur POI pixel-grid** : grille zoomée, palette couleurs, outils crayon/gomme/fill/pipette
- **Labels multilingues** : grille de 35 codes langue (0x00–0x22), nom de langue, "Set as Default"
- **Import PNG/JPG** : conversion automatique en XPM avec quantisation palette
- **Load/Save** `.txt` CP1252 direct, compilation/décompilation `.typ` binaire intégrée
- **Encodage CP1252 natif** : via `encoding_rs`, round-trip sans perte
- **Cross-platform** : Linux en priorité, Windows/macOS à terme

## Prérequis

- **Rust** 1.70+ ([rustup](https://rustup.rs/))
- **Bibliothèques système** (Linux) :

  ```bash
  # Fedora / RHEL
  sudo dnf install libxkbcommon-devel wayland-devel

  # Debian / Ubuntu
  sudo apt install libxkbcommon-dev libwayland-dev
  ```

## Mode développement

```bash
cd tools/typforge

# Lancer directement depuis les sources
cargo run --features ui

# Ouvrir un fichier au démarrage
cargo run --features ui -- ../../pipeline/resources/typfiles/IGNBDTOPO.txt
cargo run --features ui -- path/to/style.typ
```

Le flag `--features ui` est obligatoire : il active Slint et le dialog fichier natif (`rfd`). Sans ce flag, seule la librairie (parser/writer TYP) est compilée — utile pour les tests.

### Tests

```bash
# Tests unitaires et d'intégration (sans feature ui)
cargo test

# Avec sortie console
cargo test -- --nocapture

# Un test spécifique
cargo test ignbdtopo_parse
```

Les tests couvrent :

| Test | Description |
|------|-------------|
| `round_trip_text` | parse `.txt` → write → parse → champs identiques |
| `round_trip_binary` | compile → decompile → recompile → bytes identiques |
| `ignbdtopo_parse` | charge `IGNBDTOPO.txt` → 132 poly, 93 lignes, 330 pts, 0 erreur |
| `ignbdtopo_compile` | compile `IGNBDTOPO.txt` → compare avec `IGNBDTOPO.typ` |
| `xpm_import_png` | import PNG 32×32 → XPM palette ≤ 16 couleurs |
| `xpm_round_trip` | parse XPM → render RGBA → regenerate → identique |

### Linting et formatage

```bash
cargo fmt --check   # vérifier le formatage
cargo fmt           # formater
cargo clippy --features ui -- -D warnings
```

## Mode production

```bash
# Compiler le binaire optimisé
cargo build --release --features ui

# Le binaire est dans
./target/release/typforge

# Installer globalement
cargo install --path . --features ui
```

Le profil release active `lto=true`, `codegen-units=1`, `strip=true`, `opt-level="z"` — binaire standalone sans dépendance dynamique au-delà des libs système.

## Utilisation

```
typforge [FICHIER]
```

| Argument | Description |
|----------|-------------|
| `FICHIER` | Optionnel — chemin vers un `.txt` (TYP texte CP1252) ou `.typ` (binaire) à ouvrir au démarrage |

L'extension du fichier détermine automatiquement le mode d'ouverture : `.typ` → décompilation binaire, tout autre → parsing texte CP1252.

### Barre d'outils

| Bouton | Action |
|--------|--------|
| **Ouvrir** | Ouvre un fichier `.txt` ou `.typ` via dialog natif |
| **Enregistrer** | Sauvegarde en `.txt` CP1252 |
| **Enregistrer .TYP** | Compile et exporte en binaire `.typ` |
| **Quitter** | Ferme l'application |

### Navigation

Les boutons **Polygones [N]**, **Lignes [N]**, **POIs [N]**, **Pois+ [N]** filtrent la galerie centrale et affichent le compteur de chaque type.

## Structure du projet

```
typforge/
├── Cargo.toml               # Dépendances ; feature "ui" active Slint + rfd
├── build.rs                 # Compile les fichiers .slint au build
├── src/
│   ├── main.rs              # Point d'entrée, callbacks Rust ↔ Slint, render thumbnails
│   ├── app.rs               # App { doc } — open_txt / save_txt / export_typ / import_typ
│   ├── error.rs             # TypforgeError (thiserror)
│   ├── lib.rs               # Exports publics pour les tests d'intégration
│   ├── typ/
│   │   ├── mod.rs
│   │   ├── model.rs         # TypDocument, TypPolygon, TypLine, TypPoint, Xpm…
│   │   ├── text_reader.rs   # parse(bytes) → TypDocument (CP1252, port mkgmap)
│   │   ├── text_writer.rs   # write(doc) → Vec<u8> CP1252 CRLF
│   │   ├── binary_reader.rs # decompile(bytes) → TypDocument
│   │   ├── binary_writer.rs # compile(doc) → Vec<u8> (.typ binaire)
│   │   └── xpm.rs           # xpm_to_image, import_image, snap_garmin_palette, trim_colours
│   └── ui/
│       ├── app_window.slint # Fenêtre principale — layout 3 panneaux, barres outils/navigation
│       ├── left_panel.slint # Métadonnées [_id], 4 listes + add/éditer/supprimer, DrawOrder
│       ├── gallery.slint    # Galerie thumbnails, onglets Text/Icons, VecModel<GalleryItem>
│       └── preview_panel.slint # Preview Day/Night, modes Single/Mosaic/Superposition
└── tests/
    ├── integration_test.rs
    └── fixtures/
        ├── sample.txt       # Extrait de référence (1 poly, 1 ligne, 1 point, 1 draworder)
        └── sample.typ       # Binaire compilé correspondant
```

## Dépendances clés

| Crate | Usage |
|-------|-------|
| `slint 1.x` | Framework UI natif (`.slint` → binaire standalone) |
| `encoding_rs` | Encodage/décodage CP1252 ↔ UTF-8 à la frontière I/O |
| `byteorder` | Lecture/écriture binaire little-endian (format `.typ`) |
| `image` | Import PNG/JPG → XPM (quantisation palette) |
| `rfd` | Dialog fichier natif (Linux : xdg-portal / zenity / kdialog) |
| `anyhow` + `thiserror` | Gestion d'erreurs |

## Références

- `tmp/mkgmap/src/uk/me/parabola/mkgmap/typ/` — parser TYP texte de référence (Java)
- `tmp/mkgmap/src/uk/me/parabola/imgfmt/app/typ/` — writer/reader binaire de référence (Java)
- `pipeline/resources/typfiles/IGNBDTOPO.txt` — corpus de référence (132 poly, 93 lignes, 330 pts)
- `tools/imgforge/src/` — patterns Rust à suivre (clap, anyhow, encoding_rs)

## Licence

Distribué sous licence MIT. Voir [LICENSE](../../LICENSE) à la racine du dépôt.
