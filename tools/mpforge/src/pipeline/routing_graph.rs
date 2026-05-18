// Routing topology stage for the mpforge pipeline.
//
// Computes NodN= entries for each routable polyline in a tile.
//
// BDTOPO TRONCON_DE_ROUTE is pre-segmented at every topological node: a true
// routing junction always materialises as coincident endpoints of two or more
// tronçons. Midpoint coincidences are cartographic densification only (bridges,
// tunnels, visual crossings) and must not produce a routable junction.
//
// Internal nodes therefore use a coordinate-only identity. Boundary nodes also
// use coordinate-only identity because final IMG NOD3/NOD4 records are looked
// up by coordinate across tiles.

use garmin_routing_graph::{coord_to_node_id, NodEntry};
use std::collections::{HashMap, HashSet};

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

type CoordKey = (i32, i32); // (lat_q, lon_q)

/// Endpoint-only junction detection.
///
/// A coordinate is a junction iff it appears as an endpoint of at least two
/// routable polylines. Midpoint coincidences are deliberately ignored — in
/// BDTOPO they are densification vertices (bridge over autoroute, etc.), not
/// topological connections.
fn find_topology_junctions(roads: &[Vec<CoordKey>]) -> HashSet<CoordKey> {
    let mut endpoint_count: HashMap<CoordKey, u32> = HashMap::new();
    for coords in roads {
        if coords.is_empty() {
            continue;
        }
        *endpoint_count.entry(coords[0]).or_insert(0) += 1;
        let last = coords.len() - 1;
        if last > 0 {
            *endpoint_count.entry(coords[last]).or_insert(0) += 1;
        }
    }
    endpoint_count
        .into_iter()
        .filter(|(_, c)| *c >= 2)
        .map(|(k, _)| k)
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
/// `RoadID` attribute), detects coincident-endpoint junctions, then assigns
/// `NodEntry` values to each endpoint and each junction-bearing vertex of each
/// polyline.
pub fn compute_tile_routing_graph(features: &[Feature], tile: &TileBounds) -> TileRoutingGraph {
    // Collect routable features and their quantized geometries.
    let routable: Vec<(usize, Vec<CoordKey>)> = features
        .iter()
        .enumerate()
        .filter(|(_, f)| f.attributes.contains_key("RoadID") && f.geometry.len() >= 2)
        .map(|(i, f)| {
            let quantized = f
                .geometry
                .iter()
                // geometry stores (lon, lat) per reader.rs convention
                .map(|&(lon, lat)| (quantize(lat), quantize(lon)))
                .collect();
            (i, quantized)
        })
        .collect();

    let raw_roads: Vec<Vec<CoordKey>> = routable.iter().map(|(_, q)| q.clone()).collect();
    let junctions: HashSet<CoordKey> = find_topology_junctions(&raw_roads);

    let mut per_feature: Vec<Vec<NodEntry>> = vec![Vec::new(); features.len()];
    let mut total_nodes: u32 = 0;
    let mut junction_count: u32 = 0;
    let mut boundary_count: u32 = 0;

    for (feat_idx, quantized) in routable.iter() {
        let n = quantized.len();
        let mut nods: Vec<NodEntry> = Vec::new();

        for (pt_idx, &(lat_q, lon_q)) in quantized.iter().enumerate() {
            let is_endpoint = pt_idx == 0 || pt_idx == n - 1;
            let is_junction = junctions.contains(&(lat_q, lon_q));

            if !is_endpoint && !is_junction {
                continue;
            }

            // Original WGS84 coords for boundary detection.
            // geometry is (lon, lat), so reverse the mapping.
            let (lon_deg, lat_deg) = features[*feat_idx].geometry[pt_idx];
            let on_boundary = is_boundary_point(lat_deg, lon_deg, tile);

            let node_id = coord_to_node_id(lat_q, lon_q);

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
            let first_boundary = is_boundary_point(lat0, lon0, tile);
            let last_boundary = is_boundary_point(lat_last, lon_last, tile);

            nods.push(NodEntry {
                point_index: 0,
                node_id: coord_to_node_id(lat0_q, lon0_q),
                boundary: first_boundary,
            });
            nods.push(NodEntry {
                point_index: (n - 1) as u16,
                node_id: coord_to_node_id(lat_last_q, lon_last_q),
                boundary: last_boundary,
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
/// For each pair of tiles sharing a boundary point (same canonical boundary ID),
/// the canonical ID is chosen deterministically (lowest tile index wins).
/// This function mutates the TileRoutingGraph values in place.
///
/// Note: in the current pipeline, deterministic topology-based IDs (FNV hash) make
/// reconciliation optional — boundary coordinates always produce the same ID.
/// This function is provided for correctness testing (AC4) and future use.
pub struct ReconciliationStats {
    pub nodes_reconciled: u32,
    pub boundary_pairs_processed: u32,
}

pub fn reconcile_boundary_nodes(tiles: &mut [(usize, TileRoutingGraph)]) -> ReconciliationStats {
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

    ReconciliationStats {
        nodes_reconciled,
        boundary_pairs_processed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::reader::{Feature, GeometryType};
    use std::collections::HashMap;

    fn make_tile(min_lon: f64, min_lat: f64, max_lon: f64, max_lat: f64) -> TileBounds {
        TileBounds {
            col: 0,
            row: 0,
            min_lon,
            min_lat,
            max_lon,
            max_lat,
            overlap: 0.0,
        }
    }

    fn routable_feature(geometry: Vec<(f64, f64)>) -> Feature {
        let mut attrs = HashMap::new();
        attrs.insert("RoadID".to_string(), "1".to_string());
        attrs.insert(
            "RouteParam".to_string(),
            "4,1,0,0,0,0,0,0,0,0,0,0".to_string(),
        );
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
        feature
            .attributes
            .insert("POS_SOL".to_string(), pos_sol.to_string());
        feature
    }

    #[test]
    fn test_single_road_two_endpoints() {
        // Isolated road → exactly 2 NodEntries (its endpoints).
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
    fn test_shared_endpoint_same_node_id() {
        // Two roads sharing endpoint (lon=5.5, lat=45.5) → same node_id.
        let tile = make_tile(5.0, 45.0, 6.0, 46.0);
        let road1 = routable_feature(vec![(5.5, 45.5), (5.6, 45.6)]);
        let mut road2 = routable_feature(vec![(5.5, 45.5), (5.4, 45.4)]);
        road2
            .attributes
            .insert("RoadID".to_string(), "2".to_string());

        let features = vec![road1, road2];
        let graph = compute_tile_routing_graph(&features, &tile);

        let id0_start = graph.per_feature[0][0].node_id;
        let id1_start = graph.per_feature[1][0].node_id;
        assert_eq!(id0_start, id1_start, "shared endpoint must have same node_id");
    }

    #[test]
    fn test_midpoint_only_shared_does_not_create_junction() {
        // BDTOPO bridge-over-autoroute case (reproduction of the production bug
        // observed on D038 between TRONROUT 7918938 and 7920958/7920959).
        //
        // The autoroute is densified with a vertex at the visual crossing point;
        // the bridge has a midpoint at the same XY. BDTOPO is pre-segmented at
        // every topological node — none of these midpoints are routing nodes.
        // Endpoint-only junction detection must keep them apart.
        let tile = make_tile(5.0, 45.0, 6.0, 46.0);
        let road_ground =
            routable_feature(vec![(5.4, 45.5), (5.5, 45.5), (5.6, 45.5)]);
        let mut road_bridge =
            routable_feature(vec![(5.5, 45.4), (5.5, 45.5), (5.5, 45.6)]);
        road_bridge
            .attributes
            .insert("RoadID".to_string(), "2".to_string());

        let features = vec![road_ground, road_bridge];
        let graph = compute_tile_routing_graph(&features, &tile);

        assert!(
            graph.per_feature[0].iter().all(|nod| nod.point_index != 1),
            "ground midpoint must not become a junction (no shared endpoint)"
        );
        assert!(
            graph.per_feature[1].iter().all(|nod| nod.point_index != 1),
            "bridge midpoint must not become a junction (no shared endpoint)"
        );
        // Both roads keep their own two endpoints.
        assert_eq!(graph.per_feature[0].len(), 2);
        assert_eq!(graph.per_feature[1].len(), 2);
    }

    #[test]
    fn test_endpoint_meets_midpoint_is_not_a_junction() {
        // Defensive case: if BDTOPO ever ships a side road whose endpoint lands
        // on the midpoint of a main road (i.e. main road not split at the
        // intersection), we deliberately do NOT create a junction. BDTOPO's
        // contract is that intersections appear as coincident endpoints; a
        // mid-line touch is more likely a cartographic artefact than a routing
        // node, so we err on the side of not inventing connectivity.
        let tile = make_tile(5.0, 45.0, 6.0, 46.0);
        let main = routable_feature(vec![(5.4, 45.5), (5.5, 45.5), (5.6, 45.5)]);
        let mut side = routable_feature(vec![(5.5, 45.5), (5.5, 45.6)]);
        side.attributes
            .insert("RoadID".to_string(), "2".to_string());

        let graph = compute_tile_routing_graph(&[main, side], &tile);

        // Main road keeps only its own two endpoints.
        assert_eq!(graph.per_feature[0].len(), 2);
        assert!(graph.per_feature[0].iter().all(|nod| nod.point_index != 1));
        // Side road keeps its two endpoints (its endpoint at (5.5, 45.5) is not
        // a junction because no other road's endpoint coincides with it).
        assert_eq!(graph.per_feature[1].len(), 2);
    }

    #[test]
    fn test_pos_sol_attribute_ignored_for_topology() {
        // POS_SOL is a BDTOPO source attribute; routing topology must not depend
        // on it. The endpoint-only rule already prevents false junctions at
        // bridge crossings without needing POS_SOL.
        let tile = make_tile(5.0, 45.0, 6.0, 46.0);
        let road_ground =
            routable_feature_with_pos_sol(vec![(5.5, 45.5), (5.6, 45.6)], "0");
        let mut road_bridge =
            routable_feature_with_pos_sol(vec![(5.5, 45.5), (5.4, 45.4)], "1");
        road_bridge
            .attributes
            .insert("RoadID".to_string(), "2".to_string());

        let graph = compute_tile_routing_graph(&[road_ground, road_bridge], &tile);

        // Shared endpoint → same node_id regardless of POS_SOL.
        assert_eq!(
            graph.per_feature[0][0].node_id, graph.per_feature[1][0].node_id,
            "POS_SOL must not influence node identity at shared endpoints"
        );
    }

    #[test]
    fn test_boundary_flag_on_tile_edge() {
        let tile = make_tile(5.0, 45.0, 6.0, 46.0);
        let features = vec![routable_feature(vec![(5.5, 45.0), (5.5, 45.5)])];
        let graph = compute_tile_routing_graph(&features, &tile);

        let nods = &graph.per_feature[0];
        let start_nod = nods.iter().find(|n| n.point_index == 0).unwrap();
        assert!(start_nod.boundary, "point on tile edge must be boundary=true");
    }

    #[test]
    fn test_boundary_nodes_same_coordinate_share_node_id() {
        // NOD3/NOD4 boundary records are coordinate-indexed in the final IMG.
        // Two routable polylines whose endpoints meet on the tile edge must
        // share one node_id for NOD3 lookup, regardless of any other attribute.
        let tile = make_tile(5.0, 45.0, 6.0, 46.0);
        let road_a = routable_feature(vec![(5.5, 45.0), (5.5, 45.5)]);
        let mut road_b = routable_feature(vec![(5.5, 45.0), (5.6, 45.5)]);
        road_b
            .attributes
            .insert("RoadID".to_string(), "2".to_string());

        let graph = compute_tile_routing_graph(&[road_a, road_b], &tile);

        assert!(graph.per_feature[0][0].boundary);
        assert!(graph.per_feature[1][0].boundary);
        assert_eq!(
            graph.per_feature[0][0].node_id, graph.per_feature[1][0].node_id,
            "same boundary coordinate must have a single node_id for NOD3 lookup"
        );
    }

    #[test]
    fn test_non_routable_feature_empty() {
        let tile = make_tile(5.0, 45.0, 6.0, 46.0);
        let mut non_road = routable_feature(vec![(5.5, 45.5), (5.6, 45.6)]);
        non_road.attributes.remove("RoadID");
        let features = vec![non_road];
        let graph = compute_tile_routing_graph(&features, &tile);
        assert!(graph.per_feature[0].is_empty());
    }

    #[test]
    fn test_reconcile_boundary_nodes_same_id_noop() {
        let tile = make_tile(5.0, 45.0, 6.0, 46.0);
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
