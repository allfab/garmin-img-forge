//! Road network graph structures for Garmin NET/NOD encoding.
//!
//! Story 14.2: Defines RouteNode, RouteArc, RoadDef, RoadNetwork and
//! RouteParam parsing for building a routable graph from Polish Map polylines.

pub mod graph_builder;

/// A node in the road network graph (intersection or endpoint).
#[derive(Debug, Clone)]
pub struct RouteNode {
    pub id: u32,
    pub coord: (f64, f64),
    pub level: i32,
    pub arcs: Vec<u32>,
}

/// A directed arc in the road network graph.
#[derive(Debug, Clone)]
pub struct RouteArc {
    pub id: u32,
    pub from_node: u32,
    pub to_node: u32,
    pub road_def_idx: usize,
    pub forward: bool,
    pub length_meters: f32,
    pub bearing_degrees: f32,
}

/// A road definition (one per polyline), consumed by NET Writer (Story 14.3).
#[derive(Debug, Clone)]
pub struct RoadDef {
    pub road_id: u32,
    pub polyline_idx: usize,
    pub speed: u8,
    pub road_class: u8,
    pub one_way: bool,
    pub toll: bool,
    pub roundabout: bool,
    pub access_mask: u16,
    pub label: Option<String>,
}

/// The complete road network graph.
#[derive(Debug, Clone)]
pub struct RoadNetwork {
    pub nodes: Vec<RouteNode>,
    pub arcs: Vec<RouteArc>,
    pub road_defs: Vec<RoadDef>,
}

/// Parsed RouteParam fields from the compact string format.
///
/// Format: `speed,road_class,one_way,toll,denied_emergency,denied_delivery,
///          denied_car,denied_bus,denied_taxi,denied_pedestrian,denied_bicycle,denied_truck`
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedRouteParam {
    pub speed: u8,
    pub road_class: u8,
    pub one_way: bool,
    pub toll: bool,
    pub denied_emergency: bool,
    pub denied_delivery: bool,
    pub denied_car: bool,
    pub denied_bus: bool,
    pub denied_taxi: bool,
    pub denied_pedestrian: bool,
    pub denied_bicycle: bool,
    pub denied_truck: bool,
}

impl ParsedRouteParam {
    /// Parse a RouteParam string into typed fields.
    ///
    /// Follows mkgmap RoadHelper.java convention: fields beyond the string
    /// length default to 0/false. Speed is clamped to 0-7, road_class to 0-4.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        let fields: Vec<&str> = s.split(',').collect();

        let get_u8 = |i: usize| -> u8 {
            fields.get(i).and_then(|v| v.parse().ok()).unwrap_or(0)
        };
        let get_bool = |i: usize| -> bool {
            fields.get(i).and_then(|v| v.parse::<u8>().ok()).unwrap_or(0) > 0
        };

        Some(Self {
            speed: get_u8(0).min(7),
            road_class: get_u8(1).min(4),
            one_way: get_bool(2),
            toll: get_bool(3),
            denied_emergency: get_bool(4),
            denied_delivery: get_bool(5),
            denied_car: get_bool(6),
            denied_bus: get_bool(7),
            denied_taxi: get_bool(8),
            denied_pedestrian: get_bool(9),
            denied_bicycle: get_bool(10),
            denied_truck: get_bool(11),
        })
    }

    /// Compute NET Table A access restriction mask from denied bits.
    ///
    /// Mapping follows mkgmap RoadDef.java:
    /// - No Car:       0x0001
    /// - No Bus:       0x0002
    /// - No Taxi:      0x0004
    /// - No Foot:      0x0010
    /// - No Bike:      0x0020
    /// - No Truck:     0x0040
    /// - No Delivery:  0x4000
    /// - No Emergency: 0x8000
    pub fn access_mask(&self) -> u16 {
        let mut mask: u16 = 0;
        if self.denied_car {
            mask |= 0x0001;
        }
        if self.denied_bus {
            mask |= 0x0002;
        }
        if self.denied_taxi {
            mask |= 0x0004;
        }
        if self.denied_pedestrian {
            mask |= 0x0010;
        }
        if self.denied_bicycle {
            mask |= 0x0020;
        }
        if self.denied_truck {
            mask |= 0x0040;
        }
        if self.denied_delivery {
            mask |= 0x4000;
        }
        if self.denied_emergency {
            mask |= 0x8000;
        }
        mask
    }
}

