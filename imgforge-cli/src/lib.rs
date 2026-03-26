//! imgforge-cli library: public API for Polish Map (.mp) to IMG compilation.

pub mod cli;
pub mod error;
pub mod img;
pub mod parser;
pub mod routing;

pub use error::ImgError;
pub use img::writer::ImgWriter;

use std::path::Path;

/// Compile a Polish Map (.mp) file to Garmin IMG format.
///
/// Parses the input `.mp` file via [`parser::MpParser`], then writes the
/// Garmin IMG filesystem (header + FAT-like directory + subfile stubs) via
/// [`ImgWriter`].
pub fn compile(input: &Path, output: &Path) -> anyhow::Result<()> {
    use parser::MpParser;

    let mp = MpParser::parse_file(input)?;
    ImgWriter::write(&mp, output)?;
    Ok(())
}
