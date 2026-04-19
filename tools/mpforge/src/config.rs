//! Configuration file parsing and structures.

use crate::pipeline::tile_naming;
use anyhow::Context;
use regex::Regex;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::LazyLock;
use tracing::{debug, info, warn};

/// Matches `${VAR_NAME}` placeholders (POSIX-valid variable names only).
static ENV_VAR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}").unwrap()
});

#[derive(Debug, Deserialize)]
// L6 code review : `deny_unknown_fields` signale une typo comme
// `generalize_profile_path` (singulier) plutôt que d'ignorer silencieusement —
// applicable à tout le Config. Sinon un utilisateur qui fait une coquille
// voit son catalogue externe silencieusement désactivé sans diagnostic.
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: u32,
    pub grid: GridConfig,
    pub inputs: Vec<InputSource>,
    pub output: OutputConfig,
    #[serde(default)]
    pub filters: Option<FilterConfig>,
    #[serde(default = "default_error_handling")]
    pub error_handling: String,
    /// Optional header configuration for Polish Map files (Story 8.1)
    #[serde(default)]
    pub header: Option<HeaderConfig>,
    /// Optional path to YAML rules file for attribute transformation (Story 9.1)
    #[serde(default)]
    pub rules: Option<PathBuf>,
    /// Global default: name of a field whose value uniquely identifies a feature
    /// within a single layer read. When set, features with duplicate values are
    /// skipped during reading. Features missing the field are always kept.
    /// Why: some BDTOPO IGN deliveries (e.g. D038 v2025.09) ship `.shp` files
    /// with every feature duplicated (same CLEABS, same geometry) — dedup makes
    /// the pipeline robust against such malformed sources.
    #[serde(default)]
    pub default_dedup_by_field: Option<String>,
    /// Tech-spec #2 Task 7: optional path to an external YAML catalog of
    /// multi-level generalization profiles, resolved relative to this
    /// `sources.yaml`. The file's root key is `profiles:` mapping
    /// `source_layer → GeneralizeProfile`.
    #[serde(default)]
    pub generalize_profiles_path: Option<PathBuf>,
    /// Tech-spec #2 Task 7: resolved profile map, built by `load_config` from
    /// the inline `generalize:` fields and the external file (when present).
    /// Keyed by `source_layer` (BDTOPO layer name resolved via
    /// `effective_layer_name`). Iteration order is deterministic (BTreeMap).
    ///
    /// H4 code review : ⚠️ champ `#[doc(hidden)]` public pour permettre aux
    /// tests d'intégration de construire des fixtures `Config`, mais **ne
    /// jamais muter directement en production** — utiliser
    /// [`Config::profile_map`] pour lire et
    /// [`Config::reset_profile_map_to_inline_only`] pour le use case
    /// `--disable-profiles`. Une mutation directe contournerait la validation
    /// `build_profile_map` (conflits inline/externe, F4 routing, bornes
    /// `LevelSpec`, contrainte `header.levels` — AC18).
    #[serde(skip, default)]
    #[doc(hidden)]
    pub resolved_profile_map: BTreeMap<String, GeneralizeProfile>,
}

impl Config {
    /// H4 code review : accessor en lecture seule vers la map résolue.
    pub fn profile_map(&self) -> &BTreeMap<String, GeneralizeProfile> {
        &self.resolved_profile_map
    }

    /// H4 code review : reset ciblé utilisé par `--disable-profiles` pour
    /// bypasser le catalogue externe tout en préservant les profils inline
    /// (Option B, cf. tech-spec Task 15 §Pré-condition). Rebuild la map
    /// depuis les seuls `inputs[].generalize` (via
    /// `From<GeneralizeConfig>`), en s'autorisant à ignorer la validation
    /// F4 parce que les inline sont le sous-ensemble capturé par le golden
    /// pré-tech-spec #2.
    pub fn reset_profile_map_to_inline_only(&mut self) {
        self.generalize_profiles_path = None;
        self.resolved_profile_map.clear();
        for input in &self.inputs {
            if let Some(ref gen) = input.generalize {
                if let Some(name) = input.effective_layer_name() {
                    self.resolved_profile_map.insert(name, gen.clone().into());
                }
            }
        }
    }
}

fn default_version() -> u32 {
    1
}

fn default_error_handling() -> String {
    "continue".to_string()
}

/// Grid configuration for spatial tiling.
/// Story 6.2: Clone trait required for TileProcessor ownership in pipeline orchestration.
#[derive(Debug, Clone, Deserialize)]
pub struct GridConfig {
    pub cell_size: f64,
    #[serde(default)]
    pub overlap: f64,
    #[serde(default)]
    pub origin: Option<[f64; 2]>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct InputSource {
    pub path: Option<String>,
    pub connection: Option<String>,
    pub layer: Option<String>,
    pub layers: Option<Vec<String>>,
    /// Override source SRS (e.g., "EPSG:2154"). Story 9.4: explicit reprojection.
    #[serde(default)]
    pub source_srs: Option<String>,
    /// Target SRS for reprojection (e.g., "EPSG:4326"). Defaults to WGS84 if source_srs is set.
    #[serde(default)]
    pub target_srs: Option<String>,
    /// OGR SQL attribute filter applied via GDAL before loading features into memory.
    /// Example: "CAST(ALTITUDE AS INTEGER) % 10 = 0"
    #[serde(default)]
    pub attribute_filter: Option<String>,
    /// Override GDAL layer name for rules engine matching.
    /// When set, replaces the native layer.name() so multiple SHP tiles match the same ruleset.
    #[serde(default)]
    pub layer_alias: Option<String>,
    /// Geometry generalization: smoothing and/or simplification applied after clipping.
    #[serde(default)]
    pub generalize: Option<GeneralizeConfig>,
    /// Spatial filter: clip features using a reference geometry (e.g., COMMUNE.shp) + buffer.
    #[serde(default)]
    pub spatial_filter: Option<SpatialFilterConfig>,
    /// Per-source dedup-by-attribute field. When set, overrides `Config::default_dedup_by_field`.
    /// Absent = pas de dedup pour cette source (et pas de warning "field not present").
    #[serde(default)]
    pub dedup_by_field: Option<String>,
}

/// Configuration for geometry generalization (smoothing + simplification).
///
/// Smoothing algorithms produce rounder curves from angular polygon/polyline vertices.
/// Optional simplification reduces vertex count after smoothing.
#[derive(Debug, Clone, Deserialize)]
pub struct GeneralizeConfig {
    /// Smoothing algorithm: "chaikin" (Chaikin corner-cutting).
    #[serde(default)]
    pub smooth: Option<String>,
    /// Number of smoothing iterations (default: 1). Each iteration doubles vertex count.
    #[serde(default = "default_iterations")]
    pub iterations: usize,
    /// Optional Douglas-Peucker simplification tolerance (in degrees WGS84).
    /// Applied after smoothing to reduce vertex count.
    #[serde(default)]
    pub simplify: Option<f64>,
}

fn default_iterations() -> usize {
    1
}

/// Tech-spec #2 Task 6: single level inside a [`GeneralizeProfile`].
///
/// `n` is the index of the detail level in `MpHeader.levels` (cf. Tech-spec #1
/// Q3): `n=0` is the most detailed geometry emitted as `Data0=`, `n=2` would
/// be emitted as `Data2=`, etc. The Polish Map writer emits one `Data<n>=`
/// line per level present in the profile.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct LevelSpec {
    pub n: u8,
    #[serde(default)]
    pub smooth: Option<String>,
    #[serde(default = "default_iterations")]
    pub iterations: usize,
    #[serde(default)]
    pub simplify: Option<f64>,
}

/// Tech-spec #2 Task 6: conditional dispatch by feature attribute.
///
/// When `feature.attributes[field]` ∈ `values`, the profile's default `levels`
/// are overridden by the clause's `levels`. Clauses are evaluated in YAML
/// order; first match wins.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct WhenClause {
    pub field: String,
    pub values: Vec<String>,
    pub levels: Vec<LevelSpec>,
}

/// Tech-spec #2 Task 6: per-`source_layer` generalization profile.
///
/// A profile declares a list of detail levels (`levels`) that become the
/// default, and optional `when` clauses to override the default based on a
/// feature attribute value. Each level generates one additional OGR geom
/// field in the `.mp` output (except `n=0`, which replaces the primary
/// geometry — cf. `geometry_smoother::apply_profile`).
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct GeneralizeProfile {
    #[serde(default)]
    pub levels: Vec<LevelSpec>,
    #[serde(default)]
    pub when: Vec<WhenClause>,
}

/// Upper bound on `LevelSpec.iterations` (Chaikin 5× ≈ 32× vertices).
const MAX_PROFILE_ITERATIONS: usize = 5;
/// Upper bound on `LevelSpec.simplify` (≈110 m in WGS84 degrees).
const MAX_PROFILE_SIMPLIFY: f64 = 0.001;

impl LevelSpec {
    /// Tech-spec #2 Task 6: per-level range validation (F15 adversarial
    /// review). `iterations ∈ [0, 5]` (ergonomic bound), `simplify ∈ [0, 0.001]`
    /// (anti-destruction guard). Fail-fast at `load_config`.
    pub fn validate(&self, context: &str) -> anyhow::Result<()> {
        if let Some(ref algo) = self.smooth {
            if !algo.is_empty() && algo != "chaikin" {
                anyhow::bail!(
                    "{context}: unknown smooth algorithm '{algo}' (supported: 'chaikin')"
                );
            }
        }
        if self.iterations > MAX_PROFILE_ITERATIONS {
            anyhow::bail!(
                "{context}: iterations={} out of range [0, {}]",
                self.iterations,
                MAX_PROFILE_ITERATIONS
            );
        }
        if let Some(s) = self.simplify {
            if !(s.is_finite() && (0.0..=MAX_PROFILE_SIMPLIFY).contains(&s)) {
                anyhow::bail!(
                    "{context}: simplify={} out of range [0.0, {}]",
                    s,
                    MAX_PROFILE_SIMPLIFY
                );
            }
        }
        Ok(())
    }
}

impl GeneralizeProfile {
    /// Validate every level in the profile (default + each `when` clause).
    pub fn validate(&self, context: &str) -> anyhow::Result<()> {
        for (i, lvl) in self.levels.iter().enumerate() {
            lvl.validate(&format!("{context}.levels[{i}]"))?;
        }
        for (wi, w) in self.when.iter().enumerate() {
            if w.values.is_empty() {
                anyhow::bail!(
                    "{context}.when[{wi}]: 'values' must list at least one attribute value"
                );
            }
            for (i, lvl) in w.levels.iter().enumerate() {
                lvl.validate(&format!("{context}.when[{wi}].levels[{i}]"))?;
            }
        }
        Ok(())
    }

    /// Return true when any level set visible to the profile (default or any
    /// `when` branch) contains `LevelSpec { n: 0, .. }`. Used by Task 7
    /// validation F4 (routable layers must always produce `Data0`).
    pub fn every_branch_has_n0(&self) -> bool {
        let default_ok = self.levels.is_empty() || self.levels.iter().any(|l| l.n == 0);
        let when_ok = self.when.iter().all(|w| w.levels.iter().any(|l| l.n == 0));
        default_ok && when_ok
    }

    /// Maximum `n` referenced anywhere in the profile. Used by Task 7 to
    /// validate `header.levels.len() >= max_n + 1` (AC18) and by the writer
    /// to size `MAX_DATA_LEVEL`.
    pub fn max_level_index(&self) -> Option<u8> {
        let default_max = self.levels.iter().map(|l| l.n).max();
        let when_max = self.when.iter().flat_map(|w| w.levels.iter().map(|l| l.n)).max();
        match (default_max, when_max) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }
}