/// Compute haversine distance between two WGS84 points (in meters).
pub fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R: f64 = 6_371_000.0;
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    R * 2.0 * a.sqrt().atan2((1.0 - a).sqrt())
}

/// Compute initial bearing from point 1 to point 2 (in degrees, 0-360).
pub fn initial_bearing(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f32 {
    let lat1 = lat1.to_radians();
    let lat2 = lat2.to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let y = dlon.sin() * lat2.cos();
    let x = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * dlon.cos();
    ((y.atan2(x).to_degrees() + 360.0) % 360.0) as f32
}

/// Compute polyline length in meters by summing haversine distances between consecutive points.
pub fn polyline_length(coords: &[(f64, f64)]) -> f64 {
    coords
        .windows(2)
        .map(|w| haversine_distance(w[0].0, w[0].1, w[1].0, w[1].1))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Task 1.2: RouteParam parsing
    // =========================================================================

    #[test]
    fn test_parse_route_param_full() {
        let rp = ParsedRouteParam::parse("6,3,1,1,0,0,0,0,0,0,0,0").unwrap();
        assert_eq!(rp.speed, 6);
        assert_eq!(rp.road_class, 3);
        assert!(rp.one_way);
        assert!(rp.toll);
        assert!(!rp.denied_car);
        assert!(!rp.denied_truck);
    }

    #[test]
    fn test_parse_route_param_minimal() {
        let rp = ParsedRouteParam::parse("4,2").unwrap();
        assert_eq!(rp.speed, 4);
        assert_eq!(rp.road_class, 2);
        assert!(!rp.one_way);
        assert!(!rp.toll);
    }

    #[test]
    fn test_parse_route_param_all_denied() {
        let rp = ParsedRouteParam::parse("3,2,0,0,1,1,1,1,1,1,1,1").unwrap();
        assert!(rp.denied_emergency);
        assert!(rp.denied_delivery);
        assert!(rp.denied_car);
        assert!(rp.denied_bus);
        assert!(rp.denied_taxi);
        assert!(rp.denied_pedestrian);
        assert!(rp.denied_bicycle);
        assert!(rp.denied_truck);
    }

    #[test]
    fn test_parse_route_param_clamp_speed() {
        let rp = ParsedRouteParam::parse("9,6").unwrap();
        assert_eq!(rp.speed, 7);
        assert_eq!(rp.road_class, 4);
    }

    #[test]
    fn test_parse_route_param_empty() {
        assert!(ParsedRouteParam::parse("").is_none());
    }

    // =========================================================================
    // Task 1.3: Access mask computation
    // =========================================================================

    #[test]
    fn test_access_mask_no_restrictions() {
        let rp = ParsedRouteParam::parse("6,3,0,0,0,0,0,0,0,0,0,0").unwrap();
        assert_eq!(rp.access_mask(), 0x0000);
    }

    #[test]
    fn test_access_mask_no_car() {
        let rp = ParsedRouteParam::parse("3,2,0,0,0,0,1,0,0,0,0,0").unwrap();
        assert_eq!(rp.access_mask(), 0x0001);
    }

    #[test]
    fn test_access_mask_all_denied() {
        let rp = ParsedRouteParam::parse("3,2,0,0,1,1,1,1,1,1,1,1").unwrap();
        // car=0x0001, bus=0x0002, taxi=0x0004, foot=0x0010, bike=0x0020, truck=0x0040,
        // delivery=0x4000, emergency=0x8000
        assert_eq!(rp.access_mask(), 0x0001 | 0x0002 | 0x0004 | 0x0010 | 0x0020 | 0x0040 | 0x4000 | 0x8000);
    }

    #[test]
    fn test_access_mask_foot_only() {
        let rp = ParsedRouteParam::parse("3,2,0,0,0,0,0,0,0,1,0,0").unwrap();
        assert_eq!(rp.access_mask(), 0x0010);
    }

    #[test]
    fn test_access_mask_emergency_delivery() {
        let rp = ParsedRouteParam::parse("3,2,0,0,1,1,0,0,0,0,0,0").unwrap();
        assert_eq!(rp.access_mask(), 0x8000 | 0x4000);
    }

    // =========================================================================
    // Task 1.4: Manual construction of structures
    // =========================================================================

    #[test]
    fn test_route_node_construction() {
        let node = RouteNode {
            id: 0,
            coord: (45.0, 5.0),
            level: 0,
            arcs: vec![0, 1],
        };
        assert_eq!(node.id, 0);
        assert_eq!(node.coord, (45.0, 5.0));
        assert_eq!(node.arcs.len(), 2);
    }

    #[test]
    fn test_route_arc_construction() {
        let arc = RouteArc {
            id: 0,
            from_node: 0,
            to_node: 1,
            road_def_idx: 0,
            forward: true,
            length_meters: 150.0,
            bearing_degrees: 45.0,
        };
        assert!(arc.forward);
        assert_eq!(arc.from_node, 0);
        assert_eq!(arc.to_node, 1);
    }

    #[test]
    fn test_road_def_construction() {
        let def = RoadDef {
            road_id: 1,
            polyline_idx: 0,
            speed: 6,
            road_class: 3,
            one_way: true,
            toll: true,
            roundabout: false,
            access_mask: 0x0000,
            label: Some("A6".to_string()),
        };
        assert!(def.one_way);
        assert!(def.toll);
        assert!(!def.roundabout);
        assert_eq!(def.label.as_deref(), Some("A6"));
    }

    #[test]
    fn test_road_network_empty() {
        let net = RoadNetwork {
            nodes: vec![],
            arcs: vec![],
            road_defs: vec![],
        };
        assert_eq!(net.nodes.len(), 0);
        assert_eq!(net.arcs.len(), 0);
        assert_eq!(net.road_defs.len(), 0);
    }

    // =========================================================================
    // Haversine and bearing tests
    // =========================================================================

    #[test]
    fn test_haversine_same_point() {
        let d = haversine_distance(45.0, 5.0, 45.0, 5.0);
        assert!(d < 0.001, "same point distance should be ~0");
    }

    #[test]
    fn test_haversine_known_distance() {
        // Paris (48.8566, 2.3522) to Lyon (45.7640, 4.8357) ≈ 392 km
        let d = haversine_distance(48.8566, 2.3522, 45.7640, 4.8357);
        assert!((d - 392_000.0).abs() < 5000.0, "Paris-Lyon ≈ 392km, got {d}");
    }

    #[test]
    fn test_initial_bearing_north() {
        let b = initial_bearing(45.0, 5.0, 46.0, 5.0);
        assert!((b - 0.0).abs() < 1.0, "due north ≈ 0°, got {b}");
    }

    #[test]
    fn test_initial_bearing_east() {
        let b = initial_bearing(0.0, 0.0, 0.0, 1.0);
        assert!((b - 90.0).abs() < 1.0, "due east ≈ 90°, got {b}");
    }

    #[test]
    fn test_polyline_length_two_points() {
        let coords = vec![(45.0, 5.0), (45.001, 5.0)];
        let len = polyline_length(&coords);
        // 0.001° lat ≈ 111m
        assert!((len - 111.0).abs() < 2.0, "expected ~111m, got {len}");
    }

    #[test]
    fn test_polyline_length_single_point() {
        let coords = vec![(45.0, 5.0)];
        assert_eq!(polyline_length(&coords), 0.0);
    }
}
