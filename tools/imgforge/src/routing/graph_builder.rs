// Road network graph builder — construct routing graph from polylines

use std::collections::{HashMap, HashSet};
use crate::img::coord::Coord;
use crate::img::net::{NO_EMERGENCY, NO_DELIVERY, NO_CAR, NO_BUS, NO_TAXI, NO_FOOT, NO_BIKE, NO_TRUCK};
use crate::img::nod::{RouteNode, RouteArc};

/// Parsed route parameters from MP RouteParam field
#[derive(Debug, Clone, Default)]
pub struct RouteParams {
    pub speed: u8,       // 0-7
    pub road_class: u8,  // 0-4
    pub one_way: bool,
    pub toll: bool,
    pub access_flags: u16,
}

/// Parse RouteParam string "speed,class,oneway,toll,denied_emergency,...,denied_truck"
/// Positions 0-3: speed, class, oneway, toll
/// Positions 4-11: denied_emergency, denied_delivery, denied_car, denied_bus,
///                 denied_taxi, denied_pedestrian, denied_bicycle, denied_truck
pub fn parse_route_param(param: &str) -> RouteParams {
    let parts: Vec<&str> = param.split(',').collect();

    let mut access_flags: u16 = 0;
    // Map positions 4-11 to access flag constants.
    // Note: CARPOOL (0x0008) and NO_THROUGHROUTE (0x0080) have no RouteParam
    // position in cGPSmapper format — they are set via other mechanisms.
    const ACCESS_MAP: [(usize, u16); 8] = [
        (4, NO_EMERGENCY), // 0x8000
        (5, NO_DELIVERY),  // 0x4000
        (6, NO_CAR),       // 0x0001
        (7, NO_BUS),       // 0x0002
        (8, NO_TAXI),      // 0x0004
        (9, NO_FOOT),      // 0x0010
        (10, NO_BIKE),     // 0x0020
        (11, NO_TRUCK),    // 0x0040
    ];
    for &(pos, flag) in &ACCESS_MAP {
        if parts.get(pos).map(|s| s.trim() == "1").unwrap_or(false) {
            access_flags |= flag;
        }
    }

    RouteParams {
        speed: parts.first().and_then(|s| s.trim().parse::<u8>().ok()).unwrap_or(0).min(7),
        road_class: parts.get(1).and_then(|s| s.trim().parse::<u8>().ok()).unwrap_or(0).min(7),
        one_way: parts.get(2).map(|s| s.trim() == "1").unwrap_or(false),
        toll: parts.get(3).map(|s| s.trim() == "1").unwrap_or(false),
        access_flags,
    }
}

/// Find junction points: points shared by 2+ roads or non-endpoint vertices.
/// Returns a set of (latitude, longitude) keys identifying junctions.
pub fn find_junctions(
    road_polylines: &[(Vec<Coord>, usize, RouteParams)],
) -> HashSet<(i32, i32)> {
    let mut point_count: HashMap<(i32, i32), Vec<(usize, usize, bool)>> = HashMap::new();

    for (road_idx, (coords, _, _)) in road_polylines.iter().enumerate() {
        for (pt_idx, coord) in coords.iter().enumerate() {
            let key = (coord.latitude(), coord.longitude());
            let is_endpoint = pt_idx == 0 || pt_idx == coords.len() - 1;
            point_count.entry(key).or_default().push((road_idx, pt_idx, is_endpoint));
        }
    }

    point_count
        .into_iter()
        .filter(|(_, refs_list)| {
            // Junction if shared by 2+ roads, OR if it's an endpoint of any road.
            // Endpoints must be RouteNodes for the routing graph to be connected.
            // Mid-points of a single road that aren't shared are NOT junctions.
            refs_list.len() >= 2 || refs_list.iter().any(|(_, _, is_ep)| *is_ep)
        })
        .map(|(key, _)| key)
        .collect()
}

