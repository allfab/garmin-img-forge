// Types for Polish Map (.mp) file format

use crate::img::coord::Coord;
use std::collections::BTreeMap;

/// Unit declared by `Elevation=` in the [IMG ID] header.
/// Garmin IMG always stores contour labels in feet; Metres triggers m→ft conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ElevationUnit {
    #[default]
    Feet,
    Metres,
}

/// Complete parsed .mp file
#[derive(Debug, Clone)]
pub struct MpFile {
    pub header: MpHeader,
    pub points: Vec<MpPoint>,
    pub polylines: Vec<MpPolyline>,
    pub polygons: Vec<MpPolygon>,
}

/// Routing generation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingMode {
    /// Auto-detect: generate NET+NOD if road_id present
    Auto,
    /// Force NET+NOD generation
    Route,
    /// Generate NET only (address search, no turn-by-turn)
    NetOnly,
    /// Disable routing entirely
    Disabled,
}

impl Default for RoutingMode {
    fn default() -> Self {
        RoutingMode::Auto
    }
}

/// [IMG ID] section header
#[derive(Debug, Clone, Default)]
pub struct MpHeader {
    pub id: u32,
    pub name: String,
    pub copyright: String,
    pub levels: Vec<u8>,       // resolution per level
    pub codepage: u16,
    pub datum: String,
    pub transparent: bool,
    pub draw_priority: u32,
    pub preview_lat: f64,
    pub preview_lon: f64,
    // Encoding
    pub lower_case: bool,
    // Rendering
    pub order_by_decreasing_area: bool,
    // Geometry optimization
    pub reduce_point_density: Option<f64>,
    pub simplify_polygons: Option<String>,
    pub min_size_polygon: Option<i32>,
    pub merge_lines: bool,
    // Geometry filter opt-out (mkgmap parité — désactivables pour analyse/debug)
    pub no_round_coords: bool,
    pub no_size_filter: bool,
    pub no_remove_obsolete_points: bool,
    // Routing
    pub routing_mode: RoutingMode,
    // Elevation
    pub elevation_unit: ElevationUnit,
    // Tile geographic bounds written by mpforge (includes grid overlap).
    // Format: [south, west, north, east] in WGS84 degrees.
    // Used by imgforge DEM encoder to ensure full-tile elevation coverage.
    pub tile_bounds: Option<[f64; 4]>,
    // Metadata (TDB-level)
    pub country_name: String,
    pub country_abbr: String,
    pub region_name: String,
    pub region_abbr: String,
    pub product_version: u16,
}

/// [POI] section
#[derive(Debug, Clone)]
pub struct MpPoint {
    pub type_code: u32,
    pub label: String,
    pub coord: Coord,
    pub end_level: Option<u8>,
}

/// [POLYLINE] section
#[derive(Debug, Clone)]
pub struct MpPolyline {
    pub type_code: u32,
    pub label: String,
    /// Géométries indexées par niveau Garmin (`N` de `DataN=`).
    /// 0 = niveau le plus détaillé. Spec MP §4.4.3.1.
    pub geometries: BTreeMap<u8, Vec<Coord>>,
    pub end_level: Option<u8>,
    pub direction: bool,
    pub road_id: Option<u32>,
    pub route_param: Option<String>,
}

impl MpPolyline {
    /// Politique B (rendu) : bucket exact, sinon plus grossier (N > level),
    /// sinon plus détaillé (N < level), sinon vide.
    pub fn geometry_for_level(&self, level: u8) -> &[Coord] {
        if let Some(g) = self.geometries.get(&level) {
            return g.as_slice();
        }
        if let Some(next_level) = level.checked_add(1) {
            if let Some((_, g)) = self.geometries.range(next_level..).next() {
                return g.as_slice();
            }
        }
        if let Some((_, g)) = self.geometries.range(..level).next_back() {
            return g.as_slice();
        }
        &[]
    }

    /// Union de tous les buckets (pour `compute_bounds`).
    pub fn all_coords(&self) -> impl Iterator<Item = &Coord> {
        self.geometries.values().flat_map(|v| v.iter())
    }

    /// Routing : strict `Data0`, sans fallback. `None` ⇒ feature exclue du graphe.
    pub fn routing_geometry(&self) -> Option<&[Coord]> {
        self.geometries.get(&0).map(Vec::as_slice)
    }
}

