use super::coord::{Coord, DELTA_SHIFT};

/// Bounding box in 24-bit map units — faithful to mkgmap Area.java
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Area {
    min_lat: i32,
    min_lon: i32,
    max_lat: i32,
    max_lon: i32,
}

impl Area {
    /// Create from map units, ensuring non-zero dimensions (mkgmap Area constructor)
    pub fn new(min_lat: i32, min_lon: i32, max_lat: i32, max_lon: i32) -> Self {
        Self {
            min_lat,
            min_lon,
            max_lat: if max_lat == min_lat { min_lat + 1 } else { max_lat },
            max_lon: if max_lon == min_lon { min_lon + 1 } else { max_lon },
        }
    }

    /// Compute bounding box from a list of coords — mkgmap Area.getBBox
    pub fn from_coords(coords: &[Coord]) -> Self {
        let mut min_lat = i32::MAX;
        let mut max_lat = i32::MIN;
        let mut min_lon = i32::MAX;
        let mut max_lon = i32::MIN;

        for co in coords {
            let lat = co.latitude();
            let lon = co.longitude();
            if lat < min_lat { min_lat = lat; }
            if lat > max_lat { max_lat = lat; }
            if lon < min_lon { min_lon = lon; }
            if lon > max_lon { max_lon = lon; }
        }
        Self::new(min_lat, min_lon, max_lat, max_lon)
    }

    pub fn min_lat(&self) -> i32 { self.min_lat }
    pub fn min_lon(&self) -> i32 { self.min_lon }
    pub fn max_lat(&self) -> i32 { self.max_lat }
    pub fn max_lon(&self) -> i32 { self.max_lon }
    pub fn width(&self) -> i32 { self.max_lon - self.min_lon }
    pub fn height(&self) -> i32 { self.max_lat - self.min_lat }
    pub fn max_dimension(&self) -> i32 { self.width().max(self.height()) }

    pub fn center(&self) -> Coord {
        Coord::new((self.min_lat + self.max_lat) / 2, (self.min_lon + self.max_lon) / 2)
    }

    /// Contains coord using high-precision comparison — mkgmap Area.contains(Coord)
    pub fn contains_coord(&self, co: &Coord) -> bool {
        let lat_hp = co.high_prec_lat();
        let lon_hp = co.high_prec_lon();
        lat_hp >= (self.min_lat << DELTA_SHIFT)
            && lat_hp <= (self.max_lat << DELTA_SHIFT)
            && lon_hp >= (self.min_lon << DELTA_SHIFT)
            && lon_hp <= (self.max_lon << DELTA_SHIFT)
    }

    /// Contains other area entirely
    pub fn contains_area(&self, other: &Area) -> bool {
        other.min_lat >= self.min_lat
            && other.max_lat <= self.max_lat
            && other.min_lon >= self.min_lon
            && other.max_lon <= self.max_lon
    }

    pub fn intersects(&self, other: &Area) -> bool {
        self.min_lat <= other.max_lat
            && self.max_lat >= other.min_lat
            && self.min_lon <= other.max_lon
            && self.max_lon >= other.min_lon
    }

    /// Intersection of two areas
    #[cfg(test)]
    fn intersect(&self, other: &Area) -> Area {
        Area::new(
            self.min_lat.max(other.min_lat),
            self.min_lon.max(other.min_lon),
            self.max_lat.min(other.max_lat),
            self.max_lon.min(other.max_lon),
        )
    }

    /// Split area into xsplit * ysplit sub-areas — mkgmap Area.split
    pub fn split(&self, xsplit: usize, ysplit: usize, resolution_shift: i32) -> Option<Vec<Area>> {
        let mut areas = Vec::with_capacity(xsplit * ysplit);
        let mut xstart = self.min_lon;

        for x in 0..xsplit {
            let xend = if x == xsplit - 1 {
                self.max_lon
            } else {
                round_pof2(
                    xstart + (self.max_lon - xstart) / (xsplit - x) as i32,
                    resolution_shift,
                )
            };

            let mut ystart = self.min_lat;
            for y in 0..ysplit {
                let yend = if y == ysplit - 1 {
                    self.max_lat
                } else {
                    round_pof2(
                        ystart + (self.max_lat - ystart) / (ysplit - y) as i32,
                        resolution_shift,
                    )
                };

                if xstart < xend && ystart < yend {
                    areas.push(Area::new(ystart, xstart, yend, xend));
                }
                ystart = yend;
            }
            xstart = xend;
        }

        if areas.len() == xsplit * ysplit {
            Some(areas)
        } else if areas.len() <= 1 {
            None
        } else {
            Some(areas)
        }
    }
}

/// Round to nearest power of 2 — mkgmap Area.roundPof2
fn round_pof2(val: i32, shift: i32) -> i32 {
    if shift <= 0 {
        return val;
    }
    (((val >> (shift - 1)) + 1) >> 1) << shift
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_non_zero_dimensions() {
        let a = Area::new(100, 200, 100, 200);
        assert_eq!(a.max_lat(), 101);
        assert_eq!(a.max_lon(), 201);
        assert_eq!(a.height(), 1);
        assert_eq!(a.width(), 1);
    }

    #[test]
    fn test_contains_coord() {
        let a = Area::new(0, 0, 100, 100);
        let inside = Coord::new(50, 50);
        let outside = Coord::new(200, 200);
        assert!(a.contains_coord(&inside));
        assert!(!a.contains_coord(&outside));
    }

    #[test]
    fn test_contains_area() {
        let big = Area::new(0, 0, 100, 100);
        let small = Area::new(10, 10, 50, 50);
        assert!(big.contains_area(&small));
        assert!(!small.contains_area(&big));
    }

    #[test]
    fn test_intersects() {
        let a = Area::new(0, 0, 100, 100);
        let b = Area::new(50, 50, 150, 150);
        let c = Area::new(200, 200, 300, 300);
        assert!(a.intersects(&b));
        assert!(!a.intersects(&c));
    }

    #[test]
    fn test_center() {
        let a = Area::new(0, 0, 100, 200);
        let c = a.center();
        assert_eq!(c.latitude(), 50);
        assert_eq!(c.longitude(), 100);
    }

    #[test]
    fn test_from_coords() {
        let coords = vec![
            Coord::new(10, 20),
            Coord::new(50, 80),
            Coord::new(30, 40),
        ];
        let a = Area::from_coords(&coords);
        assert_eq!(a.min_lat(), 10);
        assert_eq!(a.min_lon(), 20);
        assert_eq!(a.max_lat(), 50);
        assert_eq!(a.max_lon(), 80);
    }

    #[test]
    fn test_split_2x2() {
        let a = Area::new(0, 0, 1000, 1000);
        let parts = a.split(2, 2, 0).unwrap();
        assert_eq!(parts.len(), 4);
    }

    #[test]
    fn test_intersect() {
        let a = Area::new(0, 0, 100, 100);
        let b = Area::new(50, 50, 150, 150);
        let c = a.intersect(&b);
        assert_eq!(c.min_lat(), 50);
        assert_eq!(c.min_lon(), 50);
        assert_eq!(c.max_lat(), 100);
        assert_eq!(c.max_lon(), 100);
    }
}
