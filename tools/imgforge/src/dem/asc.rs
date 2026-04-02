// ASC reader — ESRI ASCII Grid (.asc) elevation data
//
// Header: ncols, nrows, xllcorner, yllcorner, cellsize, NODATA_value
// Data: space-separated floats, row-major north→south, west→east.
// Supports reprojection to WGS84 via proj4rs for Lambert 93 (EPSG:2154) etc.

use std::path::Path;
use anyhow::{Result, bail, Context};

use super::{ElevationGrid, GeoBounds};

/// Lambert 93 proj4 definition
const LAMBERT93_PROJ4: &str = "+proj=lcc +lat_1=49 +lat_2=44 +lat_0=46.5 +lon_0=3 +x_0=700000 +y_0=6600000 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs";
const WGS84_PROJ4: &str = "+proj=longlat +datum=WGS84 +no_defs";

pub struct AscReader {
    pub ncols: u32,
    pub nrows: u32,
    pub xllcorner: f64,
    pub yllcorner: f64,
    pub cellsize: f64,
    pub nodata: f64,
    /// Row-major north→south data
    pub data: Vec<f64>,
}

impl AscReader {
    /// Open and parse an ASC file
    pub fn open(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Cannot read ASC file: {}", path.display()))?;
        Self::parse(&content)
    }

    /// Parse ASC content from a string
    pub fn parse(content: &str) -> Result<Self> {
        let mut lines = content.lines();

        let ncols = parse_header_value(&mut lines, "ncols")?;
        let nrows = parse_header_value(&mut lines, "nrows")?;
        let xllcorner = parse_header_f64(&mut lines, "xllcorner")?;
        let yllcorner = parse_header_f64(&mut lines, "yllcorner")?;
        let cellsize = parse_header_f64(&mut lines, "cellsize")?;
        let nodata = parse_header_f64(&mut lines, "NODATA_value")?;

        let expected = (ncols * nrows) as usize;
        let mut data = Vec::with_capacity(expected);

        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            for token in trimmed.split_whitespace() {
                let val: f64 = token.parse()
                    .with_context(|| format!("Invalid float in ASC data: '{}'", token))?;
                data.push(val);
            }
        }

        if data.len() != expected {
            bail!(
                "ASC data count mismatch: expected {} ({}×{}), got {}",
                expected, ncols, nrows, data.len()
            );
        }

