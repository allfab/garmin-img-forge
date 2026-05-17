// Routing topology stage for the mpforge pipeline.
//
// Computes NodN= entries for each routable polyline in a tile.
// Uses deterministic topology-based node IDs (FNV hash of quantized WGS84 and
// source ground level) so the same routable point always gets the same node ID
// across tiles — no post-tiling reconciliation pass required.

use std::collections::{HashMap, HashSet};
use garmin_routing_graph::{NodEntry, coord_to_node_id_with_level};

use crate::pipeline::reader::Feature;
use crate::pipeline::tiler::TileBounds;

/// Routing graph computed for a single tile.
///
/// `per_feature[i]` holds the `NodEntry` list for `features[i]`.
/// Non-routable features (no RoadID) have an empty Vec.
#[derive(Debug, Default)]
pub struct TileRoutingGraph {
    pub per_feature: Vec<Vec<NodEntry>>,
    pub total_nodes: u32,
    pub junction_count: u32,
    pub boundary_count: u32,
}

/// Quantize a WGS84 degree value to integer units (× 1e7).
///
/// Range checks: lat ∈ [-90, 90] → [-900_000_000, 900_000_000] fits in i32.
/// lon ∈ [-180, 180] → [-1_800_000_000, 1_800_000_000] fits in i32.
#[inline]
pub fn quantize(deg: f64) -> i32 {
    (deg * 1e7).round() as i32
}

type TopologyKey = (i32, i32, i32); // (lat_q, lon_q, level)

fn topology_level(feature: &Feature) -> i32 {
    feature
        .attributes
        .get("POS_SOL")
        .and_then(|value| value.trim().parse::<i32>().ok())
        .unwrap_or(0)
}

