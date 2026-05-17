// Shared routing topology types and logic for mpforge (emission) and imgforge (parsing).

use std::collections::{HashMap, HashSet};

// ── Access flag constants (same bit positions as mkgmap AccessTagsAndBits) ──
pub const NO_CAR: u16 = 0x0001;
pub const NO_BUS: u16 = 0x0002;
pub const NO_TAXI: u16 = 0x0004;
pub const NO_FOOT: u16 = 0x0010;
pub const NO_BIKE: u16 = 0x0020;
pub const NO_TRUCK: u16 = 0x0040;
pub const NO_DELIVERY: u16 = 0x4000;
pub const NO_EMERGENCY: u16 = 0x8000;

/// A routing node entry in a Polish Map polyline section.
///
/// Format emitted: `Nod<N>=<point_index>,<node_id>,<boundary>`
/// N is 1-based; point_index is 0-based into Data0= coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodEntry {
    /// 0-based index into the Data0 coordinate array
    pub point_index: u16,
    /// Globally unique (or tile-local + reconciled) node ID
    pub node_id: u32,
    /// True if this node is shared with an adjacent tile
    pub boundary: bool,
}

/// Parsed route parameters from Polish Map RouteParam field.
///
/// Format: `speed,class,oneway,toll,denied_emergency,denied_delivery,denied_car,
///          denied_bus,denied_taxi,denied_pedestrian,denied_bicycle,denied_truck`
#[derive(Debug, Clone, Default)]
pub struct RouteParams {
    pub speed: u8,
    pub road_class: u8,
    pub one_way: bool,
    pub toll: bool,
    /// Bitmask using NO_* constants above
    pub access_flags: u16,
}

/// Parse RouteParam string into a `RouteParams` struct.
///
/// Robust: accepts 2-field (speed,class) through 12-field canonical format.
pub fn parse_route_param(param: &str) -> RouteParams {
    let parts: Vec<&str> = param.split(',').collect();

    // Positions 4-11: denied_emergency, denied_delivery, denied_car, denied_bus,
    //                 denied_taxi, denied_pedestrian, denied_bicycle, denied_truck
    const ACCESS_MAP: [(usize, u16); 8] = [
        (4, NO_EMERGENCY),
        (5, NO_DELIVERY),
        (6, NO_CAR),
        (7, NO_BUS),
        (8, NO_TAXI),
        (9, NO_FOOT),
        (10, NO_BIKE),
        (11, NO_TRUCK),
    ];
    let mut access_flags: u16 = 0;
    for &(pos, flag) in &ACCESS_MAP {
        if parts.get(pos).map(|s| s.trim() == "1").unwrap_or(false) {
            access_flags |= flag;
        }
    }

    RouteParams {
        speed: parts.first().and_then(|s| s.trim().parse::<u8>().ok()).unwrap_or(0).min(7),
        road_class: parts.get(1).and_then(|s| s.trim().parse::<u8>().ok()).unwrap_or(0).min(4),
        one_way: parts.get(2).map(|s| s.trim() == "1").unwrap_or(false),
        toll: parts.get(3).map(|s| s.trim() == "1").unwrap_or(false),
        access_flags,
    }
}

/// Find junction points in a set of roads.
///
/// Input: each road is a sequence of `(lat, lon)` integer pairs (e.g. 24-bit
/// map units for imgforge, or quantized WGS84 × 1e7 for mpforge).
///
/// Returns the set of keys `(lat, lon)` that are junctions:
/// - shared by 2+ roads, OR
/// - endpoints of any road (endpoints must be RouteNodes for graph connectivity)
pub fn find_junctions(roads: &[Vec<(i32, i32)>]) -> HashSet<(i32, i32)> {
    let mut point_count: HashMap<(i32, i32), Vec<(usize, usize, bool)>> = HashMap::new();

    for (road_idx, coords) in roads.iter().enumerate() {
        for (pt_idx, &key) in coords.iter().enumerate() {
            let is_endpoint = pt_idx == 0 || pt_idx == coords.len() - 1;
            point_count.entry(key).or_default().push((road_idx, pt_idx, is_endpoint));
        }
    }

    point_count
        .into_iter()
        .filter(|(_, refs_list)| {
            refs_list.len() >= 2 || refs_list.iter().any(|(_, _, is_ep)| *is_ep)
        })
        .map(|(key, _)| key)
        .collect()
}

