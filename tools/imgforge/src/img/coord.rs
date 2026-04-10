use std::f64::consts::PI;

/// 24-bit map unit coordinate system, faithful to mkgmap Coord.java
pub const HIGH_PREC_BITS: i32 = 30;
pub const DELTA_SHIFT: i32 = HIGH_PREC_BITS - 24; // = 6
pub const FACTOR_HP: f64 = (1i64 << HIGH_PREC_BITS) as f64;
pub const R_WGS84: f64 = 6378137.0;
pub const U_WGS84: f64 = R_WGS84 * 2.0 * PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Coord {
    latitude: i32,
    longitude: i32,
    lat_delta: i8,
    lon_delta: i8,
}

impl Coord {
    /// From 24-bit map units (no high-prec delta)
    pub fn new(latitude: i32, longitude: i32) -> Self {
        Self {
            latitude,
            longitude,
            lat_delta: 0,
            lon_delta: 0,
        }
    }

    /// From WGS84 degrees — mkgmap Coord(double, double)
    pub fn from_degrees(lat: f64, lon: f64) -> Self {
        let latitude = to_map_unit(lat);
        let longitude = to_map_unit(lon);
        let lat_hp = to_high_prec(lat);
        let lon_hp = to_high_prec(lon);
        let lat_delta = ((latitude << DELTA_SHIFT) - lat_hp) as i8;
        let lon_delta = ((longitude << DELTA_SHIFT) - lon_hp) as i8;
        Self {
            latitude,
            longitude,
            lat_delta,
            lon_delta,
        }
    }

    /// From 30-bit high precision values — mkgmap makeHighPrecCoord
    #[cfg(test)]
    fn from_high_prec(lat_hp: i32, lon_hp: i32) -> Self {
        let rounding = 1 << (DELTA_SHIFT - 1);
        let lat24 = (lat_hp + rounding) >> DELTA_SHIFT;
        let lon24 = (lon_hp + rounding) >> DELTA_SHIFT;
        let d_lat = ((lat24 << DELTA_SHIFT) - lat_hp) as i8;
        let d_lon = ((lon24 << DELTA_SHIFT) - lon_hp) as i8;
        Self {
            latitude: lat24,
            longitude: lon24,
            lat_delta: d_lat,
            lon_delta: d_lon,
        }
    }

    pub fn latitude(&self) -> i32 {
        self.latitude
    }

    pub fn longitude(&self) -> i32 {
        self.longitude
    }

    pub fn high_prec_lat(&self) -> i32 {
        (self.latitude << DELTA_SHIFT) - self.lat_delta as i32
    }

    pub fn high_prec_lon(&self) -> i32 {
        (self.longitude << DELTA_SHIFT) - self.lon_delta as i32
    }

    pub fn lat_degrees(&self) -> f64 {
        to_degrees(self.latitude)
    }

    pub fn lon_degrees(&self) -> f64 {
        to_degrees(self.longitude)
    }

    /// Flat earth distance in meters, with haversine fallback for large distances
    pub fn distance(&self, other: &Coord) -> f64 {
        let d1 = U_WGS84 / 360.0 * self.distance_in_degrees(other).sqrt();
        if d1 < 10000.0 {
            return d1;
        }
        self.distance_haversine(other)
    }

    fn distance_in_degrees(&self, other: &Coord) -> f64 {
        let lat1 = self.lat_degrees();
        let lat2 = other.lat_degrees();
        let lon1 = self.lon_degrees();
        let lon2 = other.lon_degrees();

        let mut lat_diff = (lat1 - lat2).abs();
        if lat_diff > 90.0 {
            lat_diff -= 180.0;
        }

        let mut lon_diff = (lon1 - lon2).abs();
        if lon_diff > 180.0 {
            lon_diff -= 360.0;
        }

        lon_diff *= ((lat1 + lat2) / 2.0).to_radians().cos();
        lat_diff * lat_diff + lon_diff * lon_diff
    }

    fn distance_haversine(&self, other: &Coord) -> f64 {
        let lat1 = hp_to_radians(self.high_prec_lat());
        let lat2 = hp_to_radians(other.high_prec_lat());
        let lon1 = hp_to_radians(self.high_prec_lon());
        let lon2 = hp_to_radians(other.high_prec_lon());
        let sin_mid_lat = ((lat1 - lat2) / 2.0).sin();
        let sin_mid_lon = ((lon1 - lon2) / 2.0).sin();
        let d_rad = 2.0
            * (sin_mid_lat * sin_mid_lat + lat1.cos() * lat2.cos() * sin_mid_lon * sin_mid_lon)
                .sqrt()
                .asin();
        d_rad * R_WGS84
    }