impl From<GeneralizeConfig> for GeneralizeProfile {
    /// Tech-spec #2 Task 6: convert a legacy inline `generalize:` into a
    /// single-level profile (`n=0`). This keeps the downstream pipeline on a
    /// single code path (cf. `geometry_smoother::apply_profile`).
    fn from(c: GeneralizeConfig) -> Self {
        GeneralizeProfile {
            levels: vec![LevelSpec {
                n: 0,
                smooth: c.smooth,
                iterations: c.iterations,
                simplify: c.simplify,
            }],
            when: vec![],
        }
    }
}

/// Configuration for spatial filtering using a reference geometry layer.
///
/// The source shapefile polygons are unioned and buffered to create a clipping
/// geometry applied via GDAL `set_spatial_filter()` before reading features.
#[derive(Debug, Clone, Deserialize)]
pub struct SpatialFilterConfig {
    /// Path to the shapefile containing the clipping geometry (e.g., COMMUNE.shp).
    pub source: String,
    /// Buffer distance in meters, applied in the source SRS (default: 0.0).
    #[serde(default)]
    pub buffer: f64,
}

#[derive(Debug, Deserialize)]
pub struct OutputConfig {
    pub directory: String,
    #[serde(default = "default_filename_pattern")]
    pub filename_pattern: String,
    /// Optional path to YAML field mapping config for ogr-polishmap driver.
    /// Story 7.4: Maps source field names (e.g., MP_TYPE, NAME) to Polish Map canonical names (Type, Label).
    #[serde(default)]
    pub field_mapping_path: Option<PathBuf>,
    /// Optional: overwrite existing tile files (default: true).
    /// Set to false for skip-existing behavior via config.
    /// Story 8.3: None or Some(true) = overwrite, Some(false) = skip existing.
    #[serde(default)]
    pub overwrite: Option<bool>,
    /// Base ID for auto-generating unique tile IDs.
    /// Formula: tile_id = base_id * 10000 + seq (1-based).
    /// When set and header.id is absent or "0", each tile gets a unique 8-digit ID.
    /// Must be in range 1..=9999 to guarantee 8-digit IDs.
    #[serde(default)]
    pub base_id: Option<u32>,
}

fn default_filename_pattern() -> String {
    "{col}_{row}.mp".to_string()
}

/// Sentinel value for header.id meaning "auto-generate from base_id".
pub const AUTO_ID: &str = "0";

/// Header configuration for Polish Map files.
/// Story 8.1: Allows configuring header options (template, name, levels, etc.) via YAML.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct HeaderConfig {
    /// Optional path to header template file (.mp) for HEADER_TEMPLATE DSCO
    #[serde(default)]
    pub template: Option<PathBuf>,
    /// Map name (Polish Map: Name)
    #[serde(default)]
    pub name: Option<String>,
    /// Map ID (Polish Map: ID).
    /// Set to "0" (AUTO_ID) to auto-generate unique IDs via output.base_id.
    #[serde(default)]
    pub id: Option<String>,
    /// Copyright notice (Polish Map: Copyright)
    #[serde(default)]
    pub copyright: Option<String>,
    /// Number of detail levels (Polish Map: Levels)
    #[serde(default)]
    pub levels: Option<String>,
    /// Level 0 zoom (Polish Map: Level0)
    #[serde(default)]
    pub level0: Option<String>,
    /// Level 1 zoom (Polish Map: Level1)
    #[serde(default)]
    pub level1: Option<String>,
    /// Level 2 zoom (Polish Map: Level2)
    #[serde(default)]
    pub level2: Option<String>,
    /// Level 3 zoom (Polish Map: Level3)
    #[serde(default)]
    pub level3: Option<String>,
    /// Level 4 zoom (Polish Map: Level4)
    #[serde(default)]
    pub level4: Option<String>,
    /// Level 5 zoom (Polish Map: Level5)
    #[serde(default)]
    pub level5: Option<String>,
    /// Level 6 zoom (Polish Map: Level6)
    #[serde(default)]
    pub level6: Option<String>,
    /// Level 7 zoom (Polish Map: Level7)
    #[serde(default)]
    pub level7: Option<String>,
    /// Level 8 zoom (Polish Map: Level8)
    #[serde(default)]
    pub level8: Option<String>,
    /// Level 9 zoom (Polish Map: Level9)
    #[serde(default)]
    pub level9: Option<String>,
    /// Tree size parameter (Polish Map: TreeSize)
    #[serde(default)]
    pub tree_size: Option<String>,
    /// Region limit parameter (Polish Map: RgnLimit)
    #[serde(default)]
    pub rgn_limit: Option<String>,
    /// Transparency setting (Polish Map: Transparent)
    #[serde(default)]
    pub transparent: Option<String>,
    /// Marine map setting (Polish Map: Marine)
    #[serde(default)]
    pub marine: Option<String>,
    /// Preprocessing mode (Polish Map: Preprocess)
    #[serde(default)]
    pub preprocess: Option<String>,
    /// Label encoding (Polish Map: LBLcoding)
    #[serde(default)]
    pub lbl_coding: Option<String>,
    /// Simplification level (Polish Map: SimplifyLevel)
    #[serde(default)]
    pub simplify_level: Option<String>,
    /// Left-side traffic setting (Polish Map: LeftSideTraffic)
    #[serde(default)]
    pub left_side_traffic: Option<String>,
    /// Routing enabled (Polish Map: Routing) — required by mkgmap when RoadID present
    #[serde(default)]
    pub routing: Option<String>,
    /// Custom arbitrary header fields
    #[serde(default)]
    pub custom: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct FilterConfig {
    /// Bounding box filter: [min_lon, min_lat, max_lon, max_lat]
    /// If FilterConfig exists, bbox is required
    pub bbox: [f64; 4],
}

/// Source type enumeration for InputSource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    File,
    PostGIS,
}

/// Error handling mode for geometry clipping operations.
/// Story 6.3: Controls behavior when encountering invalid geometries during clipping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ErrorMode {
    /// Continue processing, skip invalid features (production-friendly default)
    #[default]
    Continue,
    /// Stop pipeline on first error (useful for debugging)
    FailFast,
}

impl std::str::FromStr for ErrorMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "continue" => Ok(Self::Continue),
            "fail-fast" => Ok(Self::FailFast),
            _ => anyhow::bail!(
                "Invalid error_handling mode '{}', expected 'continue' or 'fail-fast'",
                s
            ),
        }
    }
}

impl InputSource {
    /// Resolve the effective layer name: `layer_alias` if set, `layer` if set,
    /// otherwise the filename stem from `path` (e.g., "ZONE_D_HABITATION" from ".../ZONE_D_HABITATION.shp").
    ///
    /// Returns `None` for wildcard paths (containing `*` or `?`) since they expand
    /// to multiple layers at runtime.
    pub fn effective_layer_name(&self) -> Option<String> {
        if let Some(ref alias) = self.layer_alias {
            return Some(alias.clone());
        }
        if let Some(ref layer) = self.layer {
            return Some(layer.clone());
        }
        if let Some(ref path) = self.path {
            // Wildcard paths expand to multiple files/layers — no single name
            if path.contains('*') || path.contains('?') {
                return None;
            }
            return Path::new(path)
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string());
        }
        None
    }

    /// Detect source type based on connection string pattern.
    pub fn source_type(&self) -> SourceType {
        if let Some(conn) = &self.connection {
            if conn.starts_with("PG:") || conn.contains("host=") {
                return SourceType::PostGIS;
            }
        }
        SourceType::File
    }
}

/// Tech-spec #2 Task 7: external YAML root for the generalization profile
/// catalog. The file lives beside (or above) `sources.yaml` and is referenced
/// via `Config.generalize_profiles_path`.
#[derive(Debug, Deserialize, Default)]
pub struct GeneralizeProfilesFile {
    #[serde(default)]
    pub profiles: BTreeMap<String, GeneralizeProfile>,
}

impl Config {
    /// Tech-spec #2 Task 7: build the unified profile map from inline
    /// `generalize:` entries and the external profiles file. Resolves the
    /// relative path against `base_dir` (usually the directory containing
    /// `sources.yaml`). Fails fast on:
    ///   - conflict between an inline profile and an external profile on the
    ///     same layer name (motivates F4 semantics clarity);
    ///   - per-level out-of-range values (Task 6 bounds);
    ///   - routable layers (`is_routable_layer`) that do not emit `n: 0` in
    ///     at least every reachable branch (F4 Tech-spec #1).
    pub fn build_profile_map(
        &self,
        base_dir: &Path,
    ) -> anyhow::Result<BTreeMap<String, GeneralizeProfile>> {
        let mut map: BTreeMap<String, GeneralizeProfile> = BTreeMap::new();
        let mut inline_origins: BTreeMap<String, String> = BTreeMap::new();

        // 1) Inline profiles — converted to GeneralizeProfile with a single
        //    level at n=0 (cf. `impl From<GeneralizeConfig>`).
        for input in &self.inputs {
            if let Some(ref inline) = input.generalize {
                let Some(layer_name) = input.effective_layer_name() else { continue; };
                let origin = input
                    .path
                    .clone()
                    .or_else(|| input.connection.clone())
                    .unwrap_or_else(|| "<unknown>".to_string());
                let profile: GeneralizeProfile = inline.clone().into();
                inline_origins.insert(layer_name.clone(), origin);
                map.insert(layer_name, profile);
            }
        }

        // 2) External catalog — merge with inline. Reject conflicts.
        if let Some(ref rel) = self.generalize_profiles_path {
            let full = if rel.is_absolute() {
                rel.clone()
            } else {
                base_dir.join(rel)
            };
            let content = std::fs::read_to_string(&full).with_context(|| {
                format!(
                    "failed to read generalize_profiles_path: {}",
                    full.display()
                )
            })?;
            let file: GeneralizeProfilesFile = serde_yml::from_str(&content).with_context(|| {
                format!(
                    "failed to parse generalize-profiles YAML: {}",
                    full.display()
                )
            })?;
            for (layer, profile) in file.profiles {
                if let Some(inline_path) = inline_origins.get(&layer) {
                    anyhow::bail!(
                        "generalize profile conflict for layer '{layer}': \
                         inline in sources.yaml (input '{inline_path}') AND defined in {}",
                        full.display()
                    );
                }
                map.insert(layer, profile);
            }
        }

        // 3) Per-profile validation + F4 routing guard.
        for (layer, profile) in &map {
            profile.validate(&format!("generalize profile '{}'", layer))?;
            if crate::pipeline::route_params::is_routable_layer(layer)
                && !profile.every_branch_has_n0()
            {
                anyhow::bail!(
                    "routable layer '{layer}' profile missing n:0 \
                     (required for routing, cf. Tech-spec #1 F4)"
                );
            }
        }

        // 4) AC18 (H2 code review, F2+F12 adversarial) : valider que le
        //    `header.levels` de `sources.yaml` peut accueillir le `max(n)`
        //    de tous les profils. Sans ce check, un profil avec `n: 4` et
        //    `header.Levels=3` voit son `Data4=` silencieusement droppé par
        //    imgforge (Tech-spec #1 Q4 warn+ignore).
        let max_n_over_all = map
            .values()
            .filter_map(|p| p.max_level_index())
            .max()
            .unwrap_or(0);
        if max_n_over_all > 0 {
            let header_levels = self
                .header
                .as_ref()
                .and_then(|h| h.levels.as_deref())
                .and_then(|s| s.parse::<u32>().ok());
            if let Some(declared) = header_levels {
                if (max_n_over_all as u32) >= declared {
                    anyhow::bail!(
                        "profile references n={} but mp header declares only {} levels \
                         (need header.levels >= {}; cf. Tech-spec #1 Q3/Q4: DataN beyond \
                         levels.len() is silently dropped by imgforge)",
                        max_n_over_all,
                        declared,
                        max_n_over_all as u32 + 1
                    );
                }
            }
            // Si `header.levels` absent, mpforge retombera sur les défauts du
            // driver PolishMap (`Levels=5`) — cohérent avec tous les profils
            // dont `max(n) <= 4`. Pas d'erreur remontée ici mais un warn
            // informatif pour signaler le reliance sur les défauts.
            else {
                warn!(
                    max_n = max_n_over_all,
                    "profiles reference n={} without explicit header.levels in sources.yaml \
                     — relying on driver default Levels=5 (AC18 passes if max(n) < 5)",
                    max_n_over_all
                );
            }
        }

        Ok(map)
    }

