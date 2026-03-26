//! Integration tests for imgforge-cli using fixture files.

use imgforge_cli::error::ParseError;
use imgforge_cli::parser::MpParser;
use imgforge_cli::ImgWriter;
use std::path::Path;

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

// ----------------------------------------------------------------
// Minimal fixture
// ----------------------------------------------------------------

#[test]
fn test_parse_minimal_fixture_header() {
    let mp = MpParser::parse_file(&fixture("minimal.mp")).unwrap();
    assert_eq!(mp.header.name, "Test Map");
    assert_eq!(mp.header.id, "63240001");
    assert_eq!(mp.header.code_page, "1252");
    assert_eq!(mp.header.levels, Some(2));
    assert_eq!(mp.header.tree_size, Some(3000));
    assert_eq!(mp.header.rgn_limit, Some(1024));
}

#[test]
fn test_parse_minimal_fixture_features() {
    let mp = MpParser::parse_file(&fixture("minimal.mp")).unwrap();
    assert_eq!(mp.points.len(), 1);
    assert_eq!(mp.polylines.len(), 1);
    assert_eq!(mp.polygons.len(), 1);
}

#[test]
fn test_parse_minimal_fixture_poi() {
    let mp = MpParser::parse_file(&fixture("minimal.mp")).unwrap();
    let poi = &mp.points[0];
    assert_eq!(poi.type_code, "0x2C00");
    assert_eq!(poi.label.as_deref(), Some("Mairie"));
    assert!((poi.lat - 45.1880).abs() < 1e-6);
    assert!((poi.lon - 5.7245).abs() < 1e-6);
    assert_eq!(poi.end_level, Some(4));
}

#[test]
fn test_parse_minimal_fixture_polyline() {
    let mp = MpParser::parse_file(&fixture("minimal.mp")).unwrap();
    let poly = &mp.polylines[0];
    assert_eq!(poly.type_code, "0x01");
    assert_eq!(poly.coords.len(), 3);
    assert!(poly.routing.is_none());
}

#[test]
fn test_parse_minimal_fixture_polygon() {
    let mp = MpParser::parse_file(&fixture("minimal.mp")).unwrap();
    let polygon = &mp.polygons[0];
    assert_eq!(polygon.type_code, "0x50");
    assert_eq!(polygon.coords.len(), 5);
    assert!(polygon.holes.is_empty());
}

// ----------------------------------------------------------------
// Routing fixture
// ----------------------------------------------------------------

#[test]
fn test_parse_routing_fixture() {
    let mp = MpParser::parse_file(&fixture("routing.mp")).unwrap();
    assert_eq!(mp.polylines.len(), 2);
}

#[test]
fn test_parse_routing_fixture_first_road() {
    let mp = MpParser::parse_file(&fixture("routing.mp")).unwrap();
    let poly = &mp.polylines[0];
    let routing = poly.routing.as_ref().unwrap();
    assert_eq!(routing.road_id.as_deref(), Some("A480_001"));
    assert_eq!(routing.route_param.as_deref(), Some("7,4,0,1,0,0,0,0,0"));
    assert_eq!(routing.dir_indicator, Some(0));
    assert!(routing.speed_type.is_none());
}

#[test]
fn test_parse_routing_fixture_second_road_speed() {
    let mp = MpParser::parse_file(&fixture("routing.mp")).unwrap();
    let poly = &mp.polylines[1];
    let routing = poly.routing.as_ref().unwrap();
    assert_eq!(routing.road_id.as_deref(), Some("N87_001"));
    assert_eq!(routing.speed_type, Some(5));
    assert_eq!(routing.dir_indicator, Some(1));
}

// ----------------------------------------------------------------
// Error fixtures
// ----------------------------------------------------------------

#[test]
fn test_parse_no_img_id_fixture() {
    let result = MpParser::parse_file(&fixture("no_img_id.mp"));
    assert!(matches!(result, Err(ParseError::MissingImgId)));
}

#[test]
fn test_parse_invalid_coords_fixture() {
    let result = MpParser::parse_file(&fixture("invalid_coords.mp"));
    assert!(matches!(result, Err(ParseError::InvalidFormat { .. })));
    if let Err(ParseError::InvalidFormat { line, message }) = result {
        assert!(line > 0, "line number must be positive, got {}", line);
        assert!(!message.is_empty(), "error message must not be empty");
    }
}

// ----------------------------------------------------------------
// Unknown fields fixture
// ----------------------------------------------------------------

#[test]
fn test_parse_unknown_fields_fixture_header() {
    let mp = MpParser::parse_file(&fixture("unknown_fields.mp")).unwrap();
    assert_eq!(
        mp.header
            .other_fields
            .get("CustomVendorField")
            .map(|s| s.as_str()),
        Some("custom_value_123")
    );
    assert_eq!(
        mp.header
            .other_fields
            .get("AnotherUnknownField")
            .map(|s| s.as_str()),
        Some("hello_world")
    );
}

#[test]
fn test_parse_unknown_fields_fixture_poi() {
    let mp = MpParser::parse_file(&fixture("unknown_fields.mp")).unwrap();
    let poi = &mp.points[0];
    assert_eq!(
        poi.other_fields
            .get("CustomPOIAttribute")
            .map(|s| s.as_str()),
        Some("poi_custom_value")
    );
    assert_eq!(
        poi.other_fields.get("ExtraField").map(|s| s.as_str()),
        Some("extra")
    );
}

#[test]
fn test_parse_unknown_fields_fixture_polyline() {
    let mp = MpParser::parse_file(&fixture("unknown_fields.mp")).unwrap();
    let poly = &mp.polylines[0];
    assert_eq!(
        poly.other_fields.get("CustomLineField").map(|s| s.as_str()),
        Some("line_value")
    );
}

#[test]
fn test_parse_unknown_fields_fixture_polygon() {
    let mp = MpParser::parse_file(&fixture("unknown_fields.mp")).unwrap();
    let poly = &mp.polygons[0];
    assert_eq!(
        poly.other_fields
            .get("CustomPolygonField")
            .map(|s| s.as_str()),
        Some("polygon_value")
    );
}

// ----------------------------------------------------------------
// CLI integration tests
// ----------------------------------------------------------------

#[test]
fn test_cli_help() {
    use assert_cmd::Command;
    let mut cmd = Command::cargo_bin("imgforge-cli").unwrap();
    cmd.arg("--help");
    cmd.assert().success();
}

#[test]
fn test_cli_compile_help() {
    use assert_cmd::Command;
    let mut cmd = Command::cargo_bin("imgforge-cli").unwrap();
    cmd.args(["compile", "--help"]);
    cmd.assert().success();
}

