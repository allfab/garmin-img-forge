//! Routing attributes post-processor for BDTOPO → Polish Map.
//!
//! Story 14.1: Computes RouteParam, DirIndicator, Roundabout, RoadID,
//! MaxHeight, MaxWeight, MaxWidth, MaxLength from BDTOPO source attributes.
//!
//! This module implements Approach 2 (dedicated post-processor) as recommended
//! in the story Dev Notes, avoiding combinatorial explosion in the rules YAML.

use std::collections::HashMap;
use tracing::debug;

/// Thread-local road ID counter for sequential assignment within a tile.
pub struct RoadIdCounter {
    next_id: u32,
}

impl Default for RoadIdCounter {
    fn default() -> Self {
        Self { next_id: 1 }
    }
}

impl RoadIdCounter {
    /// Create a new counter starting at 1.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the next road ID and increment the counter.
    pub fn next_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

/// Convert BDTOPO VIT_MOY_VL (km/h) to Garmin Speed class (0-7).
///
/// Continuous ranges (no gaps):
/// - [0, 10)   → 0 (Chemin/Sentier)
/// - [10, 20)  → 1 (Route empierrée lente)
/// - [20, 30)  → 2 (Zone 30/résidentiel)
/// - [30, 45)  → 3 (Urbain standard)
/// - [45, 60)  → 4 (Route communale)
/// - [60, 75)  → 5 (Départementale)
/// - [75, 95)  → 6 (Nationale/express)
/// - [95, +∞)  → 7 (Autoroute)
pub fn vit_to_speed(vit_moy_vl: f64) -> u8 {
    if vit_moy_vl < 10.0 {
        0
    } else if vit_moy_vl < 20.0 {
        1
    } else if vit_moy_vl < 30.0 {
        2
    } else if vit_moy_vl < 45.0 {
        3
    } else if vit_moy_vl < 60.0 {
        4
    } else if vit_moy_vl < 75.0 {
        5
    } else if vit_moy_vl < 95.0 {
        6
    } else {
        7
    }
}

/// Convert BDTOPO CL_ADMIN + NATURE to Garmin Road Class (0-4).
///
/// Priority: CL_ADMIN first, then NATURE fallback.
pub fn admin_nature_to_class(cl_admin: &str, nature: &str) -> u8 {
    // CL_ADMIN-based classification (highest priority)
    match cl_admin {
        "Autoroute" => 4,
        "Nationale" => 3,
        "Départementale" => 2,
        "Route intercommunale" => 1,
        _ => {
            // NATURE-based fallback when CL_ADMIN is empty or unrecognized
            match nature {
                "Type autoroutier" => 4,
                "Route à 2 chaussées" | "Route à 1 chaussée" | "Bretelle" => 1,
                "Route empierrée" | "Chemin" | "Sentier" | "Escalier"
                | "Bac ou liaison maritime" | "Bac auto" | "Bac piéton" => 0,
                // Default for unrecognized NATURE: class 1
                _ => 1,
            }
        }
    }
}

/// Convert BDTOPO SENS to (oneway, dir_indicator).
///
/// Returns (oneway_bit, dir_indicator_value).
pub fn sens_to_oneway(sens: &str) -> (u8, i32) {
    match sens {
        "Sens direct" => (1, 1),
        "Sens inverse" => (1, -1),
        "Double sens" | "Sans objet" | "" => (0, 0),
        _ => (0, 0),
    }
}

/// Convert BDTOPO ACCES_VL to (toll, denied_car, denied_bus, denied_truck).
pub fn acces_vl_to_bits(acces_vl: &str) -> (u8, u8, u8, u8) {
    match acces_vl {
        "A péage" | "À péage" => (1, 0, 0, 0),
        "Physiquement impossible" => (0, 1, 1, 1),
        // "Libre" and "Restreint aux ayants droit" → all zeros
        _ => (0, 0, 0, 0),
    }
}

/// Convert BDTOPO ACCES_PED to denied_foot bit.
pub fn acces_ped_to_denied_foot(acces_ped: &str) -> u8 {
    match acces_ped {
        "Libre" | "" => 0,
        // "Restreint", "Passage difficile", "A péage" → denied
        _ => 1,
    }
}

/// Convert BDTOPO restriction values to Polish Map format (centimeters for height, centithons for weight).
///
/// RESTR_H (meters) → MaxHeight (centimeters as integer)
/// RESTR_P (tonnes) → MaxWeight (centithons as integer, i.e., tonnes × 1000)
/// RESTR_LAR (meters) → MaxWidth (centimeters as integer)
/// RESTR_LON (meters) → MaxLength (centimeters as integer)
///
/// Panics: values must be non-negative. Caller must guard with `> 0.0`.
pub fn meters_to_centimeters(value_m: f64) -> u32 {
    debug_assert!(value_m >= 0.0, "meters_to_centimeters: negative input {value_m}");
    (value_m * 100.0).round() as u32
}

pub fn tonnes_to_centithons(value_t: f64) -> u32 {
    debug_assert!(value_t >= 0.0, "tonnes_to_centithons: negative input {value_t}");
    (value_t * 1000.0).round() as u32
}

/// RouteParam components for composing the Polish Map RouteParam string.
pub struct RouteParamComponents {
    pub speed: u8,
    pub road_class: u8,
    pub oneway: u8,
    pub toll: u8,
    pub denied_car: u8,
    pub denied_bus: u8,
    pub denied_foot: u8,
    pub denied_truck: u8,
}

/// Compose RouteParam string from individual components.
///
/// Format: speed,road_class,one_way,toll,denied_emergency,denied_delivery,
///         denied_car,denied_bus,denied_taxi,denied_pedestrian,denied_bicycle,denied_truck
pub fn compose_route_param(c: &RouteParamComponents) -> String {
    format!(
        "{},{},{},{},0,0,{},{},0,{},0,{}",
        c.speed, c.road_class, c.oneway, c.toll, c.denied_car, c.denied_bus, c.denied_foot, c.denied_truck
    )
}

/// Compute routing attributes from BDTOPO source attributes.
///
/// Returns a HashMap of Polish Map routing attributes to merge into the
/// feature attributes after rule application.
///
/// Only computes routing for features from TRONCON_DE_ROUTE layer.
pub fn compute_route_attrs(
    source_attrs: &HashMap<String, String>,
    road_id_counter: &mut RoadIdCounter,
) -> HashMap<String, String> {
    let mut result = HashMap::new();

    // Speed from VIT_MOY_VL
    let vit_moy_vl: f64 = source_attrs
        .get("VIT_MOY_VL")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0);
    let speed = vit_to_speed(vit_moy_vl);

