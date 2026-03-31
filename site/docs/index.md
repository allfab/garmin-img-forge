# Cartes Garmin IGN BD TOPO

**Des cartes topographiques Garmin gratuites, précises et à jour — générées depuis les données ouvertes de l'IGN.**

Ce projet produit des cartes Garmin (`.img`) pour GPS à partir de la **BD TOPO IGN** (données ouvertes, licence Etalab). Le pipeline est entièrement automatisé, open-source et 100 % reproductible.

---

## Télécharger une carte

<div class="grid cards" markdown>

-   :material-map: **France métropolitaine**

    ---

    Cartes par grandes zones (Nord / Sud) ou par région

    [:octicons-arrow-right-24: Accéder aux téléchargements](06-telechargements/france.md)

-   :material-island: **Outre-mer**

    ---

    Guadeloupe, Martinique, Guyane, La Réunion, Mayotte

    [:octicons-arrow-right-24: Outre-mer](06-telechargements/outre-mer.md)

</div>

---

## Comment ça marche ?

```
BD TOPO IGN  →  mpforge build  →  imgforge  →  gmapsupp.img
(données .gpkg)  (tuiles .mp)    (Garmin .img)  (sur GPS)
```

1. **Téléchargement** — `download-bdtopo.sh` récupère les données IGN par département
2. **Tuilage** — `mpforge` découpe et catégorise les objets géographiques en tuiles Polish Map
3. **Compilation** — `imgforge` produit le fichier binaire Garmin
4. **Installation** — copiez `gmapsupp.img` sur la carte SD de votre GPS

---

!!! info "Données sources"
    Les cartes sont générées depuis la **BD TOPO IGN** — mise à jour semestrielle, précision métrique,
    couvrant l'ensemble du territoire français. Licence ouverte [Etalab 2.0](https://www.etalab.gouv.fr/licence-ouverte-open-licence).

!!! tip "Ancien site"
    L'ancien site reste disponible à l'adresse [https://allfab.github.io/garmin-ign-bdtopo-map/](https://allfab.github.io/garmin-ign-bdtopo-map/).
