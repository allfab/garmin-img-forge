# Logiciels et outils

| Outil | Rôle | Langage |
|-------|------|---------|
| `mpforge` | Découpe vecteur → Polish Map tiles | Rust |
| `imgforge` | Compilation Polish Map → Garmin IMG | Rust |
| `ogr-polishmap` | Driver OGR/GDAL pour lire/écrire le format `.mp` | C++/GDAL |
| `download-bdtopo.sh` | Téléchargement automatisé BD TOPO IGN | Bash |
| `build-garmin-map.sh` | Orchestration du pipeline complet | Bash |

## Données sources

- **BD TOPO IGN** — données vecteur du territoire français, licence ouverte (Etalab)
- Mise à jour semestrielle (janvier/juillet)
- Disponible par département ou région
