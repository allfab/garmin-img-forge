---
description: Référence technique du format Garmin IMG, types de features, correspondances BD TOPO, profils de généralisation, styles TYP et logs mpforge/imgforge.
---

# Référence technique

Cette section rassemble les spécifications techniques du projet : format binaire Garmin IMG, codes types, correspondances BD TOPO, styles, outils et observabilité.

---

## Format & Architecture

<div class="grid cards" markdown>

-   :material-layers-outline: **Format Garmin IMG**

    ---

    Architecture pédagogique du format binaire : FAT, sous-fichiers TRE / RGN / LBL / NET / NOD / DEM, encodage delta, subdivisions, GMP — enrichi des découvertes firmware Alpha 100.

    [:octicons-arrow-right-24: Consulter](format-garmin-img.md)

-   :material-code-tags: **Types Garmin**

    ---

    Codes hexadécimaux pour les POI, routes et polygones du format Polish Map / Garmin IMG.

    [:octicons-arrow-right-24: Consulter](types-garmin.md)

-   :material-alert-outline: **Limites des formats**

    ---

    Contraintes techniques du format Polish Map et du format Garmin IMG (points, niveaux, taille FAT…).

    [:octicons-arrow-right-24: Consulter](limites-formats.md)

</div>

---

## Données & Transformation

<div class="grid cards" markdown>

-   :material-swap-horizontal: **Correspondances BD TOPO**

    ---

    Table de transposition des couches BD TOPO IGN vers les types Garmin, par source_layer.

    [:octicons-arrow-right-24: Consulter](correspondances-bdtopo.md)

-   :material-magnify-plus-outline: **Niveaux de zoom et EndLevel**

    ---

    Correspondance entre `EndLevel` (fichier .mp), `--levels` (imgforge) et les niveaux de zoom GPS — header 7 niveaux `24/23/22/21/20/18/16`.

    [:octicons-arrow-right-24: Consulter](niveaux-et-zoom.md)

-   :material-tune: **Profils de généralisation**

    ---

    Référence du catalogue `generalize-profiles.yaml` : structure YAML, algorithmes Douglas-Peucker / Visvalingam-Whyatt, dispatch conditionnel, profils BDTOPO de production.

    [:octicons-arrow-right-24: Consulter](generalize-profiles.md)

-   :material-layers-triple-outline: **Niveaux de simplification**

    ---

    Les 4 niveaux du moins au plus détaillé : de la simplification maximale (profils mpforge + filtres imgforge) aux données brutes shapefile sans aucun filtre — triplets de commandes pour chaque niveau.

    [:octicons-arrow-right-24: Consulter](niveaux-de-simplification.md)

</div>

---

## Outils & Configuration

<div class="grid cards" markdown>

-   :material-identifier: **Convention `--base-id`**

    ---

    Plages de `base-id` par couverture (départements, régions, quadrants, national) — évite les collisions INSEE / DOM-COM.

    [:octicons-arrow-right-24: Consulter](base-id-convention.md)

-   :material-tag-outline: **Versioning des binaires**

    ---

    Comment `imgforge` et `mpforge` calculent leur version à la compilation : lecture de `--version`, convention de tags, workflow de release.

    [:octicons-arrow-right-24: Consulter](versioning-binaires.md)

</div>

---

## Styles visuels

<div class="grid cards" markdown>

-   :material-palette-outline: **Styles TYP**

    ---

    Référence complète des styles TYP (POI, lignes, polygones) utilisés pour personnaliser le rendu des cartes Garmin — généré depuis `I2023100.typ`.

    [:octicons-arrow-right-24: Consulter](styles-typ.md)

-   :material-image-outline: **Styles OpenTopo**

    ---

    Catalogue des styles OpenTopo (POI, lignes, polygones) — rendu alternatif pour les données OSM.

    [:octicons-arrow-right-24: Consulter](styles-opentopo.md)

</div>

---

## Observabilité

<div class="grid cards" markdown>

-   :material-console-line: **Logs mpforge**

    ---

    Guide de lecture des messages de logs mpforge : niveaux de verbosité, phases du pipeline, avertissements courants, filtrage `RUST_LOG`, rapport JSON.

    [:octicons-arrow-right-24: Consulter](logs-mpforge.md)

-   :material-text-search: **Logs imgforge**

    ---

    Guide de lecture des messages de logs imgforge : niveaux de verbosité, barre de progression, résumé console, avertissements courants, rapport JSON.

    [:octicons-arrow-right-24: Consulter](logs-imgforge.md)

</div>
