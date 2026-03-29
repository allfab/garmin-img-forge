// MapObject trait — base for all map elements, faithful to mkgmap MapObject.java

use super::coord::Coord;

/// Common properties for all map objects (points, polylines, polygons)
pub trait MapObject {
    fn type_code(&self) -> u16;
    fn sub_type(&self) -> u8 { 0 }
    fn label_offset(&self) -> u32;
    fn coords(&self) -> &[Coord];
    fn has_extended_type(&self) -> bool {
        self.type_code() >= 0x100
    }
}
