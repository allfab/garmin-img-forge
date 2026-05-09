---
description: Technical reference for the Garmin IMG format, feature types, BD TOPO mappings, generalization profiles, TYP styles and mpforge/imgforge logs.
---

# Technical Reference

This section gathers the technical specifications of the project: Garmin IMG binary format, type codes, BD TOPO mappings, styles, tools and observability.

---

## Format & Architecture

<div class="grid cards" markdown>

-   :material-layers-outline: **Garmin IMG Format**

    ---

    Pedagogical architecture of the binary format: FAT, TRE / RGN / LBL / NET / NOD / DEM sub-files, delta encoding, subdivisions, GMP — enriched with Alpha 100 firmware discoveries.

    [:octicons-arrow-right-24: View](garmin-img-format.md)

-   :material-code-tags: **Garmin Types**

    ---

    Hexadecimal codes for POI, roads and polygons in the Polish Map / Garmin IMG format.

    [:octicons-arrow-right-24: View](garmin-types.md)

-   :material-alert-outline: **Format Limits**

    ---

    Technical constraints of the Polish Map format and Garmin IMG format (points, levels, FAT size...).

    [:octicons-arrow-right-24: View](format-limits.md)

</div>

---

## Data & Transformation

<div class="grid cards" markdown>

-   :material-swap-horizontal: **BD TOPO Mappings**

    ---

    Transposition table of IGN BD TOPO layers to Garmin types, by source_layer.

    [:octicons-arrow-right-24: View](bdtopo-mappings.md)

-   :material-magnify-plus-outline: **Zoom Levels and EndLevel**

    ---

    Correspondence between `EndLevel` (.mp file), `--levels` (imgforge) and GPS zoom levels — 7-level header `24/23/22/21/20/18/16`.

    [:octicons-arrow-right-24: View](zoom-levels.md)

-   :material-tune: **Generalization Profiles**

    ---

    Reference for the `generalize-profiles.yaml` catalog: YAML structure, Douglas-Peucker / Visvalingam-Whyatt algorithms, conditional dispatch, production BDTOPO profiles.

    [:octicons-arrow-right-24: View](generalize-profiles.md)

-   :material-layers-triple-outline: **Simplification Levels**

    ---

    The 4 levels from least to most detailed: from maximum simplification (mpforge profiles + imgforge filters) to raw shapefile data without any filter — command triplets for each level.

    [:octicons-arrow-right-24: View](simplification-levels.md)

</div>

---

## Tools & Configuration

<div class="grid cards" markdown>

-   :material-identifier: **`--base-id` Convention**

    ---

    `base-id` ranges by coverage (departments, regions, quadrants, national) — avoids INSEE / DOM-COM collisions.

    [:octicons-arrow-right-24: View](base-id-convention.md)

-   :material-tag-outline: **Binary Versioning**

    ---

    How `imgforge` and `mpforge` compute their version at compilation: reading `--version`, tag convention, release workflow.

    [:octicons-arrow-right-24: View](binary-versioning.md)

</div>

---

## Visual Styles

<div class="grid cards" markdown>

-   :material-palette-outline: **TYP Styles**

    ---

    Complete reference of TYP styles (POI, lines, polygons) used to customize the rendering of Garmin maps — generated from `I2023100.typ`.

    [:octicons-arrow-right-24: View](typ-styles.md)

-   :material-image-outline: **OpenTopo Styles**

    ---

    Catalog of OpenTopo styles (POI, lines, polygons) — alternative rendering for OSM data.

    [:octicons-arrow-right-24: View](opentopo-styles.md)

</div>

---

## Observability

<div class="grid cards" markdown>

-   :material-console-line: **mpforge Logs**

    ---

    Guide to reading mpforge log messages: verbosity levels, pipeline phases, common warnings, `RUST_LOG` filtering, JSON report.

    [:octicons-arrow-right-24: View](mpforge-logs.md)

-   :material-text-search: **imgforge Logs**

    ---

    Guide to reading imgforge log messages: verbosity levels, progress bar, console summary, common warnings, JSON report.

    [:octicons-arrow-right-24: View](imgforge-logs.md)

</div>
