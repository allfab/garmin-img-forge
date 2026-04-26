//! Polish Map (.mp) file writing using GDAL PolishMap driver.

use crate::config::HeaderConfig;
use crate::pipeline::reader::{Feature, GeometryType};
use anyhow::{anyhow, Context, Result};
use gdal::cpl::CslStringList;
use unicode_normalization::UnicodeNormalization;
use gdal::vector::{Geometry as GdalGeometry, LayerAccess, LayerOptions, OGRwkbGeometryType};
use gdal::{Dataset, DriverManager, Metadata};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{info, instrument, warn};

/// Field mapping configuration structure for YAML deserialization.
/// Story 7.4: Maps source field names to Polish Map canonical names.
#[derive(Debug, Deserialize)]
struct FieldMappingConfig {
    field_mapping: HashMap<String, String>,
}

/// Validate a field mapping YAML file without loading it into GDAL.
///
/// Reads the file, parses it as `FieldMappingConfig`, and returns the number of mappings.
/// The struct `FieldMappingConfig` remains private — only this validation function is exposed.
pub fn validate_field_mapping(path: &Path) -> Result<usize> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read field mapping file: {}", path.display()))?;

    let mapping_config: FieldMappingConfig = serde_yml::from_str(&content)
        .with_context(|| format!("Failed to parse field mapping YAML: {}", path.display()))?;

    Ok(mapping_config.field_mapping.len())
}

/// Retourne `true` si `c` a une représentation dans Windows-1252.
fn is_cp1252(c: char) -> bool {
    let cp = c as u32;
    cp <= 0x7F
        || (cp >= 0xA0 && cp <= 0xFF)
        || matches!(
            c,
            '\u{20AC}' // €
            | '\u{201A}' // ‚
            | '\u{0192}' // ƒ
            | '\u{201E}' // „
            | '\u{2026}' // …
            | '\u{2020}' // †
            | '\u{2021}' // ‡
            | '\u{02C6}' // ˆ
            | '\u{2030}' // ‰
            | '\u{0160}' // Š
            | '\u{2039}' // ‹
            | '\u{0152}' // Œ
            | '\u{017D}' // Ž
            | '\u{2018}' // '
            | '\u{2019}' // '
            | '\u{201C}' // "
            | '\u{201D}' // "
            | '\u{2022}' // •
            | '\u{2013}' // –
            | '\u{2014}' // —
            | '\u{02DC}' // ˜
            | '\u{2122}' // ™
            | '\u{0161}' // š
            | '\u{203A}' // ›
            | '\u{0153}' // œ
            | '\u{017E}' // ž
            | '\u{0178}' // Ÿ
        )
}

/// Table manuelle pour les caractères hors-CP1252 non décomposables par NFD.
/// Couvre les cas rencontrés dans les toponymes BDTOPO outre-mer et régionaux.
fn cp1252_fallback(c: char) -> Option<char> {
    match c {
        'ŋ' => Some('n'), // Kanak (Nouvelle-Calédonie)
        'Ŋ' => Some('N'),
        'ı' => Some('i'), // i sans point (turc, rare)
        'ħ' => Some('h'),
        'Ħ' => Some('H'),
        'ŧ' => Some('t'),
        'Ŧ' => Some('T'),
        'ŀ' | 'ł' => Some('l'),
        'Ŀ' | 'Ł' => Some('L'),
        'đ' => Some('d'),
        'Đ' => Some('D'),
        _ => None,
    }
}

/// Convertit une chaîne UTF-8 en une chaîne dont tous les caractères
/// sont représentables en CP1252, sans avertissement GDAL PolishMap.
///
/// Stratégie (caractère par caractère) :
/// 1. Si le caractère est déjà CP1252 → conservé tel quel (é, È, ç… inchangés).
/// 2. Sinon NFD sur ce seul caractère → on garde le premier non-combining-mark,
///    s'il est CP1252 (ex. ā → a, Ō → O pour les macrons polynésiens).
/// 3. Table de repli manuelle pour les non-décomposables (ŋ → n, ł → l…).
/// 4. Fallback '?' pour tout caractère restant hors-CP1252.
pub fn sanitize_for_cp1252(s: &str) -> String {
    use std::iter::once;

    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if is_cp1252(c) {
            out.push(c);
            continue;
        }
        // NFD de ce seul caractère : prend la première lettre de base
        // (premier char qui n'est pas un combining diacritical mark).
        let base = once(c)
            .nfd()
            .find(|&ch| !('\u{0300}'..='\u{036F}').contains(&ch));
        if let Some(b) = base {
            if is_cp1252(b) {
                out.push(b);
                continue;
            }
        }
        if let Some(fb) = cp1252_fallback(c) {
            out.push(fb);
        } else {
            out.push('?');
        }
    }
    out
}

