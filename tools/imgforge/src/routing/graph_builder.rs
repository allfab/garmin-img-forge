// Road network graph builder — construct routing graph from polylines

use std::collections::{HashMap, HashSet};
use crate::img::coord::Coord;
use crate::img::nod::{RouteNode, RouteArc};

// Re-export shared types and functions from garmin-routing-graph
pub use garmin_routing_graph::{
    NodEntry, RouteParams, parse_route_param,
    NO_EMERGENCY, NO_DELIVERY, NO_CAR, NO_BUS, NO_TAXI, NO_FOOT, NO_BIKE, NO_TRUCK,
};

// ── Coord-based adapters for imgforge ──────────────────────────────────────

/// Find junction points from Coord-based road polylines.
///
/// Converts Coord to (i32, i32) map units before delegating to the
/// coordinate-agnostic `garmin_routing_graph::find_junctions`.
pub fn find_junctions(
    road_polylines: &[(Vec<Coord>, usize, RouteParams)],
) -> HashSet<(i32, i32)> {
    let raw: Vec<Vec<(i32, i32)>> = road_polylines
        .iter()
        .map(|(coords, _, _)| coords.iter().map(|c| (c.latitude(), c.longitude())).collect())
        .collect();
    garmin_routing_graph::find_junctions(&raw)
}

/// Compute node_flags from Coord-based road polylines.
pub fn compute_node_flags(
    road_polylines: &[(Vec<Coord>, usize, RouteParams)],
    junctions: &HashSet<(i32, i32)>,
) -> Vec<Vec<bool>> {
    let raw: Vec<Vec<(i32, i32)>> = road_polylines
        .iter()
        .map(|(coords, _, _)| coords.iter().map(|c| (c.latitude(), c.longitude())).collect())
        .collect();
    garmin_routing_graph::compute_node_flags(&raw, junctions)
}

// ── Full graph builder (imgforge-internal, uses IMG types) ─────────────────

/// Build route nodes using a pre-computed junction set (avoids redundant find_junctions call).
///
/// `boundary_coords` flags which junction coordinates are actual tile-edge nodes
/// — only those go into the NOD3 boundary section. The previous behaviour of
/// flagging every junction broke cross-tile routing in BaseCamp / Alpha 100.
pub fn build_graph_with_junctions(
    road_polylines: &[(Vec<Coord>, usize, RouteParams)],
    junction_set: &HashSet<(i32, i32)>,
    boundary_coords: &HashSet<(i32, i32)>,
) -> Vec<RouteNode> {
    if road_polylines.is_empty() {
        return Vec::new();
    }

    let junctions: HashMap<(i32, i32), usize> = junction_set
        .iter()
        .copied()
        .enumerate()
        .map(|(idx, key)| (key, idx))
        .collect();

    let mut nodes: Vec<RouteNode> = vec![
        RouteNode {
            lat: 0,
            lon: 0,
            arcs: Vec::new(),
            is_boundary: false,
            node_class: 0,
            node_group: 0,
        };
        junctions.len()
    ];
    for (&(lat, lon), &idx) in &junctions {
        let is_boundary = boundary_coords.contains(&(lat, lon));
        nodes[idx] = RouteNode {
            lat,
            lon,
            arcs: Vec::new(),
            is_boundary,
            node_class: 0,
            node_group: 0,
        };
    }

    for (coords, road_def_idx, params) in road_polylines {
        let mut last_junction_idx: Option<usize> = None;
        let mut last_junction_coord_idx: usize = 0;
        let mut distance_from_last: f64 = 0.0;

        for i in 0..coords.len() {
            let key = (coords[i].latitude(), coords[i].longitude());

            if i > 0 {
                distance_from_last += coords[i - 1].distance(&coords[i]);
            }

            if let Some(&node_idx) = junctions.get(&key) {
                if let Some(prev_idx) = last_junction_idx {
                    let len = distance_from_last as u32;
                    // Skip degenerate arcs: self-loops or zero-length segments.
                    // These are poison for Dijkstra (zero-cost edges cause arbitrary path
                    // selection / cycle traversal) and arise when two adjacent polyline
                    // vertices land on the same junction key after coordinate quantization.
                    if prev_idx == node_idx || len == 0 {
                        last_junction_idx = Some(node_idx);
                        last_junction_coord_idx = i;
                        distance_from_last = 0.0;
                        continue;
                    }
                    let fwd_heading = direction_from_degrees(
                        coords[last_junction_coord_idx].bearing_to(&coords[last_junction_coord_idx + 1])
                    );
                    let rev_heading = direction_from_degrees(
                        coords[i].bearing_to(&coords[i - 1])
                    );
                    nodes[prev_idx].arcs.push(RouteArc {
                        dest_node_index: node_idx,
                        road_def_index: *road_def_idx,
                        length_meters: len,
                        forward: true,
                        road_class: params.road_class,
                        speed: params.speed,
                        access: params.access_flags,
                        toll: params.toll,
                        one_way: params.one_way,
                        initial_heading: fwd_heading,
                    });
                    // mkgmap-faithful : crée toujours l'arc reverse, même pour
                    // les roads oneway (RouteNetwork.java:211-219). L'arc reverse
                    // d'une oneway porte forward=false ; le routeur Garmin lit
                    // l'oneway flag depuis le RoadDef (NOD2 TableA) et n'emprunte
                    // pas l'arc à contre-sens, mais sa présence est nécessaire à
                    // la complétude du graphe (sinon le node terminal d'une
                    // oneway est orphelin → asymétrie start/end aléatoire en
                    // routing BaseCamp / Alpha 100).
                    nodes[node_idx].arcs.push(RouteArc {
                        dest_node_index: prev_idx,
                        road_def_index: *road_def_idx,
                        length_meters: len,
                        forward: false,
                        road_class: params.road_class,
                        speed: params.speed,
                        access: params.access_flags,
                        toll: params.toll,
                        one_way: params.one_way,
                        initial_heading: rev_heading,
                    });
                }
                last_junction_idx = Some(node_idx);
                last_junction_coord_idx = i;
                distance_from_last = 0.0;
            }
        }
    }

    for node in &mut nodes {
        node.node_class = calculate_node_class(&node.arcs);
        node.node_group = calculate_node_group(&node.arcs);
    }

    nodes
}

