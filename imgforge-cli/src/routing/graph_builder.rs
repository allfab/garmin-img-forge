//! Graph builder: constructs a RoadNetwork from parsed Polish Map polylines.
//!
//! Story 14.2: 4-phase algorithm:
//! 1. Spatial indexing of endpoints (quantized HashMap)
//! 2. RouteNode creation (one per unique position+level)
//! 3. RouteArc creation (forward/reverse per one_way/dir_indicator) + length/bearing computation
//! 4. Assembly of final RoadNetwork

use std::collections::HashMap;

use crate::parser::mp_types::MpPolyline;
use crate::routing::{
    initial_bearing, polyline_length, ParsedRouteParam, RoadDef, RoadNetwork, RouteArc, RouteNode,
};

/// Default snap tolerance (~0.01m, matching BDTOPO native topology precision).
pub const DEFAULT_SNAP_TOLERANCE_M: f64 = 0.01;

/// Key for spatial grouping of endpoints: (quantized_lat, quantized_lon, level).
type EndpointKey = (i64, i64, i32);

/// Compute quantization factor from snap tolerance in meters.
/// Uses latitude-based approximation: 1° latitude ≈ 111,320 m.
fn quant_factor_from_tolerance(tolerance_m: f64) -> f64 {
    111_320.0 / tolerance_m
}

/// Quantize a coordinate to the grid.
fn quantize(lat: f64, lon: f64, factor: f64) -> (i64, i64) {
    (
        (lat * factor).round() as i64,
        (lon * factor).round() as i64,
    )
}