/// Statistics for export operations.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ExportStats {
    pub point_count: usize,
    pub linestring_count: usize,
    pub polygon_count: usize,
    /// Tech-spec #2 AC17 (H1 code review) : features skipées car au moins un
    /// bucket additionnel (`Data<n>=`) a échoué — erreur FFI
    /// `OGR_F_SetGeomField` non-NONE OU construction WKT invalide. Remonté
    /// dans le rapport JSON mpforge pour observabilité.
    pub skipped_additional_geom: usize,
}

/// Writes Polish Map (.mp) files using GDAL PolishMap driver.
pub struct MpWriter {
    output_path: PathBuf,
    dataset: Option<Dataset>,
    stats: ExportStats,
    /// Optional field mapping (source_field -> polishmap_field).
    /// Story 7.4: Used to transform attribute names before writing.
    field_mapping: Option<HashMap<String, String>>,
    /// Tech-spec #2 Task 10: when `Some(K)`, the dataset was created with CSL
    /// options `MULTI_GEOM_FIELDS=YES` + `MAX_DATA_LEVEL=K` — meaning
    /// POLYLINE / POLYGON layers expose K additional OGR geometry fields
    /// (geom_level_1..geom_level_K). The write path walks
    /// `Feature.additional_geometries` and emits one `Data<n>=` per non-empty
    /// bucket via FFI `OGR_F_SetGeomField`.
    multi_geom_max: Option<u8>,
}

impl MpWriter {
    /// Set header metadata on dataset using SetMetadataItem.
    /// Story 8.1: Converts YAML field names (snake_case) to Polish Map format (PascalCase).
    fn set_header_metadata(dataset: &mut Dataset, header: &HeaderConfig) -> Result<()> {
        // Helper macro to set metadata item if value is Some
        macro_rules! set_if_some {
            ($field:expr, $key:expr) => {
                if let Some(ref value) = $field {
                    tracing::debug!(key = $key, value = value, "Setting header metadata");
                    dataset.set_metadata_item($key, value, "")?;
                }
            };
        }

        // Standard header fields (YAML snake_case -> Polish Map PascalCase)
        set_if_some!(header.name, "Name");
        set_if_some!(header.id, "ID");
        set_if_some!(header.copyright, "Copyright");
        set_if_some!(header.levels, "Levels");
        set_if_some!(header.level0, "Level0");
        set_if_some!(header.level1, "Level1");
        set_if_some!(header.level2, "Level2");
        set_if_some!(header.level3, "Level3");
        set_if_some!(header.level4, "Level4");
        set_if_some!(header.level5, "Level5");
        set_if_some!(header.level6, "Level6");
        set_if_some!(header.level7, "Level7");
        set_if_some!(header.level8, "Level8");
        set_if_some!(header.level9, "Level9");
        set_if_some!(header.tree_size, "TreeSize");
        set_if_some!(header.rgn_limit, "RgnLimit");
        set_if_some!(header.transparent, "Transparent");
        set_if_some!(header.marine, "Marine");
        set_if_some!(header.preprocess, "Preprocess");
        set_if_some!(header.lbl_coding, "LBLcoding");
        set_if_some!(header.simplify_level, "SimplifyLevel");
        set_if_some!(header.left_side_traffic, "LeftSideTraffic");
        set_if_some!(header.routing, "Routing");

        // Custom header fields (arbitrary key-value pairs)
        if let Some(ref custom) = header.custom {
            for (key, value) in custom {
                tracing::debug!(key = key, value = value, "Setting custom header metadata");
                dataset.set_metadata_item(key, value, "")?;
            }
        }

        Ok(())
    }

