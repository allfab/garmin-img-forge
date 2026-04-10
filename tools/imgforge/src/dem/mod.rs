// DEM module — elevation data loading, reprojection, and resampling
//
// Reads HGT (SRTM) and ASC (ESRI ASCII Grid) elevation files,
// reprojects via proj4rs, and provides a unified ElevationGrid interface
// for the Garmin DEM encoder (src/img/dem.rs).

pub mod hgt;
pub mod asc;
pub mod converter;

use std::path::{Path, PathBuf};
use anyhow::{Context, Result, bail};

/// Unified elevation grid in WGS84
#[derive(Clone, Debug)]
pub struct ElevationGrid {
    /// Number of columns
    pub width: u32,
    /// Number of rows
    pub height: u32,
    /// Elevation data, row-major (north→south, west→east), in metres
    pub data: Vec<f64>,
    /// NODATA sentinel value
    pub nodata: f64,
    /// Geographic bounds in WGS84 degrees
    pub bounds: GeoBounds,
    /// Cell size in latitude degrees
    pub cellsize_lat: f64,
    /// Cell size in longitude degrees
    pub cellsize_lon: f64,
}

impl ElevationGrid {
    /// Get elevation at grid coordinates, or None if out of bounds or NODATA
    pub fn get(&self, row: u32, col: u32) -> Option<f64> {
        if row >= self.height || col >= self.width {
            return None;
        }
        let val = self.data[(row * self.width + col) as usize];
        if (val - self.nodata).abs() < 0.5 {
            None
        } else {
            Some(val)
        }
    }

    /// Check if a WGS84 point falls within this grid's bounds
    pub fn contains(&self, lat: f64, lon: f64) -> bool {
        lat >= self.bounds.south && lat <= self.bounds.north
            && lon >= self.bounds.west && lon <= self.bounds.east
    }
}

/// Geographic bounding box in WGS84 degrees
#[derive(Clone, Debug, PartialEq)]
pub struct GeoBounds {
    pub north: f64,
    pub south: f64,
    pub east: f64,
    pub west: f64,
}


/// Interpolation method for DEM resampling
#[derive(Clone, Debug, PartialEq)]
pub enum InterpolationMethod {
    Bilinear,
    Bicubic,
    Auto,
}

impl InterpolationMethod {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "bilinear" => Self::Bilinear,
            "bicubic" => Self::Bicubic,
            "auto" => Self::Auto,
            other => {
                tracing::warn!(
                    "Unknown interpolation method '{}', falling back to 'auto'. Valid: auto, bilinear, bicubic",
                    other
                );
                Self::Auto
            }
        }
    }
}

/// CLI configuration for DEM processing
#[derive(Clone, Debug)]
pub struct DemConfig {
    /// Paths to DEM source files or directories
    pub paths: Vec<PathBuf>,
    /// Distances between DEM points per zoom level
    pub dists: Vec<i32>,
    /// Interpolation method
    pub interpolation: InterpolationMethod,
    /// Source SRS for ASC files (e.g. "EPSG:2154")
    pub source_srs: Option<String>,
}

/// Detected DEM file format
#[derive(Debug, PartialEq)]
pub enum DemFormat {
    Hgt,
    Asc,
    Unknown,
}

/// Detect whether a file is HGT or ASC
pub fn detect_format(path: &Path) -> DemFormat {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        match ext.to_lowercase().as_str() {
            "hgt" => return DemFormat::Hgt,
            "asc" => return DemFormat::Asc,
            _ => {}
        }
    }

    // Try size-based detection for HGT (no extension)
    if let Ok(meta) = std::fs::metadata(path) {
        let size = meta.len();
        if size > 0 && size % 2 == 0 {
            let side_sq = size / 2;
            let side = (side_sq as f64).sqrt() as u64;
            if side * side == side_sq && (side == 1201 || side == 3601) {
                return DemFormat::Hgt;
            }
        }
    }

    // Try content-based detection for ASC (read only first 256 bytes — F4 fix)
    if let Ok(file) = std::fs::File::open(path) {
        use std::io::Read;
        let mut buf = [0u8; 256];
        let mut reader = std::io::BufReader::new(file);
        if let Ok(n) = reader.read(&mut buf) {
            let snippet = String::from_utf8_lossy(&buf[..n]);
            if let Some(first_line) = snippet.lines().next() {
                if first_line.to_lowercase().contains("ncols") {
                    return DemFormat::Asc;
                }
            }
        }
    }

    DemFormat::Unknown
}

/// Scan directories and load all elevation sources as WGS84 grids
pub fn load_elevation_sources(
    paths: &[PathBuf],
    source_srs: Option<&str>,
) -> Result<Vec<ElevationGrid>> {
    let mut grids = Vec::new();

    for path in paths {
        if path.is_dir() {
            // Scan directory recursively for .hgt and .asc files
            let entries = scan_dem_files(path)?;
            for entry in entries {
                let grid = load_single_file(&entry, source_srs)?;
                grids.push(grid);
            }
        } else if path.is_file() {
            let grid = load_single_file(path, source_srs)?;
            grids.push(grid);
        } else {
            tracing::warn!("DEM path not found: {}", path.display());
        }
    }

    if grids.is_empty() {
        bail!("No DEM elevation files found in provided paths");
    }

    tracing::info!("Loaded {} DEM elevation grids", grids.len());
    Ok(grids)
}

/// Load a single DEM file and return a WGS84 ElevationGrid
fn load_single_file(path: &Path, source_srs: Option<&str>) -> Result<ElevationGrid> {
    match detect_format(path) {
        DemFormat::Hgt => {
            let reader = hgt::HgtReader::open(path)
                .with_context(|| format!("Failed to read HGT: {}", path.display()))?;
            Ok(reader.to_grid())
        }
        DemFormat::Asc => {
            let reader = asc::AscReader::open(path)
                .with_context(|| format!("Failed to read ASC: {}", path.display()))?;
            if let Some(srs) = source_srs {
                if srs != "EPSG:4326" && !srs.is_empty() {
                    return reader.to_grid_wgs84(srs)
                        .with_context(|| format!("Failed to reproject ASC: {}", path.display()));
                }
            }
            Ok(reader.to_grid())
        }
        DemFormat::Unknown => {
            bail!("Unknown DEM format: {}", path.display());
        }
    }
}

/// Recursively scan a directory for .hgt and .asc files
fn scan_dem_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)
        .with_context(|| format!("Failed to read directory: {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(scan_dem_files(&path)?);
        } else if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                match ext.to_lowercase().as_str() {
                    "hgt" | "asc" => files.push(path),
                    _ => {}
                }
            }
        }
    }
    // Sort for deterministic ordering (like mkgmap)
    files.sort();
    Ok(files)
}