    /// Bearing in degrees from this point to other (rhumb line)
    pub fn bearing_to(&self, other: &Coord) -> f64 {
        let lat1 = hp_to_radians(self.high_prec_lat());
        let lat2 = hp_to_radians(other.high_prec_lat());
        let lon1 = hp_to_radians(self.high_prec_lon());
        let lon2 = hp_to_radians(other.high_prec_lon());

        let mut d_lon = lon2 - lon1;
        let d_phi = ((lat2 / 2.0 + PI / 4.0).tan() / (lat1 / 2.0 + PI / 4.0).tan()).ln();

        if d_lon.abs() > PI {
            d_lon = if d_lon > 0.0 {
                -(2.0 * PI - d_lon)
            } else {
                2.0 * PI + d_lon
            };
        }

        d_lon.atan2(d_phi).to_degrees().rem_euclid(360.0)
    }
}

/// Convert degrees to 24-bit map units — mkgmap Utils.toMapUnit
pub fn to_map_unit(degrees: f64) -> i32 {
    let delta = 360.0 / (1 << 24) as f64 / 2.0;
    if degrees > 0.0 {
        ((degrees + delta) * (1 << 24) as f64 / 360.0) as i32
    } else {
        ((degrees - delta) * (1 << 24) as f64 / 360.0) as i32
    }
}

/// Convert 24-bit map units to degrees — mkgmap Utils.toDegrees
pub fn to_degrees(map_unit: i32) -> f64 {
    map_unit as f64 * (360.0 / (1 << 24) as f64)
}

/// Convert degrees to 30-bit high precision
pub fn to_high_prec(degrees: f64) -> i32 {
    (degrees * FACTOR_HP / 360.0).round() as i32
}

/// Convert high-precision units to radians
fn hp_to_radians(hp: i32) -> f64 {
    hp as f64 * (2.0 * PI / FACTOR_HP)
}

/// Convert 24-bit map units to 32-bit semicircles (for NOD)
#[cfg(test)]
fn to_semicircles(map_unit: i32) -> i32 {
    ((map_unit as i64) << 8) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_map_unit_positive() {
        assert_eq!(to_map_unit(45.0), 2_097_152);
    }

    #[test]
    fn test_to_map_unit_negative() {
        assert_eq!(to_map_unit(-180.0), -8_388_608);
    }

    #[test]
    fn test_to_degrees_roundtrip() {
        let deg = 48.5734;
        let mu = to_map_unit(deg);
        let back = to_degrees(mu);
        assert!((deg - back).abs() < 0.0001, "got {back}");
    }

    #[test]
    fn test_from_degrees() {
        let c = Coord::from_degrees(48.5734, 7.7521);
        assert!((c.lat_degrees() - 48.5734).abs() < 0.001);
        assert!((c.lon_degrees() - 7.7521).abs() < 0.001);
    }

    #[test]
    fn test_high_prec_roundtrip() {
        let c = Coord::from_degrees(48.5734, 7.7521);
        let hp_lat = c.high_prec_lat();
        let hp_lon = c.high_prec_lon();
        let c2 = Coord::from_high_prec(hp_lat, hp_lon);
        assert_eq!(c.latitude(), c2.latitude());
        assert_eq!(c.longitude(), c2.longitude());
    }

    #[test]
    fn test_new_map_units() {
        let c = Coord::new(2_097_152, -4_194_304);
        assert_eq!(c.latitude(), 2_097_152);
        assert_eq!(c.longitude(), -4_194_304);
        assert_eq!(c.lat_delta, 0);
    }

    #[test]
    fn test_distance_short() {
        let a = Coord::from_degrees(48.5734, 7.7521);
        let b = Coord::from_degrees(48.5735, 7.7522);
        let d = a.distance(&b);
        assert!(d > 0.0 && d < 50.0, "short distance: {d}");
    }

    #[test]
    fn test_to_semicircles() {
        let mu = to_map_unit(45.0);
        let sc = to_semicircles(mu);
        // 45 degrees = 2^30 semicircles = 1073741824
        // mu=2097152, sc=2097152*256=536870912... that's 2^29
        // Actually semicircles for 45° = 45/360 * 2^32 = 536870912
        assert_eq!(sc, 536870912);
    }

    #[test]
    fn test_zero_coord() {
        let c = Coord::from_degrees(0.0, 0.0);
        assert_eq!(c.latitude(), 0);
        assert_eq!(c.longitude(), 0);
    }
}
