# Limites des formats

Tout format a ses contraintes. Les connaître permet d'anticiper les problèmes et de configurer le pipeline en conséquence.

---

## Format Polish Map (.mp)

| Contrainte | Détail |
|-----------|--------|
| **Géométries simples uniquement** | Point, LineString, Polygon. Pas de MultiPolygon ni GeometryCollection. |
| **Coordonnées WGS84** | Latitude/longitude en degrés décimaux (EPSG:4326). Les données en projection locale doivent être reprojetées. |
| **Max 1024 points par polyligne** | Les lignes plus longues doivent être découpées. |
| **Encodage CP1252** | Par défaut. UTF-8 possible via `CodePage=65001` mais moins courant. |
| **Format texte** | Verbeux : un gros fichier `.mp` peut atteindre plusieurs centaines de Mo. |
| **Pas de topologie** | Chaque feature est indépendante. Les relations topologiques (réseau routier) sont reconstruites par le compilateur. |

### Workaround : multi-géométries

Si vos données contiennent des MultiPolygon, décomposez-les avant import :

```bash
# Avec ogr2ogr
ogr2ogr -f "ESRI Shapefile" output.shp input.shp -explodecollections

# Avec mpforge / ogr-polishmap
# → La décomposition est automatique à l'écriture
```

Le driver ogr-polishmap décompose automatiquement les multi-géométries lors de l'écriture. mpforge filtre silencieusement les types non supportés et affiche un résumé en fin de traitement.

## Format Garmin IMG

| Contrainte | Détail |
|-----------|--------|
| **Taille max ~4 Go** | Limite du système de fichiers FAT interne au format IMG. |
| **Résolution fixe par niveau** | Le rendu n'est pas vectoriel sub-pixel. Chaque niveau de zoom a une résolution de coordonnées fixe (définie par le champ `Level`). |
| **Routing limité** | Seules les polylignes avec l'attribut `RouteParam` sont routables. Le routing Garmin n'est pas aussi flexible que celui d'un logiciel desktop. |
| **Encodage des labels** | Format 6 (ASCII) ne supporte que A-Z, 0-9. Pour les accents français, il faut Format 9 (CP1252) ou Format 10 (UTF-8). |
| **Pas de mise à jour incrémentale** | Pour modifier la carte, il faut recompiler l'intégralité du `gmapsupp.img`. |
| **Subdivisions** | Chaque tuile est découpée en subdivisions de taille limitée. Un trop grand nombre de features par tuile peut générer des subdivisions trop nombreuses. |

### Impact sur la configuration

Ces limites influencent directement les choix de configuration :

- **`cell_size: 0.15`** — Produit des tuiles de taille raisonnable (quelques Mo chacune)
- **`--reduce-point-density 3.0`** — Réduit la taille en simplifiant les géométries
- **`--min-size-polygon 8`** — Élimine les micro-polygones invisibles
- **`--latin1`** — Active le Format 9 pour les accents français

## Comparaison des formats

| Critère | Polish Map (.mp) | Garmin IMG (.img) |
|---------|-----------------|-------------------|
| Type | Texte (INI) | Binaire |
| Lisible | Oui (éditeur texte) | Non |
| Taille | Volumineux | Compact |
| Éditable | Oui | Non |
| Utilisable sur GPS | Non | Oui |
| Multi-niveaux de zoom | Non (décrit dans le header) | Oui (natif) |
| Routing | Attributs seulement | Topologie complète |

Le format Polish Map est un **format de travail** — on l'inspecte, on le corrige, on le valide. Le format Garmin IMG est un **format de distribution** — optimisé pour l'affichage et la navigation sur appareil embarqué.