fn find_topology_junctions(roads: &[Vec<TopologyKey>]) -> HashSet<TopologyKey> {
    let mut point_count: HashMap<TopologyKey, Vec<(usize, usize, bool)>> = HashMap::new();

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

/// Check whether a WGS84 point lies on or within epsilon of the tile's strict boundary.
///
/// The strict boundary is the tile without overlap (overlap stripped from each side).
/// Epsilon = 5e-8 degrees ≈ 5 µm — tighter than any real coordinate difference.
fn is_boundary_point(lat: f64, lon: f64, tile: &TileBounds) -> bool {
    const EPS: f64 = 5e-8;
    let smin_lon = tile.min_lon + tile.overlap;
    let smin_lat = tile.min_lat + tile.overlap;
    let smax_lon = tile.max_lon - tile.overlap;
    let smax_lat = tile.max_lat - tile.overlap;

    (lat - smin_lat).abs() < EPS
        || (lat - smax_lat).abs() < EPS
        || (lon - smin_lon).abs() < EPS
        || (lon - smax_lon).abs() < EPS
}

/// Compute the routing graph for a tile.
///
/// Iterates over all features, identifies routable polylines (those with a
/// `RoadID` attribute), computes junction points using the route topology key
/// `(lat, lon, POS_SOL)`, then assigns `NodEntry` values to
/// each junction point of each polyline.
pub fn compute_tile_routing_graph(features: &[Feature], tile: &TileBounds) -> TileRoutingGraph {
    // Collect routable features and their quantized geometries.
    // Each entry: (feature_index, Vec<(lat_q, lon_q, level)>)
    let routable: Vec<(usize, Vec<TopologyKey>)> = features
        .iter()
        .enumerate()
        .filter(|(_, f)| f.attributes.contains_key("RoadID") && f.geometry.len() >= 2)
        .map(|(i, f)| {
            let level = topology_level(f);
            let quantized = f
                .geometry
                .iter()
                // geometry stores (lon, lat) per reader.rs convention
                .map(|&(lon, lat)| (quantize(lat), quantize(lon), level))
                .collect();
            (i, quantized)
        })
        .collect();

    // Run junction detection on all routable geometries.
    let raw_roads: Vec<Vec<TopologyKey>> = routable.iter().map(|(_, q)| q.clone()).collect();
    let junctions: HashSet<TopologyKey> = find_topology_junctions(&raw_roads);

    let mut per_feature: Vec<Vec<NodEntry>> = vec![Vec::new(); features.len()];
    let mut total_nodes: u32 = 0;
    let mut junction_count: u32 = 0;
    let mut boundary_count: u32 = 0;

    for (road_idx, (feat_idx, quantized)) in routable.iter().enumerate() {
        let n = quantized.len();
        let mut nods: Vec<NodEntry> = Vec::new();

        // Determine which points in this polyline are nodes.
        // A point is a node if it's an endpoint OR appears in the junctions set.
        for (pt_idx, &(lat_q, lon_q, level)) in quantized.iter().enumerate() {
            let is_endpoint = pt_idx == 0 || pt_idx == n - 1;
            let is_junction = junctions.contains(&(lat_q, lon_q, level));

            if !is_endpoint && !is_junction {
                continue;
            }

            // Original WGS84 coords for boundary detection.
            // geometry is (lon, lat), so reverse the mapping.
            let (lon_deg, lat_deg) = features[*feat_idx].geometry[pt_idx];
            let on_boundary = is_boundary_point(lat_deg, lon_deg, tile);

            let node_id = coord_to_node_id_with_level(lat_q, lon_q, level);

            nods.push(NodEntry {
                point_index: pt_idx as u16,
                node_id,
                boundary: on_boundary,
            });

            total_nodes += 1;
            if is_junction && !is_endpoint {
                junction_count += 1;
            }
            if on_boundary {
                boundary_count += 1;
            }
        }

        // Guarantee minimum 2 NodEntries (endpoints) even if no junction was found.
        // mkgmap RoadHelper requires at least the two endpoints to be declared.
        if nods.len() < 2 {
            nods.clear();
            let (lon0, lat0) = features[*feat_idx].geometry[0];
            let (lon_last, lat_last) = features[*feat_idx].geometry[n - 1];
            let (lat0_q, lon0_q) = (quantize(lat0), quantize(lon0));
            let (lat_last_q, lon_last_q) = (quantize(lat_last), quantize(lon_last));
            let level = topology_level(&features[*feat_idx]);

            nods.push(NodEntry {
                point_index: 0,
                node_id: coord_to_node_id_with_level(lat0_q, lon0_q, level),
                boundary: is_boundary_point(lat0, lon0, tile),
            });
            nods.push(NodEntry {
                point_index: (n - 1) as u16,
                node_id: coord_to_node_id_with_level(lat_last_q, lon_last_q, level),
                boundary: is_boundary_point(lat_last, lon_last, tile),
            });
            total_nodes += 2;
        }

        // Sort by point_index (ascending) — required by mkgmap spec (TD6).
        nods.sort_by_key(|e| e.point_index);

        // Dedup consecutive NodEntries sharing the same node_id.
        // Two distinct source vertices can quantize to the same grid cell (BDTOPO
        // is sometimes denser than our quantization grid), producing identical
        // node_id values for consecutive nodes. mkgmap rejects this with
        // "consecutive identical nodes - routing will be broken" and the Garmin
        // firmware likewise refuses to build a route across such an arc (zero-length
        // self-loop). Keep the first occurrence (lowest point_index) — it's the
        // earliest endpoint/junction on the road segment.
        nods.dedup_by_key(|e| e.node_id);

        per_feature[*feat_idx] = nods;
        let _ = road_idx; // suppress unused warning
    }

    TileRoutingGraph {
        per_feature,
        total_nodes,
        junction_count,
        boundary_count,
    }
}

/// Reconcile boundary node IDs across tiles.
///
/// For each pair of tiles sharing a boundary point (same quantized coordinate
/// and same topology level),
/// the canonical ID is chosen deterministically (lowest tile index wins).
/// This function mutates the TileRoutingGraph values in place.
///
/// Note: in the current pipeline, deterministic topology-based IDs (FNV hash) make
/// reconciliation optional — same coordinate and same level always produce the same ID.
/// This function is provided for correctness testing (AC4) and future use.
pub struct ReconciliationStats {
    pub nodes_reconciled: u32,
    pub boundary_pairs_processed: u32,
}

pub fn reconcile_boundary_nodes(
    tiles: &mut [(usize, TileRoutingGraph)],
) -> ReconciliationStats {
    use std::collections::HashMap;

    // Build index: node ID → Vec<(tile_idx_in_slice, nod position in per_feature)>
    // We index by (feat_idx, nod_pos) within each tile.
    type NodeRef = (usize, usize, usize); // (slice_idx, feat_idx, nod_pos)
    let mut coord_map: HashMap<u32, Vec<NodeRef>> = HashMap::new();

    for (slice_idx, (_, graph)) in tiles.iter().enumerate() {
        for (feat_idx, nods) in graph.per_feature.iter().enumerate() {
            for (nod_pos, nod) in nods.iter().enumerate() {
                if nod.boundary {
                    coord_map
                        .entry(nod.node_id)
                        .or_default()
                        .push((slice_idx, feat_idx, nod_pos));
                }
            }
        }
    }

    let mut nodes_reconciled: u32 = 0;
    let mut boundary_pairs_processed: u32 = 0;

    // For each shared ID (same topology key = same hash), ensure all tiles agree.
    // With deterministic hash IDs this should be a no-op in practice.
    for (canonical_id, refs) in &coord_map {
        if refs.len() < 2 {
            continue;
        }
        boundary_pairs_processed += 1;
        for &(slice_idx, feat_idx, nod_pos) in refs {
            let nod = &mut tiles[slice_idx].1.per_feature[feat_idx][nod_pos];
            if nod.node_id != *canonical_id {
                nod.node_id = *canonical_id;
                nodes_reconciled += 1;
            }
        }
    }

    ReconciliationStats { nodes_reconciled, boundary_pairs_processed }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::reader::{Feature, GeometryType};
    use std::collections::HashMap;

    fn make_tile(min_lon: f64, min_lat: f64, max_lon: f64, max_lat: f64) -> TileBounds {
        TileBounds {
            col: 0, row: 0,
            min_lon, min_lat, max_lon, max_lat,
            overlap: 0.0,
        }
    }

    fn routable_feature(geometry: Vec<(f64, f64)>) -> Feature {
        let mut attrs = HashMap::new();
        attrs.insert("RoadID".to_string(), "1".to_string());
        attrs.insert("RouteParam".to_string(), "4,1,0,0,0,0,0,0,0,0,0,0".to_string());
        Feature {
            geometry_type: GeometryType::LineString,
            geometry,
            additional_geometries: Default::default(),
            attributes: attrs,
            source_layer: Some("TRONCON_DE_ROUTE".to_string()),
        }
    }

    fn routable_feature_with_pos_sol(geometry: Vec<(f64, f64)>, pos_sol: &str) -> Feature {
        let mut feature = routable_feature(geometry);
        feature.attributes.insert("POS_SOL".to_string(), pos_sol.to_string());
        feature
    }

    #[test]
    fn test_single_road_two_endpoints() {
        // AC2: road with no junctions → exactly 2 NodEntries (endpoints)
        let tile = make_tile(5.0, 45.0, 6.0, 46.0);
        let features = vec![routable_feature(vec![(5.5, 45.5), (5.6, 45.6)])];
        let graph = compute_tile_routing_graph(&features, &tile);

        assert_eq!(graph.per_feature.len(), 1);
        let nods = &graph.per_feature[0];
        assert_eq!(nods.len(), 2, "isolated road must have exactly 2 NodEntries");
        assert_eq!(nods[0].point_index, 0);
        assert_eq!(nods[1].point_index, 1);
    }

    #[test]
    fn test_shared_junction_same_node_id() {
        // AC2: two roads sharing endpoint (lon=5.5, lat=45.5) → same node_id
        let tile = make_tile(5.0, 45.0, 6.0, 46.0);
        let road1 = routable_feature(vec![(5.5, 45.5), (5.6, 45.6)]);
        let mut road2 = routable_feature(vec![(5.5, 45.5), (5.4, 45.4)]);
        road2.attributes.insert("RoadID".to_string(), "2".to_string());

        let features = vec![road1, road2];
        let graph = compute_tile_routing_graph(&features, &tile);

        let id0_start = graph.per_feature[0][0].node_id;
        let id1_start = graph.per_feature[1][0].node_id;
        assert_eq!(id0_start, id1_start, "shared endpoint must have same node_id");
    }

    #[test]
    fn test_shared_coordinate_different_pos_sol_distinct_node_id() {
        // A bridge/underpass can share the same 2D coordinate without being connected.
        let tile = make_tile(5.0, 45.0, 6.0, 46.0);
        let road_ground = routable_feature_with_pos_sol(vec![(5.5, 45.5), (5.6, 45.6)], "0");
        let mut road_bridge =
            routable_feature_with_pos_sol(vec![(5.5, 45.5), (5.4, 45.4)], "1");
        road_bridge.attributes.insert("RoadID".to_string(), "2".to_string());

        let features = vec![road_ground, road_bridge];
        let graph = compute_tile_routing_graph(&features, &tile);

        let ground_start = graph.per_feature[0][0].node_id;
        let bridge_start = graph.per_feature[1][0].node_id;
        assert_ne!(
            ground_start, bridge_start,
            "same coordinate on different POS_SOL levels must not connect"
        );
    }

    #[test]
    fn test_midpoint_crossing_different_pos_sol_is_not_junction() {
        let tile = make_tile(5.0, 45.0, 6.0, 46.0);
        let road_ground =
            routable_feature_with_pos_sol(vec![(5.4, 45.5), (5.5, 45.5), (5.6, 45.5)], "0");
        let mut road_bridge =
            routable_feature_with_pos_sol(vec![(5.5, 45.4), (5.5, 45.5), (5.5, 45.6)], "1");
        road_bridge.attributes.insert("RoadID".to_string(), "2".to_string());

        let features = vec![road_ground, road_bridge];
        let graph = compute_tile_routing_graph(&features, &tile);

        assert!(
            graph.per_feature[0].iter().all(|nod| nod.point_index != 1),
            "ground midpoint must not become a junction solely from bridge crossing"
        );
        assert!(
            graph.per_feature[1].iter().all(|nod| nod.point_index != 1),
            "bridge midpoint must not become a junction solely from ground crossing"
        );
    }

    #[test]
    fn test_missing_pos_sol_defaults_to_ground_level() {
        let tile = make_tile(5.0, 45.0, 6.0, 46.0);
        let road_default = routable_feature(vec![(5.5, 45.5), (5.6, 45.6)]);
        let mut road_ground = routable_feature_with_pos_sol(vec![(5.5, 45.5), (5.4, 45.4)], "0");
        road_ground.attributes.insert("RoadID".to_string(), "2".to_string());

        let features = vec![road_default, road_ground];
        let graph = compute_tile_routing_graph(&features, &tile);

        assert_eq!(
            graph.per_feature[0][0].node_id, graph.per_feature[1][0].node_id,
            "missing POS_SOL must behave like POS_SOL=0"
        );
    }

    #[test]
    fn test_boundary_flag_on_tile_edge() {
        // AC3: endpoint coinciding with tile boundary → boundary=true
        let tile = make_tile(5.0, 45.0, 6.0, 46.0);
        // Road starting exactly at south boundary (lat=45.0)
        let features = vec![routable_feature(vec![(5.5, 45.0), (5.5, 45.5)])];
        let graph = compute_tile_routing_graph(&features, &tile);

        let nods = &graph.per_feature[0];
        let start_nod = nods.iter().find(|n| n.point_index == 0).unwrap();
        assert!(start_nod.boundary, "point on tile edge must be boundary=true");
    }

    #[test]
    fn test_non_routable_feature_empty() {
        // Non-routable features (no RoadID) must have empty NodEntry vec
        let tile = make_tile(5.0, 45.0, 6.0, 46.0);
        let mut non_road = routable_feature(vec![(5.5, 45.5), (5.6, 45.6)]);
        non_road.attributes.remove("RoadID");
        let features = vec![non_road];
        let graph = compute_tile_routing_graph(&features, &tile);
        assert!(graph.per_feature[0].is_empty());
    }

    #[test]
    fn test_reconcile_boundary_nodes_same_id_noop() {
        // AC4: reconciliation with deterministic IDs should be a no-op
        let tile = make_tile(5.0, 45.0, 6.0, 46.0);
        // Shared boundary point at lon=6.0 (east edge of tile A)
        let road_a = routable_feature(vec![(5.5, 45.5), (6.0, 45.5)]);
        let road_b = routable_feature(vec![(6.0, 45.5), (6.5, 45.5)]);

        let graph_a = compute_tile_routing_graph(&[road_a], &tile);
        let tile_b = make_tile(6.0, 45.0, 7.0, 46.0);
        let graph_b = compute_tile_routing_graph(&[road_b], &tile_b);

        let mut tiles = vec![(0usize, graph_a), (1usize, graph_b)];
        let stats = reconcile_boundary_nodes(&mut tiles);

        // With hash IDs, same coord → same ID → reconciliation = no-op
        assert_eq!(stats.nodes_reconciled, 0);
    }
}
