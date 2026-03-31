// Road network graph builder — construct routing graph from polylines

use std::collections::HashMap;
use crate::img::coord::Coord;
use crate::img::nod::{RouteNode, RouteArc};

/// Parsed route parameters from MP RouteParam field
#[derive(Debug, Clone, Default)]
pub struct RouteParams {
    pub speed: u8,       // 0-7
    pub road_class: u8,  // 0-4
    pub one_way: bool,
    pub toll: bool,
}

/// Parse RouteParam string "speed,class,oneway,toll,..."
pub fn parse_route_param(param: &str) -> RouteParams {
    let parts: Vec<&str> = param.split(',').collect();
    RouteParams {
        speed: parts.first().and_then(|s| s.trim().parse().ok()).unwrap_or(0),
        road_class: parts.get(1).and_then(|s| s.trim().parse().ok()).unwrap_or(0),
        one_way: parts.get(2).map(|s| s.trim() == "1").unwrap_or(false),
        toll: parts.get(3).map(|s| s.trim() == "1").unwrap_or(false),
    }
}

/// Build route nodes from routable polylines
/// Returns a list of RouteNodes with arcs connecting them
pub fn build_graph(
    road_polylines: &[(Vec<Coord>, usize, RouteParams)], // (coords, road_def_index, params)
) -> Vec<RouteNode> {
    if road_polylines.is_empty() {
        return Vec::new();
    }

    // Find junctions: points shared by multiple roads
    let mut point_count: HashMap<(i32, i32), Vec<(usize, usize, bool)>> = HashMap::new();
    // (road_idx, point_idx_in_road, is_endpoint)

    for (road_idx, (coords, _, _)) in road_polylines.iter().enumerate() {
        for (pt_idx, coord) in coords.iter().enumerate() {
            let key = (coord.latitude(), coord.longitude());
            let is_endpoint = pt_idx == 0 || pt_idx == coords.len() - 1;
            point_count.entry(key).or_default().push((road_idx, pt_idx, is_endpoint));
        }
    }

    // A point is a junction if it's an endpoint shared by 2+ roads, or a non-endpoint
    let junctions: HashMap<(i32, i32), usize> = point_count
        .iter()
        .filter(|(_, refs)| {
            refs.len() >= 2 || refs.iter().any(|(_, _, is_ep)| !is_ep)
        })
        .enumerate()
        .map(|(idx, (key, _))| (*key, idx))
        .collect();

    // Create route nodes
    let mut nodes: Vec<RouteNode> = junctions
        .iter()
        .map(|(&(lat, lon), _)| RouteNode {
            lat,
            lon,
            arcs: Vec::new(),
            is_boundary: false,
            node_class: 0,
        })
        .collect();

    // Create arcs between nodes along each road
    for (coords, road_def_idx, params) in road_polylines {
        let mut last_junction_idx: Option<usize> = None;
        let mut distance_from_last: f64 = 0.0;

        for i in 0..coords.len() {
            let key = (coords[i].latitude(), coords[i].longitude());

            if i > 0 {
                distance_from_last += coords[i - 1].distance(&coords[i]);
            }

            if let Some(&node_idx) = junctions.get(&key) {
                if let Some(prev_idx) = last_junction_idx {
                    let len = distance_from_last as u32;
                    // Forward arc
                    nodes[prev_idx].arcs.push(RouteArc {
                        dest_node_index: node_idx,
                        road_def_index: *road_def_idx,
                        length_meters: len,
                        forward: true,
                        road_class: params.road_class,
                        speed: params.speed,
                        access: 0,
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
                            access: 0,
                        });
                    }
                }
                last_junction_idx = Some(node_idx);
                distance_from_last = 0.0;
            }
        }
    }

    nodes
}

/// Calculate haversine distance between two coords in meters
pub fn haversine_distance(a: &Coord, b: &Coord) -> f64 {
    a.distance(b)
}

/// Calculate bearing from a to b in degrees
pub fn bearing(a: &Coord, b: &Coord) -> f64 {
    a.bearing_to(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_graph() {
        let nodes = build_graph(&[]);
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
        let nodes = build_graph(&roads);

        // The shared point is a junction, plus endpoints that are unique aren't junctions
        // Actually shared point appears in both roads → junction
        assert!(!nodes.is_empty());

        // Find the junction node at (100, 100)
        let junction = nodes.iter().find(|n| n.lat == 100 && n.lon == 100);
        assert!(junction.is_some());
    }

    #[test]
    fn test_haversine() {
        let a = Coord::from_degrees(48.5734, 7.7521);
        let b = Coord::from_degrees(48.5834, 7.7621);
        let d = haversine_distance(&a, &b);
        assert!(d > 500.0 && d < 2000.0);
    }
}
