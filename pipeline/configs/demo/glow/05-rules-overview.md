# garmin-rules.yaml

Fichier de **mapping BDTOPO → types Garmin**.
Il définit comment chaque objet géographique est traduit en symbologie sur la carte GPS.

## Principe

Chaque **ruleset** est associé à une couche source BDTOPO.
Pour chaque objet, les règles sont évaluées de haut en bas :
la première règle dont le **match** est satisfait s'applique.

```
Objet BDTOPO ──► match champ/valeur ──► set Type Garmin + EndLevel + Label
```

## Rulesets de la configuration démo

| Couche BDTOPO | Géométrie | Règles |
|---------------|-----------|--------|
| TRONCON_DE_ROUTE | Polyligne | Autoroute, Nationale, Chemin, Sentier, défaut |
| TRONCON_HYDROGRAPHIQUE | Polyligne | Permanent, défaut |
| BATIMENT | Polygone | Religieux, Industriel, défaut |
| TOPONYMIE | Point | Sommet, Lac, Forêt… |

> La configuration de production contient **plus de 200 règles** couvrant l'ensemble de la BDTOPO.