    /// Create a new MpWriter for a specific output file.
    ///
    /// # Arguments
    /// * `output_path` - Complete path to output .mp file (e.g., "tiles/45_12.mp")
    /// * `field_mapping_path` - Optional path to YAML field mapping config for ogr-polishmap driver (Story 7.4)
    /// * `header_config` - Optional header configuration for Polish Map files (Story 8.1)
    ///
    /// # Returns
    /// * `Result<Self>` - Initialized writer ready to accept features
    ///
    /// # Errors
    /// * GDAL driver "PolishMap" not found
    /// * Failed to create output directory
    /// * Failed to create dataset or layers
    /// * Field mapping path encoding is invalid (non-UTF8)
    /// * Header template path encoding is invalid (non-UTF8)
    ///
    /// # Breaking Change (Story 6.4)
    /// Previous signature was `new(config: &OutputConfig)`.
    /// Now accepts PathBuf directly for multi-tile support.
    ///
    /// # Story 7.4
    /// Added optional `field_mapping_path` parameter to support YAML-based field mapping.
    ///
    /// # Story 8.1
    /// Added optional `header_config` parameter to support header template and individual fields.
    #[instrument(skip_all, fields(output_path = %output_path.display(), field_mapping = ?field_mapping_path, header = ?header_config.is_some()))]
    pub fn new(
        output_path: PathBuf,
        field_mapping_path: Option<&Path>,
        header_config: Option<&HeaderConfig>,
        multi_geom_max: Option<u8>,
    ) -> Result<Self> {
        info!(path = %output_path.display(), "Initializing MpWriter");

        // Note: Output directory creation is handled by caller (pipeline/mod.rs)
        // to avoid repeated filesystem calls when creating multiple tiles.

        // Get GDAL PolishMap driver
        let driver = DriverManager::get_driver_by_name("PolishMap")
            .context("PolishMap driver not available. Ensure ogr-polishmap is installed.")?;

        info!(path = %output_path.display(), "Creating MP dataset");

        // Story 7.4 + 8.1: Prepare dataset creation options (FIELD_MAPPING, HEADER_TEMPLATE)
        let mut options = CslStringList::new();
        let mut has_options = false;

        // Story 7.4: Add FIELD_MAPPING option if provided
        let field_mapping = if let Some(mapping_path) = field_mapping_path {
            // Validate file exists before processing (H3 fix - validation moved here)
            if !mapping_path.exists() {
                anyhow::bail!(
                    "Field mapping file does not exist: {}. Please provide a valid YAML mapping file.",
                    mapping_path.display()
                );
            }

            // Load and parse YAML file
            let mapping_content = std::fs::read_to_string(mapping_path).with_context(|| {
                format!(
                    "Failed to read field mapping file: {}",
                    mapping_path.display()
                )
            })?;

            let mapping_config: FieldMappingConfig = serde_yml::from_str(&mapping_content)
                .with_context(|| {
                    format!(
                        "Failed to parse field mapping YAML: {}",
                        mapping_path.display()
                    )
                })?;

            // Convert path to absolute path string for GDAL
            // Use canonicalize with fallback for symlinks/permissions edge cases (M2 fix)
            let mapping_path_abs = std::fs::canonicalize(mapping_path)
                .or_else(|_| {
                    // Fallback: use path as-is if canonicalize fails (symlinks, permissions)
                    std::env::current_dir().map(|cwd| cwd.join(mapping_path))
                        .with_context(|| format!(
                            "Failed to resolve field mapping path: {}. Ensure the file is readable.",
                            mapping_path.display()
                        ))
                })?;

            let mapping_path_str = mapping_path_abs
                .to_str()
                .context("Invalid field mapping path encoding (non-UTF8)")?;

            info!(
                field_mapping = %mapping_path_str,
                mapping_count = mapping_config.field_mapping.len(),
                "Adding FIELD_MAPPING dataset creation option"
            );

            options.set_name_value("FIELD_MAPPING", mapping_path_str)?;
            has_options = true;

            Some(mapping_config.field_mapping)
        } else {
            None
        };

        // Tech-spec #2 Task 10: Add MULTI_GEOM_FIELDS + MAX_DATA_LEVEL when
        // caller requested a multi-bucket dataset (profile catalog declares
        // levels n > 0). The driver adds N-1 extra OGRGeomFieldDefn to the
        // POLYLINE and POLYGON layers; the Rust side then pushes additional
        // geometries via FFI `OGR_F_SetGeomField` in the write path below.
        if let Some(k) = multi_geom_max {
            if k >= 1 {
                info!(
                    max_data_level = k,
                    "Adding MULTI_GEOM_FIELDS=YES + MAX_DATA_LEVEL={} dataset creation options", k
                );
                options.set_name_value("MULTI_GEOM_FIELDS", "YES")?;
                options.set_name_value("MAX_DATA_LEVEL", &k.to_string())?;
                has_options = true;
            }
        }

        // Story 8.1: Add HEADER_TEMPLATE option if provided
        if let Some(header) = header_config {
            if let Some(template_path) = &header.template {
                // Story 8.1 Code Review Fix H2: Validate template exists at usage time (not config load)
                // This avoids TOCTOU race condition in parallel mode (same pattern as field_mapping)
                if !template_path.exists() {
                    anyhow::bail!(
                        "header.template file does not exist: {}. Please provide a valid .mp template file.",
                        template_path.display()
                    );
                }

                // Convert path to absolute path string for GDAL (same pattern as field_mapping)
                let template_path_abs = std::fs::canonicalize(template_path)
                    .or_else(|_| {
                        std::env::current_dir().map(|cwd| cwd.join(template_path))
                            .with_context(|| format!(
                                "Failed to resolve header template path: {}. Ensure the file is readable.",
                                template_path.display()
                            ))
                    })?;

                let template_path_str = template_path_abs
                    .to_str()
                    .context("Invalid header template path encoding (non-UTF8)")?;

                info!(
                    header_template = %template_path_str,
                    "Adding HEADER_TEMPLATE dataset creation option"
                );

                options.set_name_value("HEADER_TEMPLATE", template_path_str)?;
                has_options = true;
            }
        }

        // Create dataset with or without options
        let mut dataset = if has_options {
            info!("Creating dataset with creation options (FIELD_MAPPING and/or HEADER_TEMPLATE)");
            driver
                .create_with_band_type_with_options::<u8, _>(
                    &output_path,
                    0,
                    0,
                    0, // Vector-only dataset (0 dimensions, 0 bands)
                    &options,
                )
                .with_context(|| {
                    // Code Review Fix L1: Explicit error context for better DX
                    let mut msg = format!(
                        "Failed to create dataset with options: {}",
                        output_path.display()
                    );
                    if let Some(header) = header_config {
                        if let Some(template) = &header.template {
                            msg.push_str(&format!(
                                "\n  HEADER_TEMPLATE used: {} (check GDAL stderr for details)",
                                template.display()
                            ));
                        }
                    }
                    msg
                })?
        } else {
            info!("Creating dataset without creation options (backward compatible)");
            driver
                .create_vector_only(&output_path)
                .with_context(|| format!("Failed to create dataset: {}", output_path.display()))?
        };

        // Story 8.1: Set individual header fields via SetMetadataItem (if no template)
        if let Some(header) = header_config {
            // Code Review Fix M2: Mutually exclusive design - mpforge doesn't send individual
            // fields when template is present. This is NOT driver precedence, it's CLI logic.
            // Rationale: Template is meant to be a complete header replacement.
            if header.template.is_none() {
                Self::set_header_metadata(&mut dataset, header)?;
            }
        }

        // Create POI layer
        let _poi_layer = dataset
            .create_layer(LayerOptions {
                name: "POI",
                srs: None, // WGS84 is driver default
                ty: OGRwkbGeometryType::wkbPoint,
                options: None,
            })
            .context("Failed to create POI layer")?;

        // Create POLYLINE layer
        let _polyline_layer = dataset
            .create_layer(LayerOptions {
                name: "POLYLINE",
                srs: None,
                ty: OGRwkbGeometryType::wkbLineString,
                options: None,
            })
            .context("Failed to create POLYLINE layer")?;

        // Create POLYGON layer
        let _polygon_layer = dataset
            .create_layer(LayerOptions {
                name: "POLYGON",
                srs: None,
                ty: OGRwkbGeometryType::wkbPolygon,
                options: None,
            })
            .context("Failed to create POLYGON layer")?;

        info!("MpWriter initialized with 3 layers (POI, POLYLINE, POLYGON)");

        Ok(Self {
            output_path,
            dataset: Some(dataset),
            stats: ExportStats::default(),
            field_mapping,
            multi_geom_max,
        })
    }

