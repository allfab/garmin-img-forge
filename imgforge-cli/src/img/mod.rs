//! IMG Garmin filesystem writer — header, directory (FAT-like) and subfile assembly.

pub mod assembler;
pub mod directory;
pub mod filesystem;
pub mod header;
pub mod lbl;
pub mod net;
pub mod nod;
pub mod rgn;
pub mod tre;
pub mod writer;

pub use assembler::{AssemblyStats, BuildConfig, GmapsuppAssembler};
pub use writer::ImgWriter;
