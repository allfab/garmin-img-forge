# About

## The Author

**Fabien ALLAMANCHE** (@allfab) — Geomatician at Vienne Condrieu Agglomération.

> *Computing and new technologies have always been my passions.*

### Background

Holder of a DUT in Civil Engineering from IUT A in Lyon (2004), Fabien is self-taught in the field of geomatics. He has over 20 years of professional experience in this sector.

### Areas of Expertise

- **General Geomatician** — Full geographic data management: acquisition, analysis, representation
- **GIS Developer** — Specialized geomatics development and system administration
- **Thematic Geomatician** — Territorial analysis of projects

He also provides project management consulting, technical assistance, and technology watch in research and development.

### Links

- [GitHub](https://github.com/allfab) — GitHub profile
- [Blog](https://f84.allfab.fr) — Personal blog

---

## The Project

**GARMIN IMG FORGE** is a personal project born from the desire to create Garmin topographic maps using exclusively free software and open data.

The project follows an end-to-end **FOSS** (Free and Open Source Software) approach: open data (IGN BD TOPO, Etalab 2.0 license) transformed by open-source tools (ogr-polishmap, mpforge, imgforge) into ready-to-use maps for Garmin GPS devices.

### Inspirations

This work builds on foundations laid by the Garmin cartographic community, notably:

- Articles from **GPSFileDepot** (2008, updated 2016) on creating custom Garmin maps
- The **mkgmap** project — open-source Java compiler that demonstrated it was possible to produce IMG files without proprietary tools
- The **cGPSmapper** documentation — which defined the Polish Map format as the intermediate standard

### Licenses

The project adopts a hybrid licensing model, adapted to the nature of each component:

| Component | License | Reason |
|-----------|---------|--------|
| **ogr-polishmap** | MIT | GDAL driver — compatibility with the GDAL ecosystem (MIT/X), facilitates potential upstream integration |
| **mpforge** | GPL v3 | Standalone tool — copyleft, derivatives must remain open-source |
| **imgforge** | GPL v3 | Garmin IMG compiler — aligned with mkgmap (GPL v2), derivatives must remain open |
| **Documentation / site** | CC BY-SA 4.0 | Standard for documentation, with mandatory attribution |
| **Produced maps** | Etalab 2.0 | Inherited from IGN data (BD TOPO) |

### Contributing

Contributions are welcome:

- **Issues**: [github.com/allfab/garmin-img-forge/issues](https://github.com/allfab/garmin-img-forge/issues)
- **Source code**: [github.com/allfab/garmin-img-forge](https://github.com/allfab/garmin-img-forge)

!!! tip "Former site"
    The former project site remains available at [allfab.github.io/garmin-ign-bdtopo-map](https://allfab.github.io/garmin-ign-bdtopo-map/).
