# Le Pipeline de fabrication

Cette section décrit **étape par étape** le processus complet de création d'une carte Garmin topographique à partir des données IGN BD TOPO. Chaque étape est illustrée avec les commandes réelles à exécuter.

---

## Vue d'ensemble

```mermaid
flowchart TD
    A["1. Téléchargement<br/>BD TOPO IGN"] --> B["2. Configuration<br/>YAML + field mapping"]
    B --> C["3. Tuilage<br/>mpforge build"]
    C --> D["4. Compilation<br/>imgforge build"]
    D --> E["5. Installation<br/>GPS Garmin"]

    style A fill:#e8f5e9,stroke:#4caf50,color:#1b5e20
    style B fill:#f3e5f5,stroke:#9c27b0,color:#4a148c
    style C fill:#fff3e0,stroke:#ff9800,color:#e65100
    style D fill:#e3f2fd,stroke:#2196f3,color:#0d47a1
    style E fill:#fce4ec,stroke:#e91e63,color:#880e4f
```

| Étape | Outil | Entrée | Sortie | Durée typique |
|-------|-------|--------|--------|---------------|
| 1. Téléchargement | `download-bdtopo.sh` | URL IGN | `.gpkg` / `.shp` | 10-30 min |
| 2. Configuration | Éditeur texte | - | `.yaml` | 5-15 min |
| 3. Tuilage | `mpforge build` | `.gpkg` / `.shp` | `tiles/*.mp` | 30 min - 3h |
| 4. Compilation | `imgforge build` | `tiles/*.mp` | `gmapsupp.img` | 10 min - 1h |
| 5. Installation | Copie fichier | `gmapsupp.img` | GPS Garmin | 2 min |

!!! info "Durées indicatives"
    Les durées dépendent de la zone géographique (un département vs la France entière), du matériel, et du nombre de threads utilisés. Les chiffres ci-dessus correspondent à un poste de travail standard avec 8 threads.