    /// Build a map of layer_name → GeneralizeConfig for all inputs that have generalization configured.
    ///
    /// The layer name is resolved as: `layer_alias` if set, otherwise the filename stem from `path`.
    pub fn build_generalize_map(&self) -> HashMap<String, GeneralizeConfig> {
        let mut map = HashMap::new();
        for input in &self.inputs {
            if let Some(ref gen_config) = input.generalize {
                match input.effective_layer_name() {
                    Some(name) => {
                        map.insert(name, gen_config.clone());
                    }
                    None => {
                        let source = input.path.as_deref()
                            .or(input.connection.as_deref())
                            .unwrap_or("<unknown>");
                        warn!(
                            source = %source,
                            "generalize directive ignored: cannot resolve layer name \
                             (use layer_alias for wildcard paths or PostGIS connections)"
                        );
                    }
                }
            }
        }
        map
    }

    /// Validate configuration semantic rules.
    pub fn validate(&self) -> anyhow::Result<()> {
        // Grid validation
        if self.grid.cell_size <= 0.0 {
            anyhow::bail!(
                "grid.cell_size must be positive, got: {}",
                self.grid.cell_size
            );
        }

        if self.grid.overlap < 0.0 {
            anyhow::bail!(
                "grid.overlap cannot be negative, got: {}",
                self.grid.overlap
            );
        }

        // Inputs validation
        if self.inputs.is_empty() {
            anyhow::bail!("At least one input source is required");
        }

        for (i, input) in self.inputs.iter().enumerate() {
            let has_path = input.path.is_some();
            let has_connection = input.connection.is_some();

            if has_path == has_connection {
                anyhow::bail!(
                    "InputSource #{} must have either 'path' or 'connection', not both or none",
                    i
                );
            }

            // Validate spatial_filter config
            if let Some(ref sf) = input.spatial_filter {
                if sf.source.is_empty() {
                    anyhow::bail!(
                        "InputSource #{}: spatial_filter.source must not be empty",
                        i
                    );
                }
                if sf.buffer < 0.0 {
                    anyhow::bail!(
                        "InputSource #{}: spatial_filter.buffer must not be negative, got {}",
                        i, sf.buffer
                    );
                }
            }

            // Validate generalize config
            if let Some(ref gen) = input.generalize {
                if gen.iterations == 0 {
                    anyhow::bail!(
                        "InputSource #{}: generalize.iterations must be >= 1, got 0",
                        i
                    );
                }
                if let Some(tol) = gen.simplify {
                    if tol <= 0.0 {
                        anyhow::bail!(
                            "InputSource #{}: generalize.simplify must be positive, got {}",
                            i, tol
                        );
                    }
                }
                if let Some(ref algo) = gen.smooth {
                    if algo != "chaikin" {
                        anyhow::bail!(
                            "InputSource #{}: unknown generalize.smooth algorithm '{}' \
                             (supported: \"chaikin\")",
                            i, algo
                        );
                    }
                }
            }
        }

        // Error handling validation (use ErrorMode::from_str for consistency)
        ErrorMode::from_str(&self.error_handling)
            .with_context(|| format!("Invalid error_handling value: '{}'", self.error_handling))?;

        // Filters validation (if present)
        if let Some(filters) = &self.filters {
            let bbox = filters.bbox;
            if bbox[0] >= bbox[2] {
                anyhow::bail!(
                    "bbox min_lon must be < max_lon, got: [{}, {}]",
                    bbox[0],
                    bbox[2]
                );
            }
            if bbox[1] >= bbox[3] {
                anyhow::bail!(
                    "bbox min_lat must be < max_lat, got: [{}, {}]",
                    bbox[1],
                    bbox[3]
                );
            }
        }

        // Story 9.4: Validate SRS definitions (fail-fast before processing)
        for (i, input) in self.inputs.iter().enumerate() {
            if let Some(ref srs) = input.source_srs {
                gdal::spatial_ref::SpatialRef::from_definition(srs).with_context(|| {
                    format!(
                        "InputSource #{}: invalid source_srs '{}' — must be a valid SRS definition (e.g., EPSG:2154)",
                        i, srs
                    )
                })?;
            }
            if let Some(ref srs) = input.target_srs {
                gdal::spatial_ref::SpatialRef::from_definition(srs).with_context(|| {
                    format!(
                        "InputSource #{}: invalid target_srs '{}' — must be a valid SRS definition (e.g., EPSG:4326)",
                        i, srs
                    )
                })?;
            }
            if input.target_srs.is_some() && input.source_srs.is_none() {
                warn!(
                    source_index = i,
                    target_srs = ?input.target_srs,
                    "InputSource #{}: target_srs without source_srs has no effect (ignored)",
                    i
                );
            }
        }

        // base_id validation: must produce valid 8-digit Garmin IDs
        if let Some(base_id) = self.output.base_id {
            if base_id == 0 || base_id > 9999 {
                anyhow::bail!(
                    "output.base_id must be in range 1..=9999, got: {}. \
                     Formula: base_id * 10000 + seq must fit in 8 digits.",
                    base_id
                );
            }
        }

        // Filename pattern validation (Story 8.2)
        tile_naming::validate_tile_pattern(&self.output.filename_pattern)
            .with_context(|| format!("Invalid filename_pattern: '{}'", self.output.filename_pattern))?;

        // Output field_mapping_path validation removed (Story 7.4)
        // Validation moved to MpWriter::new() to avoid race condition in parallel mode
        // where file could be deleted between config validation and usage.
        // See writer.rs:65-69 for canonicalize() with proper error context.

        // Header template validation removed (Story 8.1 Code Review Fix H2)
        // Validation moved to MpWriter::new() to avoid TOCTOU race condition in parallel mode.
        // Same rationale as field_mapping: file could be deleted between config load and first tile export.
        // See writer.rs for validation with proper error context at actual usage time.

        // Soft validation of header numeric values (warnings, not errors)
        if let Some(ref header) = self.header {
            // Validate header.name pattern if it contains variables (hard error — will fail every tile)
            if let Some(ref name) = header.name {
                if name.contains('{') {
                    tile_naming::validate_tile_pattern(name)
                        .with_context(|| format!("Invalid header.name pattern: '{}'", name))?;
                }
            }

            // Helper: parse and warn if out of range or non-numeric
            fn warn_range(field_name: &str, value: &Option<String>, min: u32, max: u32) {
                if let Some(ref v) = value {
                    match v.parse::<u32>() {
                        Ok(n) if n < min || n > max => {
                            tracing::warn!(
                                field = field_name,
                                value = n,
                                min = min,
                                max = max,
                                "header.{} = {} hors de la plage recommandée [{}, {}]",
                                field_name, n, min, max
                            );
                        }
                        Err(_) => {
                            tracing::warn!(
                                field = field_name,
                                value = %v,
                                "header.{} = '{}' n'est pas un entier valide",
                                field_name, v
                            );
                        }
                        _ => {}
                    }
                }
            }

            // Warn on invalid boolean/enum values (expected: Y/N, T/F)
            fn warn_yn(field_name: &str, value: &Option<String>) {
                if let Some(ref v) = value {
                    if !matches!(v.as_str(), "Y" | "N") {
                        tracing::warn!(
                            field = field_name,
                            value = %v,
                            "header.{} = '{}' — valeur attendue : Y ou N",
                            field_name, v
                        );
                    }
                }
            }
            fn warn_tf(field_name: &str, value: &Option<String>) {
                if let Some(ref v) = value {
                    if !matches!(v.as_str(), "T" | "F") {
                        tracing::warn!(
                            field = field_name,
                            value = %v,
                            "header.{} = '{}' — valeur attendue : T ou F",
                            field_name, v
                        );
                    }
                }
            }

            warn_range("levels", &header.levels, 1, 10);
            warn_range("level0", &header.level0, 10, 24);
            warn_range("level1", &header.level1, 10, 24);
            warn_range("level2", &header.level2, 10, 24);
            warn_range("level3", &header.level3, 10, 24);
            warn_range("level4", &header.level4, 10, 24);
            warn_range("level5", &header.level5, 10, 24);
            warn_range("level6", &header.level6, 10, 24);
            warn_range("level7", &header.level7, 10, 24);
            warn_range("level8", &header.level8, 10, 24);
            warn_range("level9", &header.level9, 10, 24);
            warn_range("tree_size", &header.tree_size, 100, 15000);
            warn_range("rgn_limit", &header.rgn_limit, 50, 1024);

            warn_yn("transparent", &header.transparent);
            warn_yn("marine", &header.marine);
            warn_tf("preprocess", &header.preprocess);
        }

        Ok(())
    }
}

/// Expand brace patterns like `{a,b,c}` into multiple strings.
///
/// Supports a single brace group per pattern (nested braces are not supported).
/// If no braces are found, returns the original pattern unchanged.
///
/// Examples:
///   `"data/{D038,D001}/TRANSPORT.shp"` → `["data/D038/TRANSPORT.shp", "data/D001/TRANSPORT.shp"]`
///   `"data/D038/TRANSPORT.shp"` → `["data/D038/TRANSPORT.shp"]`
pub fn expand_braces(pattern: &str) -> Vec<String> {
    let Some(open) = pattern.find('{') else {
        return vec![pattern.to_string()];
    };
    let Some(close) = pattern[open..].find('}') else {
        return vec![pattern.to_string()];
    };
    let close = open + close;

    let prefix = &pattern[..open];
    let suffix = &pattern[close + 1..];
    let alternatives = &pattern[open + 1..close];

    alternatives
        .split(',')
        .map(|alt| alt.trim())
        .filter(|alt| !alt.is_empty())
        .map(|alt| format!("{}{}{}", prefix, alt, suffix))
        .collect()
}

/// Resolve wildcard patterns to actual file paths.
///
/// Supports brace expansion (`{a,b}`) in addition to standard glob wildcards (`*`, `?`).
/// Brace expansion is applied first, then each resulting pattern is resolved via glob.
fn resolve_wildcard_paths(pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
    let expanded_patterns = expand_braces(pattern);
    let mut paths = Vec::new();

    for pat in &expanded_patterns {
        let matched: Vec<PathBuf> = glob::glob(pat)
            .with_context(|| format!("Invalid glob pattern: {}", pat))?
            .filter_map(|entry| entry.ok())
            .collect();
        paths.extend(matched);
    }

    // Deduplicate in case brace expansion produces overlapping patterns
    paths.sort();
    paths.dedup();

    if paths.is_empty() {
        warn!(pattern, "No files matched wildcard pattern");
    } else {
        info!(pattern, count = paths.len(), "Resolved wildcard pattern");
    }

    Ok(paths)
}

