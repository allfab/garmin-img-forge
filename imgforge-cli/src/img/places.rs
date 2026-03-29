// PlacesFile — cities, regions, countries, zips
// Faithful to mkgmap PlacesFile.java, PlacesHeader.java

/// Country record: 3 bytes
#[derive(Debug, Clone)]
pub struct Country {
    pub label_offset: u32, // 3 bytes in LBL
}

impl Country {
    pub fn write(&self) -> Vec<u8> {
        let b = self.label_offset.to_le_bytes();
        vec![b[0], b[1], b[2]]
    }
}

/// Region record: 5 bytes (country_index u16 + label_offset u24)
#[derive(Debug, Clone)]
pub struct Region {
    pub country_index: u16,
    pub label_offset: u32,
}

impl Region {
    pub fn write(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(5);
        buf.extend_from_slice(&self.country_index.to_le_bytes());
        let b = self.label_offset.to_le_bytes();
        buf.extend_from_slice(&b[..3]);
        buf
    }
}

/// City record: 5 bytes (various formats)
#[derive(Debug, Clone)]
pub struct City {
    pub region_index: u16,
    pub label_offset: u32,
    pub point_index: u8,
}

impl City {
    pub fn write(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(5);
        buf.extend_from_slice(&self.region_index.to_le_bytes());
        let b = self.label_offset.to_le_bytes();
        buf.extend_from_slice(&b[..3]);
        buf
    }
}

/// Zip code record: 3 bytes
#[derive(Debug, Clone)]
pub struct Zip {
    pub label_offset: u32,
}

impl Zip {
    pub fn write(&self) -> Vec<u8> {
        let b = self.label_offset.to_le_bytes();
        vec![b[0], b[1], b[2]]
    }
}

/// Manages all place records
pub struct PlacesWriter {
    pub countries: Vec<Country>,
    pub regions: Vec<Region>,
    pub cities: Vec<City>,
    pub zips: Vec<Zip>,
}

impl PlacesWriter {
    pub fn new() -> Self {
        Self {
            countries: Vec::new(),
            regions: Vec::new(),
            cities: Vec::new(),
            zips: Vec::new(),
        }
    }

    pub fn add_country(&mut self, label_offset: u32) -> u16 {
        let idx = self.countries.len() as u16 + 1; // 1-based
        self.countries.push(Country { label_offset });
        idx
    }

    pub fn add_region(&mut self, country_index: u16, label_offset: u32) -> u16 {
        let idx = self.regions.len() as u16 + 1;
        self.regions.push(Region {
            country_index,
            label_offset,
        });
        idx
    }

    pub fn add_city(&mut self, region_index: u16, label_offset: u32) -> u16 {
        let idx = self.cities.len() as u16 + 1;
        self.cities.push(City {
            region_index,
            label_offset,
            point_index: 0,
        });
        idx
    }

    pub fn add_zip(&mut self, label_offset: u32) -> u16 {
        let idx = self.zips.len() as u16 + 1;
        self.zips.push(Zip { label_offset });
        idx
    }

    /// Write all place sections, returns (countries_bytes, regions_bytes, cities_bytes, zips_bytes)
    pub fn write(&self) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
        let countries: Vec<u8> = self.countries.iter().flat_map(|c| c.write()).collect();
        let regions: Vec<u8> = self.regions.iter().flat_map(|r| r.write()).collect();
        let cities: Vec<u8> = self.cities.iter().flat_map(|c| c.write()).collect();
        let zips: Vec<u8> = self.zips.iter().flat_map(|z| z.write()).collect();
        (countries, regions, cities, zips)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_country_3_bytes() {
        let c = Country { label_offset: 0x123456 };
        let b = c.write();
        assert_eq!(b.len(), 3);
        assert_eq!(b, vec![0x56, 0x34, 0x12]);
    }

    #[test]
    fn test_region_5_bytes() {
        let r = Region {
            country_index: 1,
            label_offset: 0x100,
        };
        let b = r.write();
        assert_eq!(b.len(), 5);
    }

    #[test]
    fn test_places_writer() {
        let mut pw = PlacesWriter::new();
        let ci = pw.add_country(100);
        let ri = pw.add_region(ci, 200);
        let _city_i = pw.add_city(ri, 300);
        let _zi = pw.add_zip(400);

        let (countries, regions, cities, zips) = pw.write();
        assert_eq!(countries.len(), 3);
        assert_eq!(regions.len(), 5);
        assert_eq!(cities.len(), 5);
        assert_eq!(zips.len(), 3);
    }

    #[test]
    fn test_1_based_indices() {
        let mut pw = PlacesWriter::new();
        assert_eq!(pw.add_country(10), 1);
        assert_eq!(pw.add_country(20), 2);
    }
}
