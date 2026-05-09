# sources.yaml

Central configuration file for **mpforge**.
It describes the geographic data to load, how to split it into tiles and the metadata of the final map.

## File Structure

| Section | Role |
|---------|------|
| `grid` | Tile size and overlap |
| `inputs` | List of SHP/GPKG layers to ingest |
| `output` | Directory and naming of `.mp` files |
| `header` | Polish Map metadata (zoom levels) |
| `rules` | Path to Garmin mapping rules |
| `generalize_profiles_path` | Geometric simplification catalog |

## Input layers — Geographic sources

| Theme | BDTOPO / other layers |
|-------|----------------------|
| Transport | Roads, Railways, Cables, Aerodromes |
| Administrative | Municipalities, Settlement areas |
| Hydrography | Segments, Surfaces, Details |
| Buildings | Buildings, Cemeteries, Constructions… |
| Vegetation | Vegetation zones |
| Contour lines | 10 m contours (filtered from 5 m IGN) |
| OSM | Amenity POIs, Natural spots, Tourism |
| Cadastre | Private pools |
| Hiking | Grande Randonnée trails |