/// Expand `${VAR}` placeholders in a string with environment variable values.
///
/// Only POSIX-valid variable names (`[A-Za-z_][A-Za-z0-9_]*`) are matched.
/// Variables that are not set in the environment are left as-is.
///
/// # Safety note
/// Values are substituted verbatim into the YAML text before parsing.
/// Env vars containing YAML metacharacters (`:`, `\n`, `{`) could alter
/// the document structure. Typed fields (e.g. `u32`) are protected by
/// serde deserialization; string fields accept any value.
fn expand_env_vars(content: &str) -> String {
    ENV_VAR_RE
        .replace_all(content, |caps: &regex::Captures| {
            let var_name = &caps[1];
            match std::env::var(var_name) {
                Ok(val) => val,
                Err(_) => caps[0].to_string(),
            }
        })
        .into_owned()
}

/// Load and parse configuration from YAML file.
pub fn load_config<P: AsRef<Path>>(path: P) -> anyhow::Result<Config> {
    let path = path.as_ref();

    // I/O error context
    let raw_content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;

    // Expand environment variables before parsing
    let content = expand_env_vars(&raw_content);

    // YAML parsing error context
    let mut config: Config = serde_yml::from_str(&content)
        .with_context(|| format!("Failed to parse YAML config: {}", path.display()))?;

    // Wildcard resolution for file inputs: expand glob patterns into N InputSource clones
    // Must happen BEFORE validation so that "at least one input" check works correctly
    let mut expanded_inputs = Vec::new();
    for input in config.inputs {
        if let Some(pattern) = &input.path {
            if pattern.contains('*') || pattern.contains('?') || pattern.contains('{') {
                let resolved = resolve_wildcard_paths(pattern)?;
                if resolved.is_empty() {
                    warn!(pattern, "Wildcard pattern matched 0 files — input dropped");
                }
                debug!(pattern, resolved = ?resolved, "Wildcard expanded");
                for resolved_path in resolved {
                    let mut cloned = input.clone();
                    cloned.path = Some(resolved_path.to_string_lossy().to_string());
                    expanded_inputs.push(cloned);
                }
                continue;
            }
        }
        expanded_inputs.push(input);
    }
    config.inputs = expanded_inputs;

    // Validation error context (after wildcard expansion so input count is accurate)
    config
        .validate()
        .with_context(|| format!("Config validation failed for: {}", path.display()))?;

    // Tech-spec #2 Task 7: resolve external generalize profiles (if any) and
    // merge with inline entries. Fails fast on conflicts, invalid bounds, or
    // routable-layer violations (F4). Stored on the Config for downstream use.
    let base_dir = path.parent().unwrap_or(Path::new("."));
    config.resolved_profile_map = config.build_profile_map(base_dir).with_context(|| {
        format!("generalize profile resolution failed for: {}", path.display())
    })?;

    // Log source type for each input
    for input in &config.inputs {
        let source_type = input.source_type();
        match source_type {
            SourceType::File => {
                if let Some(path) = &input.path {
                    info!(path, "Detected File input source");
                }
            }
            SourceType::PostGIS => {
                if let Some(conn) = &input.connection {
                    info!(connection = conn, "Detected PostGIS input source");
                }
            }
        }
    }

    Ok(config)
}

