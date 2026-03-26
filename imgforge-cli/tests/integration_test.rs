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