        Ok(Self {
            ncols,
            nrows,
            xllcorner,
            yllcorner,
            cellsize,
            nodata,
            data,
        })
    }

    /// Convert to an ElevationGrid in source coordinates (no reprojection)
    pub fn to_grid(&self) -> ElevationGrid {
        let width = self.ncols;
        let height = self.nrows;
        let total_width = self.cellsize * (self.ncols - 1) as f64;
        let total_height = self.cellsize * (self.nrows - 1) as f64;

        ElevationGrid {
            width,
            height,
            data: self.data.clone(),
            nodata: self.nodata,
            bounds: GeoBounds {
                south: self.yllcorner,
                west: self.xllcorner,
                north: self.yllcorner + total_height,
                east: self.xllcorner + total_width,
            },
            cellsize_lat: self.cellsize,
            cellsize_lon: self.cellsize,
        }
    }

    /// Convert to a WGS84 ElevationGrid by reprojecting from the given source SRS.
    /// Uses proj4rs for coordinate transformation.
    pub fn to_grid_wgs84(&self, source_srs: &str) -> Result<ElevationGrid> {
        let src_proj4 = srs_to_proj4(source_srs)?;

        let src_proj = proj4rs::proj::Proj::from_user_string(&src_proj4)
            .map_err(|e| anyhow::anyhow!("Failed to parse source projection '{}': {:?}", src_proj4, e))?;
        let dst_proj = proj4rs::proj::Proj::from_user_string(WGS84_PROJ4)
            .map_err(|e| anyhow::anyhow!("Failed to parse WGS84 projection: {:?}", e))?;

        // Transform the 4 corners to WGS84 to determine output bounds
        let corners = [
            (self.xllcorner, self.yllcorner),                                                              // SW
            (self.xllcorner + self.cellsize * (self.ncols - 1) as f64, self.yllcorner),                   // SE
            (self.xllcorner, self.yllcorner + self.cellsize * (self.nrows - 1) as f64),                   // NW
            (self.xllcorner + self.cellsize * (self.ncols - 1) as f64, self.yllcorner + self.cellsize * (self.nrows - 1) as f64), // NE
        ];

        let mut wgs_corners = Vec::with_capacity(4);
        for (x, y) in &corners {
            let mut point = (*x, *y, 0.0);
            proj4rs::transform::transform(&src_proj, &dst_proj, &mut point)
                .map_err(|e| anyhow::anyhow!("Reprojection failed: {:?}", e))?;
            // proj4rs output for longlat is in radians
            wgs_corners.push((point.0.to_degrees(), point.1.to_degrees()));
        }

        let west = wgs_corners.iter().map(|c| c.0).fold(f64::INFINITY, f64::min);
        let east = wgs_corners.iter().map(|c| c.0).fold(f64::NEG_INFINITY, f64::max);
        let south = wgs_corners.iter().map(|c| c.1).fold(f64::INFINITY, f64::min);
        let north = wgs_corners.iter().map(|c| c.1).fold(f64::NEG_INFINITY, f64::max);

        // Compute output cellsize in degrees (approximate from source cellsize in metres)
        // At mid-latitude, 1 degree ≈ 111km, so cellsize_deg ≈ cellsize_m / 111000
        let mid_lat = (north + south) / 2.0;
        let cellsize_lat = self.cellsize / 111_000.0;
        let cellsize_lon = self.cellsize / (111_000.0 * mid_lat.to_radians().cos());

        let out_width = ((east - west) / cellsize_lon).ceil() as u32 + 1;
        let out_height = ((north - south) / cellsize_lat).ceil() as u32 + 1;

        // Inverse projection: for each output WGS84 point, find source coordinates
        let mut out_data = vec![self.nodata; (out_width * out_height) as usize];

        for row in 0..out_height {
            let lat = north - row as f64 * cellsize_lat;
            for col in 0..out_width {
                let lon = west + col as f64 * cellsize_lon;

                // Transform WGS84 → source
                let mut point = (lon.to_radians(), lat.to_radians(), 0.0);
                if proj4rs::transform::transform(&dst_proj, &src_proj, &mut point).is_err() {
                    continue;
                }
                let src_x = point.0;
                let src_y = point.1;

                // Bilinear interpolation in source grid
                let col_f = (src_x - self.xllcorner) / self.cellsize;
                let row_f = (self.yllcorner + self.cellsize * (self.nrows - 1) as f64 - src_y) / self.cellsize;

                if col_f < 0.0 || row_f < 0.0 {
                    continue;
                }

                let c0 = col_f.floor() as u32;
                let r0 = row_f.floor() as u32;
                if c0 + 1 >= self.ncols || r0 + 1 >= self.nrows {
                    continue;
                }

                let qx = col_f - c0 as f64;
                let qy = row_f - r0 as f64;

                let get = |r: u32, c: u32| -> Option<f64> {
                    let idx = (r * self.ncols + c) as usize;
                    let v = self.data[idx];
                    if (v - self.nodata).abs() < 0.5 { None } else { Some(v) }
                };

                let h00 = get(r0, c0);
                let h10 = get(r0, c0 + 1);
                let h01 = get(r0 + 1, c0);
                let h11 = get(r0 + 1, c0 + 1);

                if let (Some(h00), Some(h10), Some(h01), Some(h11)) = (h00, h10, h01, h11) {
                    let val = (1.0 - qy) * ((1.0 - qx) * h00 + qx * h10)
                            + qy * ((1.0 - qx) * h01 + qx * h11);
                    out_data[(row * out_width + col) as usize] = val;
                }
            }
        }

        Ok(ElevationGrid {
            width: out_width,
            height: out_height,
            data: out_data,
            nodata: self.nodata,
            bounds: GeoBounds { north, south, east, west },
            cellsize_lat,
            cellsize_lon,
        })
    }
}

/// Convert SRS identifier to proj4 string
fn srs_to_proj4(srs: &str) -> Result<String> {
    match srs.to_uppercase().as_str() {
        "EPSG:2154" => Ok(LAMBERT93_PROJ4.to_string()),
        "EPSG:4326" => Ok(WGS84_PROJ4.to_string()),
        // Common UTM zones for European elevation data
        "EPSG:32631" => Ok("+proj=utm +zone=31 +datum=WGS84 +units=m +no_defs".to_string()),
        "EPSG:32632" => Ok("+proj=utm +zone=32 +datum=WGS84 +units=m +no_defs".to_string()),
        "EPSG:32633" => Ok("+proj=utm +zone=33 +datum=WGS84 +units=m +no_defs".to_string()),
        "EPSG:25831" => Ok("+proj=utm +zone=31 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs".to_string()),
        "EPSG:25832" => Ok("+proj=utm +zone=32 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs".to_string()),
        "EPSG:25833" => Ok("+proj=utm +zone=33 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs".to_string()),
        "EPSG:3035" => Ok("+proj=laea +lat_0=52 +lon_0=10 +x_0=4321000 +y_0=3210000 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs".to_string()),
        "EPSG:3857" => Ok("+proj=merc +a=6378137 +b=6378137 +lat_ts=0 +lon_0=0 +x_0=0 +y_0=0 +k=1 +units=m +no_defs".to_string()),
        s if s.starts_with('+') => Ok(srs.to_string()), // Already a proj4 string
        _ => bail!("Unsupported SRS: '{}'. Use EPSG:2154, EPSG:4326, EPSG:326xx, EPSG:258xx, EPSG:3035, or a proj4 string.", srs),
    }
}

