// HGT reader — SRTM elevation data (binary big-endian i16)
//
// File structure: (res+1)×(res+1) big-endian i16 values, row-major north→south, west→east.
// Resolution detected from file size: 1201×1201 (3 arc-sec) or 3601×3601 (1 arc-sec).
// Filename encodes SW corner: N45E005.hgt → south=45, west=5.

use std::path::Path;
use anyhow::{Result, bail, Context};
use byteorder::{BigEndian, ByteOrder};

use super::{ElevationGrid, GeoBounds};

/// UNDEF value for HGT void cells
pub const UNDEF: i16 = -32768;

pub struct HgtReader {
    /// Raw data buffer
    data: Vec<u8>,
    /// Resolution: 1200 (3 arc-sec) or 3600 (1 arc-sec)
    resolution: u32,
    /// Southwest corner latitude
    south: f64,
    /// Southwest corner longitude
    west: f64,
}

impl HgtReader {
    /// Open and read an HGT file
    pub fn open(path: &Path) -> Result<Self> {
        let data = std::fs::read(path)
            .with_context(|| format!("Cannot read HGT file: {}", path.display()))?;

        let resolution = detect_resolution(data.len())?;
        let (south, west) = parse_filename(path)?;

        Ok(Self {
            data,
            resolution,
            south,
            west,
        })
    }

    /// Resolution in arc-seconds intervals (1200 or 3600)
    pub fn resolution(&self) -> u32 {
        self.resolution
    }

    /// Grid side length (res + 1)
    pub fn side(&self) -> u32 {
        self.resolution + 1
    }

    /// Read elevation at (row, col). Row 0 = northernmost row.
    /// Returns None if the value is UNDEF (-32768).
    pub fn elevation(&self, row: u32, col: u32) -> Option<i16> {
        let side = self.side();
        if row >= side || col >= side {
            return None;
        }
        let offset = ((row * side + col) * 2) as usize;
        if offset + 2 > self.data.len() {
            return None;
        }
        let val = BigEndian::read_i16(&self.data[offset..offset + 2]);
        if val == UNDEF {
            None
        } else {
            Some(val)
        }
    }

    /// Convert to a WGS84 ElevationGrid
    pub fn to_grid(&self) -> ElevationGrid {
        let side = self.side();
        let mut grid_data = Vec::with_capacity((side * side) as usize);

        for row in 0..side {
            for col in 0..side {
                let val = match self.elevation(row, col) {
                    Some(v) => v as f64,
                    None => UNDEF as f64,
                };
                grid_data.push(val);
            }
        }

        let cellsize = 1.0 / self.resolution as f64;

        ElevationGrid {
            width: side,
            height: side,
            data: grid_data,
            nodata: UNDEF as f64,
            bounds: GeoBounds {
                south: self.south,
                west: self.west,
                north: self.south + 1.0,
                east: self.west + 1.0,
            },
            cellsize_lat: cellsize,
            cellsize_lon: cellsize,
        }
    }
}

/// Detect resolution from file size: side = sqrt(filesize / 2)
fn detect_resolution(file_size: usize) -> Result<u32> {
    if file_size == 0 || file_size % 2 != 0 {
        bail!("Invalid HGT file size: {} (must be even)", file_size);
    }
    let num_values = file_size / 2;
    let side = (num_values as f64).sqrt() as u32;
    if (side * side) as usize != num_values {
        bail!("HGT file size {} does not form a square grid", file_size);
    }
    let resolution = side - 1;
    if resolution != 1200 && resolution != 3600 {
        // Accept non-standard resolutions but warn
        tracing::warn!("Non-standard HGT resolution: {} (expected 1200 or 3600)", resolution);
    }
    Ok(resolution)
}