#[test]
fn test_cli_compile_real_file() {
    use assert_cmd::Command;
    use tempfile::NamedTempFile;

    let output = NamedTempFile::new().unwrap();
    let mut cmd = Command::cargo_bin("imgforge-cli").unwrap();
    cmd.args([
        "compile",
        fixture("minimal.mp").to_str().unwrap(),
        "-o",
        output.path().to_str().unwrap(),
    ]);
    // Should succeed: parses .mp and writes a valid .img filesystem.
    cmd.assert().success();
}

#[test]
fn test_cli_compile_nonexistent_file() {
    use assert_cmd::Command;
    let mut cmd = Command::cargo_bin("imgforge-cli").unwrap();
    cmd.args(["compile", "/nonexistent/path/file.mp", "-o", "/tmp/out.img"]);
    cmd.assert().failure();
}

// ----------------------------------------------------------------
// IMG filesystem integration tests (Story 13.2)
// ----------------------------------------------------------------

#[test]
fn test_compile_creates_img() {
    use assert_cmd::Command;
    use tempfile::NamedTempFile;

    let output = NamedTempFile::new().unwrap();
    let mut cmd = Command::cargo_bin("imgforge-cli").unwrap();
    cmd.args([
        "compile",
        fixture("minimal_for_img.mp").to_str().unwrap(),
        "-o",
        output.path().to_str().unwrap(),
    ]);
    cmd.assert().success();
    let metadata = std::fs::metadata(output.path()).unwrap();
    assert!(metadata.len() > 0, "compiled .img must be non-empty");
}