    // Road class from CL_ADMIN + NATURE
    let cl_admin = source_attrs.get("CL_ADMIN").map(|s| s.as_str()).unwrap_or("");
    let nature = source_attrs.get("NATURE").map(|s| s.as_str()).unwrap_or("");
    let road_class = admin_nature_to_class(cl_admin, nature);

    // Oneway + DirIndicator from SENS
    let sens = source_attrs.get("SENS").map(|s| s.as_str()).unwrap_or("");
    let (oneway, dir_indicator) = sens_to_oneway(sens);

    // Toll + denied bits from ACCES_VL
    let acces_vl = source_attrs.get("ACCES_VL").map(|s| s.as_str()).unwrap_or("");
    let (toll, denied_car, denied_bus, denied_truck) = acces_vl_to_bits(acces_vl);

    // Denied foot from ACCES_PED
    let acces_ped = source_attrs.get("ACCES_PED").map(|s| s.as_str()).unwrap_or("");
    let denied_foot = acces_ped_to_denied_foot(acces_ped);

    // Compose RouteParam
    let route_param = compose_route_param(&RouteParamComponents {
        speed, road_class, oneway, toll, denied_car, denied_bus, denied_foot, denied_truck,
    });
    result.insert("RouteParam".to_string(), route_param);

    // DirIndicator (Option B: -1/0/1 convention for mpforge→imgforge)
    result.insert("DirIndicator".to_string(), dir_indicator.to_string());