/// Run configuration validation and produce a `ValidationReport`.
///
/// Orchestrates all checks:
/// 1. YAML syntax + semantic validation via `load_config()`
/// 2. Input file existence (non-wildcard paths after resolution)
/// 3. Rules file loading and validation
/// 4. Field mapping file parsing
/// 5. Header template existence
///
/// If `load_config()` fails (YAML syntax or semantic error), remaining checks are skipped.
pub fn run_validate(
    config_path: &str,
) -> anyhow::Result<crate::report::ValidationReport> {
    use crate::report::*;

    let mut checks = Vec::new();
    let mut errors = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    // Step 1a: Read + parse YAML (syntax check)
    let raw_content = match std::fs::read_to_string(config_path) {
        Ok(c) => c,
        Err(e) => {
            let err_msg = format!("Failed to read config file: {}", e);
            checks.push(ValidationCheck {
                name: "yaml_syntax".to_string(),
                status: CheckStatus::Fail,
                details: err_msg.clone(),
            });
            errors.push(err_msg);
            return Ok(ValidationReport {
                status: ValidationStatus::Invalid,
                config_file: config_path.to_string(),
                checks,
                errors,
                warnings,
                summary: None,
            });
        }
    };

    // Expand environment variables before parsing
    let content = expand_env_vars(&raw_content);

    // Warn about unresolved env var placeholders (deduplicated)
    let mut seen_vars = std::collections::HashSet::new();
    for caps in ENV_VAR_RE.captures_iter(&content) {
        let var_name = caps[1].to_string();
        if seen_vars.insert(var_name.clone()) {
            warnings.push(format!(
                "Unresolved environment variable: ${{{}}} (not set)",
                var_name
            ));
        }
    }

    let mut config: Config = match serde_yml::from_str(&content) {
        Ok(c) => {
            checks.push(ValidationCheck {
                name: "yaml_syntax".to_string(),
                status: CheckStatus::Pass,
                details: "Parsed successfully".to_string(),
            });
            c
        }
        Err(e) => {
            let err_msg = format!("YAML syntax error: {}", e);
            checks.push(ValidationCheck {
                name: "yaml_syntax".to_string(),
                status: CheckStatus::Fail,
                details: err_msg.clone(),
            });
            errors.push(err_msg);
            return Ok(ValidationReport {
                status: ValidationStatus::Invalid,
                config_file: config_path.to_string(),
                checks,
                errors,
                warnings,
                summary: None,
            });
        }
    };

    // Step 1b: Wildcard resolution (before validation so input count is accurate)
    let mut expanded_inputs = Vec::new();
    for input in config.inputs {
        if let Some(pattern) = &input.path {
            if pattern.contains('*') || pattern.contains('?') || pattern.contains('{') {
                let resolved = resolve_wildcard_paths(pattern)?;
                if resolved.is_empty() {
                    warnings.push(format!("Wildcard pattern '{}' matched 0 files", pattern));
                }
                for resolved_path in resolved {
                    let mut cloned = input.clone();
                    cloned.path = Some(resolved_path.to_string_lossy().to_string());
                    expanded_inputs.push(cloned);
                }
                continue;
            }
        }
        expanded_inputs.push(input);
    }
    config.inputs = expanded_inputs;

    // Step 1c: Semantic validation (structurally separate from YAML parsing)
    if let Err(e) = config.validate() {
        let err_msg = format!("{:#}", e);
        checks.push(ValidationCheck {
            name: "semantic_validation".to_string(),
            status: CheckStatus::Fail,
            details: err_msg.clone(),
        });
        errors.push(err_msg);
        return Ok(ValidationReport {
            status: ValidationStatus::Invalid,
            config_file: config_path.to_string(),
            checks,
            errors,
            warnings,
            summary: None,
        });
    }
    checks.push(ValidationCheck {
        name: "semantic_validation".to_string(),
        status: CheckStatus::Pass,
        details: "All validations passed".to_string(),
    });

    // Step 2: Input file existence
    let mut input_count = 0usize;
    let mut missing_inputs = Vec::new();
    for input in &config.inputs {
        if let Some(ref path_str) = input.path {
            input_count += 1;
            if !Path::new(path_str).exists() {
                missing_inputs.push(path_str.clone());
            }
        }
    }

    if missing_inputs.is_empty() {
        checks.push(ValidationCheck {
            name: "input_files".to_string(),
            status: CheckStatus::Pass,
            details: format!("{} files found", input_count),
        });
    } else {
        let detail = format!(
            "{}/{} files missing: {}",
            missing_inputs.len(),
            input_count,
            missing_inputs.join(", ")
        );
        checks.push(ValidationCheck {
            name: "input_files".to_string(),
            status: CheckStatus::Fail,
            details: detail.clone(),
        });
        errors.push(detail);
    }

    if input_count == 0 {
        warnings.push("No input file paths to check (all inputs may be PostGIS connections)".to_string());
    }

    // Step 3: Rules file (optional)
    if let Some(ref rules_path) = config.rules {
        match crate::rules::load_rules(rules_path) {
            Ok(rules_file) => {
                let total_rules: usize = rules_file.rulesets.iter().map(|rs| rs.rules.len()).sum();
                checks.push(ValidationCheck {
                    name: "rules_file".to_string(),
                    status: CheckStatus::Pass,
                    details: format!(
                        "{} rulesets, {} rules total",
                        rules_file.rulesets.len(),
                        total_rules
                    ),
                });
            }
            Err(e) => {
                let err_msg = format!("Rules file error: {:#}", e);
                checks.push(ValidationCheck {
                    name: "rules_file".to_string(),
                    status: CheckStatus::Fail,
                    details: err_msg.clone(),
                });
                errors.push(err_msg);
            }
        }
    } else {
        checks.push(ValidationCheck {
            name: "rules_file".to_string(),
            status: CheckStatus::Skipped,
            details: "Not configured".to_string(),
        });
    }

    // Step 4: Field mapping (optional)
    if let Some(ref mapping_path) = config.output.field_mapping_path {
        match crate::pipeline::writer::validate_field_mapping(mapping_path) {
            Ok(count) => {
                checks.push(ValidationCheck {
                    name: "field_mapping".to_string(),
                    status: CheckStatus::Pass,
                    details: format!("{} mappings defined", count),
                });
            }
            Err(e) => {
                let err_msg = format!("Field mapping error: {:#}", e);
                checks.push(ValidationCheck {
                    name: "field_mapping".to_string(),
                    status: CheckStatus::Fail,
                    details: err_msg.clone(),
                });
                errors.push(err_msg);
            }
        }
    } else {
        checks.push(ValidationCheck {
            name: "field_mapping".to_string(),
            status: CheckStatus::Skipped,
            details: "Not configured".to_string(),
        });
    }

    // Step 5: Header template existence (optional)
    if let Some(ref header) = config.header {
        if let Some(ref template_path) = header.template {
            if template_path.exists() {
                checks.push(ValidationCheck {
                    name: "header_template".to_string(),
                    status: CheckStatus::Pass,
                    details: "File exists".to_string(),
                });
            } else {
                let err_msg = format!(
                    "Header template file does not exist: {}",
                    template_path.display()
                );
                checks.push(ValidationCheck {
                    name: "header_template".to_string(),
                    status: CheckStatus::Fail,
                    details: err_msg.clone(),
                });
                errors.push(err_msg);
            }
        } else {
            checks.push(ValidationCheck {
                name: "header_template".to_string(),
                status: CheckStatus::Skipped,
                details: "No template configured".to_string(),
            });
        }
    } else {
        checks.push(ValidationCheck {
            name: "header_template".to_string(),
            status: CheckStatus::Skipped,
            details: "Not configured".to_string(),
        });
    }

    // Step 6: Spatial filter source files (optional, per-input)
    {
        let mut sf_sources: Vec<(usize, &str)> = Vec::new();
        for (i, input) in config.inputs.iter().enumerate() {
            if let Some(ref sf) = input.spatial_filter {
                sf_sources.push((i, &sf.source));
            }
        }
        if sf_sources.is_empty() {
            checks.push(ValidationCheck {
                name: "spatial_filter".to_string(),
                status: CheckStatus::Skipped,
                details: "Not configured".to_string(),
            });
        } else {
            let mut all_ok = true;
            let mut details_parts = Vec::new();
            for (i, source) in &sf_sources {
                // Support brace/glob patterns in spatial_filter.source
                if source.contains('{') || source.contains('*') || source.contains('?') {
                    let expanded = expand_braces(source);
                    let mut matched = false;
                    for pat in &expanded {
                        if let Ok(entries) = glob::glob(pat) {
                            if entries.filter_map(|e| e.ok()).next().is_some() {
                                matched = true;
                                break;
                            }
                        }
                    }
                    if matched {
                        details_parts.push(format!("input #{}: {} (pattern)", i, source));
                    } else {
                        all_ok = false;
                        let err_msg = format!(
                            "Input #{}: spatial_filter.source pattern matched no files: {}",
                            i, source
                        );
                        errors.push(err_msg.clone());
                        details_parts.push(err_msg);
                    }
                } else {
                    let path = std::path::Path::new(source);
                    if path.exists() {
                        details_parts.push(format!("input #{}: {}", i, source));
                    } else {
                        all_ok = false;
                        let err_msg = format!(
                            "Input #{}: spatial_filter.source file does not exist: {}",
                            i, source
                        );
                        errors.push(err_msg.clone());
                        details_parts.push(err_msg);
                    }
                }
            }
            checks.push(ValidationCheck {
                name: "spatial_filter".to_string(),
                status: if all_ok { CheckStatus::Pass } else { CheckStatus::Fail },
                details: details_parts.join("; "),
            });
        }
    }

    // Step 7: Generalize configs (optional, per-input)
    {
        let mut gen_details: Vec<String> = Vec::new();
        for (i, input) in config.inputs.iter().enumerate() {
            if let Some(ref gen) = input.generalize {
                let mut parts = Vec::new();
                if let Some(ref algo) = gen.smooth {
                    parts.push(format!("smooth={}", algo));
                }
                parts.push(format!("iterations={}", gen.iterations));
                if let Some(tol) = gen.simplify {
                    parts.push(format!("simplify={}", tol));
                }
                gen_details.push(format!("input #{}: {}", i, parts.join(", ")));
            }
        }
        if gen_details.is_empty() {
            checks.push(ValidationCheck {
                name: "generalize".to_string(),
                status: CheckStatus::Skipped,
                details: "Not configured".to_string(),
            });
        } else {
            checks.push(ValidationCheck {
                name: "generalize".to_string(),
                status: CheckStatus::Pass,
                details: gen_details.join("; "),
            });
        }
    }

    // Step 8: Label case validation (optional, in rules)
    if let Some(ref rules_path) = config.rules {
        if let Ok(rules_file) = crate::rules::load_rules(rules_path) {
            let mut lc_details: Vec<String> = Vec::new();
            for ruleset in &rules_file.rulesets {
                let rs_name = ruleset.name.as_deref().unwrap_or(&ruleset.source_layer);
                let has_label_in_rules = ruleset.rules.iter().any(|r| r.set.contains_key("Label"));

                if let Some(lc) = ruleset.label_case {
                    lc_details.push(format!("{}: {:?}", rs_name, lc));
                    if !has_label_in_rules {
                        warnings.push(format!(
                            "Ruleset '{}': label_case is {:?} but no rule sets a Label field",
                            rs_name, lc
                        ));
                    }
                }

                for (j, rule) in ruleset.rules.iter().enumerate() {
                    if let Some(lc) = rule.label_case {
                        if !rule.set.contains_key("Label") {
                            warnings.push(format!(
                                "Ruleset '{}', rule #{}: label_case is {:?} but rule does not set Label",
                                rs_name, j + 1, lc
                            ));
                        }
                    }
                }
            }
            if lc_details.is_empty() {
                checks.push(ValidationCheck {
                    name: "label_case".to_string(),
                    status: CheckStatus::Skipped,
                    details: "Not configured in any ruleset".to_string(),
                });
            } else {
                checks.push(ValidationCheck {
                    name: "label_case".to_string(),
                    status: CheckStatus::Pass,
                    details: format!("{} ruleset(s): {}", lc_details.len(), lc_details.join(", ")),
                });
            }
        }
    }

    // Build summary
    let status = if errors.is_empty() {
        ValidationStatus::Valid
    } else {
        ValidationStatus::Invalid
    };

    let summary = Some(crate::report::ValidationSummary {
        grid_cell_size: config.grid.cell_size,
        grid_overlap: config.grid.overlap,
        input_sources: config.inputs.len(),
        output_directory: config.output.directory.clone(),
        filename_pattern: config.output.filename_pattern.clone(),
    });

    Ok(ValidationReport {
        status,
        config_file: config_path.to_string(),
        checks,
        errors,
        warnings,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert_eq!(config.version, 1);
        assert_eq!(config.grid.cell_size, 0.15);
        assert_eq!(config.grid.overlap, 0.0);
        assert_eq!(config.output.filename_pattern, "{col}_{row}.mp");
        assert_eq!(config.error_handling, "continue");
    }

    #[test]
    fn test_grid_config_with_origin() {
        let yaml = r#"
cell_size: 0.15
overlap: 0.01
origin: [0.0, 0.0]
"#;
        let grid: GridConfig = serde_yml::from_str(yaml).unwrap();
        assert_eq!(grid.cell_size, 0.15);
        assert_eq!(grid.overlap, 0.01);
        assert_eq!(grid.origin, Some([0.0, 0.0]));
    }

    #[test]
    fn test_input_source_path() {
        let yaml = r#"
path: "data/*.shp"
"#;
        let input: InputSource = serde_yml::from_str(yaml).unwrap();
        assert_eq!(input.path, Some("data/*.shp".to_string()));
        assert!(input.connection.is_none());
    }

    #[test]
    fn test_input_source_connection() {
        let yaml = r#"
connection: "PG:host=localhost"
layer: "roads"
"#;
        let input: InputSource = serde_yml::from_str(yaml).unwrap();
        assert_eq!(input.connection, Some("PG:host=localhost".to_string()));
        assert_eq!(input.layer, Some("roads".to_string()));
    }

    // Tests for Config::validate()
    #[test]
    fn test_config_validate_positive_cell_size() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validate_negative_cell_size_error() {
        let yaml = r#"
version: 1
grid:
  cell_size: -0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cell_size must be positive"));
    }

    #[test]
    fn test_config_validate_zero_cell_size_error() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.0
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cell_size must be positive"));
    }

    #[test]
    fn test_config_validate_non_negative_overlap() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
  overlap: 0.005
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validate_negative_overlap_error() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
  overlap: -0.01
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("overlap cannot be negative"));
    }

    #[test]
    fn test_config_validate_error_handling_values() {
        let yaml_continue = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
error_handling: "continue"
"#;
        let config: Config = serde_yml::from_str(yaml_continue).unwrap();
        assert!(config.validate().is_ok());

        let yaml_fail_fast = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
error_handling: "fail-fast"
"#;
        let config: Config = serde_yml::from_str(yaml_fail_fast).unwrap();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validate_invalid_error_handling() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
error_handling: "invalid"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        // Updated for L2 fix: validation now uses ErrorMode::from_str
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Invalid error_handling")
                || error_msg.contains("expected 'continue' or 'fail-fast'")
        );
    }

    #[test]
    fn test_config_validate_at_least_one_input() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs: []
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("At least one input source"));
    }

    #[test]
    fn test_input_source_mutual_exclusion() {
        // Valid: has path only
        let yaml_path = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml_path).unwrap();
        assert!(config.validate().is_ok());

        // Valid: has connection only
        let yaml_conn = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - connection: "PG:host=localhost"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml_conn).unwrap();
        assert!(config.validate().is_ok());

        // Invalid: has both (note: serde won't allow this in YAML, but we test struct validation)
        let mut config = Config {
            version: 1,
            grid: GridConfig {
                cell_size: 0.15,
                overlap: 0.0,
                origin: None,
            },
            inputs: vec![InputSource {
                path: Some("data.shp".to_string()),
                connection: Some("PG:host=localhost".to_string()),
                layer: None,
                layers: None,
                source_srs: None,
                target_srs: None,
                attribute_filter: None,
                layer_alias: None,
                generalize: None,
                spatial_filter: None,
                dedup_by_field: None,
            }],
            output: OutputConfig {
                directory: "tiles/".to_string(),
                filename_pattern: "{col}_{row}.mp".to_string(),
                field_mapping_path: None,
                overwrite: None,
                base_id: None,
            },
            filters: None,
            error_handling: "continue".to_string(),
            header: None,
            rules: None,
            default_dedup_by_field: None,
            generalize_profiles_path: None,
            resolved_profile_map: Default::default(),
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must have either 'path' or 'connection'"));

        // Invalid: has neither
        config.inputs[0].path = None;
        config.inputs[0].connection = None;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must have either 'path' or 'connection'"));
    }

    #[test]
    fn test_filter_bbox_validation() {
        // Valid bbox
        let yaml_valid = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
filters:
  bbox: [-5.0, 41.0, 10.0, 51.5]
"#;
        let config: Config = serde_yml::from_str(yaml_valid).unwrap();
        assert!(config.validate().is_ok());

        // Invalid: min_lon >= max_lon
        let yaml_invalid_lon = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
filters:
  bbox: [10.0, 41.0, -5.0, 51.5]
"#;
        let config: Config = serde_yml::from_str(yaml_invalid_lon).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("min_lon must be < max_lon"));

        // Invalid: min_lat >= max_lat
        let yaml_invalid_lat = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
filters:
  bbox: [-5.0, 51.5, 10.0, 41.0]
