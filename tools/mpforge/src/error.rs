//! Domain-specific error types for the pipeline.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("Configuration invalide: {message}")]
    ConfigError {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    #[error("Échec lecture source: {path}")]
    SourceReadError {
        path: String,
        #[source]
        source: gdal::errors::GdalError,
    },

    #[error("Export tuile échoué: {tile_id}")]
    TileExportError {
        tile_id: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Géométrie invalide au feature {fid}")]
    InvalidGeometry { fid: i64 },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_config_error_display() {
        let err = PipelineError::ConfigError {
            message: "missing field".to_string(),
            source: None,
        };
        assert_eq!(err.to_string(), "Configuration invalide: missing field");
    }

    #[test]
    fn test_source_read_error_display() {
        let gdal_err = gdal::errors::GdalError::NullPointer {
            method_name: "test",
            msg: "test error".to_string(),
        };
        let err = PipelineError::SourceReadError {
            path: "/path/to/file".to_string(),
            source: gdal_err,
        };
        assert!(err
            .to_string()
            .contains("Échec lecture source: /path/to/file"));
    }

    #[test]
    fn test_tile_export_error_display() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let err = PipelineError::TileExportError {
            tile_id: "tile_0_0".to_string(),
            source: io_err,
        };
        assert!(err.to_string().contains("Export tuile échoué: tile_0_0"));
    }

    #[test]
    fn test_invalid_geometry_display() {
        let err = PipelineError::InvalidGeometry { fid: 42 };
        assert_eq!(err.to_string(), "Géométrie invalide au feature 42");
    }
}