    // RoadID auto-incremental
    let road_id = road_id_counter.next_id();
    result.insert("RoadID".to_string(), road_id.to_string());

    // Roundabout detection
    if nature == "Rond-point" {
        result.insert("Roundabout".to_string(), "1".to_string());
    }

    // Physical restrictions (custom extension, AC3)
    if let Some(restr_h) = source_attrs.get("RESTR_H").and_then(|v| v.parse::<f64>().ok()) {
        if restr_h > 0.0 {
            result.insert("MaxHeight".to_string(), meters_to_centimeters(restr_h).to_string());
        }
    }
    if let Some(restr_p) = source_attrs.get("RESTR_P").and_then(|v| v.parse::<f64>().ok()) {
        if restr_p > 0.0 {
            result.insert("MaxWeight".to_string(), tonnes_to_centithons(restr_p).to_string());
        }
    }
    if let Some(restr_lar) = source_attrs.get("RESTR_LAR").and_then(|v| v.parse::<f64>().ok()) {
        if restr_lar > 0.0 {
            result.insert("MaxWidth".to_string(), meters_to_centimeters(restr_lar).to_string());
        }
    }
    if let Some(restr_lon) = source_attrs.get("RESTR_LON").and_then(|v| v.parse::<f64>().ok()) {
        if restr_lon > 0.0 {
            result.insert("MaxLength".to_string(), meters_to_centimeters(restr_lon).to_string());
        }
    }

    debug!(
        route_param = %result.get("RouteParam").unwrap(),
        road_id = road_id,
        dir_indicator = dir_indicator,
        "Computed routing attributes"
    );

    result
}