"#;
        let config: Config = serde_yml::from_str(yaml_invalid_lat).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("min_lat must be < max_lat"));
    }

    // =================================================================
    // Tests: effective_layer_name
    // =================================================================

    #[test]
    fn test_effective_layer_name_from_path() {
        let input = InputSource {
            path: Some("/data/LIEUX_NOMMES/ZONE_D_HABITATION.shp".to_string()),
            ..Default::default()
        };
        assert_eq!(input.effective_layer_name(), Some("ZONE_D_HABITATION".to_string()));
    }

    #[test]
    fn test_effective_layer_name_layer_alias_wins() {
        let input = InputSource {
            path: Some("/data/ZONE_D_HABITATION.shp".to_string()),
            layer_alias: Some("MY_ALIAS".to_string()),
            ..Default::default()
        };
        assert_eq!(input.effective_layer_name(), Some("MY_ALIAS".to_string()));
    }

    #[test]
    fn test_effective_layer_name_layer_over_path() {
        let input = InputSource {
            path: Some("/data/ZONE_D_HABITATION.shp".to_string()),
            layer: Some("my_layer".to_string()),
            ..Default::default()
        };
        assert_eq!(input.effective_layer_name(), Some("my_layer".to_string()));
    }

    #[test]
    fn test_effective_layer_name_wildcard_returns_none() {
        let input = InputSource {
            path: Some("/data/**/*.shp".to_string()),
            ..Default::default()
        };
        assert_eq!(input.effective_layer_name(), None);
    }

    #[test]
    fn test_effective_layer_name_wildcard_with_alias() {
        // layer_alias takes precedence even for wildcard paths
        let input = InputSource {
            path: Some("/data/**/*.shp".to_string()),
            layer_alias: Some("MY_LAYER".to_string()),
            ..Default::default()
        };
        assert_eq!(input.effective_layer_name(), Some("MY_LAYER".to_string()));
    }

    #[test]
    fn test_effective_layer_name_no_path_no_layer() {
        let input = InputSource {
            connection: Some("PG:host=localhost".to_string()),
            ..Default::default()
        };
        assert_eq!(input.effective_layer_name(), None);
    }

    // =================================================================
    // Tests: build_generalize_map
    // =================================================================

    #[test]
    fn test_build_generalize_map_basic() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data/ZONE_D_HABITATION.shp"
    generalize:
      smooth: "chaikin"
      iterations: 2
  - path: "data/BATIMENT.shp"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let map = config.build_generalize_map();
        assert_eq!(map.len(), 1);
        assert!(map.contains_key("ZONE_D_HABITATION"));
        assert_eq!(map["ZONE_D_HABITATION"].iterations, 2);
    }

    #[test]
    fn test_build_generalize_map_wildcard_ignored() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data/**/*.shp"
    generalize:
      smooth: "chaikin"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let map = config.build_generalize_map();
        assert!(map.is_empty());
    }

    #[test]
    fn test_build_generalize_map_empty_when_no_generalize() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data/ZONE.shp"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert!(config.build_generalize_map().is_empty());
    }

    // =================================================================
    // Tests: GeneralizeConfig validation (H4)
    // =================================================================

    #[test]
    fn test_validate_generalize_iterations_zero() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data/ZONE.shp"
    generalize:
      smooth: "chaikin"
      iterations: 0
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("iterations must be >= 1"));
    }

    #[test]
    fn test_validate_generalize_negative_simplify() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data/ZONE.shp"
    generalize:
      simplify: -0.001
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("simplify must be positive"));
    }

    #[test]
    fn test_validate_generalize_unknown_algorithm() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data/ZONE.shp"
    generalize:
      smooth: "mcmaster"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown generalize.smooth algorithm"));
    }

    #[test]
    fn test_validate_generalize_valid_config() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data/ZONE.shp"
    generalize:
      smooth: "chaikin"
      iterations: 2
      simplify: 0.00005
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_input_source_type_detection() {
        // File type (path)
        let input_file = InputSource {
            path: Some("data.shp".to_string()),
            connection: None,
            layer: None,
            layers: None,
            source_srs: None,
            target_srs: None,
            attribute_filter: None,
            layer_alias: None,
            generalize: None,
            spatial_filter: None,
            dedup_by_field: None,
        };
        assert_eq!(input_file.source_type(), SourceType::File);

        // PostGIS type (PG: prefix)
        let input_pg1 = InputSource {
            path: None,
            connection: Some("PG:host=localhost dbname=gis".to_string()),
            layer: Some("roads".to_string()),
            layers: None,
            source_srs: None,
            target_srs: None,
            attribute_filter: None,
            layer_alias: None,
            generalize: None,
            spatial_filter: None,
            dedup_by_field: None,
        };
        assert_eq!(input_pg1.source_type(), SourceType::PostGIS);

        // PostGIS type (host= pattern)
        let input_pg2 = InputSource {
            path: None,
            connection: Some("host=localhost dbname=gis user=admin".to_string()),
            layer: None,
            layers: None,
            source_srs: None,
            target_srs: None,
            attribute_filter: None,
            layer_alias: None,
            generalize: None,
            spatial_filter: None,
            dedup_by_field: None,
        };
        assert_eq!(input_pg2.source_type(), SourceType::PostGIS);

        // File type (connection is not PostGIS-like)
        let input_other = InputSource {
            path: None,
            connection: Some("sqlite://db.sqlite".to_string()),
            layer: None,
            layers: None,
            source_srs: None,
            target_srs: None,
            attribute_filter: None,
            layer_alias: None,
            generalize: None,
            spatial_filter: None,
            dedup_by_field: None,
        };
        assert_eq!(input_other.source_type(), SourceType::File);
    }

    #[test]
    fn test_resolve_wildcard_paths() {
        use std::fs;
        use tempfile::TempDir;

        // Create temp directory with test files
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        fs::write(temp_path.join("file1.shp"), "").unwrap();
        fs::write(temp_path.join("file2.shp"), "").unwrap();
        fs::write(temp_path.join("roads.gpkg"), "").unwrap();

        // Test wildcard expansion
        let pattern = format!("{}/*.shp", temp_path.display());
        let resolved = resolve_wildcard_paths(&pattern).unwrap();

        assert_eq!(resolved.len(), 2);
        assert!(resolved
            .iter()
            .any(|p| p.file_name().unwrap() == "file1.shp"));
        assert!(resolved
            .iter()
            .any(|p| p.file_name().unwrap() == "file2.shp"));

        // Test no matches (warning logged)
        let pattern_no_match = format!("{}/*.xyz", temp_path.display());
        let resolved_empty = resolve_wildcard_paths(&pattern_no_match).unwrap();
        assert_eq!(resolved_empty.len(), 0);

        // Test brace expansion with glob
        let sub_a = temp_path.join("A");
        let sub_b = temp_path.join("B");
        let sub_c = temp_path.join("C");
        fs::create_dir_all(&sub_a).unwrap();
        fs::create_dir_all(&sub_b).unwrap();
        fs::create_dir_all(&sub_c).unwrap();
        fs::write(sub_a.join("data.shp"), "").unwrap();
        fs::write(sub_b.join("data.shp"), "").unwrap();
        fs::write(sub_c.join("data.shp"), "").unwrap();

        let pattern_braces = format!("{}/{{A,B}}/data.shp", temp_path.display());
        let resolved_braces = resolve_wildcard_paths(&pattern_braces).unwrap();
        assert_eq!(resolved_braces.len(), 2);
        assert!(resolved_braces.iter().any(|p| p.ends_with("A/data.shp")));
        assert!(resolved_braces.iter().any(|p| p.ends_with("B/data.shp")));
        // C must not be included
        assert!(!resolved_braces.iter().any(|p| p.ends_with("C/data.shp")));
    }

    #[test]
    fn test_expand_braces() {
        // Single brace group
        assert_eq!(
            expand_braces("data/{D038,D001}/TRANSPORT.shp"),
            vec!["data/D038/TRANSPORT.shp", "data/D001/TRANSPORT.shp"]
        );

        // Three alternatives
        assert_eq!(
            expand_braces("root/{a,b,c}/file"),
            vec!["root/a/file", "root/b/file", "root/c/file"]
        );

        // No braces — passthrough
        assert_eq!(
            expand_braces("data/D038/TRANSPORT.shp"),
            vec!["data/D038/TRANSPORT.shp"]
        );

        // Unclosed brace — passthrough
        assert_eq!(
            expand_braces("data/{D038/TRANSPORT.shp"),
            vec!["data/{D038/TRANSPORT.shp"]
        );

        // Braces with spaces (trimmed)
        assert_eq!(
            expand_braces("data/{ D038 , D001 }/file"),
            vec!["data/D038/file", "data/D001/file"]
        );

        // Empty alternatives are filtered out
        assert_eq!(
            expand_braces("data/{D038,}/file"),
            vec!["data/D038/file"]
        );
        assert_eq!(
            expand_braces("data/{,D038}/file"),
            vec!["data/D038/file"]
        );
    }

    // Story 7.4: field_mapping_path tests
    #[test]
    fn test_config_with_field_mapping_path() {
        use std::fs;
        use tempfile::TempDir;

        // Create temp file for mapping
        let temp_dir = TempDir::new().unwrap();
        let mapping_path = temp_dir.path().join("mapping.yaml");
        fs::write(&mapping_path, "MP_TYPE: Type\nNAME: Label").unwrap();

        let yaml = format!(
            r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
  field_mapping_path: "{}"
"#,
            mapping_path.display()
        );
        let config: Config = serde_yml::from_str(&yaml).unwrap();
        assert!(config.output.field_mapping_path.is_some());
        assert_eq!(
            config.output.field_mapping_path.as_ref().unwrap(),
            &mapping_path
        );
        // Validation should pass when file exists
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_without_field_mapping_path() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert!(config.output.field_mapping_path.is_none());
        // Validation should pass even without field_mapping_path (backward compat)
        assert!(config.validate().is_ok());
    }

    // Test removed (Story 7.4 Code Review Fix H3):
    // field_mapping_path validation moved from config.validate() to MpWriter::new()
    // to avoid race condition in parallel mode. See test_field_mapping_invalid_path_error
    // in tests/integration_export.rs for validation coverage.

    // Story 8.1: HeaderConfig tests
    #[test]
    fn test_config_with_header_section() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
header:
  name: "Ma Carte"
  levels: "4"
  transparent: "Y"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert!(config.header.is_some());
        let header = config.header.clone().unwrap();
        assert_eq!(header.name, Some("Ma Carte".to_string()));
        assert_eq!(header.levels, Some("4".to_string()));
        assert_eq!(header.transparent, Some("Y".to_string()));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_without_header_section() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert!(config.header.is_none());
        // Validation should pass without header (backward compat)
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_with_header_template() {
        use std::fs;
        use tempfile::TempDir;

        // Create temp template file
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("template.mp");
        fs::write(&template_path, "[IMG ID]\nName=Template").unwrap();

        let yaml = format!(
            r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
header:
  template: "{}"
"#,
            template_path.display()
        );
        let config: Config = serde_yml::from_str(&yaml).unwrap();
        assert!(config.header.is_some());
        let header = config.header.clone().unwrap();
        assert!(header.template.is_some());
        assert_eq!(header.template.as_ref().unwrap(), &template_path);
        // Validation should pass when template exists
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_header_template_no_early_validation() {
        // Story 8.1 Code Review Fix H2: Template validation moved to MpWriter::new()
        // Config::validate() no longer checks template existence to avoid TOCTOU race
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
header:
  template: "/nonexistent/template.mp"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        // Should pass - validation happens at usage time in MpWriter::new()
        assert!(result.is_ok(), "Config validation should not check template existence (TOCTOU fix)");
    }

    #[test]
    fn test_config_header_custom_fields() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
header:
  custom:
    DrawPriority: "25"
    MG: "N"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert!(config.header.is_some());
        let header = config.header.clone().unwrap();
        assert!(header.custom.is_some());
        let custom = header.custom.unwrap();
        assert_eq!(custom.get("DrawPriority"), Some(&"25".to_string()));
        assert_eq!(custom.get("MG"), Some(&"N".to_string()));
        assert!(config.validate().is_ok());
    }

    // Story 8.2: Filename pattern validation tests
    #[test]
    fn test_config_validate_valid_filename_pattern() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
  filename_pattern: "{col:03}_{row:03}.mp"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validate_invalid_filename_pattern() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
  filename_pattern: "{invalid_var}.mp"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Invalid filename_pattern"));
    }

    #[test]
    fn test_config_validate_default_pattern_valid() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        // Default {col}_{row}.mp should validate
        assert!(config.validate().is_ok());
    }

    // Story 8.3: OutputConfig overwrite field tests
    #[test]
    fn test_config_output_overwrite_absent() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert!(config.output.overwrite.is_none());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_output_overwrite_true() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
  overwrite: true
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert_eq!(config.output.overwrite, Some(true));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_output_overwrite_false() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
  overwrite: false
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert_eq!(config.output.overwrite, Some(false));
        assert!(config.validate().is_ok());
    }

    // Story 9.4: source_srs / target_srs tests
    #[test]
    fn test_input_source_with_source_srs_and_target_srs() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:4326"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert_eq!(
            config.inputs[0].source_srs,
            Some("EPSG:2154".to_string())
        );
        assert_eq!(
            config.inputs[0].target_srs,
            Some("EPSG:4326".to_string())
        );
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_input_source_with_source_srs_only() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
    source_srs: "EPSG:2154"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert_eq!(
            config.inputs[0].source_srs,
            Some("EPSG:2154".to_string())
        );
        assert!(config.inputs[0].target_srs.is_none());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_input_source_without_srs_backward_compat() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert!(config.inputs[0].source_srs.is_none());
        assert!(config.inputs[0].target_srs.is_none());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_input_source_invalid_source_srs_error() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
    source_srs: "EPSG:99999"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid source_srs"));
        assert!(err.contains("EPSG:99999"));
    }

    #[test]
    fn test_input_source_invalid_target_srs_error() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:99999"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid target_srs"));
        assert!(err.contains("EPSG:99999"));
    }

    #[test]
    fn test_input_source_target_srs_without_source_srs_warning() {
        // target_srs without source_srs should still parse and validate OK (just warns)
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
    target_srs: "EPSG:4326"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert!(config.inputs[0].source_srs.is_none());
        assert_eq!(
            config.inputs[0].target_srs,
            Some("EPSG:4326".to_string())
        );
        // Should validate OK (warning only, not error)
        assert!(config.validate().is_ok());
    }

    // mp-header-config: Header with all BDTOPO fields parses and validates
    #[test]
    fn test_config_header_bdtopo_complete() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
