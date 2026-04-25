# Logs imgforge — Guide de lecture

`imgforge` utilise la bibliothèque **tracing** (Rust) pour émettre des messages structurés. Par défaut (sans `-v`), seuls les avertissements (`WARN`) et erreurs (`ERROR`) sont affichés — la sortie console se limite à la barre de progression et au résumé final. Chaque niveau de verbosité débloque une couche de détail supplémentaire.

## Niveaux de verbosité

| Flag | Niveau activé | Usage recommandé |
|------|---------------|-----------------|
| *(aucun)* | `WARN` + `ERROR` | Production — barre de progression + résumé uniquement |
| `-v` | + `INFO` | Suivi tuile par tuile, messages routing |
| `-vv` | + `DEBUG` | Diagnostic encodage, barre désactivée |
| `-vvv` | + `TRACE` | Débogage fin (bitstream, subdivisions) |

En production, imgforge n'imprime aucun message de log tant qu'il n'y a pas d'avertissement ou d'erreur. La barre de progression s'affiche pendant la compilation des tuiles, suivie du résumé structuré.

---

## Sortie console de production

Sans `-v`, imgforge affiche successivement la barre de progression puis le résumé :

```
[████████████████████████████████████████] 55/55 tuiles (100%) — ETA : 0s

✅ Compilation terminée — Statut: SUCCÈS
╔════════════════════════════════════════════════════════╗
║ RÉSUMÉ D'EXÉCUTION                                     ║
╠════════════════════════════════════════════════════════╣
║ Tuiles compilées :         55                      ║
║ Tuiles échouées  :          0                      ║
║ Points           :     182340                      ║
║ Polylignes       :      94710                      ║
║ Polygones        :      31820                      ║
║ Taille IMG       :   50.0 Mo                       ║
║ Durée totale     :    8.4 sec                      ║
╚════════════════════════════════════════════════════════╝
   Fichier de sortie : gmapsupp.img

💡 Astuce : Utilisez -vv pour des logs de débogage détaillés
```

---

## Messages par niveau

### Niveau INFO (`-v`)

Ces messages apparaissent uniquement avec `-v`.

| Message | Signification |
|---------|---------------|
| `Compilation de N tuile(s) .mp` | Nombre de fichiers `.mp` détectés dans le répertoire d'entrée |
| `Tuile compilée` | Une tuile a été compilée avec succès (avec compteurs points/polylignes/polygones) |
| `--route/--net specified but no RoadID found in .mp data — Routing inactif dans cette tuile : aucun tronçon routable (RoadID inexistant)` | Le routing a été demandé (`--route`) mais les données `.mp` ne contiennent pas de `RoadID` — la tuile est compilée sans NET/NOD. Comportement attendu avec BDTOPO. |
| `JSON report written` | Le rapport JSON a été écrit au chemin spécifié par `--report` |
| `Barre de progression désactivée (verbose >= 2)` | En mode `-vv`, la barre de progression est désactivée pour ne pas interférer avec les logs détaillés |

### Niveau DEBUG (`-vv`)

Avec `-vv`, imgforge affiche le détail du traitement interne :

| Message | Signification |
|---------|---------------|
| `File is not UTF-8, using CP1252 fallback` | Le fichier `.mp` n'est pas encodé en UTF-8 — fallback CP1252 appliqué (BDTOPO standard) |

---

## Avertissements courants (`WARN`)

Ces messages apparaissent toujours, quel que soit le niveau de verbosité.

| Message | Cause | Action |
|---------|-------|--------|
| `DEM generation failed: <raison>` | Impossible de générer les données d'élévation pour cette tuile | Vérifier que les fichiers DEM couvrent l'emprise de la tuile et que le SRS est correct |
| `DEM loading failed: <raison>` | Erreur lors du chargement des sources d'élévation | Vérifier les chemins `--dem` et l'existence des fichiers HGT/ASC |
| `N tiles compiled, N errors` | Certaines tuiles ont échoué en mode `--keep-going` | Inspecter les messages d'erreur des tuiles concernées |
| `Ignoring malformed level entry: '<valeur>'` | Une valeur dans `--levels` n'est pas un entier valide | Corriger la syntaxe : `"24,20,16"` ou `"0:24,1:20,2:16"` |

---

## Rapport JSON (`--report`)

Avec `--report build-report.json`, imgforge écrit un fichier JSON structuré en complément du résumé console :

```bash
imgforge build tiles/ --output gmapsupp.img --jobs 8 --report build-report.json
```

```json
{
  "tiles_compiled": 55,
  "tiles_failed": 0,
  "total_points": 182340,
  "total_polylines": 94710,
  "total_polygons": 31820,
  "duration_ms": 8420,
  "duration_seconds": 8.42,
  "output_file": "gmapsupp.img",
  "img_size_bytes": 52428800
}
```

| Champ | Description |
|-------|-------------|
| `tiles_compiled` | Tuiles compilées avec succès |
| `tiles_failed` | Tuiles en erreur (non-zero = problème) |
| `total_points` | Total POI compilés (toutes tuiles) |
| `total_polylines` | Total polylignes compilées |
| `total_polygons` | Total polygones compilés |
| `duration_ms` | Durée d'exécution en millisecondes |
| `duration_seconds` | Durée d'exécution en secondes (flottant) |
| `output_file` | Chemin du fichier IMG produit |
| `img_size_bytes` | Taille du fichier IMG en octets |

### Lecture dans un script shell

```bash
TILES=$(jq '.tiles_compiled' build-report.json)
FAILED=$(jq '.tiles_failed' build-report.json)
DURATION=$(jq '.duration_seconds' build-report.json)
SIZE=$(jq '.img_size_bytes' build-report.json)

echo "Tuiles : ${TILES} (${FAILED} echec(s))"
echo "Duree  : ${DURATION}s"
echo "Taille : $((SIZE / 1048576)) Mo"
```

---

## Utilisation avec `build-garmin-map.sh`

Le script `scripts/build-garmin-map.sh` passe automatiquement `--report` à imgforge et lit les métriques du rapport JSON en fin de pipeline pour les afficher dans le résumé global.