/// Check if a source layer is routable (should receive routing attributes).
pub fn is_routable_layer(layer_name: &str) -> bool {
    layer_name == "TRONCON_DE_ROUTE"
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Task 1.5: Speed conversion tests (8 speed classes)
    // =========================================================================

    #[test]
    fn test_vit_to_speed_class_0_chemin() {
        assert_eq!(vit_to_speed(0.0), 0);
        assert_eq!(vit_to_speed(5.0), 0);
        assert_eq!(vit_to_speed(8.0), 0);
    }

    #[test]
    fn test_vit_to_speed_class_1_empierree() {
        assert_eq!(vit_to_speed(10.0), 1);
        assert_eq!(vit_to_speed(15.0), 1);
    }

    #[test]
    fn test_vit_to_speed_class_2_zone30() {
        assert_eq!(vit_to_speed(20.0), 2);
        assert_eq!(vit_to_speed(25.0), 2);
    }

    #[test]
    fn test_vit_to_speed_class_3_urbain() {
        assert_eq!(vit_to_speed(30.0), 3);
        assert_eq!(vit_to_speed(40.0), 3);
    }

    #[test]
    fn test_vit_to_speed_class_4_communale() {
        assert_eq!(vit_to_speed(45.0), 4);
        assert_eq!(vit_to_speed(50.0), 4);
        assert_eq!(vit_to_speed(55.0), 4);
    }

    #[test]
    fn test_vit_to_speed_class_5_departementale() {
        assert_eq!(vit_to_speed(60.0), 5);
        assert_eq!(vit_to_speed(70.0), 5);
    }

    #[test]
    fn test_vit_to_speed_class_6_nationale() {
        assert_eq!(vit_to_speed(75.0), 6);
        assert_eq!(vit_to_speed(80.0), 6);
        assert_eq!(vit_to_speed(90.0), 6);
    }

    #[test]
    fn test_vit_to_speed_class_7_autoroute() {
        assert_eq!(vit_to_speed(95.0), 7);
        assert_eq!(vit_to_speed(110.0), 7);
        assert_eq!(vit_to_speed(130.0), 7);
    }

    // =========================================================================
    // Task 1.5: Road class conversion tests (5 classes)
    // =========================================================================

    #[test]
    fn test_class_autoroute() {
        assert_eq!(admin_nature_to_class("Autoroute", ""), 4);
        assert_eq!(admin_nature_to_class("Autoroute", "Route à 2 chaussées"), 4);
    }

    #[test]
    fn test_class_nationale() {
        assert_eq!(admin_nature_to_class("Nationale", ""), 3);
        assert_eq!(admin_nature_to_class("Nationale", "Rond-point"), 3);
    }

    #[test]
    fn test_class_departementale() {
        assert_eq!(admin_nature_to_class("Départementale", ""), 2);
    }

    #[test]
    fn test_class_intercommunale() {
        assert_eq!(admin_nature_to_class("Route intercommunale", ""), 1);
    }

    #[test]
    fn test_class_nature_type_autoroutier() {
        assert_eq!(admin_nature_to_class("", "Type autoroutier"), 4);
    }

    #[test]
    fn test_class_nature_2_chaussees() {
        assert_eq!(admin_nature_to_class("", "Route à 2 chaussées"), 1);
    }

    #[test]
    fn test_class_nature_1_chaussee() {
        assert_eq!(admin_nature_to_class("", "Route à 1 chaussée"), 1);
    }

    #[test]
    fn test_class_nature_bretelle() {
        assert_eq!(admin_nature_to_class("", "Bretelle"), 1);
    }

    #[test]
    fn test_class_nature_empierree() {
        assert_eq!(admin_nature_to_class("", "Route empierrée"), 0);
    }

    #[test]
    fn test_class_nature_chemin() {
        assert_eq!(admin_nature_to_class("", "Chemin"), 0);
    }

    #[test]
    fn test_class_nature_sentier() {
        assert_eq!(admin_nature_to_class("", "Sentier"), 0);
    }

    #[test]
    fn test_class_nature_bac() {
        assert_eq!(admin_nature_to_class("", "Bac ou liaison maritime"), 0);
    }

    // =========================================================================
    // Task 1.5: SENS conversion tests (4 values)
    // =========================================================================

    #[test]
    fn test_sens_double_sens() {
        let (oneway, dir) = sens_to_oneway("Double sens");
        assert_eq!(oneway, 0);
        assert_eq!(dir, 0);
    }

    #[test]
    fn test_sens_direct() {
        let (oneway, dir) = sens_to_oneway("Sens direct");
        assert_eq!(oneway, 1);
        assert_eq!(dir, 1);
    }

    #[test]
    fn test_sens_inverse() {
        let (oneway, dir) = sens_to_oneway("Sens inverse");
        assert_eq!(oneway, 1);
        assert_eq!(dir, -1);
    }

    #[test]
    fn test_sens_sans_objet() {
        let (oneway, dir) = sens_to_oneway("Sans objet");
        assert_eq!(oneway, 0);
        assert_eq!(dir, 0);
    }

    #[test]
    fn test_sens_empty() {
        let (oneway, dir) = sens_to_oneway("");
        assert_eq!(oneway, 0);
        assert_eq!(dir, 0);
    }

    // =========================================================================
    // Task 1.5: ACCES_VL conversion tests (4 values)
    // =========================================================================

    #[test]
    fn test_acces_vl_libre() {
        let (toll, car, bus, truck) = acces_vl_to_bits("Libre");
        assert_eq!((toll, car, bus, truck), (0, 0, 0, 0));
    }

    #[test]
    fn test_acces_vl_peage() {
        let (toll, car, bus, truck) = acces_vl_to_bits("A péage");
        assert_eq!((toll, car, bus, truck), (1, 0, 0, 0));
    }

    #[test]
    fn test_acces_vl_restreint() {
        let (toll, car, bus, truck) = acces_vl_to_bits("Restreint aux ayants droit");
        assert_eq!((toll, car, bus, truck), (0, 0, 0, 0));
    }

    #[test]
    fn test_acces_vl_impossible() {
        let (toll, car, bus, truck) = acces_vl_to_bits("Physiquement impossible");
        assert_eq!((toll, car, bus, truck), (0, 1, 1, 1));
    }

    // =========================================================================
    // Task 1.5: ACCES_PED conversion tests
    // =========================================================================

    #[test]
    fn test_acces_ped_libre() {
        assert_eq!(acces_ped_to_denied_foot("Libre"), 0);
    }

    #[test]
    fn test_acces_ped_restreint() {
        assert_eq!(acces_ped_to_denied_foot("Restreint"), 1);
    }

    #[test]
    fn test_acces_ped_passage_difficile() {
        assert_eq!(acces_ped_to_denied_foot("Passage difficile"), 1);
    }

    #[test]
    fn test_acces_ped_empty() {
        assert_eq!(acces_ped_to_denied_foot(""), 0);
    }

    // =========================================================================
    // Task 1.5: Physical restrictions conversion
    // =========================================================================

    #[test]
    fn test_meters_to_centimeters() {
        assert_eq!(meters_to_centimeters(3.50), 350);
        assert_eq!(meters_to_centimeters(2.80), 280);
        assert_eq!(meters_to_centimeters(0.0), 0);
    }

    #[test]
    fn test_tonnes_to_centithons() {
        assert_eq!(tonnes_to_centithons(19.0), 19000);
        assert_eq!(tonnes_to_centithons(3.5), 3500);
        assert_eq!(tonnes_to_centithons(0.0), 0);
    }

    // =========================================================================
    // Task 1.5: RouteParam composition
    // =========================================================================

    #[test]
    fn test_compose_route_param_autoroute_peage() {
        // AC1: VIT_MOY_VL=80→speed=6, CL_ADMIN=Nationale→class=3, Sens direct→oneway=1, péage→toll=1
        let rp = compose_route_param(&RouteParamComponents {
            speed: 6, road_class: 3, oneway: 1, toll: 1,
            denied_car: 0, denied_bus: 0, denied_foot: 0, denied_truck: 0,
        });
        assert_eq!(rp, "6,3,1,1,0,0,0,0,0,0,0,0");
    }

    #[test]
    fn test_compose_route_param_chemin() {
        let rp = compose_route_param(&RouteParamComponents {
            speed: 0, road_class: 0, oneway: 0, toll: 0,
            denied_car: 0, denied_bus: 0, denied_foot: 0, denied_truck: 0,
        });
        assert_eq!(rp, "0,0,0,0,0,0,0,0,0,0,0,0");
    }

    #[test]
    fn test_compose_route_param_denied_all() {
        let rp = compose_route_param(&RouteParamComponents {
            speed: 3, road_class: 2, oneway: 0, toll: 0,
            denied_car: 1, denied_bus: 1, denied_foot: 1, denied_truck: 1,
        });
        assert_eq!(rp, "3,2,0,0,0,0,1,1,0,1,0,1");
    }

    // =========================================================================
    // Task 1.5: Full compute_route_attrs integration tests
    // =========================================================================

    /// AC1: Complete RouteParam from BDTOPO attributes
    #[test]
    fn test_compute_ac1_nationale_peage_sens_direct() {
        let source = HashMap::from([
            ("VIT_MOY_VL".into(), "80".into()),
            ("SENS".into(), "Sens direct".into()),
            ("CL_ADMIN".into(), "Nationale".into()),
            ("ACCES_VL".into(), "A péage".into()),
            ("NATURE".into(), "Route à 1 chaussée".into()),
        ]);
        let mut counter = RoadIdCounter::new();
        let result = compute_route_attrs(&source, &mut counter);

        assert_eq!(result.get("RouteParam").unwrap(), "6,3,1,1,0,0,0,0,0,0,0,0");
        assert_eq!(result.get("DirIndicator").unwrap(), "1");
        assert_eq!(result.get("RoadID").unwrap(), "1");
        assert!(!result.contains_key("Roundabout"));
    }

    /// AC2: Roundabout detection
    #[test]
    fn test_compute_ac2_roundabout() {
        let source = HashMap::from([
            ("VIT_MOY_VL".into(), "30".into()),
            ("NATURE".into(), "Rond-point".into()),
            ("SENS".into(), "Sens direct".into()),
        ]);
        let mut counter = RoadIdCounter::new();
        let result = compute_route_attrs(&source, &mut counter);

        assert_eq!(result.get("Roundabout").unwrap(), "1");
    }

    /// AC3: Physical restrictions
    #[test]
    fn test_compute_ac3_restrictions() {
        let source = HashMap::from([
            ("VIT_MOY_VL".into(), "50".into()),
            ("RESTR_H".into(), "3.50".into()),
            ("RESTR_P".into(), "19.00".into()),
            ("NATURE".into(), "Route à 1 chaussée".into()),
        ]);
        let mut counter = RoadIdCounter::new();
        let result = compute_route_attrs(&source, &mut counter);

        assert_eq!(result.get("MaxHeight").unwrap(), "350");
        assert_eq!(result.get("MaxWeight").unwrap(), "19000");
    }

    /// AC4: DirIndicator for Sens inverse
    #[test]
    fn test_compute_ac4_sens_inverse() {
        let source = HashMap::from([
            ("VIT_MOY_VL".into(), "50".into()),
            ("SENS".into(), "Sens inverse".into()),
            ("CL_ADMIN".into(), "Départementale".into()),
            ("NATURE".into(), "Route à 1 chaussée".into()),
        ]);
        let mut counter = RoadIdCounter::new();
        let result = compute_route_attrs(&source, &mut counter);

        assert_eq!(result.get("DirIndicator").unwrap(), "-1");
        // oneway=1 in RouteParam
        let rp = result.get("RouteParam").unwrap();
        let parts: Vec<&str> = rp.split(',').collect();
        assert_eq!(parts[2], "1", "oneway should be 1 for Sens inverse");
    }

    /// AC5: RoadID auto-incremental
    #[test]
    fn test_compute_ac5_road_id_incremental() {
        let source = HashMap::from([
            ("VIT_MOY_VL".into(), "50".into()),
            ("NATURE".into(), "Route à 1 chaussée".into()),
        ]);
        let mut counter = RoadIdCounter::new();

        let r1 = compute_route_attrs(&source, &mut counter);
        let r2 = compute_route_attrs(&source, &mut counter);
        let r3 = compute_route_attrs(&source, &mut counter);

        assert_eq!(r1.get("RoadID").unwrap(), "1");
        assert_eq!(r2.get("RoadID").unwrap(), "2");
        assert_eq!(r3.get("RoadID").unwrap(), "3");
    }

    /// AC6: Physiquement impossible → denied bits
    #[test]
    fn test_compute_ac6_impossible_access() {
        let source = HashMap::from([
            ("VIT_MOY_VL".into(), "30".into()),
            ("ACCES_VL".into(), "Physiquement impossible".into()),
            ("ACCES_PED".into(), "Passage difficile".into()),
            ("NATURE".into(), "Route à 1 chaussée".into()),
        ]);
        let mut counter = RoadIdCounter::new();
        let result = compute_route_attrs(&source, &mut counter);

        let rp = result.get("RouteParam").unwrap();
        // speed=3, class=1, oneway=0, toll=0, 0, 0, denied_car=1, denied_bus=1, 0, denied_foot=1, 0, denied_truck=1
        assert_eq!(rp, "3,1,0,0,0,0,1,1,0,1,0,1");
    }

    /// Test is_routable_layer
    #[test]
    fn test_is_routable_layer() {
        assert!(is_routable_layer("TRONCON_DE_ROUTE"));
        assert!(!is_routable_layer("COURS_D_EAU"));
        assert!(!is_routable_layer("BATIMENT"));
        assert!(!is_routable_layer(""));
    }

    /// Test no restrictions when RESTR_ fields are absent
    #[test]
    fn test_compute_no_restrictions_when_absent() {
        let source = HashMap::from([
            ("VIT_MOY_VL".into(), "50".into()),
            ("NATURE".into(), "Route à 1 chaussée".into()),
        ]);
        let mut counter = RoadIdCounter::new();
        let result = compute_route_attrs(&source, &mut counter);

        assert!(!result.contains_key("MaxHeight"));
        assert!(!result.contains_key("MaxWeight"));
        assert!(!result.contains_key("MaxWidth"));
        assert!(!result.contains_key("MaxLength"));
    }

    /// Test no Roundabout when NATURE is not "Rond-point"
    #[test]
    fn test_compute_no_roundabout_normal_road() {
        let source = HashMap::from([
            ("VIT_MOY_VL".into(), "50".into()),
            ("NATURE".into(), "Route à 1 chaussée".into()),
        ]);
        let mut counter = RoadIdCounter::new();
        let result = compute_route_attrs(&source, &mut counter);

        assert!(!result.contains_key("Roundabout"));
    }

    /// Test zero restriction values are not emitted
    #[test]
    fn test_compute_zero_restrictions_not_emitted() {
        let source = HashMap::from([
            ("VIT_MOY_VL".into(), "50".into()),
            ("NATURE".into(), "Route à 1 chaussée".into()),
            ("RESTR_H".into(), "0.0".into()),
            ("RESTR_P".into(), "0.00".into()),
        ]);
        let mut counter = RoadIdCounter::new();
        let result = compute_route_attrs(&source, &mut counter);

        assert!(!result.contains_key("MaxHeight"));
        assert!(!result.contains_key("MaxWeight"));
    }

    /// M3: Verify merge semantics — routing attrs override rule-generated attrs
    /// This tests the contract used by pipeline/mod.rs: `new_attrs.extend(routing)`
    #[test]
    fn test_routing_attrs_merge_overrides_rule_output() {
        // Simulate rule output with a conflicting RouteParam
        let mut rule_output = HashMap::from([
            ("Type".into(), "0x01".into()),
            ("Label".into(), "A6".into()),
            ("RouteParam".into(), "STALE_VALUE".into()),
        ]);

        // Compute routing attrs
        let source = HashMap::from([
            ("VIT_MOY_VL".into(), "130".into()),
            ("CL_ADMIN".into(), "Autoroute".into()),
            ("SENS".into(), "Double sens".into()),
            ("NATURE".into(), "Type autoroutier".into()),
        ]);
        let mut counter = RoadIdCounter::new();
        let routing = compute_route_attrs(&source, &mut counter);

        // Merge: routing wins for RouteParam, rule output kept for Type/Label
        rule_output.extend(routing);

        assert_eq!(rule_output.get("Type").unwrap(), "0x01");
        assert_eq!(rule_output.get("Label").unwrap(), "A6");
        assert_eq!(rule_output.get("RouteParam").unwrap(), "7,4,0,0,0,0,0,0,0,0,0,0");
        assert!(rule_output.contains_key("RoadID"));
        assert!(rule_output.contains_key("DirIndicator"));
    }

    // =========================================================================
    // POS_SOL — Level field no longer injected (driver ogr-polishmap does not
    // support "Level", only "EndLevel"). graph_builder.rs reads Level from the
    // parsed .mp other_fields but it was never written, so always defaults to 0.
    // =========================================================================

    #[test]
    fn test_compute_pos_sol_not_injected() {
        let source = HashMap::from([
            ("VIT_MOY_VL".into(), "50".into()),
            ("NATURE".into(), "Route à 1 chaussée".into()),
            ("POS_SOL".into(), "1".into()),
        ]);
        let mut counter = RoadIdCounter::new();
        let result = compute_route_attrs(&source, &mut counter);
        assert!(result.get("Level").is_none(), "Level should no longer be injected");
    }

    /// Test MaxWidth and MaxLength
    #[test]
    fn test_compute_width_length_restrictions() {
        let source = HashMap::from([
            ("VIT_MOY_VL".into(), "50".into()),
            ("NATURE".into(), "Route à 1 chaussée".into()),
            ("RESTR_LAR".into(), "2.50".into()),
            ("RESTR_LON".into(), "12.00".into()),
        ]);
        let mut counter = RoadIdCounter::new();
        let result = compute_route_attrs(&source, &mut counter);

        assert_eq!(result.get("MaxWidth").unwrap(), "250");
        assert_eq!(result.get("MaxLength").unwrap(), "1200");
    }
}
