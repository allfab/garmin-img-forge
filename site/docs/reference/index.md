# Référence technique

Cette section rassemble les spécifications techniques du projet : codes types Garmin, correspondances avec la BD TOPO IGN, architecture du format IMG et limites connues des formats.

---

<div class="grid cards" markdown>

-   **Types Garmin**

    ---

    Codes hexadécimaux pour les POI, routes et polygones du format Polish Map / Garmin IMG.

    [:octicons-arrow-right-24: Consulter](types-garmin.md)

-   **Correspondances BD TOPO**

    ---

    Table de transposition des couches BD TOPO IGN vers les types Garmin.

    [:octicons-arrow-right-24: Consulter](correspondances-bdtopo.md)

-   **Niveaux de zoom et EndLevel**

    ---

    Comprendre la correspondance entre `EndLevel` (fichier .mp), `--levels` (imgforge) et les niveaux de zoom GPS.

    [:octicons-arrow-right-24: Consulter](niveaux-et-zoom.md)

-   **Limites des formats**

    ---

    Contraintes techniques du format Polish Map et du format Garmin IMG.

    [:octicons-arrow-right-24: Consulter](limites-formats.md)

-   **Styles TYP**

    ---

    Référence complète des styles TYP (POI, lignes, polygones) utilisés pour personnaliser le rendu des cartes Garmin.

    [:octicons-arrow-right-24: Consulter](styles-typ.md)

-   **Convention `--base-id`**

    ---

    Plages de `base-id` par couverture (départements, régions, quadrants, national) — évite les collisions INSEE/DOM-COM.

    [:octicons-arrow-right-24: Consulter](base-id-convention.md)

-   **Versioning des binaires**

    ---

    Comment `imgforge` et `mpforge` calculent leur version à la compilation : lecture de `--version`, convention de tags, workflow de release.

    [:octicons-arrow-right-24: Consulter](versioning-binaires.md)

-   **Format Garmin IMG**

    ---

    Architecture pédagogique du format binaire Garmin IMG : FAT, sous-fichiers TRE/RGN/LBL/NET/NOD/DEM, encodage delta, subdivision, et chaîne Polish Map → IMG.

    [:octicons-arrow-right-24: Consulter](format-garmin-img.md)

-   **Profils de généralisation**

    ---

    Référence du catalogue `generalize-profiles.yaml` : structure YAML, algorithmes Douglas-Peucker / Visvalingam-Whyatt, dispatch conditionnel, profils BDTOPO de production.

    [:octicons-arrow-right-24: Consulter](generalize-profiles.md)

-   **Logs mpforge**

    ---

    Guide de lecture des messages de logs mpforge : niveaux de verbosité, phases du pipeline, avertissements courants, filtrage RUST\_LOG, rapport JSON.

    [:octicons-arrow-right-24: Consulter](logs-mpforge.md)

</div>
