# Installation des outils

La chaîne **garmin-img-forge** repose sur deux binaires Rust autonomes,
sans dépendance système (pas de JVM, pas de Python, pas de GDAL séparé).

## mpforge — Forge les tuiles Polish Map

Lit les couches SHP/GPKG, découpe selon la grille de tuilage,
applique les règles Garmin et la simplification géométrique,
écrit les fichiers **Polish Map** (`.mp`) — un par tuile.

## imgforge — Compile le fichier Garmin IMG

Prend un dossier de fichiers `.mp` et les compile en un seul
binaire **Garmin IMG** (`.img`), prêt pour tout GPS Garmin ou QMapShack.

## Installation

Téléchargement depuis les releases GitHub, extraction et copie dans `~/.local/bin/`.
Même procédure pour `mpforge` et `imgforge`.