/// Convert bearing in degrees to Garmin direction byte: round(deg * 256 / 360) as i8.
pub fn direction_from_degrees(deg: f64) -> i8 {
    ((deg * 256.0 / 360.0).round() as i32) as i8
}

/// Calculate node_class: maximum outgoing road class written into RouteNode flags.
pub fn calculate_node_class(arcs: &[RouteArc]) -> u8 {
    arcs.iter().map(|arc| arc.road_class.min(4)).max().unwrap_or(0)
}

/// Calculate node_group using mkgmap RouteNode.getGroup().
///
/// `node_class` and `node_group` are deliberately different. mkgmap writes
/// node_class into NOD1 node flags, but uses node_group for RouteCenter grouping
/// and arc destination class hierarchy.
pub fn calculate_node_group(arcs: &[RouteArc]) -> u8 {
    if arcs.is_empty() {
        return 0;
    }
    let mut class_roads: [HashSet<usize>; 5] = Default::default();
    for arc in arcs {
        class_roads[arc.road_class.min(4) as usize].insert(arc.road_def_index);
    }

    let used_classes: Vec<u8> = (0..5u8)
        .rev()
        .filter(|&c| !class_roads[c as usize].is_empty())
        .collect();

    if used_classes.is_empty() {
        return 0;
    }
    if used_classes.len() == 1 {
        return used_classes[0];
    }

    for &c in &used_classes {
        if class_roads[c as usize].len() >= 2 {
            return c;
        }
    }

    used_classes[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::img::coord::Coord;

    #[test]
    fn test_empty_graph() {
        let nodes = build_graph_with_junctions(&[], &HashSet::new(), &HashSet::new());
        assert!(nodes.is_empty());
    }

    #[test]
    fn test_two_connected_roads() {
        let shared = Coord::new(100, 100);
        let road1 = vec![Coord::new(0, 0), shared, Coord::new(200, 200)];
        let road2 = vec![Coord::new(0, 200), shared, Coord::new(200, 0)];

        let roads = vec![
            (road1, 0, RouteParams::default()),
            (road2, 1, RouteParams::default()),
        ];
        let junctions = find_junctions(&roads);
        let nodes = build_graph_with_junctions(&roads, &junctions, &HashSet::new());

        assert!(!nodes.is_empty());
        let junction = nodes.iter().find(|n| n.lat == 100 && n.lon == 100);
        assert!(junction.is_some());
    }

    #[test]
    fn test_haversine() {
        let a = Coord::from_degrees(48.5734, 7.7521);
        let b = Coord::from_degrees(48.5834, 7.7621);
        let d = a.distance(&b);
        assert!(d > 500.0 && d < 2000.0);
    }

    #[test]
    fn test_parse_route_param_full_12_fields() {
        let p = parse_route_param("6,3,1,1,0,0,1,0,0,1,0,0");
        assert_eq!(p.speed, 6);
        assert_eq!(p.road_class, 3);
        assert!(p.one_way);
        assert!(p.toll);
        assert_eq!(p.access_flags, NO_CAR | NO_FOOT);
    }

    #[test]
    fn test_parse_route_param_partial_2_fields() {
        let p = parse_route_param("4,2");
        assert_eq!(p.speed, 4);
        assert_eq!(p.road_class, 2);
        assert!(!p.one_way);
        assert_eq!(p.access_flags, 0);
    }

    #[test]
    fn test_parse_route_param_partial_4_fields() {
        let p = parse_route_param("3,1,1,0");
        assert_eq!(p.speed, 3);
        assert_eq!(p.road_class, 1);
        assert!(p.one_way);
        assert_eq!(p.access_flags, 0);
    }

    #[test]
    fn test_parse_route_param_all_denied() {
        let p = parse_route_param("0,0,0,0,1,1,1,1,1,1,1,1");
        assert_eq!(p.access_flags, NO_EMERGENCY | NO_DELIVERY | NO_CAR | NO_BUS | NO_TAXI | NO_FOOT | NO_BIKE | NO_TRUCK);
    }

    #[test]
    fn test_parse_route_param_empty_string() {
        let p = parse_route_param("");
        assert_eq!(p.speed, 0);
        assert_eq!(p.access_flags, 0);
    }

    #[test]
    fn test_find_junctions_shared_point() {
        let shared = Coord::new(100, 100);
        let road1 = vec![Coord::new(0, 0), shared, Coord::new(200, 200)];
        let road2 = vec![Coord::new(0, 200), shared, Coord::new(200, 0)];
        let roads = vec![
            (road1, 0, RouteParams::default()),
            (road2, 1, RouteParams::default()),
        ];
        let junctions = find_junctions(&roads);
        assert!(junctions.contains(&(100, 100)), "Shared point should be a junction");
    }

    #[test]
    fn test_find_junctions_no_shared() {
        let road1 = vec![Coord::new(0, 0), Coord::new(100, 100)];
        let road2 = vec![Coord::new(200, 200), Coord::new(300, 300)];
        let roads = vec![
            (road1, 0, RouteParams::default()),
            (road2, 1, RouteParams::default()),
        ];
        let junctions = find_junctions(&roads);
        assert!(junctions.contains(&(0, 0)));
        assert!(junctions.contains(&(100, 100)));
        assert!(junctions.contains(&(200, 200)));
        assert!(junctions.contains(&(300, 300)));
    }

    #[test]
    fn test_compute_node_flags_endpoints() {
        let road = vec![Coord::new(0, 0), Coord::new(50, 50), Coord::new(100, 100)];
        let roads = vec![(road, 0, RouteParams::default())];
        let junctions = HashSet::new();
        let flags = compute_node_flags(&roads, &junctions);
        assert_eq!(flags[0], vec![true, false, true]);
    }

    #[test]
    fn test_compute_node_flags_junction() {
        let shared = Coord::new(100, 100);
        let road1 = vec![Coord::new(0, 0), shared, Coord::new(200, 200)];
        let road2 = vec![Coord::new(0, 200), shared, Coord::new(200, 0)];
        let roads = vec![
            (road1, 0, RouteParams::default()),
            (road2, 1, RouteParams::default()),
        ];
        let junctions = find_junctions(&roads);
        let flags = compute_node_flags(&roads, &junctions);
        assert_eq!(flags[0], vec![true, true, true]);
        assert_eq!(flags[1], vec![true, true, true]);
    }

    #[test]
    fn test_initial_heading() {
        assert_eq!(direction_from_degrees(0.0), 0);
        assert_eq!(direction_from_degrees(90.0), 64);
        assert_eq!(direction_from_degrees(180.0), -128);
        assert_eq!(direction_from_degrees(270.0), -64);
    }

    #[test]
    fn test_node_class_calculation() {
        use crate::img::nod::RouteArc;
        let arcs = vec![
            RouteArc { dest_node_index: 1, road_def_index: 0, length_meters: 100,
                forward: true, road_class: 3, speed: 0, access: 0, toll: false, one_way: false, initial_heading: 0 },
            RouteArc { dest_node_index: 2, road_def_index: 1, length_meters: 100,
                forward: true, road_class: 3, speed: 0, access: 0, toll: false, one_way: false, initial_heading: 0 },
            RouteArc { dest_node_index: 3, road_def_index: 2, length_meters: 100,
                forward: true, road_class: 4, speed: 0, access: 0, toll: false, one_way: false, initial_heading: 0 },
        ];
        assert_eq!(calculate_node_class(&arcs), 4);
        assert_eq!(calculate_node_group(&arcs), 3);

        let arcs_single = vec![
            RouteArc { dest_node_index: 1, road_def_index: 0, length_meters: 100,
                forward: true, road_class: 2, speed: 0, access: 0, toll: false, one_way: false, initial_heading: 0 },
        ];
        assert_eq!(calculate_node_class(&arcs_single), 2);
        assert_eq!(calculate_node_group(&arcs_single), 2);
        assert_eq!(calculate_node_class(&[]), 0);
        assert_eq!(calculate_node_group(&[]), 0);
    }

    #[test]
    fn test_node_class_and_group_diverge_for_mixed_junction() {
        use crate::img::nod::RouteArc;
        let arcs = vec![
            RouteArc { dest_node_index: 1, road_def_index: 0, length_meters: 100,
                forward: true, road_class: 4, speed: 0, access: 0, toll: false, one_way: false, initial_heading: 0 },
            RouteArc { dest_node_index: 2, road_def_index: 1, length_meters: 100,
                forward: true, road_class: 1, speed: 0, access: 0, toll: false, one_way: false, initial_heading: 0 },
        ];

        assert_eq!(calculate_node_class(&arcs), 4);
        assert_eq!(calculate_node_group(&arcs), 1);
    }
}
