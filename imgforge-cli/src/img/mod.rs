//! IMG Garmin filesystem writer — header, directory (FAT-like) and subfile assembly.

pub mod directory;
pub mod filesystem;
pub mod header;
pub mod lbl;
pub mod net;
pub mod rgn;
pub mod tre;
pub mod writer;

pub use writer::ImgWriter;