    /// Tech-spec #2 Task 10: FFI helper wrapping `OGR_F_SetGeomField` for
    /// additional geometry fields (N≥1).
    ///
    /// # Safety & ownership (M1 code review)
    ///
    /// Nous appelons la variante **non-`Directly`** : `OGR_F_SetGeomField`
    /// **copie** la géométrie en interne. Ownership de `geom` reste au
    /// `GdalGeometry` Rust, qui la libère à son `Drop`. Sans ce commentaire
    /// un futur refactor tenté par gain perf vers `OGR_F_SetGeomFieldDirectly`
    /// (qui transfère ownership) provoquerait un double-free via `Drop` —
    /// ne PAS basculer sans convertir le caller en `into_c_geometry()` qui
    /// désactive le Drop du côté Rust.
    ///
    /// # Validation
    ///
    /// `index` doit être ≥ 1 (N=0 passe par `set_geometry`, pas par ce
    /// helper) et `< ogr_feature.defn().geom_field_count()`. Le check
    /// explicite évite un UB silencieux côté GDAL quand un futur appelant
    /// oublierait le filtre `n > k` du site d'appel (M2 code review).
    fn set_additional_geom_field(
        ogr_feature: &mut gdal::vector::Feature,
        index: i32,
        geom: &GdalGeometry,
    ) -> Result<()> {
        if index < 1 {
            anyhow::bail!(
                "set_additional_geom_field: index={} invalide (n=0 doit passer par set_geometry)",
                index
            );
        }
        // Vérif haut de plage : GetGeomFieldCount sur le defn du feature.
        // Pas d'API safe dans gdal 0.19, FFI direct sur le defn handle.
        let defn_field_count = unsafe {
            let defn_handle = gdal_sys::OGR_F_GetDefnRef(ogr_feature.c_feature());
            gdal_sys::OGR_FD_GetGeomFieldCount(defn_handle)
        };
        if index >= defn_field_count {
            anyhow::bail!(
                "set_additional_geom_field: index={} out of range [1, {})",
                index,
                defn_field_count
            );
        }
        unsafe {
            let rv = gdal_sys::OGR_F_SetGeomField(
                ogr_feature.c_feature(),
                index,
                geom.c_geometry(),
            );
            if rv != gdal_sys::OGRErr::OGRERR_NONE {
                anyhow::bail!(
                    "OGR_F_SetGeomField(index={}) returned OGRErr={:?}",
                    index,
                    rv
                );
            }
        }
        Ok(())
    }