/// Parse a header line like "ncols        3" → u32
fn parse_header_value<'a>(lines: &mut impl Iterator<Item = &'a str>, expected_key: &str) -> Result<u32> {
    let line = lines.next()
        .ok_or_else(|| anyhow::anyhow!("Missing ASC header line: {}", expected_key))?;
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 {
        bail!("Invalid ASC header line: '{}'", line);
    }
    if !parts[0].eq_ignore_ascii_case(expected_key) {
        bail!("Expected '{}' header, got '{}'", expected_key, parts[0]);
    }
    parts[1].parse::<u32>()
        .with_context(|| format!("Cannot parse {} value: '{}'", expected_key, parts[1]))
}

/// Parse a header line with f64 value
fn parse_header_f64<'a>(lines: &mut impl Iterator<Item = &'a str>, expected_key: &str) -> Result<f64> {
    let line = lines.next()
        .ok_or_else(|| anyhow::anyhow!("Missing ASC header line: {}", expected_key))?;
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 {
        bail!("Invalid ASC header line: '{}'", line);
    }
    if !parts[0].eq_ignore_ascii_case(expected_key) {
        bail!("Expected '{}' header, got '{}'", expected_key, parts[0]);
    }
    parts[1].parse::<f64>()
        .with_context(|| format!("Cannot parse {} value: '{}'", expected_key, parts[1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINI_ASC: &str = "\
ncols        3
nrows        3
xllcorner    5.000000
yllcorner    45.000000
cellsize     0.001000
NODATA_value -99999.00
100.0 200.0 300.0
150.0 250.0 350.0
200.0 300.0 400.0";

    #[test]
    fn test_header_parsing() {
        let reader = AscReader::parse(MINI_ASC).unwrap();
        assert_eq!(reader.ncols, 3);
        assert_eq!(reader.nrows, 3);
        assert_eq!(reader.xllcorner, 5.0);
        assert_eq!(reader.yllcorner, 45.0);
        assert_eq!(reader.cellsize, 0.001);
        assert_eq!(reader.nodata, -99999.0);
    }

    #[test]
    fn test_grid_values() {
        let reader = AscReader::parse(MINI_ASC).unwrap();
        assert_eq!(reader.data.len(), 9);
        assert_eq!(reader.data[0], 100.0);
        assert_eq!(reader.data[4], 250.0);
        assert_eq!(reader.data[8], 400.0);
    }

    #[test]
    fn test_nodata_detection() {
        let asc = "\
ncols        3
nrows        2
xllcorner    5.000000
yllcorner    45.000000
cellsize     0.001000
NODATA_value -99999.00
100.0 -99999.00 300.0
150.0 250.0 350.0";

        let reader = AscReader::parse(asc).unwrap();
        let grid = reader.to_grid();
        assert_eq!(grid.get(0, 0), Some(100.0));
        assert_eq!(grid.get(0, 1), None); // NODATA
        assert_eq!(grid.get(0, 2), Some(300.0));
    }

    #[test]
    fn test_to_grid_bounds() {
        let reader = AscReader::parse(MINI_ASC).unwrap();
        let grid = reader.to_grid();
        assert_eq!(grid.width, 3);
        assert_eq!(grid.height, 3);
        assert_eq!(grid.bounds.south, 45.0);
        assert_eq!(grid.bounds.west, 5.0);
        assert!((grid.bounds.north - 45.002).abs() < 1e-9);
        assert!((grid.bounds.east - 5.002).abs() < 1e-9);
    }

    #[test]
    fn test_srs_to_proj4() {
        assert!(srs_to_proj4("EPSG:2154").is_ok());
        assert!(srs_to_proj4("EPSG:4326").is_ok());
        assert!(srs_to_proj4("+proj=longlat").is_ok());
        assert!(srs_to_proj4("EPSG:9999").is_err());
    }
}