header:
  name: "BDTOPO {col:03}_{row:03}"
  copyright: "IGN BDTOPO 2025"
  levels: "2"
  level0: "24"
  level1: "18"
  tree_size: "1000"
  rgn_limit: "1024"
  transparent: "N"
  marine: "N"
  preprocess: "F"
  lbl_coding: "9"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert!(config.header.is_some());
        let header = config.header.as_ref().unwrap();
        assert_eq!(header.name, Some("BDTOPO {col:03}_{row:03}".to_string()));
        assert_eq!(header.copyright, Some("IGN BDTOPO 2025".to_string()));
        assert_eq!(header.levels, Some("2".to_string()));
        assert_eq!(header.level0, Some("24".to_string()));
        assert_eq!(header.level1, Some("18".to_string()));
        assert_eq!(header.tree_size, Some("1000".to_string()));
        assert_eq!(header.rgn_limit, Some("1024".to_string()));
        assert_eq!(header.transparent, Some("N".to_string()));
        assert_eq!(header.marine, Some("N".to_string()));
        assert_eq!(header.preprocess, Some("F".to_string()));
        assert_eq!(header.lbl_coding, Some("9".to_string()));
        // Validation should pass (all values within range)
        assert!(config.validate().is_ok());
    }

    // mp-header-config: Header name with variable pattern validates OK
    #[test]
    fn test_config_header_name_pattern_valid() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
header:
  name: "MyMap {col}_{row}"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert!(config.validate().is_ok());
    }

    // mp-header-config: Header name with invalid pattern is a hard error (F1 fix)
    #[test]
    fn test_config_header_name_invalid_pattern_error() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
header:
  name: "MyMap {invalid_var}"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Invalid header.name pattern"));
    }

    // mp-header-config: base_id without header section creates default header (F3 coverage)
    #[test]
    fn test_config_base_id_without_header_section() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
  base_id: 6324
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert!(config.header.is_none());
        assert_eq!(config.output.base_id, Some(6324));
        assert!(config.validate().is_ok());
    }

    // mp-header-config: Header values out of range warn but don't error
    #[test]
    fn test_config_header_out_of_range_warns_no_error() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