/// Compute per-road node flags: `true` = vertex is a RouteNode.
///
/// Endpoints of each road are always RouteNodes.
pub fn compute_node_flags(
    roads: &[Vec<(i32, i32)>],
    junctions: &HashSet<(i32, i32)>,
) -> Vec<Vec<bool>> {
    roads.iter().map(|coords| {
        coords.iter().enumerate().map(|(i, key)| {
            let is_endpoint = i == 0 || i == coords.len() - 1;
            is_endpoint || junctions.contains(key)
        }).collect()
    }).collect()
}

/// Deterministic node ID from quantized coordinates.
///
/// Uses FNV-1a hash of (lat_q, lon_q) masked to 31 bits to fit Java signed int
/// (mkgmap RoadHelper parses Nod1 nodeId via Integer.parseInt, max 0x7FFFFFFF).
/// The Garmin firmware also reads node identifiers as signed int32; values with
/// the high bit set are interpreted as negative offsets and break routing.
/// Result is always in [1, 0x7FFFFFFF] (0 reserved as "no node" sentinel).
pub fn coord_to_node_id(lat_q: i32, lon_q: i32) -> u32 {
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;
    let mut hash = FNV_OFFSET;
    for b in lat_q.to_le_bytes().iter().chain(lon_q.to_le_bytes().iter()) {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    ((hash & 0x7FFF_FFFF) as u32).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_junctions_shared_point() {
        let road1 = vec![(0, 0), (100, 100), (200, 200)];
        let road2 = vec![(0, 200), (100, 100), (200, 0)];
        let junctions = find_junctions(&[road1, road2]);
        assert!(junctions.contains(&(100, 100)), "shared midpoint should be junction");
    }

    #[test]
    fn test_find_junctions_endpoints_always_included() {
        let road1 = vec![(0, 0), (100, 100)];
        let road2 = vec![(200, 200), (300, 300)];
        let junctions = find_junctions(&[road1, road2]);
        assert!(junctions.contains(&(0, 0)));
        assert!(junctions.contains(&(100, 100)));
        assert!(junctions.contains(&(200, 200)));
        assert!(junctions.contains(&(300, 300)));
    }

    #[test]
    fn test_compute_node_flags_endpoints() {
        let road = vec![(0, 0), (50, 50), (100, 100)];
        let junctions = HashSet::new();
        let flags = compute_node_flags(&[road], &junctions);
        assert_eq!(flags[0], vec![true, false, true]);
    }

    #[test]
    fn test_compute_node_flags_junction_mid() {
        let road1 = vec![(0, 0), (50, 50), (100, 100)];
        let road2 = vec![(0, 100), (50, 50), (100, 0)];
        let junctions = find_junctions(&[road1.clone(), road2.clone()]);
        let flags = compute_node_flags(&[road1, road2], &junctions);
        assert_eq!(flags[0], vec![true, true, true]);
        assert_eq!(flags[1], vec![true, true, true]);
    }

    #[test]
    fn test_parse_route_param_12_fields() {
        let p = parse_route_param("6,3,1,1,0,0,1,0,0,1,0,0");
        assert_eq!(p.speed, 6);
        assert_eq!(p.road_class, 3);
        assert!(p.one_way);
        assert!(p.toll);
        assert_eq!(p.access_flags, NO_CAR | NO_FOOT);
    }

    #[test]
    fn test_parse_route_param_all_denied() {
        let p = parse_route_param("0,0,0,0,1,1,1,1,1,1,1,1");
        assert_eq!(p.access_flags, NO_EMERGENCY | NO_DELIVERY | NO_CAR | NO_BUS | NO_TAXI | NO_FOOT | NO_BIKE | NO_TRUCK);
    }

    #[test]
    fn test_parse_route_param_partial() {
        let p = parse_route_param("4,2");
        assert_eq!(p.speed, 4);
        assert_eq!(p.road_class, 2);
        assert_eq!(p.access_flags, 0);
    }

    #[test]
    fn test_coord_to_node_id_deterministic() {
        let id1 = coord_to_node_id(45_5000000, 5_7000000);
        let id2 = coord_to_node_id(45_5000000, 5_7000000);
        assert_eq!(id1, id2, "same coords must give same ID");
        assert!(id1 >= 1, "ID must be >= 1");
    }

    #[test]
    fn test_coord_to_node_id_distinct() {
        let id1 = coord_to_node_id(45_5000000, 5_7000000);
        let id2 = coord_to_node_id(45_5000001, 5_7000000);
        assert_ne!(id1, id2, "different coords should give different IDs");
    }

    #[test]
    fn test_nod_entry_fields() {
        let n = NodEntry { point_index: 3, node_id: 42, boundary: true };
        assert_eq!(n.point_index, 3);
        assert_eq!(n.node_id, 42);
        assert!(n.boundary);
    }
}
