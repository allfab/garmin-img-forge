# Limites des formats

## Limitations du format Polish Map

- Géométries simples uniquement (pas de MultiPolygon)
- Coordonnées en degrés décimaux WGS84
- Maximum 1024 points par polyligne
- Encodage CP1252 pour les caractères accentués

## Limitations Garmin IMG

- Pas de rendu vectoriel sub-pixel (tiles à résolution fixe)
- Routage limité aux routes avec attribut `RouteParam`
- Taille maximale d'un fichier `.img` : ~4 Go

## Données non reprises

Certaines couches BD TOPO ne sont pas (encore) intégrées :
- Courbes de niveau (nécessite le RGE ALTI)
- Réseau électrique haute tension
- Zones réglementées détaillées
