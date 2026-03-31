// Overview — Point/Polyline/Polygon overviews for TRE, faithful to mkgmap Overview.java

/// Point overview: 3 bytes (type + max_level + sub_type)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PointOverview {
    pub type_code: u8,
    pub max_level: u8,
    pub sub_type: u8,
}

impl PointOverview {
    pub fn new(type_code: u8, max_level: u8, sub_type: u8) -> Self {
        Self { type_code, max_level, sub_type }
    }

    pub fn write(&self) -> [u8; 3] {
        [self.type_code, self.max_level, self.sub_type]
    }
}

/// Polyline overview: 2 bytes (type + max_level)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PolylineOverview {
    pub type_code: u8,
    pub max_level: u8,
}

impl PolylineOverview {
    pub fn new(type_code: u8, max_level: u8) -> Self {
        Self { type_code, max_level }
    }

    pub fn write(&self) -> [u8; 2] {
        [self.type_code, self.max_level]
    }
}

/// Polygon overview: 2 bytes (type + max_level)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PolygonOverview {
    pub type_code: u8,
    pub max_level: u8,
}

impl PolygonOverview {
    pub fn new(type_code: u8, max_level: u8) -> Self {
        Self { type_code, max_level }
    }

    pub fn write(&self) -> [u8; 2] {
        [self.type_code, self.max_level]
    }
}

/// Extended point overview: 4 bytes (type_high + max_level + type_low + 0x00)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExtPointOverview {
    pub type_high: u8,
    pub max_level: u8,
    pub type_low: u8,
}

impl ExtPointOverview {
    pub fn from_type_code(type_code: u32, max_level: u8) -> Self {
        Self {
            type_high: ((type_code >> 8) & 0xFF) as u8,
            max_level,
            type_low: (type_code & 0xFF) as u8,
        }
    }

    pub fn write(&self) -> [u8; 4] {
        [self.type_high, self.max_level, self.type_low, 0x00]
    }
}

/// Extended polyline overview: 4 bytes (type_high + max_level + type_low + 0x00)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExtPolylineOverview {
    pub type_high: u8,
    pub max_level: u8,
    pub type_low: u8,
}

impl ExtPolylineOverview {
    pub fn from_type_code(type_code: u32, max_level: u8) -> Self {
        Self {
            type_high: ((type_code >> 8) & 0xFF) as u8,
            max_level,
            type_low: (type_code & 0xFF) as u8,
        }
    }

    pub fn write(&self) -> [u8; 4] {
        [self.type_high, self.max_level, self.type_low, 0x00]
    }
}

/// Extended polygon overview: 4 bytes (type_high + max_level + type_low + 0x00)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExtPolygonOverview {
    pub type_high: u8,
    pub max_level: u8,
    pub type_low: u8,
}

impl ExtPolygonOverview {
    pub fn from_type_code(type_code: u32, max_level: u8) -> Self {
        Self {
            type_high: ((type_code >> 8) & 0xFF) as u8,
            max_level,
            type_low: (type_code & 0xFF) as u8,
        }
    }

    pub fn write(&self) -> [u8; 4] {
        [self.type_high, self.max_level, self.type_low, 0x00]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_overview_3_bytes() {
        let po = PointOverview::new(0x2C, 3, 0x00);
        assert_eq!(po.write(), [0x2C, 3, 0]);
    }

    #[test]
    fn test_polyline_overview_2_bytes() {
        let lo = PolylineOverview::new(0x01, 2);
        assert_eq!(lo.write(), [0x01, 2]);
    }

    #[test]
    fn test_polygon_overview_2_bytes() {
        let go = PolygonOverview::new(0x03, 1);
        assert_eq!(go.write(), [0x03, 1]);
    }

    #[test]
    fn test_ext_polygon_overview_4_bytes() {
        let ov = ExtPolygonOverview::from_type_code(0x10f04, 0);
        assert_eq!(ov.write(), [0x0f, 0, 0x04, 0x00]);
    }

    #[test]
    fn test_ext_point_overview_4_bytes() {
        let ov = ExtPointOverview::from_type_code(0x2C04, 0);
        assert_eq!(ov.write(), [0x2C, 0, 0x04, 0x00]);
    }

    #[test]
    fn test_sorting() {
        let mut overviews = vec![
            PolylineOverview::new(3, 1),
            PolylineOverview::new(1, 2),
            PolylineOverview::new(2, 1),
        ];
        overviews.sort();
        assert_eq!(overviews[0].type_code, 1);
        assert_eq!(overviews[1].type_code, 2);
        assert_eq!(overviews[2].type_code, 3);
    }
}