/// Compute node_flags for each road polyline: true = vertex is a RouteNode, false = geometry only.
/// Endpoints of each road are always marked as RouteNodes.
pub fn compute_node_flags(
    road_polylines: &[(Vec<Coord>, usize, RouteParams)],
    junctions: &HashSet<(i32, i32)>,
) -> Vec<Vec<bool>> {
    road_polylines.iter().map(|(coords, _, _)| {
        coords.iter().enumerate().map(|(i, coord)| {
            let is_endpoint = i == 0 || i == coords.len() - 1;
            let key = (coord.latitude(), coord.longitude());
            is_endpoint || junctions.contains(&key)
        }).collect()
    }).collect()
}

/// Build route nodes using a pre-computed junction set (avoids redundant find_junctions call)
pub fn build_graph_with_junctions(
    road_polylines: &[(Vec<Coord>, usize, RouteParams)],
    junction_set: &HashSet<(i32, i32)>,
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

    // Create route nodes — indexed by enumeration index, NOT HashMap iteration order.
    // nodes[idx] must correspond to the junction with enumeration index idx.
    // All nodes are marked as boundary (required for NOD3 — GPSMapEdit needs
    // non-empty NOD3 to display routing nodes).
    let mut nodes: Vec<RouteNode> = vec![
        RouteNode { lat: 0, lon: 0, arcs: Vec::new(), is_boundary: false, node_class: 0 };
        junctions.len()
    ];
    for (&(lat, lon), &idx) in &junctions {
        nodes[idx] = RouteNode { lat, lon, arcs: Vec::new(), is_boundary: true, node_class: 0 };
    }

    // Create arcs between nodes along each road
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
                    // Forward heading: bearing from prev junction to next vertex
                    let fwd_heading = direction_from_degrees(
                        coords[last_junction_coord_idx].bearing_to(&coords[last_junction_coord_idx + 1])
                    );
                    // Reverse heading: bearing from current junction to previous vertex
                    let rev_heading = direction_from_degrees(
                        coords[i].bearing_to(&coords[i - 1])
                    );
                    // Forward arc
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
                    // Reverse arc (unless one-way)
                    if !params.one_way {
                        nodes[node_idx].arcs.push(RouteArc {
                            dest_node_index: prev_idx,
                            road_def_index: *road_def_idx,
                            length_meters: len,
                            forward: false,
                            road_class: params.road_class,
                            speed: params.speed,
                            access: params.access_flags,
                            toll: params.toll,
                            one_way: false,
                            initial_heading: rev_heading,
                        });
                    }
                }
                last_junction_idx = Some(node_idx);
                last_junction_coord_idx = i;
                distance_from_last = 0.0;
            }
        }
    }

    // Calculate node_class for each node based on connected arcs
    for node in &mut nodes {
        node.node_class = calculate_node_class(&node.arcs);
    }

    nodes
}

/// Convert bearing in degrees to Garmin direction byte: round(deg * 256 / 360) as i8.
/// Uses wrapping cast to match Java's `(byte)` behavior.
pub fn direction_from_degrees(deg: f64) -> i8 {
    ((deg * 256.0 / 360.0).round() as i32) as i8
}

