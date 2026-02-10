//! Polish Map (.mp) file writing.

use crate::config::OutputConfig;
use crate::error::PipelineError;

/// Writes Polish Map (.mp) files using GDAL driver.
/// Stub implementation - will be fully implemented in Story 5.4.
#[allow(dead_code)] // Stub - will be implemented in Story 5.4
pub struct MpWriter {
    config: OutputConfig,
}

#[allow(dead_code)] // Stub - will be implemented in Story 5.4
impl MpWriter {
    pub fn new(config: OutputConfig) -> Self {
        Self { config }
    }

    /// Write a single tile to .mp file.
    /// Story 5.4 - Implement GDAL dataset creation and feature writing
    pub fn write_tile(&self, _tile_id: &str) -> Result<(), PipelineError> {
        todo!("MP file writing will be implemented in Story 5.4")
    }

    /// Finalize and close output file.
    /// Story 5.4 - Implement proper GDAL dataset cleanup
    pub fn finalize(&mut self) -> Result<(), PipelineError> {
        todo!("Writer finalization will be implemented in Story 5.4")
    }
}