/// Extract the level (POS_SOL) from a polyline's other_fields.
fn get_level(polyline: &MpPolyline) -> i32 {
    polyline
        .other_fields
        .get("Level")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// Build a road network graph from routable polylines using default snap tolerance (~0.01m).
///
/// Only polylines with `routing.is_some()` are included.
pub fn build_road_network(polylines: &[MpPolyline]) -> RoadNetwork {
    build_road_network_with_tolerance(polylines, DEFAULT_SNAP_TOLERANCE_M)
}

/// Build a road network graph with configurable snap tolerance.
///
/// `snap_tolerance_m`: endpoints within this distance (meters) are merged into one node.
/// Uses grid-based quantization — actual merge radius is approximate.
pub fn build_road_network_with_tolerance(
    polylines: &[MpPolyline],
    snap_tolerance_m: f64,
) -> RoadNetwork {
    let quant_factor = quant_factor_from_tolerance(snap_tolerance_m);

    let routable: Vec<(usize, &MpPolyline)> = polylines
        .iter()
        .enumerate()
        .filter(|(_, p)| p.routing.is_some() && p.coords.len() >= 2)
        .collect();

    // Phase 1: Spatial indexing of endpoints
    let mut endpoint_groups: HashMap<EndpointKey, Vec<(usize, bool)>> = HashMap::new();
    // (polyline_index_in_routable, is_last_point)

    for (ri, (_, polyline)) in routable.iter().enumerate() {
        let level = get_level(polyline);
        let first = polyline.coords[0];
        let last = *polyline.coords.last().unwrap();

        let (qlat, qlon) = quantize(first.0, first.1, quant_factor);
        endpoint_groups
            .entry((qlat, qlon, level))
            .or_default()
            .push((ri, false));

        let (qlat, qlon) = quantize(last.0, last.1, quant_factor);
        endpoint_groups
            .entry((qlat, qlon, level))
            .or_default()
            .push((ri, true));
    }

    // Phase 2: Create RouteNodes
    let mut nodes: Vec<RouteNode> = Vec::new();
    // Map from endpoint key to node ID
    let mut key_to_node: HashMap<EndpointKey, u32> = HashMap::new();

    for (key, endpoints) in &endpoint_groups {
        let node_id = nodes.len() as u32;
        // Use the first endpoint's actual coordinates for the node
        let (ri, is_last) = endpoints[0];
        let polyline = routable[ri].1;
        let coord = if is_last {
            *polyline.coords.last().unwrap()
        } else {
            polyline.coords[0]
        };

        nodes.push(RouteNode {
            id: node_id,
            coord,
            level: key.2,
            arcs: Vec::new(),
        });
        key_to_node.insert(*key, node_id);
    }

    // Phase 3: Create RouteArcs and RoadDefs (includes length/bearing computation)
    let mut arcs: Vec<RouteArc> = Vec::new();
    let mut road_defs: Vec<RoadDef> = Vec::new();

    for (orig_idx, polyline) in &routable {
        let routing = polyline.routing.as_ref().unwrap();
        let level = get_level(polyline);

        // Parse RouteParam
        let parsed = routing
            .route_param
            .as_ref()
            .and_then(|rp| ParsedRouteParam::parse(rp));

        let (speed, road_class, one_way, toll, access_mask) = match &parsed {
            Some(p) => (p.speed, p.road_class, p.one_way, p.toll, p.access_mask()),
            None => (0, 0, false, false, 0),
        };

        let dir_indicator = routing.dir_indicator.unwrap_or(0);
        let roundabout = routing.roundabout.unwrap_or(false);
        let road_id = routing
            .road_id
            .as_ref()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        // Create RoadDef
        let road_def_idx = road_defs.len();
        road_defs.push(RoadDef {
            road_id,
            polyline_idx: *orig_idx,
            speed,
            road_class,
            one_way,
            toll,
            roundabout,
            access_mask,
            label: polyline.label.clone(),
        });

        // Find from/to nodes
        let first = polyline.coords[0];
        let last = *polyline.coords.last().unwrap();
        let (qlat_f, qlon_f) = quantize(first.0, first.1, quant_factor);
        let (qlat_l, qlon_l) = quantize(last.0, last.1, quant_factor);

        let from_node_id = key_to_node[&(qlat_f, qlon_f, level)];
        let to_node_id = key_to_node[&(qlat_l, qlon_l, level)];

        // Compute length and bearings for this arc
        let length = polyline_length(&polyline.coords) as f32;

        let forward_bearing = if polyline.coords.len() >= 2 {
            initial_bearing(
                polyline.coords[0].0,
                polyline.coords[0].1,
                polyline.coords[1].0,
                polyline.coords[1].1,
            )
        } else {
            0.0
        };

        let reverse_bearing = if polyline.coords.len() >= 2 {
            let n = polyline.coords.len();
            initial_bearing(
                polyline.coords[n - 1].0,
                polyline.coords[n - 1].1,
                polyline.coords[n - 2].0,
                polyline.coords[n - 2].1,
            )
        } else {
            0.0
        };

        // Create arcs based on direction
        let create_forward = match dir_indicator {
            -1 => false, // Sens inverse: only reverse
            _ => true,   // 0 (bidirectional) or 1 (forward only)
        };
        let create_reverse = match dir_indicator {
            1 => false,    // Sens direct: only forward
            -1 => true,    // Sens inverse: only reverse
            _ => !one_way, // 0: reverse if bidirectional
        };

        if create_forward {
            let arc_id = arcs.len() as u32;
            arcs.push(RouteArc {
                id: arc_id,
                from_node: from_node_id,
                to_node: to_node_id,
                road_def_idx,
                forward: true,
                length_meters: length,
                bearing_degrees: forward_bearing,
            });
            nodes[from_node_id as usize].arcs.push(arc_id);
        }

        if create_reverse {
            let arc_id = arcs.len() as u32;
            arcs.push(RouteArc {
                id: arc_id,
                from_node: to_node_id,
                to_node: from_node_id,
                road_def_idx,
                forward: false,
                length_meters: length,
                bearing_degrees: reverse_bearing,
            });
            nodes[to_node_id as usize].arcs.push(arc_id);
        }
    }

    RoadNetwork {
        nodes,
        arcs,
        road_defs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::mp_types::{MpPolyline, MpRoutingAttrs};
    use std::collections::HashMap;

    /// Helper to create a routable polyline with given coordinates and routing params.
    fn make_polyline(
        coords: Vec<(f64, f64)>,
        route_param: &str,
        dir_indicator: i32,
        road_id: u32,
        level: i32,
        roundabout: bool,
    ) -> MpPolyline {
        let mut other_fields = HashMap::new();
        other_fields.insert("Level".to_string(), level.to_string());
        MpPolyline {
            type_code: "0x01".to_string(),
            label: None,
            end_level: None,
            coords,
            routing: Some(MpRoutingAttrs {
                road_id: Some(road_id.to_string()),
                route_param: Some(route_param.to_string()),
                speed_type: None,
                dir_indicator: Some(dir_indicator),
                roundabout: if roundabout { Some(true) } else { None },
                max_height: None,
                max_weight: None,
                max_width: None,
                max_length: None,
            }),
            other_fields,
        }
    }

    // =========================================================================
    // Task 2.6: T-intersection (1 node shared + 3 arcs for bidirectional)
    // =========================================================================

    #[test]
    fn test_t_intersection() {
        // Three roads meeting at (45.0, 5.0):
        // Road A: (45.001, 5.0) → (45.0, 5.0)  bidirectional
        // Road B: (45.0, 5.0) → (45.0, 5.001)  bidirectional
        // Road C: (45.0, 5.0) → (44.999, 5.0)  bidirectional
        let polylines = vec![
            make_polyline(vec![(45.001, 5.0), (45.0, 5.0)], "4,2,0,0,0,0,0,0,0,0,0,0", 0, 1, 0, false),
            make_polyline(vec![(45.0, 5.0), (45.0, 5.001)], "4,2,0,0,0,0,0,0,0,0,0,0", 0, 2, 0, false),
            make_polyline(vec![(45.0, 5.0), (44.999, 5.0)], "4,2,0,0,0,0,0,0,0,0,0,0", 0, 3, 0, false),
        ];

        let network = build_road_network(&polylines);

        // 4 unique endpoints → 4 nodes (center shared)
        assert_eq!(network.nodes.len(), 4, "T-intersection: 4 nodes (center + 3 ends)");
        // 3 roads × 2 arcs (bidirectional) = 6 arcs
        assert_eq!(network.arcs.len(), 6, "T-intersection: 6 arcs (3 roads × 2 dirs)");
        assert_eq!(network.road_defs.len(), 3, "T-intersection: 3 road defs");

        // Find center node (the one with 3+ arcs from 3 roads connecting)
        let center_node = network.nodes.iter().find(|n| n.arcs.len() >= 3);
        assert!(center_node.is_some(), "should have a center node with 3+ arcs");
    }

    // =========================================================================
    // Task 2.6: Crossroad (1 shared node + 4 arcs for bidirectional)
    // =========================================================================

    #[test]
    fn test_crossroad() {
        // Four roads meeting at (45.0, 5.0):
        let polylines = vec![
            make_polyline(vec![(45.001, 5.0), (45.0, 5.0)], "4,2,0,0,0,0,0,0,0,0,0,0", 0, 1, 0, false),
            make_polyline(vec![(45.0, 5.0), (44.999, 5.0)], "4,2,0,0,0,0,0,0,0,0,0,0", 0, 2, 0, false),
            make_polyline(vec![(45.0, 5.001), (45.0, 5.0)], "4,2,0,0,0,0,0,0,0,0,0,0", 0, 3, 0, false),
            make_polyline(vec![(45.0, 5.0), (45.0, 4.999)], "4,2,0,0,0,0,0,0,0,0,0,0", 0, 4, 0, false),
        ];

        let network = build_road_network(&polylines);

        // 5 unique endpoints → 5 nodes
        assert_eq!(network.nodes.len(), 5);
        // 4 roads × 2 = 8 arcs
        assert_eq!(network.arcs.len(), 8);
        assert_eq!(network.road_defs.len(), 4);
    }

    // =========================================================================
    // Task 2.6: Two parallel roads (4 nodes, no shared node)
    // =========================================================================

    #[test]
    fn test_parallel_roads_no_connection() {
        // Road A: (45.0, 5.0) → (45.0, 5.01) — south
        // Road B: (45.001, 5.0) → (45.001, 5.01) — north, parallel
        let polylines = vec![
            make_polyline(vec![(45.0, 5.0), (45.0, 5.01)], "4,2,0,0,0,0,0,0,0,0,0,0", 0, 1, 0, false),
            make_polyline(vec![(45.001, 5.0), (45.001, 5.01)], "4,2,0,0,0,0,0,0,0,0,0,0", 0, 2, 0, false),
        ];

        let network = build_road_network(&polylines);

        // 4 unique endpoints → 4 nodes
        assert_eq!(network.nodes.len(), 4);
        // 2 roads × 2 = 4 arcs
        assert_eq!(network.arcs.len(), 4);
        // No node has arcs from both roads
        for node in &network.nodes {
            assert!(node.arcs.len() <= 2, "parallel roads should not share nodes");
        }
    }

    // =========================================================================
    // Task 3.2: Bridge over road (level isolation)
    // =========================================================================

    #[test]
    fn test_bridge_level_isolation() {
        // Road at ground (level=0): (45.0, 5.0) → (45.0, 5.001)
        // Bridge over it (level=1): (44.999, 5.0005) → (45.001, 5.0005)
        // The bridge passes over the road at approximately (45.0, 5.0005)
        // but they should NOT connect because different levels.
        let polylines = vec![
            make_polyline(vec![(45.0, 5.0), (45.0, 5.001)], "4,2,0,0,0,0,0,0,0,0,0,0", 0, 1, 0, false),
            make_polyline(vec![(44.999, 5.0005), (45.001, 5.0005)], "4,2,0,0,0,0,0,0,0,0,0,0", 0, 2, 1, false),
        ];

        let network = build_road_network(&polylines);

        // 4 unique endpoints (2 per road) → 4 nodes
        assert_eq!(network.nodes.len(), 4);
        // No connection between level=0 and level=1
        let ground_nodes: Vec<_> = network.nodes.iter().filter(|n| n.level == 0).collect();
        let bridge_nodes: Vec<_> = network.nodes.iter().filter(|n| n.level == 1).collect();
        assert_eq!(ground_nodes.len(), 2);
        assert_eq!(bridge_nodes.len(), 2);
    }

    // =========================================================================
    // Task 3.2: Coincident vertices at different levels → separate nodes
    // =========================================================================

    #[test]
    fn test_coincident_different_levels_separate_nodes() {
        // Both polylines share exact same endpoint (45.0, 5.0), but different levels
        let polylines = vec![
            make_polyline(vec![(45.0, 5.0), (45.001, 5.0)], "4,2,0,0,0,0,0,0,0,0,0,0", 0, 1, 0, false),
            make_polyline(vec![(45.0, 5.0), (44.999, 5.0)], "4,2,0,0,0,0,0,0,0,0,0,0", 0, 2, 1, false),
        ];

        let network = build_road_network(&polylines);

        // 4 nodes: 2 for level=0 and 2 for level=1 (even though endpoint is shared geometrically)
        assert_eq!(network.nodes.len(), 4);
    }

    // =========================================================================
    // Task 3.4: Tunnel (level=-1) under road (level=0)
    // =========================================================================

    #[test]
    fn test_tunnel_level_isolation() {
        let polylines = vec![
            make_polyline(vec![(45.0, 5.0), (45.001, 5.0)], "4,2,0,0,0,0,0,0,0,0,0,0", 0, 1, 0, false),
            make_polyline(vec![(45.0, 5.0), (44.999, 5.0)], "4,2,0,0,0,0,0,0,0,0,0,0", 0, 2, -1, false),
        ];

        let network = build_road_network(&polylines);
        assert_eq!(network.nodes.len(), 4);
    }

    // =========================================================================
    // Task 4.3: Vertex snapping — close endpoints within tolerance → same node
    // =========================================================================

    #[test]
    fn test_snapping_close_endpoints_with_tolerance() {
        // Two endpoints ~0.4m apart (0.000004° lat ≈ 0.445m).
        // With 1m snap tolerance (quant_factor ≈ 111_320), they land in the same bucket.
        let polylines = vec![
            make_polyline(
                vec![(45.0, 5.0), (45.0, 5.001)],
                "4,2,0,0,0,0,0,0,0,0,0,0", 0, 1, 0, false,
            ),
            make_polyline(
                vec![(45.000004, 5.001), (45.001, 5.001)],
                "4,2,0,0,0,0,0,0,0,0,0,0", 0, 2, 0, false,
            ),
        ];

        // With 1m tolerance, the ~0.4m-apart endpoints should snap
        let network = build_road_network_with_tolerance(&polylines, 1.0);
        assert_eq!(network.nodes.len(), 3, "endpoints within 1m tolerance should snap to 1 node");
    }

    #[test]
    fn test_snapping_close_endpoints_default_no_snap() {
        // Same ~0.4m-apart endpoints, but with default tolerance (~0.01m) they should NOT snap
        let polylines = vec![
            make_polyline(
                vec![(45.0, 5.0), (45.0, 5.001)],
                "4,2,0,0,0,0,0,0,0,0,0,0", 0, 1, 0, false,
            ),
            make_polyline(
                vec![(45.000004, 5.001), (45.001, 5.001)],
                "4,2,0,0,0,0,0,0,0,0,0,0", 0, 2, 0, false,
            ),
        ];

        let network = build_road_network(&polylines);
        assert_eq!(network.nodes.len(), 4, "0.4m apart should NOT snap at default 0.01m tolerance");
    }

    // =========================================================================
    // Task 4.4: Endpoints at 2m apart → different nodes (even with 1m tolerance)
    // =========================================================================

    #[test]
    fn test_no_snapping_far_endpoints() {
        // ~2m apart: 0.00002° ≈ 2.2m — should not snap even with 1m tolerance
        let polylines = vec![
            make_polyline(
                vec![(45.0, 5.0), (45.0, 5.001)],
                "4,2,0,0,0,0,0,0,0,0,0,0", 0, 1, 0, false,
            ),
            make_polyline(
                vec![(45.0, 5.00102), (45.001, 5.001)],
                "4,2,0,0,0,0,0,0,0,0,0,0", 0, 2, 0, false,
            ),
        ];

        let network = build_road_network_with_tolerance(&polylines, 1.0);
        assert_eq!(network.nodes.len(), 4, "2m apart should NOT snap at 1m tolerance");
    }

    // =========================================================================
    // Task 4.5: Three endpoints within <1m → single node (triangle)
    // =========================================================================

    #[test]
    fn test_snapping_triangle_three_endpoints() {
        // Three polylines ending at nearly the same point (within quantization)
        let center = (45.0, 5.0);
        let polylines = vec![
            make_polyline(vec![(45.001, 5.0), center], "4,2,0,0,0,0,0,0,0,0,0,0", 0, 1, 0, false),
            make_polyline(vec![(45.0, 5.001), center], "4,2,0,0,0,0,0,0,0,0,0,0", 0, 2, 0, false),
            make_polyline(vec![(44.999, 5.0), center], "4,2,0,0,0,0,0,0,0,0,0,0", 0, 3, 0, false),
        ];

        let network = build_road_network(&polylines);

        // Center is shared → 1 node, plus 3 outer endpoints = 4 nodes
        assert_eq!(network.nodes.len(), 4);
        // The center node has 3+ arcs
        let center_node = network.nodes.iter().find(|n| n.arcs.len() >= 3);
        assert!(center_node.is_some());
    }

    // =========================================================================
    // One-way forward (DirIndicator=1)
    // =========================================================================

    #[test]
    fn test_one_way_forward() {
        let polylines = vec![
            make_polyline(
                vec![(45.0, 5.0), (45.001, 5.0)],
                "4,2,1,0,0,0,0,0,0,0,0,0", // one_way=1
                1, // DirIndicator=1 (forward only)
                1, 0, false,
            ),
        ];

        let network = build_road_network(&polylines);

        assert_eq!(network.arcs.len(), 1, "one_way forward: only 1 arc");
        assert!(network.arcs[0].forward);
    }

    // =========================================================================
    // One-way reverse (DirIndicator=-1)
    // =========================================================================

    #[test]
    fn test_one_way_reverse() {
        let polylines = vec![
            make_polyline(
                vec![(45.0, 5.0), (45.001, 5.0)],
                "4,2,1,0,0,0,0,0,0,0,0,0", // one_way=1
                -1, // DirIndicator=-1 (reverse only)
                1, 0, false,
            ),
        ];

        let network = build_road_network(&polylines);

        assert_eq!(network.arcs.len(), 1, "one_way reverse: only 1 arc");
        assert!(!network.arcs[0].forward, "should be reverse arc");
        // Reverse arc: from_node = last point's node, to_node = first point's node
        let from_node = &network.nodes[network.arcs[0].from_node as usize];
        let to_node = &network.nodes[network.arcs[0].to_node as usize];
        // from_node should be at (45.001, 5.0) (last point)
        assert!((from_node.coord.0 - 45.001).abs() < 0.0001);
        // to_node should be at (45.0, 5.0) (first point)
        assert!((to_node.coord.0 - 45.0).abs() < 0.0001);
    }

    // =========================================================================
    // Bidirectional (DirIndicator=0, one_way=0)
    // =========================================================================

    #[test]
    fn test_bidirectional() {
        let polylines = vec![
            make_polyline(
                vec![(45.0, 5.0), (45.001, 5.0)],
                "4,2,0,0,0,0,0,0,0,0,0,0", // one_way=0
                0, // DirIndicator=0
                1, 0, false,
            ),
        ];

        let network = build_road_network(&polylines);

        assert_eq!(network.arcs.len(), 2, "bidirectional: 2 arcs");
        let forward = network.arcs.iter().find(|a| a.forward).unwrap();
        let reverse = network.arcs.iter().find(|a| !a.forward).unwrap();
        assert_eq!(forward.from_node, reverse.to_node);
        assert_eq!(forward.to_node, reverse.from_node);
    }

    // =========================================================================
    // Length and bearing computation
    // =========================================================================

    #[test]
    fn test_arc_length_and_bearing() {
        // Road going north: (45.0, 5.0) → (45.001, 5.0) ≈ 111m
        let polylines = vec![
            make_polyline(
                vec![(45.0, 5.0), (45.001, 5.0)],
                "4,2,1,0,0,0,0,0,0,0,0,0",
                1, 1, 0, false,
            ),
        ];

        let network = build_road_network(&polylines);

        assert_eq!(network.arcs.len(), 1);
        let arc = &network.arcs[0];
        assert!((arc.length_meters - 111.0).abs() < 2.0, "expected ~111m, got {}", arc.length_meters);
        assert!((arc.bearing_degrees - 0.0).abs() < 1.0, "expected ~0° (north), got {}", arc.bearing_degrees);
    }

    // =========================================================================
    // Non-routable polylines are skipped
    // =========================================================================

    #[test]
    fn test_non_routable_skipped() {
        let non_routable = MpPolyline {
            type_code: "0x01".to_string(),
            label: None,
            end_level: None,
            coords: vec![(45.0, 5.0), (45.001, 5.0)],
            routing: None,
            other_fields: HashMap::new(),
        };

        let routable = make_polyline(
            vec![(45.0, 5.0), (45.001, 5.0)],
            "4,2,0,0,0,0,0,0,0,0,0,0", 0, 1, 0, false,
        );

        let polylines = vec![non_routable, routable];
        let network = build_road_network(&polylines);

        assert_eq!(network.road_defs.len(), 1);
    }

    // =========================================================================
    // Isolated polyline (cul-de-sac)
    // =========================================================================

    #[test]
    fn test_isolated_polyline() {
        let polylines = vec![
            make_polyline(
                vec![(45.0, 5.0), (45.001, 5.0)],
                "4,2,0,0,0,0,0,0,0,0,0,0", 0, 1, 0, false,
            ),
        ];

        let network = build_road_network(&polylines);

        assert_eq!(network.nodes.len(), 2);
        assert_eq!(network.arcs.len(), 2); // bidirectional
        assert_eq!(network.road_defs.len(), 1);
    }

    // =========================================================================
    // Roundabout flag propagation
    // =========================================================================

    #[test]
    fn test_roundabout_flag() {
        let polylines = vec![
            make_polyline(
                vec![(45.0, 5.0), (45.0, 5.001), (45.001, 5.0)],
                "3,1,1,0,0,0,0,0,0,0,0,0",
                1, 1, 0, true, // roundabout
            ),
        ];

        let network = build_road_network(&polylines);
        assert!(network.road_defs[0].roundabout);
    }

    // =========================================================================
    // Empty input
    // =========================================================================

    #[test]
    fn test_empty_input() {
        let network = build_road_network(&[]);
        assert_eq!(network.nodes.len(), 0);
        assert_eq!(network.arcs.len(), 0);
        assert_eq!(network.road_defs.len(), 0);
    }

    // =========================================================================
    // Task 3.3: Ramp (level 0→1) — verify connectivity, not just counts
    // =========================================================================

    #[test]
    fn test_ramp_level_transition() {
        // Ground road at level=0: (45.0, 5.0) → (45.001, 5.0)
        // Ramp at level=0: (45.001, 5.0) → (45.002, 5.0) (connects to ground)
        // Bridge at level=1: (45.002, 5.0) → (45.003, 5.0)
        let polylines = vec![
            // Ground
            make_polyline(vec![(45.0, 5.0), (45.001, 5.0)], "4,2,0,0,0,0,0,0,0,0,0,0", 0, 1, 0, false),
            // Ramp on ground level
            make_polyline(vec![(45.001, 5.0), (45.002, 5.0)], "4,2,0,0,0,0,0,0,0,0,0,0", 0, 2, 0, false),
            // Bridge at level=1
            make_polyline(vec![(45.002, 5.0), (45.003, 5.0)], "4,2,0,0,0,0,0,0,0,0,0,0", 0, 3, 1, false),
        ];

        let network = build_road_network(&polylines);

        // Node counts per level
        let ground_nodes: Vec<_> = network.nodes.iter().filter(|n| n.level == 0).collect();
        let bridge_nodes: Vec<_> = network.nodes.iter().filter(|n| n.level == 1).collect();
        assert_eq!(ground_nodes.len(), 3, "ground: (45.0), (45.001), (45.002)");
        assert_eq!(bridge_nodes.len(), 2, "bridge: (45.002), (45.003)");

        // Verify ground connectivity: node at (45.001) connects ground road and ramp
        let junction_node = ground_nodes.iter().find(|n| {
            (n.coord.0 - 45.001).abs() < 0.0001 && (n.coord.1 - 5.0).abs() < 0.0001
        });
        assert!(junction_node.is_some(), "should have ground junction node at (45.001, 5.0)");
        // Junction has arcs from both ground road and ramp (2 roads × 2 dirs = 4 arcs)
        assert!(
            junction_node.unwrap().arcs.len() >= 2,
            "ground junction should have arcs from both ground road and ramp, got {}",
            junction_node.unwrap().arcs.len(),
        );

        // Bridge node at (45.002) level=1 does NOT connect to ramp at (45.002) level=0
        let bridge_start = bridge_nodes.iter().find(|n| (n.coord.0 - 45.002).abs() < 0.0001);
        let ramp_end = ground_nodes.iter().find(|n| (n.coord.0 - 45.002).abs() < 0.0001);
        assert!(bridge_start.is_some(), "bridge has node at (45.002, 5.0) level=1");
        assert!(ramp_end.is_some(), "ramp has node at (45.002, 5.0) level=0");
        assert_ne!(
            bridge_start.unwrap().id, ramp_end.unwrap().id,
            "bridge and ramp at same coords but different levels must be different nodes"
        );
    }
}
