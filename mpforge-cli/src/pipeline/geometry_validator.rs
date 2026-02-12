//! Geometry validation and repair for pipeline features.
//!
//! Story 6.5: Validates coordinates and topology, attempts repair of invalid
//! geometries before clipping. Prevents data loss from NaN/Inf coordinates
//! and topological errors.

use crate::pipeline::reader::Feature;
use crate::pipeline::tiler::feature_to_gdal_geometry;
use gdal::cpl::CslStringList;
use gdal::vector::{Geometry, OGRwkbGeometryType};
use tracing::{debug, error, info, instrument};

/// Strategy used to repair an invalid geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairStrategy {
    /// OGR_G_MakeValid() - GEOS LINEWORK algorithm
    MakeValid,
    /// OGR_G_Buffer(0) - Fallback for self-intersections
    BufferZero,
}

/// Result of geometry validation.
#[derive(Debug)]
pub enum ValidationResult {
    /// Geometry is valid, no repair needed.
    Valid(Geometry),
    /// Geometry was repaired successfully.
    Repaired(Geometry, RepairStrategy),
    /// Geometry is irrecoverable.
    Rejected(String),
}

/// Cumulative statistics for geometry validation.
#[derive(Debug, Default, Clone)]
pub struct ValidationStats {
    /// Features with valid geometry (no repair needed).
    pub valid_count: usize,
    /// Features repaired via MakeValid.
    pub repaired_make_valid: usize,
    /// Features repaired via Buffer(0).
    pub repaired_buffer_zero: usize,
    /// Features with invalid coordinates (NaN/Inf).
    pub rejected_invalid_coords: usize,
    /// Features with irrecoverable topology.
    pub rejected_irrecoverable: usize,
}

impl ValidationStats {
    /// Total features processed.
    pub fn total(&self) -> usize {
        self.valid_count
            + self.repaired_make_valid
            + self.repaired_buffer_zero
            + self.rejected_invalid_coords
            + self.rejected_irrecoverable
    }

    /// Total features repaired.
    pub fn repaired_count(&self) -> usize {
        self.repaired_make_valid + self.repaired_buffer_zero
    }

    /// Total features rejected.
    pub fn rejected_count(&self) -> usize {
        self.rejected_invalid_coords + self.rejected_irrecoverable
    }

    /// Topology recovery rate: repaired / (repaired + irrecoverable).
    /// Excludes NaN/Inf rejections which are irrecoverable by design.
    pub fn recovery_rate(&self) -> f64 {
        let attempted = self.repaired_count() + self.rejected_irrecoverable;
        if attempted == 0 {
            1.0 // No topology issues = 100% "recovery"
        } else {
            self.repaired_count() as f64 / attempted as f64
        }
    }
}

/// Check if all coordinates in a feature are finite (no NaN/Inf).
///
/// This MUST be called BEFORE feature_to_gdal_geometry() because
/// GDAL/GEOS cannot handle NaN/Inf values and will crash or produce
/// cryptic errors ("invalid float literal").
#[instrument(skip(feature))]
pub fn validate_coordinates(feature: &Feature) -> bool {
    feature
        .geometry
        .iter()
        .all(|(x, y)| x.is_finite() && y.is_finite())
}

/// Attempt to repair an invalid GDAL geometry.
///
/// Strategy chain:
/// 1. make_valid() with LINEWORK method (default)
/// 2. buffer(0.0, 8) as fallback
///
/// Returns None if all strategies fail.
#[instrument(skip(geom))]
pub fn try_repair(geom: &Geometry) -> Option<(Geometry, RepairStrategy)> {
    // Strategy 1: MakeValid (GEOS LINEWORK)
    let opts = CslStringList::new();
    if let Ok(repaired) = geom.make_valid(&opts) {
        if repaired.is_valid() && !repaired.is_empty() && is_simple_geometry_type(&repaired) {
            return Some((repaired, RepairStrategy::MakeValid));
        }
        // MakeValid may return MultiPolygon/GeometryCollection for complex invalidity
        // (e.g., bow-tie → 2 triangles). Fall through to buffer(0).
    }

    // Strategy 2: Buffer(0) fallback
    if let Ok(buffered) = geom.buffer(0.0, 8) {
        if buffered.is_valid() && !buffered.is_empty() && is_simple_geometry_type(&buffered) {
            return Some((buffered, RepairStrategy::BufferZero));
        }
    }

    None // All strategies failed or returned unsupported collection types
}

/// Check if geometry is a simple (non-collection) type compatible with coordinate extraction.
/// Returns false for MultiPolygon, MultiLineString, GeometryCollection, etc.
fn is_simple_geometry_type(geom: &Geometry) -> bool {
    matches!(
        geom.geometry_type(),
        OGRwkbGeometryType::wkbPoint
            | OGRwkbGeometryType::wkbLineString
            | OGRwkbGeometryType::wkbPolygon
    )
}

/// Validate a feature's geometry and attempt repair if invalid.
///
/// # Arguments
/// * `feature` - Source feature to validate
/// * `stats` - Mutable stats counter to update
///
/// # Returns
/// * `ValidationResult::Valid(geom)` - Ready for clipping
/// * `ValidationResult::Repaired(geom, strategy)` - Repaired, ready for clipping
/// * `ValidationResult::Rejected(reason)` - Skip this feature
#[instrument(skip(feature, stats))]
pub fn validate_and_repair(feature: &Feature, stats: &mut ValidationStats) -> ValidationResult {
    // Step 1: Validate coordinates (Rust pure, before GDAL)
    if !validate_coordinates(feature) {
        stats.rejected_invalid_coords += 1;
        error!(
            geom_type = ?feature.geometry_type,
            reason = "invalid coordinates: NaN/Inf",
            "Feature rejected: invalid coordinates detected"
        );
        return ValidationResult::Rejected(
            "Invalid coordinates: NaN or Infinity detected".to_string(),
        );
    }

    // Step 2: Convert to GDAL Geometry
    let geom = match feature_to_gdal_geometry(feature) {
        Ok(g) => g,
        Err(e) => {
            stats.rejected_irrecoverable += 1;
            error!(
                geom_type = ?feature.geometry_type,
                reason = %e,
                "Feature rejected: WKT conversion failed"
            );
            return ValidationResult::Rejected(format!("WKT conversion failed: {}", e));
        }
    };

    // Step 3: Check topology
    if geom.is_valid() {
        stats.valid_count += 1;
        return ValidationResult::Valid(geom);
    }

    // Step 4: Attempt repair
    debug!(
        geom_type = ?feature.geometry_type,
        "Geometry invalid, attempting repair"
    );
    match try_repair(&geom) {
        Some((repaired, strategy)) => {
            match strategy {
                RepairStrategy::MakeValid => stats.repaired_make_valid += 1,
                RepairStrategy::BufferZero => stats.repaired_buffer_zero += 1,
            }
            info!(
                geom_type = ?feature.geometry_type,
                strategy = ?strategy,
                "Geometry repaired successfully"
            );
            ValidationResult::Repaired(repaired, strategy)
        }
        None => {
            stats.rejected_irrecoverable += 1;
            error!(
                geom_type = ?feature.geometry_type,
                reason = "Irrecoverable topology: MakeValid and Buffer(0) both failed",
                "Feature rejected: geometry irrecoverable"
            );
            ValidationResult::Rejected(
                "Irrecoverable topology: MakeValid and Buffer(0) both failed".to_string(),
            )
        }
    }
}