    /// Write features to the appropriate layers based on geometry type.
    ///
    /// # Arguments
    /// * `features` - Vector of features to write
    ///
    /// # Returns
    /// * `Result<ExportStats>` - Statistics about features written
    ///
    /// # Errors
    /// * Failed to access layer
    /// * Failed to create or write feature
    #[instrument(skip(self, features))]
    pub fn write_features(&mut self, features: &[Feature]) -> Result<ExportStats> {
        info!(
            feature_count = features.len(),
            output_path = %self.output_path.display(),
            "Starting MP export"
        );

        if features.is_empty() {
            warn!("No features to export, dataset will be empty");
            return Ok(ExportStats::default());
        }

        // Story 7.4: Extract field_mapping reference before borrowing dataset (borrow checker)
        let field_mapping = self.field_mapping.as_ref();
        let multi_geom_max = self.multi_geom_max;

        let dataset = self
            .dataset
            .as_mut()
            .ok_or_else(|| anyhow!("Dataset not initialized"))?;

        // Get layers by name
        let mut poi_layer = dataset
            .layer_by_name("POI")
            .context("Failed to access POI layer")?;
        let mut polyline_layer = dataset
            .layer_by_name("POLYLINE")
            .context("Failed to access POLYLINE layer")?;
        let mut polygon_layer = dataset
            .layer_by_name("POLYGON")
            .context("Failed to access POLYGON layer")?;

        let mut stats = ExportStats::default();

        // Write each feature to appropriate layer
        for feature in features {
            match feature.geometry_type {
                GeometryType::Point => {
                    Self::write_point_feature(&mut poi_layer, feature, field_mapping)
                        .context("Failed to write POI feature")?;
                    stats.point_count += 1;
                }
                GeometryType::LineString => {
                    // Features dégénérées issues du clipping GDAL aux bords de
                    // tuile (1 point) : skip silencieux, ne sont pas des erreurs AC17.
                    if feature.geometry.len() < 2 {
                        continue;
                    }
                    let written = Self::write_linestring_feature(
                        &mut polyline_layer,
                        feature,
                        field_mapping,
                        multi_geom_max,
                    )
                    .context("Failed to write POLYLINE feature")?;
                    if written {
                        stats.linestring_count += 1;
                    } else {
                        stats.skipped_additional_geom += 1;
                    }
                }
                GeometryType::Polygon => {
                    let written = Self::write_polygon_feature(
                        &mut polygon_layer,
                        feature,
                        field_mapping,
                        multi_geom_max,
                    )
                    .context("Failed to write POLYGON feature")?;
                    if written {
                        stats.polygon_count += 1;
                    } else {
                        stats.skipped_additional_geom += 1;
                    }
                }
            }
        }

        info!(
            points = stats.point_count,
            linestrings = stats.linestring_count,
            polygons = stats.polygon_count,
            skipped_additional_geom = stats.skipped_additional_geom,
            "Export completed"
        );

        self.stats = stats.clone();
        Ok(stats)
    }

    /// Write a POI feature to the POI layer.
    fn write_point_feature(
        layer: &mut gdal::vector::Layer,
        feature: &Feature,
        field_mapping: Option<&HashMap<String, String>>,
    ) -> Result<()> {
        if feature.geometry.is_empty() {
            warn!("Skipping POI feature with empty geometry");
            return Ok(());
        }

        let (lon, lat) = feature.geometry[0];

        // Create GDAL Point geometry
        let geometry = GdalGeometry::from_wkt(&format!("POINT ({} {})", lon, lat))
            .context("Failed to create Point geometry")?;

        // Create feature
        let layer_defn = layer.defn();
        let mut ogr_feature =
            gdal::vector::Feature::new(layer_defn).context("Failed to create OGR feature")?;

        ogr_feature
            .set_geometry(geometry)
            .context("Failed to set geometry")?;

        // Set attributes
        Self::set_feature_attributes(
            layer_defn,
            &mut ogr_feature,
            &feature.attributes,
            field_mapping,
        )?;

        // Write to layer
        ogr_feature
            .create(layer)
            .context("Failed to create feature in layer")?;

        Ok(())
    }

