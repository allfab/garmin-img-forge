//! PROJ database initialization
//!
//! Embeds proj.db into the binary and extracts it to a temporary directory
//! at startup, then configures PROJ_DATA environment variable automatically.

use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Embedded proj.db file (included at compile time)
/// This file is copied from PROJ installation during the build process
const PROJ_DB: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/resources/proj.db"));

/// Global temporary directory for PROJ data (cleaned up on program exit)
static PROJ_TEMP_DIR: OnceLock<tempfile::TempDir> = OnceLock::new();

/// Initialize PROJ by extracting embedded proj.db to a temporary directory
/// and setting PROJ_DATA environment variable.
///
/// This function is idempotent - it can be called multiple times safely.
/// The temporary directory is automatically cleaned up when the program exits.
///
/// # Errors
///
/// Returns an error if:
/// - Failed to create temporary directory
/// - Failed to write proj.db file
/// - Failed to set environment variable
pub fn init_proj() -> anyhow::Result<()> {
    // Only initialize once
    if PROJ_TEMP_DIR.get().is_some() {
        return Ok(());
    }

    // Create a temporary directory that will be cleaned up on program exit
    let temp_dir = tempfile::tempdir()
        .map_err(|e| anyhow::anyhow!("Failed to create temp directory for PROJ data: {}", e))?;

    // Write embedded proj.db to temp directory
    let proj_db_path = temp_dir.path().join("proj.db");
    let mut file = fs::File::create(&proj_db_path)
        .map_err(|e| anyhow::anyhow!("Failed to create proj.db file: {}", e))?;

    file.write_all(PROJ_DB)
        .map_err(|e| anyhow::anyhow!("Failed to write proj.db: {}", e))?;

    // Set PROJ_DATA environment variable (only if not already set by user)
    if env::var("PROJ_DATA").is_err() {
        env::set_var("PROJ_DATA", temp_dir.path());
    }

    // Store temp_dir in global static to prevent cleanup until program exit
    let _ = PROJ_TEMP_DIR.set(temp_dir);

    Ok(())
}

/// Get the path to the PROJ data directory
///
/// Returns None if PROJ has not been initialized yet
pub fn proj_data_dir() -> Option<PathBuf> {
    PROJ_TEMP_DIR.get().map(|dir| dir.path().to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_proj() {
        init_proj().expect("Failed to initialize PROJ");

        // Check that PROJ_DATA is set
        assert!(env::var("PROJ_DATA").is_ok());

        // Check that proj.db exists
        let proj_dir = proj_data_dir().expect("PROJ not initialized");
        assert!(proj_dir.join("proj.db").exists());
    }

    #[test]
    fn test_init_proj_idempotent() {
        // Initialize multiple times
        init_proj().expect("Failed to initialize PROJ");
        let dir1 = proj_data_dir();

        init_proj().expect("Failed to initialize PROJ again");
        let dir2 = proj_data_dir();

        // Should return the same directory
        assert_eq!(dir1, dir2);
    }
}