#[test]
fn test_img_header_magic() {
    let mp = MpParser::parse_file(&fixture("minimal_for_img.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();
    assert_eq!(
        &bytes[0x002..0x008],
        b"GARMIN",
        "IMG magic must be 'GARMIN'"
    );
}

#[test]
fn test_img_signature() {
    let mp = MpParser::parse_file(&fixture("minimal_for_img.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();
    assert_eq!(bytes[0x1FE], 0x55, "DOS signature byte 1 must be 0x55");
    assert_eq!(bytes[0x1FF], 0xAA, "DOS signature byte 2 must be 0xAA");
}

#[test]
fn test_img_subfile_names() {
    let mp = MpParser::parse_file(&fixture("minimal_for_img.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();

    // Directory is at block 1 (offset 512 for block_size=512).
    let dir_start = 512usize;
    // Each Dirent is 32 bytes.
    let extensions = ["TRE", "RGN", "LBL"];
    for (i, expected_ext) in extensions.iter().enumerate() {
        let offset = dir_start + i * 32;
        let name = &bytes[offset..offset + 8];
        let ext = &bytes[offset + 8..offset + 11];
        // Name must be "63240001" (no padding since it's exactly 8 chars).
        assert_eq!(name, b"63240001", "subfile {i} name must be '63240001'");
        assert_eq!(
            ext,
            expected_ext.as_bytes(),
            "subfile {i} extension must be '{expected_ext}'"
        );
    }
}

#[test]
fn test_img_header_xor() {
    let mp = MpParser::parse_file(&fixture("minimal_for_img.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();
    let xor = bytes[..512].iter().fold(0u8, |acc, &b| acc ^ b);
    assert_eq!(xor, 0x00, "XOR of all 512 header bytes must be 0x00");
}

// ----------------------------------------------------------------
// RGN subfile integration tests (Story 13.4)
// ----------------------------------------------------------------

#[test]
fn test_img_rgn_subfile_not_empty() {
    // After Story 13.4, the RGN subfile contains real binary content.
    let mp = MpParser::parse_file(&fixture("multi_type.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();

    // Directory at block 1 (block_size=512). RGN is Dirent index 1 (32 bytes each).
    let dir_start = 512usize;
    let rgn_dirent = dir_start + 1 * 32;
    let size_used = u32::from_le_bytes([
        bytes[rgn_dirent + 0x12],
        bytes[rgn_dirent + 0x13],
        bytes[rgn_dirent + 0x14],
        bytes[rgn_dirent + 0x15],
    ]);
    assert!(size_used > 0, "RGN subfile size_used must be > 0 (real RGN content)");
}

#[test]
fn test_img_rgn_header_magic() {
    // RGN subfile must start with [0x1D, 0x00, 0x01, 0x00] (header_length=29, version=1).
    let mp = MpParser::parse_file(&fixture("multi_type.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();

    // Read RGN block_start from Dirent index 1.
    let dir_start = 512usize;
    let rgn_dirent = dir_start + 1 * 32;
    let block_start =
        u16::from_le_bytes([bytes[rgn_dirent + 0x0C], bytes[rgn_dirent + 0x0D]]) as usize;
    let rgn_offset = block_start * 512;

    assert_eq!(
        &bytes[rgn_offset..rgn_offset + 4],
        &[0x1D, 0x00, 0x01, 0x00],
        "RGN header must start with [0x1D, 0x00, 0x01, 0x00] (header_length=29, version=1)"
    );
}

#[test]
fn test_img_tre_subdivisions_rgn_offset_nonzero() {
    // When features are present, level 1's subdivision must have rgn_offset > 0
    // (level 0 starts at the beginning of the data section = offset 0).
    let mp = MpParser::parse_file(&fixture("multi_type.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();

    // Locate TRE block.
    let dir_start = 512usize;
    let tre_block_start =
        u16::from_le_bytes([bytes[dir_start + 0x0C], bytes[dir_start + 0x0D]]) as usize;
    let tre_offset = tre_block_start * 512;

    // Read subdivisions_offset from TRE header at byte 0x1C (robust across level counts).
    let subdivs_offset = u32::from_le_bytes([
        bytes[tre_offset + 0x1C],
        bytes[tre_offset + 0x1D],
        bytes[tre_offset + 0x1E],
        bytes[tre_offset + 0x1F],
    ]) as usize;
    let subdivs_start = tre_offset + subdivs_offset;

    // Subdivision 0 (level 0): rgn_offset must be 0 (data starts here).
    let rgn_off0 = (bytes[subdivs_start] as u32)
        | ((bytes[subdivs_start + 1] as u32) << 8)
        | ((bytes[subdivs_start + 2] as u32) << 16);
    assert_eq!(rgn_off0, 0, "level 0 subdivision rgn_offset must be 0 (start of data section)");

    // Subdivision 1 (level 1): rgn_offset must be > 0 since level 0 has features.
    let subdiv1_start = subdivs_start + 16; // each subdivision is 16 bytes
    let rgn_off1 = (bytes[subdiv1_start] as u32)
        | ((bytes[subdiv1_start + 1] as u32) << 8)
        | ((bytes[subdiv1_start + 2] as u32) << 16);
    assert!(
        rgn_off1 > 0,
        "level 1 subdivision rgn_offset must be > 0 when level 0 has features"
    );
}

#[test]
fn test_img_level_filtering_subdivision_size() {
    // multi_type.mp has a POLYLINE with EndLevel=0 → included only in level 0.
    // Level 0 must have more feature data than level 1.
    let mp = MpParser::parse_file(&fixture("multi_type.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();

    // Locate TRE subdivisions — read subdivisions_offset from TRE header (robust across level counts).
    let dir_start = 512usize;
    let tre_block_start =
        u16::from_le_bytes([bytes[dir_start + 0x0C], bytes[dir_start + 0x0D]]) as usize;
    let tre_offset = tre_block_start * 512;
    let subdivs_offset = u32::from_le_bytes([
        bytes[tre_offset + 0x1C],
        bytes[tre_offset + 0x1D],
        bytes[tre_offset + 0x1E],
        bytes[tre_offset + 0x1F],
    ]) as usize;
    let subdivs_start = tre_offset + subdivs_offset;

    let rgn_off0 = (bytes[subdivs_start] as u32)
        | ((bytes[subdivs_start + 1] as u32) << 8)
        | ((bytes[subdivs_start + 2] as u32) << 16);
    let subdiv1_start = subdivs_start + 16;
    let rgn_off1 = (bytes[subdiv1_start] as u32)
        | ((bytes[subdiv1_start + 1] as u32) << 8)
        | ((bytes[subdiv1_start + 2] as u32) << 16);

    // Locate RGN data section (after the 29-byte header).
    let rgn_dirent = dir_start + 1 * 32;
    let rgn_block_start =
        u16::from_le_bytes([bytes[rgn_dirent + 0x0C], bytes[rgn_dirent + 0x0D]]) as usize;
    let rgn_file_start = rgn_block_start * 512;
    // data_size is at offset 0x08 in the RGN header
    let data_size = u32::from_le_bytes([
        bytes[rgn_file_start + 8],
        bytes[rgn_file_start + 9],
        bytes[rgn_file_start + 10],
        bytes[rgn_file_start + 11],
    ]);

    // Level 0 size = rgn_off1 - rgn_off0 = rgn_off1 (since rgn_off0=0)
    let level0_size = rgn_off1 - rgn_off0;
    // Level 1 size = total data - rgn_off1
    let level1_size = data_size - rgn_off1;

    assert!(
        level0_size > level1_size,
        "level 0 (detailed) must have more data than level 1 (coarse): {} vs {}",
        level0_size,
        level1_size
    );
}

// ----------------------------------------------------------------
// TRE subfile integration tests (Story 13.3)
// ----------------------------------------------------------------

#[test]
fn test_img_tre_subfile_not_empty() {
    // After Story 13.3, the TRE subfile contains real binary content (not zeros).
    let mp = MpParser::parse_file(&fixture("minimal_for_img.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();

    // Directory at block 1 (offset = 1 × 512 for block_size = 512).
    // TRE is the first Dirent (index 0), each entry is 32 bytes.
    // size_used is at offset 0x12 within the Dirent.
    let dir_start = 512usize;
    let size_used = u32::from_le_bytes([
        bytes[dir_start + 0x12],
        bytes[dir_start + 0x13],
        bytes[dir_start + 0x14],
        bytes[dir_start + 0x15],
    ]);
    assert!(
        size_used > 0,
        "TRE subfile size_used must be > 0 (real TRE content)"
    );
}

#[test]
fn test_img_tre_header_version() {
    // Verify that the TRE subfile starts with the correct version 3 magic.
    let mp = MpParser::parse_file(&fixture("minimal_for_img.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();

    // Read TRE block_start from the first Dirent (offset 0x0C within the Dirent).
    let dir_start = 512usize;
    let block_start =
        u16::from_le_bytes([bytes[dir_start + 0x0C], bytes[dir_start + 0x0D]]) as usize;
    let tre_offset = block_start * 512;

    assert_eq!(
        &bytes[tre_offset..tre_offset + 4],
        &[0x94, 0x00, 0x03, 0x00],
        "TRE header must start with [0x94, 0x00, 0x03, 0x00] (header_length=148, version=3)"
    );
}

#[test]
fn test_img_tre_bounds_nonzero() {
    // minimal_for_img.mp has a POI in France → max_lat garmin value must be positive.
    let mp = MpParser::parse_file(&fixture("minimal_for_img.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();

    let dir_start = 512usize;
    let block_start =
        u16::from_le_bytes([bytes[dir_start + 0x0C], bytes[dir_start + 0x0D]]) as usize;
    let tre_offset = block_start * 512;

    // max_lat is at offset 0x04 of the TRE subfile, encoded as LE24 signed.
    let raw = (bytes[tre_offset + 4] as i32)
        | ((bytes[tre_offset + 5] as i32) << 8)
        | ((bytes[tre_offset + 6] as i32) << 16);
    let max_lat_g = if raw & 0x80_0000 != 0 {
        raw | !0xFF_FFFF
    } else {
        raw
    };
    assert!(
        max_lat_g > 0,
        "France tile max_lat garmin must be > 0, got {}",
        max_lat_g
    );
}

// ----------------------------------------------------------------
// LBL subfile integration tests (Story 13.5)
// ----------------------------------------------------------------

fn read_lbl_block(bytes: &[u8]) -> (usize, u32) {
    // LBL is Dirent index 2 in the directory (block 1 for block_size=512).
    let dir_start = 512usize;
    let lbl_dirent = dir_start + 2 * 32;
    let block_start =
        u16::from_le_bytes([bytes[lbl_dirent + 0x0C], bytes[lbl_dirent + 0x0D]]) as usize;
    let size_used = u32::from_le_bytes([
        bytes[lbl_dirent + 0x12],
        bytes[lbl_dirent + 0x13],
        bytes[lbl_dirent + 0x14],
        bytes[lbl_dirent + 0x15],
    ]);
    (block_start * 512, size_used)
}

#[test]
fn test_img_lbl_subfile_not_empty() {
    // After Story 13.5, LBL subfile must contain real content when features have labels.
    let mp = MpParser::parse_file(&fixture("multi_type.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();
    let (_lbl_offset, size_used) = read_lbl_block(&bytes);
    assert!(size_used > 0, "LBL subfile size_used must be > 0 after Story 13.5");
}

#[test]
fn test_img_lbl_header_magic() {
    // LBL subfile must start with [0x1C, 0x00, 0x01, 0x00] (header_length=28, version=1).
    let mp = MpParser::parse_file(&fixture("multi_type.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();
    let (lbl_offset, _) = read_lbl_block(&bytes);
    assert_eq!(
        &bytes[lbl_offset..lbl_offset + 4],
        &[0x1C, 0x00, 0x01, 0x00],
        "LBL header must start with [0x1C, 0x00, 0x01, 0x00] (header_length=28, version=1)"
    );
}

#[test]
fn test_img_lbl_contains_label_bytes() {
    // "Mairie" (bytes CP1252) must be found in the LBL data section (after offset 0x1C).
    let mp = MpParser::parse_file(&fixture("multi_type.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();
    let (lbl_offset, _) = read_lbl_block(&bytes);
    // LBL data section starts at lbl_offset + 0x1C (header size = 28)
    let data_start = lbl_offset + 0x1C;
    // "Mairie" encoded as CP1252 = ASCII (no accents)
    let mairie = b"Mairie";
    let lbl_data = &bytes[data_start..];
    let found = lbl_data.windows(mairie.len()).any(|w| w == mairie);
    assert!(found, "LBL data section must contain the bytes for 'Mairie'");
}

#[test]
fn test_img_rgn_poi_label_offset_nonzero() {
    // Parse the POI record from RGN and verify label_offset ≠ [0x00, 0x00, 0x00].
    //
    // Fixture assumption: multi_type.mp first feature is a POI ("Mairie") WITH a label.
    // POI record layout (with label): base_type(1) + delta_lon(2) + delta_lat(2) + flags(1)
    //   + label_offset(3) = 9 bytes. label_offset at bytes 6-8 within the first record.
    // If multi_type.mp is modified (e.g. first feature reordered or label removed), update
    // the byte offsets accordingly.
    let mp = MpParser::parse_file(&fixture("multi_type.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();

    // Locate RGN block.
    let dir_start = 512usize;
    let rgn_dirent = dir_start + 1 * 32;
    let rgn_block_start =
        u16::from_le_bytes([bytes[rgn_dirent + 0x0C], bytes[rgn_dirent + 0x0D]]) as usize;
    let rgn_file_start = rgn_block_start * 512;
    // RGN data section starts after the 29-byte header.
    let rgn_data_start = rgn_file_start + 29;

    // POI record format: [base_type(1)][delta_lon(2)][delta_lat(2)][flags(1)][label_offset(3)]
    // = 9 bytes total. label_offset at bytes 6-8 of the record.
    let label_offset_bytes = &bytes[rgn_data_start + 6..rgn_data_start + 9];
    assert_ne!(
        label_offset_bytes,
        &[0x00, 0x00, 0x00],
        "POI label_offset in RGN must be non-zero when LBL is populated"
    );
}

#[test]
fn test_img_lbl_accented_chars() {
    // Verify that accented labels are encoded in CP1252 in the LBL subfile.
    // "Église": É = 0xC9
    let mp = MpParser::parse_file(&fixture("labels_accented.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();
    let (lbl_offset, _) = read_lbl_block(&bytes);
    let data_start = lbl_offset + 0x1C;
    // "Église" in CP1252: [0xC9, 0x67, 0x6C, 0x69, 0x73, 0x65]
    let eglise_cp1252 = &[0xC9u8, 0x67, 0x6C, 0x69, 0x73, 0x65];
    let lbl_data = &bytes[data_start..];
    let found = lbl_data.windows(eglise_cp1252.len()).any(|w| w == eglise_cp1252);
    assert!(found, "LBL data section must contain 'Église' encoded in CP1252");
}

#[test]
fn test_img_lbl_shield_code() {
    // Verify that shield codes are encoded correctly: "~[0x04]D1075" → [0x04, 0x44, ...]
    let mp = MpParser::parse_file(&fixture("labels_accented.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();
    let (lbl_offset, _) = read_lbl_block(&bytes);
    let data_start = lbl_offset + 0x1C;
    // "~[0x04]D1075" → [0x04, 0x44, 0x31, 0x30, 0x37, 0x35]
    let shield_bytes = &[0x04u8, 0x44, 0x31, 0x30, 0x37, 0x35];
    let lbl_data = &bytes[data_start..];
    let found = lbl_data.windows(shield_bytes.len()).any(|w| w == shield_bytes);
    assert!(found, "LBL data section must contain shield-encoded '~[0x04]D1075'");
}

#[test]
fn test_img_lbl_deduplication() {
    // "Église" appears twice in labels_accented.mp — LBL must store it only once.
    // Verify by checking that the POI record for the duplicate "Église" references the same offset
    // as the first "Église" (by checking the LBL data size stays consistent).
    let mp = MpParser::parse_file(&fixture("labels_accented.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();
    let (lbl_offset, size_used) = read_lbl_block(&bytes);
    let data_start = lbl_offset + 0x1C;
    // Count occurrences of 0xC9 (É byte) in label data section
    let lbl_data = &bytes[data_start..data_start + size_used as usize - 28];
    let eglise_cp1252 = &[0xC9u8, 0x67, 0x6C, 0x69, 0x73, 0x65, 0x00];
    let count = lbl_data
        .windows(eglise_cp1252.len())
        .filter(|&w| w == eglise_cp1252)
        .count();
    assert_eq!(count, 1, "deduplicated 'Église' must appear exactly once in LBL data section");
}

// ----------------------------------------------------------------
// E2E Story 13.6 — validation tuile BDTOPO (bdtopo_tile.mp → .img)
// ----------------------------------------------------------------

#[test]
fn test_e2e_compile_bdtopo_tile_succeeds() {
    // AC1 : compilation sans erreur de la fixture BDTOPO réaliste.
    use assert_cmd::Command;
    use tempfile::NamedTempFile;

    let output = NamedTempFile::new().unwrap();
    let mut cmd = Command::cargo_bin("imgforge-cli").unwrap();
    cmd.args([
        "compile",
        fixture("bdtopo_tile.mp").to_str().unwrap(),
        "-o",
        output.path().to_str().unwrap(),
    ]);
    cmd.assert().success();
}

#[test]
fn test_e2e_img_all_subfiles_present_and_nonzero() {
    // AC1 : TRE/RGN/LBL présents dans le directory avec size_used > 0.
    // Dirent index 0=TRE, 1=RGN, 2=LBL — chacun 32 bytes à partir du bloc 1 (offset 512).
    let mp = MpParser::parse_file(&fixture("bdtopo_tile.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();

    // Minimum meaningful sizes: TRE header alone = 148 bytes, RGN header = 29 bytes, LBL header = 28 bytes.
    let dir_start = 512usize;
    let min_sizes = [("TRE", 148u32), ("RGN", 29u32), ("LBL", 28u32)];
    for (i, (name, min_size)) in min_sizes.iter().enumerate() {
        // size_used is at offset 0x12 within each 32-byte Dirent.
        let dirent = dir_start + i * 32;
        let size_used = u32::from_le_bytes([
            bytes[dirent + 0x12],
            bytes[dirent + 0x13],
            bytes[dirent + 0x14],
            bytes[dirent + 0x15],
        ]);
        assert!(
            size_used > *min_size,
            "{} size_used must be > {} bytes (header-only minimum), got {}",
            name,
            min_size,
            size_used
        );
    }
}

#[test]
fn test_e2e_img_map_id_in_header() {
    // AC2 : le map ID "63240038" doit figurer dans le bloc directory (offset 512-1023).
    // Chaque Dirent commence par les 8 octets du nom de la carte (= map ID).
    let mp = MpParser::parse_file(&fixture("bdtopo_tile.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();

    let map_id = b"63240038";
    // Scan only the 3 Dirent entries (3 × 32 = 96 bytes) starting at offset 512.
    // Each Dirent begins with the 8-byte map name (= map ID), so this is the precise location.
    let dirents = &bytes[512..512 + 3 * 32];
    let found = dirents.windows(map_id.len()).any(|w| w == map_id);
    assert!(found, "Map ID '63240038' must be present in the Dirent name field of the directory block");
}

#[test]
fn test_e2e_img_tre_bounds_france() {
    // AC1 : bounding box TRE dans la plage Isère (lat ≈ 45.15–45.25°, lon ≈ 5.71–5.88°).
    // TRE header layout (from tre.rs):
    //   0x04: max_lat (LE24s), 0x07: max_lon (LE24s),
    //   0x0A: min_lat (LE24s), 0x0D: min_lon (LE24s)
    // Garmin units : val = round(deg × 2^24 / 360) → France : max_lat ≈ 2_105_344 (positif)
    let mp = MpParser::parse_file(&fixture("bdtopo_tile.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();

    // Locate TRE subfile via Dirent index 0 (block_start at offset 0x0C within the Dirent).
    let dir_start = 512usize;
    let block_start =
        u16::from_le_bytes([bytes[dir_start + 0x0C], bytes[dir_start + 0x0D]]) as usize;
    let tre_offset = block_start * 512;

    // Read max_lat (LE24 signed) at TRE offset 0x04
    let raw_max = (bytes[tre_offset + 4] as i32)
        | ((bytes[tre_offset + 5] as i32) << 8)
        | ((bytes[tre_offset + 6] as i32) << 16);
    let max_lat_g = if raw_max & 0x80_0000 != 0 { raw_max | !0xFF_FFFF } else { raw_max };

    // Read min_lat (LE24 signed) at TRE offset 0x0A
    let raw_min = (bytes[tre_offset + 10] as i32)
        | ((bytes[tre_offset + 11] as i32) << 8)
        | ((bytes[tre_offset + 12] as i32) << 16);
    let min_lat_g = if raw_min & 0x80_0000 != 0 { raw_min | !0xFF_FFFF } else { raw_min };

    // Read max_lon (LE24 signed) at TRE offset 0x07
    let raw_max_lon = (bytes[tre_offset + 7] as i32)
        | ((bytes[tre_offset + 8] as i32) << 8)
        | ((bytes[tre_offset + 9] as i32) << 16);
    let max_lon_g = if raw_max_lon & 0x80_0000 != 0 { raw_max_lon | !0xFF_FFFF } else { raw_max_lon };

    // Read min_lon (LE24 signed) at TRE offset 0x0D
    let raw_min_lon = (bytes[tre_offset + 13] as i32)
        | ((bytes[tre_offset + 14] as i32) << 8)
        | ((bytes[tre_offset + 15] as i32) << 16);
    let min_lon_g = if raw_min_lon & 0x80_0000 != 0 { raw_min_lon | !0xFF_FFFF } else { raw_min_lon };

    assert!(
        max_lat_g > 0,
        "Isère tile max_lat_garmin must be positive (France zone), got {}",
        max_lat_g
    );
    assert!(
        min_lat_g > 0,
        "Isère tile min_lat_garmin must be positive (France zone), got {}",
        min_lat_g
    );
    assert!(
        max_lat_g > min_lat_g,
        "max_lat_garmin must be > min_lat_garmin, got {} vs {}",
        max_lat_g,
        min_lat_g
    );
    assert!(
        max_lon_g > 0,
        "Isère tile max_lon_garmin must be positive (Eastern France, lon > 0°), got {}",
        max_lon_g
    );
    assert!(
        max_lon_g > min_lon_g,
        "max_lon_garmin must be > min_lon_garmin, got {} vs {}",
        max_lon_g,
        min_lon_g
    );
}

#[test]
fn test_e2e_img_lbl_contains_bdtopo_labels() {
    // AC2 : LBL data section contient "D1075" (route) et "Mairie de Saint-" (POI accentué).
    // LBL header_length = 28, data section starts at lbl_offset + 0x1C.
    let mp = MpParser::parse_file(&fixture("bdtopo_tile.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();
    let (lbl_offset, lbl_size) = read_lbl_block(&bytes);
    // LBL data section starts after the 28-byte header (0x1C = 28).
    let data_start = lbl_offset + 0x1C;
    let data_end = lbl_offset + lbl_size as usize;
    assert!(
        data_start < bytes.len() && data_start < data_end,
        "LBL data section out of bounds: offset={}, size_used={}",
        lbl_offset,
        lbl_size
    );
    let lbl_data = &bytes[data_start..data_end];

    // "D1075" encoded in CP1252 (pure ASCII)
    let d1075 = b"D1075";
    let found_d1075 = lbl_data.windows(d1075.len()).any(|w| w == d1075);
    assert!(found_d1075, "LBL data section must contain bytes for 'D1075'");

    // "Mairie de Saint-" is the ASCII prefix of "Mairie de Saint-Égrève" (CP1252)
    let mairie_prefix = b"Mairie de Saint-";
    let found_mairie = lbl_data.windows(mairie_prefix.len()).any(|w| w == mairie_prefix);
    assert!(
        found_mairie,
        "LBL data section must contain bytes for 'Mairie de Saint-' (CP1252 prefix of 'Mairie de Saint-Égrève')"
    );

    // "É" in CP1252 = 0xC9 — validates that the accented char in "Saint-Égrève" is encoded correctly.
    let saint_e_cp1252 = b"Saint-\xC9";
    let found_accent = lbl_data.windows(saint_e_cp1252.len()).any(|w| w == saint_e_cp1252);
    assert!(
        found_accent,
        "LBL data section must contain CP1252 'Saint-\\xC9' (É=0xC9) — validates accented char encoding"
    );

    // "â" in CP1252 = 0xE2 — validates "Châtaigneraie" encoding.
    let chatai_cp1252 = b"Ch\xE2taigneraie";
    let found_chatai = lbl_data.windows(chatai_cp1252.len()).any(|w| w == chatai_cp1252);
    assert!(
        found_chatai,
        "LBL data section must contain CP1252 'Ch\\xE2taigneraie' (â=0xE2) — validates Châtaigneraie encoding"
    );
}

#[test]
fn test_e2e_img_rgn_all_feature_types() {
    // AC1 : RGN size_used > 29 bytes (header-only = 29 bytes), confirming features were encoded.
    // Avec 6 polylines + 4 POI + 4 polygones, size_used est substantiellement plus grand que 29,
    // mais ce test valide uniquement le seuil minimal (pas le décompte exact par type de feature).
    // RGN is Dirent index 1 (block 1, block_size=512).
    let mp = MpParser::parse_file(&fixture("bdtopo_tile.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();

    // size_used is at offset 0x12 within Dirent index 1 (RGN).
    let dir_start = 512usize;
    let rgn_dirent = dir_start + 1 * 32;
    let size_used = u32::from_le_bytes([
        bytes[rgn_dirent + 0x12],
        bytes[rgn_dirent + 0x13],
        bytes[rgn_dirent + 0x14],
        bytes[rgn_dirent + 0x15],
    ]);
    assert!(
        size_used > 29,
        "RGN size_used must be > 29 (header-only = 29 bytes), got {}",
        size_used
    );
}

#[test]
fn test_e2e_img_dos_header_valid() {
    // AC2 : header DOS 512 bytes valide — magic GARMIN, signature 0x55/0xAA, XOR de tous les bytes = 0.
    let mp = MpParser::parse_file(&fixture("bdtopo_tile.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();

    // Magic "GARMIN" at offset 0x002
    assert_eq!(
        &bytes[0x002..0x008],
        b"GARMIN",
        "IMG header must contain GARMIN magic at offset 0x002"
    );
    // DOS partition signature at 0x1FE/0x1FF
    assert_eq!(bytes[0x1FE], 0x55, "DOS signature byte 0x1FE must be 0x55, got 0x{:02X}", bytes[0x1FE]);
    assert_eq!(bytes[0x1FF], 0xAA, "DOS signature byte 0x1FF must be 0xAA, got 0x{:02X}", bytes[0x1FF]);
    // XOR of all 512 header bytes must be 0x00 (XOR byte at 0x000 is computed to ensure this)
    let xor = bytes[..512].iter().fold(0u8, |acc, &b| acc ^ b);
    assert_eq!(xor, 0x00, "XOR of all 512 header bytes must be 0x00, got 0x{:02X}", xor);
}

// ----------------------------------------------------------------
// Story 14.2: Road network graph builder integration tests
// ----------------------------------------------------------------

#[test]
fn test_routing_graph_fixture_builds_without_crash() {
    // Task 5.5: Compile routing.mp → verify graph is built (no crash)
    let mp = MpParser::parse_file(&fixture("routing.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();
    assert!(bytes.len() > 512, "IMG output must be non-empty");
}

#[test]
fn test_routing_graph_fixture_network_stats() {
    // Task 6.2: Verify node/arc/road_def counts for routing_graph.mp fixture
    use imgforge_cli::routing::graph_builder::build_road_network;

    let mp = MpParser::parse_file(&fixture("routing_graph.mp")).unwrap();
    let network = build_road_network(&mp.polylines);

    // 10 routable polylines (road 1-10), 1 non-routable (river)
    assert_eq!(network.road_defs.len(), 10, "10 routable polylines → 10 road defs");

    // Node count: unique (position, level) endpoints
    // Level 0 nodes: A, B, C, D, E, F, G + roundabout end = at least 8
    // Level 1 nodes: H, I = 2
    assert!(network.nodes.len() >= 9, "expected at least 9 nodes, got {}", network.nodes.len());

    // Arc count: 7 bidirectional (×2) + 2 one-way (×1) + 1 one-way roundabout (×1) = 17
    assert!(network.arcs.len() >= 17, "expected at least 17 arcs, got {}", network.arcs.len());
}

#[test]
fn test_routing_graph_fixture_arcs_nodes_ratio() {
    // Task 6.3: arcs/nodes ratio should be 2-3 for a road network
    use imgforge_cli::routing::graph_builder::build_road_network;

    let mp = MpParser::parse_file(&fixture("routing_graph.mp")).unwrap();
    let network = build_road_network(&mp.polylines);

    let ratio = network.arcs.len() as f64 / network.nodes.len() as f64;
    assert!(
        (1.5..=4.0).contains(&ratio),
        "arcs/nodes ratio should be ~2-3 for a road network, got {:.2} ({} arcs / {} nodes)",
        ratio,
        network.arcs.len(),
        network.nodes.len(),
    );
}

#[test]
fn test_routing_graph_fixture_roundabout_marked() {
    // Task 6.4: Roundabout road_def has roundabout=true
    use imgforge_cli::routing::graph_builder::build_road_network;

    let mp = MpParser::parse_file(&fixture("routing_graph.mp")).unwrap();
    let network = build_road_network(&mp.polylines);

    let roundabout_defs: Vec<_> = network.road_defs.iter().filter(|d| d.roundabout).collect();
    assert_eq!(roundabout_defs.len(), 1, "exactly 1 roundabout road_def expected");
    assert_eq!(roundabout_defs[0].road_id, 9);
}

#[test]
fn test_routing_graph_fixture_bridge_isolation() {
    // Task 6: Bridge (Level=1) does not share nodes with ground (Level=0)
    use imgforge_cli::routing::graph_builder::build_road_network;

    let mp = MpParser::parse_file(&fixture("routing_graph.mp")).unwrap();
    let network = build_road_network(&mp.polylines);

    let level0_nodes: Vec<_> = network.nodes.iter().filter(|n| n.level == 0).collect();
    let level1_nodes: Vec<_> = network.nodes.iter().filter(|n| n.level == 1).collect();
    assert_eq!(level1_nodes.len(), 2, "bridge has 2 endpoints at level=1");
    assert!(level0_nodes.len() >= 8, "ground network has at least 8 nodes");
}

#[test]
fn test_routing_graph_compile_no_crash() {
    // Task 5.5: Full compile pipeline with routing_graph.mp
    let mp = MpParser::parse_file(&fixture("routing_graph.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();
    assert_eq!(&bytes[0x002..0x008], b"GARMIN");
}

// ----------------------------------------------------------------
// Story 14.3: NET Writer integration tests
// ----------------------------------------------------------------

#[test]
fn test_net_validation_compile_produces_net_subfile() {
    // Task 6.1/5.4: Compile net_validation.mp → .img contains NET subfile
    let mp = MpParser::parse_file(&fixture("net_validation.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();

    assert!(bytes.len() > 512, "IMG output must be non-empty");
    assert_eq!(&bytes[0x002..0x008], b"GARMIN", "valid GARMIN header");

    // With routing, 4 subfiles: TRE(0), RGN(1), LBL(2), NET(3).
    // NET directory entry is at dir_start + 3*32.
    let dir_start = 512usize;
    let net_dirent = dir_start + 3 * 32;

    // Read size_used for NET subfile (offset 0x12 within dirent)
    let net_size = u32::from_le_bytes([
        bytes[net_dirent + 0x12],
        bytes[net_dirent + 0x13],
        bytes[net_dirent + 0x14],
        bytes[net_dirent + 0x15],
    ]);
    assert!(
        net_size > 55,
        "NET subfile must be > 55 bytes (header=55), got {}",
        net_size
    );
}

#[test]
fn test_net_validation_header_parsable() {
    // Task 5.4: NET header is parsable — signature "GARMIN NET" at correct offset
    let mp = MpParser::parse_file(&fixture("net_validation.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();

    // Find NET subfile start: dirent[3].block_start × block_size
    let dir_start = 512usize;
    let net_dirent = dir_start + 3 * 32;
    let block_start = u16::from_le_bytes([
        bytes[net_dirent + 0x0C],
        bytes[net_dirent + 0x0D],
    ]) as usize;
    let block_size = 512usize; // exponent=9 in ImgWriter::write
    let net_start = block_start * block_size;

    // NET header: "GARMIN NET" at offset 0x0B from subfile start
    assert_eq!(
        &bytes[net_start + 0x0B..net_start + 0x15],
        b"GARMIN NET",
        "NET subfile must contain GARMIN NET signature"
    );

    // NET1 length > 0
    let net1_len = u32::from_le_bytes([
        bytes[net_start + 0x19],
        bytes[net_start + 0x1A],
        bytes[net_start + 0x1B],
        bytes[net_start + 0x1C],
    ]);
    assert!(net1_len > 0, "NET1 section length must be > 0, got {}", net1_len);

    // NET3 length = 5 roads with labels × 3 bytes = 15 bytes
    let net3_len = u32::from_le_bytes([
        bytes[net_start + 0x2B],
        bytes[net_start + 0x2C],
        bytes[net_start + 0x2D],
        bytes[net_start + 0x2E],
    ]);
    assert_eq!(net3_len, 15, "5 labeled roads → 5 NET3 records × 3 bytes = 15");
}

#[test]
fn test_net_validation_net1_oneway_flag() {
    // Task 6.3: Verify NET1 flags — oneway road has NET_FLAG_ONEWAY set
    use imgforge_cli::img::net::NetWriter;
    use imgforge_cli::img::lbl::LblWriter;
    use imgforge_cli::routing::graph_builder::build_road_network;

    let mp = MpParser::parse_file(&fixture("net_validation.mp")).unwrap();
    let network = build_road_network(&mp.polylines);
    let lbl = LblWriter::build(&mp);
    let net = NetWriter::build(&network, &lbl.label_offsets, &[], &mp.polylines);

    // Road 3 (index 2): "Rue Victor Hugo", oneway=true
    // Its NET1 record flags should have NET_FLAG_ONEWAY (0x02) set
    assert!(net.road_offsets.len() >= 3);
    let offset = net.road_offsets[2] as usize;
    let flags = net.data[55 + offset + 3]; // header(55) + label(3) → flags at byte 3

    assert_ne!(flags & 0x02, 0, "oneway road must have NET_FLAG_ONEWAY (0x02) set, got 0x{:02X}", flags);
    assert_ne!(flags & 0x04, 0, "NET_FLAG_UNK1 (0x04) must always be set, got 0x{:02X}", flags);
    assert_ne!(flags & 0x40, 0, "NET_FLAG_NODINFO (0x40) must be set, got 0x{:02X}", flags);
}

#[test]
fn test_net_validation_net1_bidirectional_flag() {
    // Task 6.3: Bidirectional road does NOT have NET_FLAG_ONEWAY
    use imgforge_cli::img::net::NetWriter;
    use imgforge_cli::img::lbl::LblWriter;
    use imgforge_cli::routing::graph_builder::build_road_network;

    let mp = MpParser::parse_file(&fixture("net_validation.mp")).unwrap();
    let network = build_road_network(&mp.polylines);
    let lbl = LblWriter::build(&mp);
    let net = NetWriter::build(&network, &lbl.label_offsets, &[], &mp.polylines);

    // Road 1 (index 0): "A480", bidirectional (oneway=false)
    let offset = net.road_offsets[0] as usize;
    let flags = net.data[55 + offset + 3];

    assert_eq!(flags & 0x02, 0, "bidirectional road must NOT have NET_FLAG_ONEWAY, got 0x{:02X}", flags);
    assert_ne!(flags & 0x04, 0, "NET_FLAG_UNK1 must be set");
}

#[test]
fn test_net_validation_net1_road_length() {
    // Task 6.3: Verify road length encoding (metres / 4.8)
    use imgforge_cli::img::net::NetWriter;
    use imgforge_cli::img::lbl::LblWriter;
    use imgforge_cli::routing::graph_builder::build_road_network;

    let mp = MpParser::parse_file(&fixture("net_validation.mp")).unwrap();
    let network = build_road_network(&mp.polylines);
    let lbl = LblWriter::build(&mp);
    let net = NetWriter::build(&network, &lbl.label_offsets, &[], &mp.polylines);

    // Road 1: A480, coords (45.19, 5.72) → (45.19, 5.74)
    // Horizontal distance at lat=45.19° ≈ 0.02° × cos(45.19°) × 111320 ≈ 1568m
    // Raw = round(1568 / 4.8) ≈ 327
    let offset = net.road_offsets[0] as usize;
    let raw_len = u32::from_le_bytes([
        net.data[55 + offset + 4],
        net.data[55 + offset + 5],
        net.data[55 + offset + 6],
        0,
    ]);
    // Allow ±50 tolerance for haversine rounding
    assert!(
        (250..450).contains(&raw_len),
        "A480 road length raw ≈ 327 (1568m / 4.8), got {} ({}m)",
        raw_len,
        (raw_len as f64) * 4.8
    );
}

#[test]
fn test_net_validation_net3_sorted_by_name() {
    // Task 6.4: NET3 records must be sorted by route name
    use imgforge_cli::img::net::NetWriter;
    use imgforge_cli::img::lbl::LblWriter;
    use imgforge_cli::routing::graph_builder::build_road_network;

    let mp = MpParser::parse_file(&fixture("net_validation.mp")).unwrap();
    let network = build_road_network(&mp.polylines);
    let lbl = LblWriter::build(&mp);
    let net = NetWriter::build(&network, &lbl.label_offsets, &[], &mp.polylines);

    // 5 routable labeled roads → 5 NET3 records
    // Expected sort order (case-insensitive):
    //   A480, D1075, Rond-point des Alpes, Rue Victor Hugo, Zone Industrielle
    let net3_offset = u32::from_le_bytes([
        net.data[0x27], net.data[0x28], net.data[0x29], net.data[0x2A],
    ]) as usize;
    let net3_len = u32::from_le_bytes([
        net.data[0x2B], net.data[0x2C], net.data[0x2D], net.data[0x2E],
    ]) as usize;

    assert_eq!(net3_len, 15, "5 records × 3 bytes");

    // Extract NET1 offsets from NET3 records
    let mut net3_offsets = Vec::new();
    for i in 0..5 {
        let base = net3_offset + i * 3;
        let val = u32::from_le_bytes([net.data[base], net.data[base + 1], net.data[base + 2], 0]);
        net3_offsets.push(val & 0x3F_FFFF);
    }

    // The offsets should map to roads in alphabetical order:
    // A480 → road_offsets[0], D1075 → road_offsets[1], Rond-point → road_offsets[3],
    // Rue Victor Hugo → road_offsets[2], Zone Industrielle → road_offsets[4]
    assert_eq!(net3_offsets[0], net.road_offsets[0], "first NET3: A480");
    assert_eq!(net3_offsets[1], net.road_offsets[1], "second NET3: D1075");
    assert_eq!(net3_offsets[2], net.road_offsets[3], "third NET3: Rond-point des Alpes");
    assert_eq!(net3_offsets[3], net.road_offsets[2], "fourth NET3: Rue Victor Hugo");
    assert_eq!(net3_offsets[4], net.road_offsets[4], "fifth NET3: Zone Industrielle");
}

#[test]
fn test_net_validation_tre_routing_bit() {
    // Task 6.5: TRE data_flags bit 1 is set when routing is present
    let mp = MpParser::parse_file(&fixture("net_validation.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();

    // Find TRE subfile: first dirent → block_start × block_size
    let dir_start = 512usize;
    let tre_dirent = dir_start; // TRE is dirent 0
    let block_start = u16::from_le_bytes([
        bytes[tre_dirent + 0x0C],
        bytes[tre_dirent + 0x0D],
    ]) as usize;
    let tre_start = block_start * 512;

    // TRE header is 148 bytes. Levels section follows.
    // With 1 level: levels = 4 bytes, subdivisions = 16 bytes
    // First subdivision starts at tre_start + 148 + 4
    let subdiv_start = tre_start + 148 + 4;

    // data_flags at byte 0x03 within subdivision
    let data_flags = bytes[subdiv_start + 3];

    // bit 1 (0x02) = has_indexed_lines (routing)
    assert_ne!(
        data_flags & 0x02,
        0,
        "TRE data_flags bit 1 (has_indexed_lines) must be set when routing is present, got 0x{:02X}",
        data_flags
    );
}

#[test]
fn test_net_validation_no_regression() {
    // Task 6.6: Full test suite — compile routing_graph.mp (existing fixture) still works
    let mp = MpParser::parse_file(&fixture("routing_graph.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();
    assert_eq!(&bytes[0x002..0x008], b"GARMIN");
    assert!(bytes.len() > 512);
}

#[test]
fn test_net_validation_minimal_no_routing_still_works() {
    // Regression: minimal_for_img.mp (no routing) should still compile without NET subfile
    let mp = MpParser::parse_file(&fixture("minimal_for_img.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();
    assert_eq!(&bytes[0x002..0x008], b"GARMIN");

    // Only 3 subfile directory entries (TRE, RGN, LBL — no NET)
    let dir_start = 512usize;
    // Check that there's no 4th dirent with valid content
    // (the 4th dirent would be at dir_start + 3*32 = dir_start + 96)
    // If no NET, the 4th slot should be zero/unused
    let fourth_dirent_start = dir_start + 3 * 32;
    if fourth_dirent_start + 32 <= bytes.len() {
        let size_used = u32::from_le_bytes([
            bytes[fourth_dirent_start + 0x12],
            bytes[fourth_dirent_start + 0x13],
            bytes[fourth_dirent_start + 0x14],
            bytes[fourth_dirent_start + 0x15],
        ]);
        // If there's no NET subfile, this should be 0 or the 4th dirent doesn't exist
        assert_eq!(
            size_used, 0,
            "minimal_for_img.mp should have no NET subfile (4th dirent size_used should be 0)"
        );
    }
}

#[test]
fn test_net_validation_roundabout_preserved_in_road_def() {
    // L1: AC3 — roundabout flag preserved in RoadDef after graph building
    use imgforge_cli::routing::graph_builder::build_road_network;

    let mp = MpParser::parse_file(&fixture("net_validation.mp")).unwrap();
    let network = build_road_network(&mp.polylines);

    // Road 4 (index 3): "Rond-point des Alpes", Roundabout=1
    assert!(
        network.road_defs.len() >= 4,
        "expected at least 4 road_defs, got {}",
        network.road_defs.len()
    );
    assert!(
        network.road_defs[3].roundabout,
        "road_def[3] (Rond-point des Alpes) must have roundabout=true"
    );
    // Non-roundabout roads must NOT have the flag
    assert!(
        !network.road_defs[0].roundabout,
        "road_def[0] (A480) must not be a roundabout"
    );
}

#[test]
fn test_net_validation_toll_preserved_in_road_def() {
    // L2: toll flag preserved in RoadDef for later use by NOD writer
    use imgforge_cli::routing::graph_builder::build_road_network;

    let mp = MpParser::parse_file(&fixture("net_validation.mp")).unwrap();
    let network = build_road_network(&mp.polylines);

    // Road 1 (index 0): "A480", toll=true
    assert!(
        network.road_defs[0].toll,
        "road_def[0] (A480) must have toll=true"
    );
    // Road 2 (index 1): "D1075", toll=false
    assert!(
        !network.road_defs[1].toll,
        "road_def[1] (D1075) must have toll=false"
    );
}