/// Calculate node_class using mkgmap's getGroup() algorithm.
/// Returns the highest road class used by 2+ roads at this node;
/// if only one class, use that; otherwise, the 2nd highest.
pub fn calculate_node_class(arcs: &[RouteArc]) -> u8 {
    if arcs.is_empty() {
        return 0;
    }
    // Count roads per class (unique road_def_index per class)
    let mut class_roads: [HashSet<usize>; 8] = Default::default();
    for arc in arcs {
        class_roads[arc.road_class as usize].insert(arc.road_def_index);
    }

    // Find distinct classes used
    let used_classes: Vec<u8> = (0..8u8)
        .rev()
        .filter(|&c| !class_roads[c as usize].is_empty())
        .collect();

    if used_classes.is_empty() {
        return 0;
    }
    if used_classes.len() == 1 {
        return used_classes[0];
    }

    // Highest class with 2+ roads
    for &c in &used_classes {
        if class_roads[c as usize].len() >= 2 {
            return c;
        }
    }

    // Fallback: 2nd highest class
    used_classes[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_graph() {
        let nodes = build_graph_with_junctions(&[], &HashSet::new());
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
        let nodes = build_graph_with_junctions(&roads, &junctions);

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
        // AC1: speed=6, class=3, oneway, toll, denied_car(pos6), denied_foot(pos9)
        let p = parse_route_param("6,3,1,1,0,0,1,0,0,1,0,0");
        assert_eq!(p.speed, 6);
        assert_eq!(p.road_class, 3);
        assert!(p.one_way);
        assert!(p.toll);
        assert_eq!(p.access_flags, NO_CAR | NO_FOOT); // 0x0011
    }

    #[test]
    fn test_parse_route_param_partial_2_fields() {
        // AC2: only speed and class
        let p = parse_route_param("4,2");
        assert_eq!(p.speed, 4);
        assert_eq!(p.road_class, 2);
        assert!(!p.one_way);
        assert!(!p.toll);
        assert_eq!(p.access_flags, 0);
    }

    #[test]
    fn test_parse_route_param_partial_4_fields() {
        // Rétrocompatible avec l'existant
        let p = parse_route_param("3,1,1,0");
        assert_eq!(p.speed, 3);
        assert_eq!(p.road_class, 1);
        assert!(p.one_way);
        assert!(!p.toll);
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
        assert_eq!(p.road_class, 0);
        assert!(!p.one_way);
        assert!(!p.toll);
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
        // No shared points, but endpoints are always junctions (needed for routing graph)
        assert!(junctions.contains(&(0, 0)));
        assert!(junctions.contains(&(100, 100)));
        assert!(junctions.contains(&(200, 200)));
        assert!(junctions.contains(&(300, 300)));
    }

    #[test]
    fn test_compute_node_flags_endpoints() {
        let road = vec![Coord::new(0, 0), Coord::new(50, 50), Coord::new(100, 100)];
        let roads = vec![(road, 0, RouteParams::default())];
        let junctions = HashSet::new(); // no junctions
        let flags = compute_node_flags(&roads, &junctions);
        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0], vec![true, false, true]); // endpoints always true
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
        assert_eq!(flags.len(), 2);
        // Road1: endpoint(true), junction(true), endpoint(true)
        assert_eq!(flags[0], vec![true, true, true]);
        // Road2: endpoint(true), junction(true), endpoint(true)
        assert_eq!(flags[1], vec![true, true, true]);
    }

    #[test]
    fn test_initial_heading() {
        // direction_from_degrees: 0°→0, 90°→64, 180°→-128, 270°→-64
        assert_eq!(direction_from_degrees(0.0), 0);
        assert_eq!(direction_from_degrees(90.0), 64);
        assert_eq!(direction_from_degrees(180.0), -128);
        assert_eq!(direction_from_degrees(270.0), -64);
    }

    #[test]
    fn test_node_class_calculation() {
        use crate::img::nod::RouteArc;
        // 2 roads class 3, 1 road class 4 → highest with 2+ = class 3
        let arcs = vec![
            RouteArc {
                dest_node_index: 1, road_def_index: 0, length_meters: 100,
                forward: true, road_class: 3, speed: 0, access: 0,
                toll: false, one_way: false, initial_heading: 0,
            },
            RouteArc {
                dest_node_index: 2, road_def_index: 1, length_meters: 100,
                forward: true, road_class: 3, speed: 0, access: 0,
                toll: false, one_way: false, initial_heading: 0,
            },
            RouteArc {
                dest_node_index: 3, road_def_index: 2, length_meters: 100,
                forward: true, road_class: 4, speed: 0, access: 0,
                toll: false, one_way: false, initial_heading: 0,
            },
        ];
        assert_eq!(calculate_node_class(&arcs), 3);

        // Only one class (2) → use that
        let arcs_single = vec![
            RouteArc {
                dest_node_index: 1, road_def_index: 0, length_meters: 100,
                forward: true, road_class: 2, speed: 0, access: 0,
                toll: false, one_way: false, initial_heading: 0,
            },
        ];
        assert_eq!(calculate_node_class(&arcs_single), 2);

        // No arcs → 0
        assert_eq!(calculate_node_class(&[]), 0);
    }
}