header:
  levels: "99"
  level0: "5"
  tree_size: "50"
  rgn_limit: "2000"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        // Soft validation: warns but does not fail
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_input_source_clone_preserves_all_fields() {
        let input = InputSource {
            path: Some("data/COURBE_0840.shp".to_string()),
            connection: None,
            layer: None,
            layers: Some(vec!["layer1".to_string()]),
            source_srs: Some("EPSG:2154".to_string()),
            target_srs: Some("EPSG:4326".to_string()),
            attribute_filter: Some("ALTITUDE > 100".to_string()),
            layer_alias: Some("COURBE".to_string()),
            generalize: None,
            spatial_filter: None,
            dedup_by_field: None,
        };
        let cloned = input.clone();
        assert_eq!(cloned.path, input.path);
        assert_eq!(cloned.layers, input.layers);
        assert_eq!(cloned.source_srs, input.source_srs);
        assert_eq!(cloned.target_srs, input.target_srs);
        assert_eq!(cloned.attribute_filter, input.attribute_filter);
        assert_eq!(cloned.layer_alias, input.layer_alias);
    }

    // -- run_validate tests --

    #[test]
    fn test_run_validate_valid_minimal_config() {
        use crate::report::{CheckStatus, ValidationStatus};
        use tempfile::NamedTempFile;
        use std::io::Write;

        // Create a minimal valid YAML config with an existing file as input
        let mut input_file = NamedTempFile::new().unwrap();
        write!(input_file, "dummy").unwrap();
        let input_path = input_file.path().to_str().unwrap().to_string();

        let yaml = format!(
            r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "{}"
output:
  directory: "tiles/"
"#,
            input_path
        );

        let mut config_file = NamedTempFile::new().unwrap();
        write!(config_file, "{}", yaml).unwrap();
        let config_path = config_file.path().to_str().unwrap().to_string();

        let report = run_validate(&config_path).unwrap();
        assert_eq!(report.status, ValidationStatus::Valid);
        assert!(report.errors.is_empty());
        assert!(report.is_valid());

        // Should have yaml_syntax (pass), semantic (pass), input_files (pass),
        // plus skipped checks for rules, field_mapping, header_template
        let passed: Vec<_> = report.checks.iter().filter(|c| c.status == CheckStatus::Pass).collect();
        assert!(passed.len() >= 3, "Expected at least 3 passed checks, got {}", passed.len());

        let skipped: Vec<_> = report.checks.iter().filter(|c| c.status == CheckStatus::Skipped).collect();
        assert_eq!(skipped.len(), 5, "Expected 5 skipped checks (rules, field_mapping, header_template, spatial_filter, generalize)");
    }

    #[test]
    fn test_run_validate_invalid_yaml_syntax() {
        use crate::report::{CheckStatus, ValidationStatus};
        use tempfile::NamedTempFile;
        use std::io::Write;

        let yaml = "grid:\n  cell_size: [invalid yaml\n  broken:";
        let mut config_file = NamedTempFile::new().unwrap();
        write!(config_file, "{}", yaml).unwrap();
        let config_path = config_file.path().to_str().unwrap().to_string();

        let report = run_validate(&config_path).unwrap();
        assert_eq!(report.status, ValidationStatus::Invalid);
        assert!(!report.errors.is_empty());

        let yaml_check = report.checks.iter().find(|c| c.name == "yaml_syntax").unwrap();
        assert_eq!(yaml_check.status, CheckStatus::Fail);
    }

    #[test]
    fn test_run_validate_semantic_error() {
        use crate::report::{CheckStatus, ValidationStatus};
        use tempfile::NamedTempFile;
        use std::io::Write;

        // Valid YAML but invalid semantics (negative cell_size)
        let yaml = r#"
version: 1
grid:
  cell_size: -1.0
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
"#;
        let mut config_file = NamedTempFile::new().unwrap();
        write!(config_file, "{}", yaml).unwrap();
        let config_path = config_file.path().to_str().unwrap().to_string();

        let report = run_validate(&config_path).unwrap();
        assert_eq!(report.status, ValidationStatus::Invalid);

        let yaml_check = report.checks.iter().find(|c| c.name == "yaml_syntax").unwrap();
        assert_eq!(yaml_check.status, CheckStatus::Pass);

        let semantic_check = report.checks.iter().find(|c| c.name == "semantic_validation").unwrap();
        assert_eq!(semantic_check.status, CheckStatus::Fail);
        assert!(semantic_check.details.contains("cell_size must be positive"));
    }

    #[test]
    fn test_run_validate_missing_config_file() {
        use crate::report::{CheckStatus, ValidationStatus};

        let report = run_validate("/nonexistent/path/config.yaml").unwrap();
        assert_eq!(report.status, ValidationStatus::Invalid);

        let yaml_check = report.checks.iter().find(|c| c.name == "yaml_syntax").unwrap();
        assert_eq!(yaml_check.status, CheckStatus::Fail);
    }

    // --- spatial_filter validation tests ---

    #[test]
    fn test_validate_spatial_filter_negative_buffer() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data/ZONE.shp"
    spatial_filter:
      source: "commune.shp"
      buffer: -100.0
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("buffer must not be negative"));
    }

    #[test]
    fn test_validate_spatial_filter_empty_source() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data/ZONE.shp"
    spatial_filter:
      source: ""
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("source must not be empty"));
    }

    #[test]
    fn test_validate_spatial_filter_valid() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data/ZONE.shp"
    spatial_filter:
      source: "commune.shp"
      buffer: 500.0
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert!(config.validate().is_ok());
    }

    // --- run_validate: spatial_filter, generalize, label_case checks ---

    #[test]
    fn test_run_validate_spatial_filter_missing_file() {
        use crate::report::{CheckStatus};
        use tempfile::NamedTempFile;
        use std::io::Write;

        let mut input_file = NamedTempFile::new().unwrap();
        write!(input_file, "dummy").unwrap();
        let input_path = input_file.path().to_str().unwrap().to_string();

        let yaml = format!(
            r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "{}"
    spatial_filter:
      source: "/nonexistent/commune.shp"
output:
  directory: "tiles/"
"#,
            input_path
        );

        let mut config_file = NamedTempFile::new().unwrap();
        write!(config_file, "{}", yaml).unwrap();
        let config_path = config_file.path().to_str().unwrap().to_string();

        let report = run_validate(&config_path).unwrap();
        let sf_check = report.checks.iter().find(|c| c.name == "spatial_filter").unwrap();
        assert_eq!(sf_check.status, CheckStatus::Fail);
        assert!(sf_check.details.contains("does not exist"));
    }

    #[test]
    fn test_run_validate_spatial_filter_existing_file() {
        use crate::report::{CheckStatus};
        use tempfile::NamedTempFile;
        use std::io::Write;

        let mut input_file = NamedTempFile::new().unwrap();
        write!(input_file, "dummy").unwrap();
        let input_path = input_file.path().to_str().unwrap().to_string();

        let mut sf_file = NamedTempFile::new().unwrap();
        write!(sf_file, "dummy").unwrap();
        let sf_path = sf_file.path().to_str().unwrap().to_string();

        let yaml = format!(
            r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "{}"
    spatial_filter:
      source: "{}"
output:
  directory: "tiles/"
"#,
            input_path, sf_path
        );

        let mut config_file = NamedTempFile::new().unwrap();
        write!(config_file, "{}", yaml).unwrap();
        let config_path = config_file.path().to_str().unwrap().to_string();

        let report = run_validate(&config_path).unwrap();
        let sf_check = report.checks.iter().find(|c| c.name == "spatial_filter").unwrap();
        assert_eq!(sf_check.status, CheckStatus::Pass);
    }

    #[test]
    fn test_run_validate_generalize_check_present() {
        use crate::report::{CheckStatus};
        use tempfile::NamedTempFile;
        use std::io::Write;

        let mut input_file = NamedTempFile::new().unwrap();
        write!(input_file, "dummy").unwrap();
        let input_path = input_file.path().to_str().unwrap().to_string();

        let yaml = format!(
            r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "{}"
    generalize:
      smooth: "chaikin"
      iterations: 2
output:
  directory: "tiles/"
"#,
            input_path
        );

        let mut config_file = NamedTempFile::new().unwrap();
        write!(config_file, "{}", yaml).unwrap();
        let config_path = config_file.path().to_str().unwrap().to_string();

        let report = run_validate(&config_path).unwrap();
        let gen_check = report.checks.iter().find(|c| c.name == "generalize").unwrap();
        assert_eq!(gen_check.status, CheckStatus::Pass);
        assert!(gen_check.details.contains("smooth=chaikin"));
        assert!(gen_check.details.contains("iterations=2"));
    }

    // --- expand_env_vars tests ---

    #[test]
    fn test_expand_env_vars_replaces_defined_var() {
        std::env::set_var("TEST_EXPAND_FOO", "/data/files");
        let result = expand_env_vars("path: ${TEST_EXPAND_FOO}/sub");
        assert_eq!(result, "path: /data/files/sub");
        std::env::remove_var("TEST_EXPAND_FOO");
    }

    #[test]
    fn test_expand_env_vars_leaves_undefined_var() {
        std::env::remove_var("TEST_EXPAND_MISSING");
        let result = expand_env_vars("path: ${TEST_EXPAND_MISSING}/sub");
        assert_eq!(result, "path: ${TEST_EXPAND_MISSING}/sub");
    }

    #[test]
    fn test_expand_env_vars_multiple_vars_same_line() {
        std::env::set_var("TEST_EXPAND_A", "alpha");
        std::env::set_var("TEST_EXPAND_B", "beta");
        let result = expand_env_vars("${TEST_EXPAND_A}/${TEST_EXPAND_B}");
        assert_eq!(result, "alpha/beta");
        std::env::remove_var("TEST_EXPAND_A");
        std::env::remove_var("TEST_EXPAND_B");
    }

    #[test]
    fn test_expand_env_vars_no_placeholders() {
        let input = "plain string without variables";
        assert_eq!(expand_env_vars(input), input);
    }

    #[test]
    fn test_expand_env_vars_numeric_value() {
        std::env::set_var("TEST_EXPAND_NUM", "42");
        let result = expand_env_vars("base_id: ${TEST_EXPAND_NUM}");
        assert_eq!(result, "base_id: 42");
        std::env::remove_var("TEST_EXPAND_NUM");
    }

    #[test]
    fn test_expand_env_vars_ignores_invalid_var_names() {
        // ${123} does not match POSIX var name pattern — should be left as-is
        let result = expand_env_vars("val: ${123}");
        assert_eq!(result, "val: ${123}");

        let result = expand_env_vars("val: ${foo bar}");
        assert_eq!(result, "val: ${foo bar}");
    }

    // =================================================================
    // Tech-spec #2 Task 11 — GeneralizeProfile parsing + validation
    // =================================================================

    fn parse_profile(yaml: &str) -> GeneralizeProfile {
        serde_yml::from_str(yaml).expect("valid YAML")
    }

    #[test]
    fn test_profile_parses_single_level() {
        let p = parse_profile("levels:\n  - { n: 0, simplify: 0.0001 }\n");
        assert_eq!(p.levels.len(), 1);
        assert_eq!(p.levels[0].n, 0);
        assert_eq!(p.levels[0].simplify, Some(0.0001));
        assert!(p.when.is_empty());
    }

    #[test]
    fn test_profile_parses_when_clause() {
        let yaml = r#"
when:
  - field: CL_ADMIN
    values: [Autoroute, Nationale]
    levels:
      - { n: 0, simplify: 0.00002 }
      - { n: 2, simplify: 0.00008 }
levels:
  - { n: 0, simplify: 0.0001 }
"#;
        let p = parse_profile(yaml);
        assert_eq!(p.when.len(), 1);
        assert_eq!(p.when[0].field, "CL_ADMIN");
        assert_eq!(p.when[0].values, vec!["Autoroute", "Nationale"]);
        assert_eq!(p.when[0].levels.len(), 2);
        assert_eq!(p.levels.len(), 1);
    }

    #[test]
    fn test_profile_max_level_index() {
        let p = parse_profile(
            "levels:\n  - { n: 0 }\n  - { n: 2 }\nwhen:\n  - field: X\n    values: [a]\n    levels:\n      - { n: 0 }\n      - { n: 4 }\n",
        );
        assert_eq!(p.max_level_index(), Some(4));
    }

    #[test]
    fn test_profile_every_branch_has_n0_false_when_default_misses() {
        let p = parse_profile("levels:\n  - { n: 2 }\n");
        assert!(!p.every_branch_has_n0());
    }

    #[test]
    fn test_profile_every_branch_has_n0_false_when_when_branch_misses() {
        let p = parse_profile(
            "levels:\n  - { n: 0 }\nwhen:\n  - field: X\n    values: [a]\n    levels:\n      - { n: 2 }\n",
        );
        assert!(!p.every_branch_has_n0());
    }

    #[test]
    fn test_profile_validates_iterations_bound() {
        let p = parse_profile("levels:\n  - { n: 0, iterations: 6 }\n");
        let err = p.validate("test").unwrap_err().to_string();
        assert!(err.contains("iterations=6"), "error: {err}");
    }

    #[test]
    fn test_profile_validates_simplify_bound() {
        let p = parse_profile("levels:\n  - { n: 0, simplify: 0.01 }\n");
        let err = p.validate("test").unwrap_err().to_string();
        assert!(err.contains("simplify=0.01"), "error: {err}");
    }

    #[test]
    fn test_profile_validates_unknown_smooth_algo() {
        let p = parse_profile("levels:\n  - { n: 0, smooth: fourier }\n");
        let err = p.validate("test").unwrap_err().to_string();
        assert!(err.contains("unknown smooth algorithm"));
    }

    #[test]
    fn test_profile_validates_empty_values_list() {
        let p = parse_profile(
            "when:\n  - field: CL_ADMIN\n    values: []\n    levels:\n      - { n: 0 }\n",
        );
        let err = p.validate("test").unwrap_err().to_string();
        assert!(err.contains("'values' must list at least one attribute value"));
    }

    #[test]
    fn test_from_generalize_config_converts_to_single_level() {
        let inline = GeneralizeConfig {
            smooth: Some("chaikin".into()),
            iterations: 2,
            simplify: Some(0.00005),
        };
        let p: GeneralizeProfile = inline.into();
        assert_eq!(p.levels.len(), 1);
        assert_eq!(p.levels[0].n, 0);
        assert_eq!(p.levels[0].iterations, 2);
        assert_eq!(p.levels[0].simplify, Some(0.00005));
        assert!(p.when.is_empty());
    }

    // =================================================================
    // Tech-spec #2 Task 11 — build_profile_map + validation F4
    // =================================================================

    fn write_sources_yaml(dir: &std::path::Path, profile_path: Option<&str>) -> std::path::PathBuf {
        let mut yaml = String::from(
            r#"version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data/ZONE.shp"
    layer_alias: "ZONE_D_HABITATION"
    generalize:
      smooth: "chaikin"
      iterations: 1
      simplify: 0.00005
output:
  directory: "tiles/"
"#,
        );
        if let Some(p) = profile_path {
            yaml.push_str(&format!("generalize_profiles_path: \"{p}\"\n"));
        }
        let path = dir.join("sources.yaml");
        std::fs::write(&path, yaml).unwrap();
        path
    }

    fn write_profiles_yaml(dir: &std::path::Path, body: &str) -> std::path::PathBuf {
        let path = dir.join("generalize-profiles.yaml");
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn test_build_profile_map_fuses_inline_and_external() {
        let tmp = tempfile::tempdir().unwrap();
        let body = r#"profiles:
  TRONCON_HYDROGRAPHIQUE:
    levels:
      - { n: 0, simplify: 0.00005 }
      - { n: 2, simplify: 0.0002 }
"#;
        write_profiles_yaml(tmp.path(), body);
        write_sources_yaml(tmp.path(), Some("generalize-profiles.yaml"));

        let cfg = load_config(tmp.path().join("sources.yaml")).unwrap();
        assert!(cfg.resolved_profile_map.contains_key("ZONE_D_HABITATION"));
        assert!(cfg.resolved_profile_map.contains_key("TRONCON_HYDROGRAPHIQUE"));
        assert_eq!(cfg.resolved_profile_map["TRONCON_HYDROGRAPHIQUE"].levels.len(), 2);
    }

    #[test]
    fn test_build_profile_map_conflict_fail_fast() {
        let tmp = tempfile::tempdir().unwrap();
        let body = "profiles:\n  ZONE_D_HABITATION:\n    levels:\n      - { n: 0, simplify: 0.0001 }\n";
        write_profiles_yaml(tmp.path(), body);
        write_sources_yaml(tmp.path(), Some("generalize-profiles.yaml"));

        let err = format!(
            "{:#}",
            load_config(tmp.path().join("sources.yaml")).unwrap_err()
        );
        assert!(err.contains("generalize profile conflict"), "error: {err}");
        assert!(err.contains("ZONE_D_HABITATION"), "error: {err}");
    }

    #[test]
    fn test_build_profile_map_f4_routable_missing_n0_default() {
        let tmp = tempfile::tempdir().unwrap();
        let body = r#"profiles:
  TRONCON_DE_ROUTE:
    levels:
      - { n: 2, simplify: 0.0001 }
"#;
        write_profiles_yaml(tmp.path(), body);
        let sources = r#"version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data/ROUTE.shp"
output:
  directory: "tiles/"
generalize_profiles_path: "generalize-profiles.yaml"
"#;
        std::fs::write(tmp.path().join("sources.yaml"), sources).unwrap();
        let err = format!(
            "{:#}",
            load_config(tmp.path().join("sources.yaml")).unwrap_err()
        );
        assert!(
            err.contains("routable layer 'TRONCON_DE_ROUTE' profile missing n:0"),
            "error: {err}"
        );
    }

    #[test]
    fn test_build_profile_map_f4_when_branch_drops_n0() {
        let tmp = tempfile::tempdir().unwrap();
        let body = r#"profiles:
  TRONCON_DE_ROUTE:
    when:
      - field: CL_ADMIN
        values: [Autoroute]
        levels:
          - { n: 2, simplify: 0.0001 }
    levels:
      - { n: 0, simplify: 0.0001 }
"#;
        write_profiles_yaml(tmp.path(), body);
        let sources = r#"version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data/ROUTE.shp"
output:
  directory: "tiles/"
generalize_profiles_path: "generalize-profiles.yaml"
"#;
        std::fs::write(tmp.path().join("sources.yaml"), sources).unwrap();
        let err = format!(
            "{:#}",
            load_config(tmp.path().join("sources.yaml")).unwrap_err()
        );
        assert!(err.contains("missing n:0"), "error: {err}");
    }

    #[test]
    fn test_build_profile_map_without_external_uses_inline_only() {
        let tmp = tempfile::tempdir().unwrap();
        write_sources_yaml(tmp.path(), None);
        let cfg = load_config(tmp.path().join("sources.yaml")).unwrap();
        assert_eq!(cfg.resolved_profile_map.len(), 1);
        let p = &cfg.resolved_profile_map["ZONE_D_HABITATION"];
        assert_eq!(p.levels.len(), 1);
        assert_eq!(p.levels[0].n, 0);
    }
}