/// Parse latitude/longitude from HGT filename (e.g. N45E005.hgt)
fn parse_filename(path: &Path) -> Result<(f64, f64)> {
    let stem = path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    // Handle .hgt.zip by stripping inner extension too
    let name = if stem.ends_with(".hgt") {
        &stem[..stem.len() - 4]
    } else {
        stem
    };

    if name.len() < 7 {
        bail!("Cannot parse HGT filename: '{}' (expected format: N45E005)", name);
    }

    let name_upper = name.to_uppercase();
    let ns = &name_upper[0..1];
    let lat: f64 = name_upper[1..3].parse()
        .with_context(|| format!("Cannot parse latitude from HGT filename: '{}'", name))?;
    let ew = &name_upper[3..4];
    let lon: f64 = name_upper[4..7].parse()
        .with_context(|| format!("Cannot parse longitude from HGT filename: '{}'", name))?;

    let lat = if ns == "S" { -lat } else { lat };
    let lon = if ew == "W" { -lon } else { lon };

    Ok((lat, lon))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Create a minimal HGT file in memory (3×3 grid, resolution=2)
    fn make_mini_hgt(values: &[i16]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(values.len() * 2);
        for &v in values {
            let mut b = [0u8; 2];
            BigEndian::write_i16(&mut b, v);
            buf.extend_from_slice(&b);
        }
        buf
    }

    #[test]
    fn test_resolution_detection_3arcsec() {
        // 1201 × 1201 × 2 bytes
        let size = 1201 * 1201 * 2;
        assert_eq!(detect_resolution(size).unwrap(), 1200);
    }

    #[test]
    fn test_resolution_detection_1arcsec() {
        let size = 3601 * 3601 * 2;
        assert_eq!(detect_resolution(size).unwrap(), 3600);
    }

    #[test]
    fn test_resolution_detection_invalid() {
        assert!(detect_resolution(0).is_err());
        assert!(detect_resolution(7).is_err());
    }

    #[test]
    fn test_filename_parsing() {
        let path = Path::new("/data/N45E005.hgt");
        let (lat, lon) = parse_filename(path).unwrap();
        assert_eq!(lat, 45.0);
        assert_eq!(lon, 5.0);
    }

    #[test]
    fn test_filename_parsing_south_west() {
        let path = Path::new("S12W045.hgt");
        let (lat, lon) = parse_filename(path).unwrap();
        assert_eq!(lat, -12.0);
        assert_eq!(lon, -45.0);
    }

    #[test]
    fn test_elevation_read() {
        // 3×3 grid (resolution=2): values row-major north→south
        let values: Vec<i16> = vec![
            100, 200, 300,
            150, 250, 350,
            200, 300, 400,
        ];
        let data = make_mini_hgt(&values);

        let tmpdir = tempfile::tempdir().unwrap();
        let path = tmpdir.path().join("N45E005.hgt");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(&data).unwrap();

        let reader = HgtReader::open(&path).unwrap();
        assert_eq!(reader.resolution(), 2);
        assert_eq!(reader.side(), 3);
        assert_eq!(reader.elevation(0, 0), Some(100));
        assert_eq!(reader.elevation(0, 2), Some(300));
        assert_eq!(reader.elevation(1, 1), Some(250));
        assert_eq!(reader.elevation(2, 2), Some(400));
    }

    #[test]
    fn test_undef_handling() {
        let values: Vec<i16> = vec![
            100, UNDEF, 300,
            150, 250, 350,
            200, 300, 400,
        ];
        let data = make_mini_hgt(&values);

        let tmpdir = tempfile::tempdir().unwrap();
        let path = tmpdir.path().join("N45E005.hgt");
        std::fs::write(&path, &data).unwrap();

        let reader = HgtReader::open(&path).unwrap();
        assert_eq!(reader.elevation(0, 1), None);
        assert_eq!(reader.elevation(0, 0), Some(100));
    }

    #[test]
    fn test_to_grid() {
        let values: Vec<i16> = vec![
            100, 200, 300,
            150, 250, 350,
            200, 300, 400,
        ];
        let data = make_mini_hgt(&values);

        let tmpdir = tempfile::tempdir().unwrap();
        let path = tmpdir.path().join("N45E005.hgt");
        std::fs::write(&path, &data).unwrap();

        let reader = HgtReader::open(&path).unwrap();
        let grid = reader.to_grid();

        assert_eq!(grid.width, 3);
        assert_eq!(grid.height, 3);
        assert_eq!(grid.bounds.south, 45.0);
        assert_eq!(grid.bounds.west, 5.0);
        assert_eq!(grid.bounds.north, 46.0);
        assert_eq!(grid.bounds.east, 6.0);
        assert_eq!(grid.cellsize_lat, 0.5);
        assert_eq!(grid.get(0, 0), Some(100.0));
        assert_eq!(grid.get(2, 2), Some(400.0));
    }
}