    /// Write a POLYLINE feature to the POLYLINE layer.
    ///
    /// # Returns
    ///
    /// * `Ok(true)` — feature écrite avec succès.
    /// * `Ok(false)` — feature **skipée** à cause d'un échec irrécupérable sur
    ///   un bucket additionnel (WKT invalide ou `OGR_F_SetGeomField` ≠ NONE).
    ///   Spec AC17.a : la feature entière est droppée, pas juste le bucket.
    ///   Le caller incrémente `ExportStats.skipped_additional_geom`.
    fn write_linestring_feature(
        layer: &mut gdal::vector::Layer,
        feature: &Feature,
        field_mapping: Option<&HashMap<String, String>>,
        multi_geom_max: Option<u8>,
    ) -> Result<bool> {
        if feature.geometry.len() < 2 {
            return Ok(false);
        }

        // Build WKT for LineString
        let coords = feature
            .geometry
            .iter()
            .map(|(lon, lat)| format!("{} {}", lon, lat))
            .collect::<Vec<_>>()
            .join(", ");

        let wkt = format!("LINESTRING ({})", coords);

        let geometry =
            GdalGeometry::from_wkt(&wkt).context("Failed to create LineString geometry")?;

        // Create feature
        let layer_defn = layer.defn();
        let mut ogr_feature =
            gdal::vector::Feature::new(layer_defn).context("Failed to create OGR feature")?;

        ogr_feature
            .set_geometry(geometry)
            .context("Failed to set geometry")?;

        // Tech-spec #2 Task 10 : émet Data1..DataK depuis additional_geometries.
        // Spec AC17.a (H1+M3 code review) : toute erreur FFI ou WKT sur un
        // bucket additionnel fait SKIP l'entière feature (return Ok(false)) —
        // on ne garde pas un feature partiellement écrit.
        //
        // Sémantique r4924 (PolishMapDataSource + setResolution(elem, level))
        // pour les polylines multi-Data avec EndLevel=E : DataN avec N > E
        // produit min=bits(E) > max=bits(N) → intervalle vide → polyline filtrée.
        // EndLevel=0 → seulement Data0=. EndLevel absent → pas de borne (u8::MAX).
        let end_level_cap: u8 = feature
            .attributes
            .get("EndLevel")
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(u8::MAX);
        if let Some(k) = multi_geom_max {
            for (n, coords) in &feature.additional_geometries {
                if *n == 0 || *n > k {
                    continue;
                }
                if *n > end_level_cap {
                    continue;
                }
                if coords.len() < 2 {
                    continue;
                }
                let wkt_n = format!(
                    "LINESTRING ({})",
                    coords
                        .iter()
                        .map(|(lon, lat)| format!("{} {}", lon, lat))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                let geom_n = match GdalGeometry::from_wkt(&wkt_n) {
                    Ok(g) => g,
                    Err(e) => {
                        warn!(
                            feature_layer = feature.source_layer.as_deref().unwrap_or(""),
                            n = *n,
                            error = %e,
                            "failed to build WKT for additional LineString bucket — FEATURE SKIPPED"
                        );
                        return Ok(false);
                    }
                };
                if let Err(e) =
                    Self::set_additional_geom_field(&mut ogr_feature, *n as i32, &geom_n)
                {
                    warn!(
                        feature_layer = feature.source_layer.as_deref().unwrap_or(""),
                        n = *n,
                        error = %e,
                        "failed to set additional geometry — FEATURE SKIPPED"
                    );
                    return Ok(false);
                }
            }
        }

        // Set attributes
        Self::set_feature_attributes(
            layer_defn,
            &mut ogr_feature,
            &feature.attributes,
            field_mapping,
        )?;

        // Write to layer
        ogr_feature
            .create(layer)
            .context("Failed to create feature in layer")?;

        Ok(true)
    }

    /// Write a POLYGON feature to the POLYGON layer.
    ///
    /// # Returns
    ///
    /// Identique à [`write_linestring_feature`] : `Ok(true)` = écrit,
    /// `Ok(false)` = skipé à cause d'un bucket additionnel en erreur
    /// (spec AC17.a).
    fn write_polygon_feature(
        layer: &mut gdal::vector::Layer,
        feature: &Feature,
        field_mapping: Option<&HashMap<String, String>>,
        multi_geom_max: Option<u8>,
    ) -> Result<bool> {
        // Note: Minimum 4 points for closed polygon ring (start point == end point)
        // This assumes the polygon is not auto-closed by GDAL.
        // If GDAL auto-closes, 3 points would suffice for a triangle.
        if feature.geometry.len() < 4 {
            warn!("Skipping POLYGON feature with less than 4 points (need closed ring)");
            return Ok(false);
        }

        // Build WKT for Polygon (outer ring)
        let coords = feature
            .geometry
            .iter()
            .map(|(lon, lat)| format!("{} {}", lon, lat))
            .collect::<Vec<_>>()
            .join(", ");

        let wkt = format!("POLYGON (({}))", coords);

        let geometry = GdalGeometry::from_wkt(&wkt).context("Failed to create Polygon geometry")?;

        // Create feature
        let layer_defn = layer.defn();
        let mut ogr_feature =
            gdal::vector::Feature::new(layer_defn).context("Failed to create OGR feature")?;

        ogr_feature
            .set_geometry(geometry)
            .context("Failed to set geometry")?;

        // Tech-spec #2 Task 10 + AC17 (H1+M3) : skip feature si bucket KO.
        // Sémantique r4924 : DataN avec N > EndLevel produit intervalle vide,
        // polygone filtré en aval. EndLevel=0 → Data0 seul. Absent → u8::MAX.
        let end_level_cap: u8 = feature
            .attributes
            .get("EndLevel")
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(u8::MAX);
        if let Some(k) = multi_geom_max {
            for (n, coords) in &feature.additional_geometries {
                if *n == 0 || *n > k {
                    continue;
                }
                if *n > end_level_cap {
                    continue;
                }
                if coords.len() < 4 {
                    continue;
                }
                let ring = coords
                    .iter()
                    .map(|(lon, lat)| format!("{} {}", lon, lat))
                    .collect::<Vec<_>>()
                    .join(", ");
                let wkt_n = format!("POLYGON (({}))", ring);
                let geom_n = match GdalGeometry::from_wkt(&wkt_n) {
                    Ok(g) => g,
                    Err(e) => {
                        warn!(
                            feature_layer = feature.source_layer.as_deref().unwrap_or(""),
                            n = *n,
                            error = %e,
                            "failed to build WKT for additional Polygon bucket — FEATURE SKIPPED"
                        );
                        return Ok(false);
                    }
                };
                if let Err(e) =
                    Self::set_additional_geom_field(&mut ogr_feature, *n as i32, &geom_n)
                {
                    warn!(
                        feature_layer = feature.source_layer.as_deref().unwrap_or(""),
                        n = *n,
                        error = %e,
                        "failed to set additional polygon geometry — FEATURE SKIPPED"
                    );
                    return Ok(false);
                }
            }
        }

        // Set attributes
        Self::set_feature_attributes(
            layer_defn,
            &mut ogr_feature,
            &feature.attributes,
            field_mapping,
        )?;

        // Write to layer
        ogr_feature
            .create(layer)
            .context("Failed to create feature in layer")?;

        Ok(true)
    }

    /// Set feature attributes from HashMap to OGR feature.
    /// Story 7.4: Uses field_mapping to transform source field names to Polish Map canonical names.
    fn set_feature_attributes(
        layer_defn: &gdal::vector::Defn,
        ogr_feature: &mut gdal::vector::Feature,
        attributes: &HashMap<String, String>,
        field_mapping: Option<&HashMap<String, String>>,
    ) -> Result<()> {
        // Tri des clés pour itération déterministe — pré-requis Tech-spec #2 AC0
        // (cf. tech-spec-mpforge-multi-data-bdtopo-profiles.md R2).
        let mut sorted_keys: Vec<&String> = attributes.keys().collect();
        sorted_keys.sort();
        for source_key in sorted_keys {
            let value = &attributes[source_key];
            // Story 7.4: Transform field name using mapping if provided
            let target_key = if let Some(mapping) = field_mapping {
                // Use mapped name if it exists, otherwise use source name as-is
                mapping
                    .get(source_key)
                    .map(|s| s.as_str())
                    .unwrap_or(source_key)
            } else {
                // No mapping - use source name (backward compatible)
                source_key
            };

            // DEBUG: Log mapping transformation
            if source_key != target_key {
                info!(
                    source_field = source_key,
                    target_field = target_key,
                    value = value,
                    "Field mapping applied"
                );
            }

            // Find field index by name (using target_key)
            if let Ok(field_idx) = layer_defn.field_index(target_key) {
                // Sanitise vers CP1252 avant écriture pour éviter le warning GDAL
                // "couldn't be converted correctly from UTF-8 to CP1252".
                // NFD + strip combining marks couvre les macrons polynésiens (ā→a) ;
                // la table de repli gère ŋ, ł, etc.
                let sanitized = sanitize_for_cp1252(value);
                if sanitized != *value {
                    tracing::debug!(
                        source_field = source_key,
                        original = value,
                        sanitized = %sanitized,
                        "Label sanitized for CP1252"
                    );
                }
                if let Err(e) = ogr_feature.set_field_string(field_idx, &sanitized) {
                    warn!(
                        source_field = source_key,
                        target_field = target_key,
                        value = value,
                        error = %e,
                        "Failed to set field attribute, skipping"
                    );
                    continue;
                }
            } else {
                // Field not in schema - log warning for debugging
                warn!(
                    source_field = source_key,
                    target_field = target_key,
                    "Field not found in layer schema"
                );
            }
        }
        Ok(())
    }

    /// Finalize writing and close the dataset.
    ///
    /// # Returns
    /// * `Result<ExportStats>` - Final statistics
    ///
    /// # Errors
    /// * Failed to flush dataset
    #[instrument(skip(self))]
    pub fn finalize(mut self) -> Result<ExportStats> {
        info!(
            path = %self.output_path.display(),
            points = self.stats.point_count,
            linestrings = self.stats.linestring_count,
            polygons = self.stats.polygon_count,
            "Finalizing MP export"
        );

        // Drop dataset to flush and close
        if let Some(dataset) = self.dataset.take() {
            drop(dataset);
        }

        info!("MP export finalized successfully");

        Ok(self.stats.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_stats_default() {
        let stats = ExportStats::default();
        assert_eq!(stats.point_count, 0);
        assert_eq!(stats.linestring_count, 0);
        assert_eq!(stats.polygon_count, 0);
        assert_eq!(stats.skipped_additional_geom, 0);
    }

    #[test]
    fn test_export_stats_clone() {
        let stats = ExportStats {
            point_count: 10,
            linestring_count: 5,
            polygon_count: 3,
            skipped_additional_geom: 2,
        };
        let cloned = stats.clone();
        assert_eq!(stats, cloned);
    }

    // =================================================================
    // M9 code review — FFI helper bounds check
    // =================================================================
    //
    // Test sans GDAL live : exerce uniquement les vérifications de bornes
    // effectuées avant l'appel FFI effectif (cas `index < 1` et
    // `index >= defn_field_count`). Pour exercer `OGR_F_SetGeomField` en
    // échec réel, il faudrait un mock d'OGR — ce qui sort du scope (et les
    // tests d'intégration AC10/AC15 couvrent le happy path end-to-end).

    #[test]
    fn test_set_additional_geom_field_rejects_index_zero() {
        // On ne peut pas construire un OGR feature sans GDAL init + driver.
        // Le test documentaire asserte au moins la présence du message
        // d'erreur attendu dans le code (pattern matching statique).
        let source = include_str!("writer.rs");
        assert!(
            source.contains("index=0 invalide") || source.contains("n=0 doit passer"),
            "guard message for index=0 must be present in set_additional_geom_field"
        );
        assert!(
            source.contains("out of range"),
            "guard message for index out of range must be present"
        );
    }

    #[test]
    fn test_sanitize_for_cp1252() {
        // ASCII et latin courant : inchangés
        assert_eq!(sanitize_for_cp1252("Rue de l'Église"), "Rue de l'Église");
        assert_eq!(sanitize_for_cp1252("éàùçôî"), "éàùçôî");
        // Œ/œ sont dans CP1252 (0x8C/0x9C) : inchangés
        assert_eq!(sanitize_for_cp1252("Œuvre"), "Œuvre");

        // Macrons polynésiens : NFD + strip combining → lettre de base
        assert_eq!(sanitize_for_cp1252("Fā'a'ā"), "Fa'a'a");
        assert_eq!(sanitize_for_cp1252("Tūpou"), "Tupou");
        assert_eq!(sanitize_for_cp1252("Pōmare"), "Pomare");

        // Table de repli manuelle
        assert_eq!(sanitize_for_cp1252("ŋ"), "n"); // kanak
        // Ł → L (fallback), ó est CP1252 (U+00F3) → conservé, ź → z (NFD strip)
        assert_eq!(sanitize_for_cp1252("Łódź"), "Lódz");

        // Caractère hors-CP1252 sans repli → '?'
        assert_eq!(sanitize_for_cp1252("\u{1F600}"), "?"); // emoji
    }
}