/// [POLYGON] section
#[derive(Debug, Clone)]
pub struct MpPolygon {
    pub type_code: u32,
    pub label: String,
    pub geometries: BTreeMap<u8, Vec<Coord>>,
    pub end_level: Option<u8>,
}

impl MpPolygon {
    pub fn geometry_for_level(&self, level: u8) -> &[Coord] {
        if let Some(g) = self.geometries.get(&level) {
            return g.as_slice();
        }
        if let Some(next_level) = level.checked_add(1) {
            if let Some((_, g)) = self.geometries.range(next_level..).next() {
                return g.as_slice();
            }
        }
        if let Some((_, g)) = self.geometries.range(..level).next_back() {
            return g.as_slice();
        }
        &[]
    }

    pub fn all_coords(&self) -> impl Iterator<Item = &Coord> {
        self.geometries.values().flat_map(|v| v.iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(lat: i32, lon: i32) -> Coord {
        Coord::new(lat, lon)
    }

    fn pl_with(buckets: &[(u8, Vec<Coord>)]) -> MpPolyline {
        let mut g = BTreeMap::new();
        for (n, v) in buckets {
            g.insert(*n, v.clone());
        }
        MpPolyline {
            type_code: 0,
            label: String::new(),
            geometries: g,
            end_level: None,
            direction: false,
            road_id: None,
            route_param: None,
        }
    }

    fn pg_with(buckets: &[(u8, Vec<Coord>)]) -> MpPolygon {
        let mut g = BTreeMap::new();
        for (n, v) in buckets {
            g.insert(*n, v.clone());
        }
        MpPolygon {
            type_code: 0,
            label: String::new(),
            geometries: g,
            end_level: None,
        }
    }

    #[test]
    fn geometry_for_level_exact_bucket() {
        let pl = pl_with(&[(0, vec![c(1, 1), c(2, 2)]), (2, vec![c(1, 1)])]);
        assert_eq!(pl.geometry_for_level(0).len(), 2);
        assert_eq!(pl.geometry_for_level(2).len(), 1);
    }

    #[test]
    fn geometry_for_level_fallback_coarser_first() {
        // Politique B: niveau 1 absent, on prend D'ABORD le plus grossier disponible (2).
        let pl = pl_with(&[(0, vec![c(1, 1), c(2, 2), c(3, 3)]), (2, vec![c(1, 1), c(3, 3)])]);
        assert_eq!(pl.geometry_for_level(1).len(), 2, "fallback to coarser N=2");
    }

    #[test]
    fn geometry_for_level_fallback_to_finer_when_no_coarser() {
        // niveau 3 demandé, max disponible est 2 → retombe sur 2.
        let pl = pl_with(&[(0, vec![c(1, 1), c(2, 2)]), (2, vec![c(1, 1)])]);
        assert_eq!(pl.geometry_for_level(3).len(), 1);
    }

    #[test]
    fn geometry_for_level_empty() {
        let pl = pl_with(&[]);
        assert!(pl.geometry_for_level(0).is_empty());
    }

    #[test]
    fn geometry_for_level_only_n2_request_n0() {
        // Demande niveau 0, seul bucket 2 présent → fallback vers plus grossier (2).
        let pl = pl_with(&[(2, vec![c(1, 1), c(2, 2)])]);
        assert_eq!(pl.geometry_for_level(0).len(), 2);
    }

    #[test]
    fn routing_geometry_strict_data0() {
        let pl_with_d0 = pl_with(&[(0, vec![c(1, 1), c(2, 2)])]);
        assert!(pl_with_d0.routing_geometry().is_some());
        let pl_no_d0 = pl_with(&[(2, vec![c(1, 1)])]);
        assert!(pl_no_d0.routing_geometry().is_none());
    }

    #[test]
    fn all_coords_unions_all_buckets() {
        let pl = pl_with(&[(0, vec![c(1, 1), c(2, 2)]), (2, vec![c(3, 3)])]);
        assert_eq!(pl.all_coords().count(), 3);
    }

    #[test]
    fn geometry_for_level_polygon_symmetric() {
        let pg = pg_with(&[(0, vec![c(1, 1), c(2, 2), c(3, 3)]), (2, vec![c(1, 1), c(3, 3)])]);
        assert_eq!(pg.geometry_for_level(0).len(), 3);
        assert_eq!(pg.geometry_for_level(1).len(), 2);
        assert_eq!(pg.geometry_for_level(2).len(), 2);
        assert_eq!(pg.all_coords().count(), 5);
    }
}
