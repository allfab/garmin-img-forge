# Introduction

Bienvenue dans la documentation du projet **GARMIN IGN BDTOPO MAP**.

Ce projet permet de générer des cartes topographiques Garmin personnalisées à partir des données
ouvertes de l'IGN (BD TOPO). L'ensemble de la chaîne de traitement est open-source et automatisée.

## Objectif

Produire des cartes Garmin `.img` prêtes à l'emploi sur GPS, reflétant le territoire français avec
la précision des données IGN — sans dépendance à des logiciels propriétaires.

## Historique

L'ancien workflow reposait sur FME → GPSMapEdit → mkgmap. Le nouveau pipeline 100 % open-source
utilise uniquement :

- **`mpforge`** — découpe les données vecteur en tuiles Polish Map (`.mp`)
- **`imgforge`** — compile les tuiles `.mp` en fichier Garmin `.img`
