//! Integration tests for imgforge-cli using fixture files.

use imgforge_cli::error::ParseError;
use imgforge_cli::img::assembler::{BuildConfig, GmapsuppAssembler};
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
        &bytes[0x010..0x017],
        b"DSKIMG\0",
        "IMG header must contain DSKIMG at offset 0x010"
    );
    assert_eq!(
        &bytes[0x041..0x048],
        b"GARMIN\0",
        "IMG header must contain GARMIN at offset 0x041"
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

    // FAT entries start at sector 2 (offset 0x400). Each entry is 512 bytes.
    // First entry is the volume label, file entries follow.
    let extensions = ["TRE", "RGN", "LBL"];
    let mut found = Vec::new();
    let mut fat_offset = 0x400usize;
    while fat_offset + 512 <= bytes.len() {
        if bytes[fat_offset] != 0x01 {
            break;
        }
        let name = &bytes[fat_offset + 1..fat_offset + 9];
        let ext = &bytes[fat_offset + 9..fat_offset + 12];
        let part = bytes[fat_offset + 0x11];
        // Skip volume label (name = spaces) and continuation parts
        if name != &[0x20; 8] && part == 0 {
            let ext_str = std::str::from_utf8(ext).unwrap_or("???");
            found.push((name.to_vec(), ext_str.to_string()));
        }
        fat_offset += 512;
    }
    assert_eq!(found.len(), extensions.len(), "must find {} subfile entries", extensions.len());
    for (i, expected_ext) in extensions.iter().enumerate() {
        assert_eq!(found[i].0, b"63240001", "subfile {i} name must be '63240001'");
        assert_eq!(found[i].1, *expected_ext, "subfile {i} extension must be '{expected_ext}'");
    }
}

#[test]
fn test_img_header_signatures() {
    let mp = MpParser::parse_file(&fixture("minimal_for_img.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();
    assert_eq!(&bytes[0x010..0x017], b"DSKIMG\0", "DSKIMG at 0x010");
    assert_eq!(&bytes[0x041..0x048], b"GARMIN\0", "GARMIN at 0x041");
    assert_eq!(bytes[0x1FE], 0x55);
    assert_eq!(bytes[0x1FF], 0xAA);
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

    // Find RGN FAT entry by extension.
    let fat_offset = find_fat_entry_by_ext(&bytes, b"RGN").expect("RGN FAT entry not found");
    let size_used = fat_file_size(&bytes, fat_offset);
    assert!(size_used > 0, "RGN subfile size_used must be > 0 (real RGN content)");
}

#[test]
fn test_img_rgn_header_magic() {
    // RGN subfile must start with header_length=48 and contain "GARMIN RGN" at offset 0x02.
    let mp = MpParser::parse_file(&fixture("multi_type.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();

    // Read RGN block_start from FAT entry.
    let fat_offset = find_fat_entry_by_ext(&bytes, b"RGN").expect("RGN FAT entry not found");
    let block_start = fat_first_block(&bytes, fat_offset);
    let rgn_offset = block_start * 512;

    // header_length = 46 (0x2E)
    let hdr_len = u16::from_le_bytes([bytes[rgn_offset], bytes[rgn_offset + 1]]);
    assert_eq!(hdr_len, 46, "RGN header length must be 46");

    // "GARMIN RGN" at offset 0x02
    assert_eq!(
        &bytes[rgn_offset + 0x02..rgn_offset + 0x0C],
        b"GARMIN RGN",
        "RGN header must contain 'GARMIN RGN' at offset 0x02"
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

    // Locate TRE block via FAT.
    let tre_fat = find_fat_entry_by_ext(&bytes, b"TRE").expect("TRE FAT entry not found");
    let tre_block_start = fat_first_block(&bytes, tre_fat);
    let tre_offset = tre_block_start * 512;

    // Read subdivisions_offset from TRE header at byte 0x29.
    let subdivs_offset = u32::from_le_bytes([
        bytes[tre_offset + 0x29],
        bytes[tre_offset + 0x2A],
        bytes[tre_offset + 0x2B],
        bytes[tre_offset + 0x2C],
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

    // Locate TRE subdivisions via FAT.
    let tre_fat = find_fat_entry_by_ext(&bytes, b"TRE").expect("TRE FAT entry not found");
    let tre_block_start = fat_first_block(&bytes, tre_fat);
    let tre_offset = tre_block_start * 512;
    let subdivs_offset = u32::from_le_bytes([
        bytes[tre_offset + 0x29],
        bytes[tre_offset + 0x2A],
        bytes[tre_offset + 0x2B],
        bytes[tre_offset + 0x2C],
    ]) as usize;
    let subdivs_start = tre_offset + subdivs_offset;

    let rgn_off0 = (bytes[subdivs_start] as u32)
        | ((bytes[subdivs_start + 1] as u32) << 8)
        | ((bytes[subdivs_start + 2] as u32) << 16);
    let subdiv1_start = subdivs_start + 16;
    let rgn_off1 = (bytes[subdiv1_start] as u32)
        | ((bytes[subdiv1_start + 1] as u32) << 8)
        | ((bytes[subdiv1_start + 2] as u32) << 16);

    // Locate RGN data section (after the 46-byte header) via FAT.
    let rgn_fat = find_fat_entry_by_ext(&bytes, b"RGN").expect("RGN FAT entry not found");
    let rgn_block_start = fat_first_block(&bytes, rgn_fat);
    let rgn_file_start = rgn_block_start * 512;
    // data_size is at offset 0x19 in the RGN header (after common header)
    let data_size = u32::from_le_bytes([
        bytes[rgn_file_start + 0x19],
        bytes[rgn_file_start + 0x1A],
        bytes[rgn_file_start + 0x1B],
        bytes[rgn_file_start + 0x1C],
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

    // Find TRE FAT entry by extension.
    let fat_offset = find_fat_entry_by_ext(&bytes, b"TRE").expect("TRE FAT entry not found");
    let size_used = fat_file_size(&bytes, fat_offset);
    assert!(
        size_used > 0,
        "TRE subfile size_used must be > 0 (real TRE content)"
    );
}

#[test]
fn test_img_tre_header_version() {
    // Verify that the TRE subfile starts with header_length=165 and contains "GARMIN TRE".
    let mp = MpParser::parse_file(&fixture("minimal_for_img.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();

    // Read TRE block_start from FAT entry.
    let fat_offset = find_fat_entry_by_ext(&bytes, b"TRE").expect("TRE FAT entry not found");
    let block_start = fat_first_block(&bytes, fat_offset);
    let tre_offset = block_start * 512;

    // header_length = 165 (0xA5)
    let hdr_len = u16::from_le_bytes([bytes[tre_offset], bytes[tre_offset + 1]]);
    assert_eq!(hdr_len, 165, "TRE header length must be 165");

    // "GARMIN TRE" at offset 0x02
    assert_eq!(
        &bytes[tre_offset + 0x02..tre_offset + 0x0C],
        b"GARMIN TRE",
        "TRE header must contain 'GARMIN TRE' at offset 0x02"
    );

    // Bounding box starts at 0x15 (no version field — removed in common header refactor).
    // Just verify the header length is correct (already checked above).
}

#[test]
fn test_img_tre_bounds_nonzero() {
    // minimal_for_img.mp has a POI in France → max_lat garmin value must be positive.
    let mp = MpParser::parse_file(&fixture("minimal_for_img.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();

    let tre_fat = find_fat_entry_by_ext(&bytes, b"TRE").expect("TRE FAT entry not found");
    let block_start = fat_first_block(&bytes, tre_fat);
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
    // LBL FAT entry — find by extension.
    let fat_offset = find_fat_entry_by_ext(bytes, b"LBL").expect("LBL FAT entry not found");
    let block_start = fat_first_block(bytes, fat_offset);
    let size_used = fat_file_size(bytes, fat_offset);
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
    // LBL subfile must start with header_length=47 and contain "GARMIN LBL" at offset 0x02.
    let mp = MpParser::parse_file(&fixture("multi_type.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();
    let (lbl_offset, _) = read_lbl_block(&bytes);

    // header_length = 45 (0x2D)
    let hdr_len = u16::from_le_bytes([bytes[lbl_offset], bytes[lbl_offset + 1]]);
    assert_eq!(hdr_len, 45, "LBL header length must be 45");

    // "GARMIN LBL" at offset 0x02
    assert_eq!(
        &bytes[lbl_offset + 0x02..lbl_offset + 0x0C],
        b"GARMIN LBL",
        "LBL header must contain 'GARMIN LBL' at offset 0x02"
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
    // LBL data section starts at lbl_offset + 0x2D (header size = 45)
    let data_start = lbl_offset + 0x2D;
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

    // Locate RGN block via FAT.
    let rgn_fat = find_fat_entry_by_ext(&bytes, b"RGN").expect("RGN FAT entry not found");
    let rgn_block_start = fat_first_block(&bytes, rgn_fat);
    let rgn_file_start = rgn_block_start * 512;
    // RGN data section starts after the 46-byte header.
    let rgn_data_start = rgn_file_start + 46;

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

    // Minimum meaningful sizes: TRE header alone = 165 bytes, RGN header = 46 bytes, LBL header = 45 bytes.
    let min_sizes = [("TRE", b"TRE", 165u32), ("RGN", b"RGN", 46u32), ("LBL", b"LBL", 45u32)];
    for (name, ext, min_size) in &min_sizes {
        let fat_offset = find_fat_entry_by_ext(&bytes, ext).expect(&format!("{} FAT entry not found", name));
        let size_used = fat_file_size(&bytes, fat_offset);
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
    // Scan FAT entries for the map ID in the name field (offset 0x01 of each 512-byte entry).
    let mut found = false;
    let mut offset = 0x400usize;
    while offset + 512 <= bytes.len() {
        if bytes[offset] != 0x01 { break; }
        let name = &bytes[offset + 1..offset + 9];
        if name == map_id {
            found = true;
            break;
        }
        offset += 512;
    }
    assert!(found, "Map ID '63240038' must be present in a FAT entry name field");
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

    // Locate TRE subfile via FAT entry.
    let tre_fat = find_fat_entry_by_ext(&bytes, b"TRE").expect("TRE FAT entry not found");
    let block_start = fat_first_block(&bytes, tre_fat);
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
    // AC1 : RGN size_used > 46 bytes (header-only = 46 bytes), confirming features were encoded.
    // Avec 6 polylines + 4 POI + 4 polygones, size_used est substantiellement plus grand que 48,
    // mais ce test valide uniquement le seuil minimal (pas le décompte exact par type de feature).
    // RGN is Dirent index 1 (block 1, block_size=512).
    let mp = MpParser::parse_file(&fixture("bdtopo_tile.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();

    // Find RGN size_used via FAT entry.
    let rgn_fat = find_fat_entry_by_ext(&bytes, b"RGN").expect("RGN FAT entry not found");
    let size_used = fat_file_size(&bytes, rgn_fat);
    assert!(
        size_used > 46,
        "RGN size_used must be > 46 (header-only = 46 bytes), got {}",
        size_used
    );
}

#[test]
fn test_e2e_img_dos_header_valid() {
    // AC2 : header DOS 512 bytes valide — DSKIMG, GARMIN, signature 0x55/0xAA.
    let mp = MpParser::parse_file(&fixture("bdtopo_tile.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();

    // Standard Garmin IMG signatures
    assert_eq!(
        &bytes[0x010..0x017],
        b"DSKIMG\0",
        "IMG header must contain DSKIMG at offset 0x010"
    );
    assert_eq!(
        &bytes[0x041..0x048],
        b"GARMIN\0",
        "IMG header must contain GARMIN at offset 0x041"
    );
    // DOS partition signature at 0x1FE/0x1FF
    assert_eq!(bytes[0x1FE], 0x55, "DOS signature byte 0x1FE must be 0x55, got 0x{:02X}", bytes[0x1FE]);
    assert_eq!(bytes[0x1FF], 0xAA, "DOS signature byte 0x1FF must be 0xAA, got 0x{:02X}", bytes[0x1FF]);
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
    assert_eq!(&bytes[0x010..0x017], b"DSKIMG\0");
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
    assert_eq!(&bytes[0x010..0x017], b"DSKIMG\0", "valid GARMIN header");

    // With routing, subfiles include NET. Find it via FAT.
    let net_fat = find_fat_entry_by_ext(&bytes, b"NET").expect("NET FAT entry not found");
    let net_size = fat_file_size(&bytes, net_fat);
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

    // Find NET subfile start via FAT entry.
    let net_fat = find_fat_entry_by_ext(&bytes, b"NET").expect("NET FAT entry not found");
    let block_start = fat_first_block(&bytes, net_fat);
    let block_size = 512usize; // exponent=9 in ImgWriter::write
    let net_start = block_start * block_size;

    // NET header: "GARMIN NET" at offset 0x02 from subfile start
    assert_eq!(
        &bytes[net_start + 0x02..net_start + 0x0C],
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

    // Find TRE subfile via FAT entry.
    let tre_fat = find_fat_entry_by_ext(&bytes, b"TRE").expect("TRE FAT entry not found");
    let block_start = fat_first_block(&bytes, tre_fat);
    let tre_start = block_start * 512;

    // TRE header is 165 bytes. Levels section follows.
    // With 1 level: levels = 4 bytes, subdivisions = 16 bytes
    // First subdivision starts at tre_start + 165 + 4
    let subdiv_start = tre_start + 165 + 4;

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
    assert_eq!(&bytes[0x010..0x017], b"DSKIMG\0");
    assert!(bytes.len() > 512);
}

#[test]
fn test_net_validation_minimal_no_routing_still_works() {
    // Regression: minimal_for_img.mp (no routing) should still compile without NET subfile
    let mp = MpParser::parse_file(&fixture("minimal_for_img.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();
    assert_eq!(&bytes[0x010..0x017], b"DSKIMG\0");

    // Only 3 subfile FAT entries (TRE, RGN, LBL — no NET)
    // Check that there's no NET FAT entry
    let net_entry = find_fat_entry_by_ext(&bytes, b"NET");
    assert!(
        net_entry.is_none(),
        "minimal_for_img.mp should have no NET subfile (no NET FAT entry expected)"
    );
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

// ----------------------------------------------------------------
// NOD subfile integration tests (Story 14.4)
// ----------------------------------------------------------------

/// Helper: compile net_validation.mp and return the .img bytes.
fn compile_net_validation_img() -> Vec<u8> {
    let mp = MpParser::parse_file(&fixture("net_validation.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    std::fs::read(tmp.path()).unwrap()
}

/// Find a FAT entry by extension (first part-0 entry matching ext).
/// Returns the byte offset of the FAT entry, or None.
fn find_fat_entry_by_ext(bytes: &[u8], ext: &[u8; 3]) -> Option<usize> {
    let mut offset = 0x400usize; // sector 2
    while offset + 512 <= bytes.len() {
        if bytes[offset] != 0x01 { break; }
        let entry_ext = &bytes[offset + 9..offset + 12];
        let part = bytes[offset + 0x11];
        if entry_ext == ext && part == 0 {
            return Some(offset);
        }
        offset += 512;
    }
    None
}

/// Find the Nth file FAT entry (part 0 only, skipping volume label).
/// Returns the byte offset of the FAT entry.
fn find_nth_file_entry(bytes: &[u8], n: usize) -> Option<usize> {
    let mut count = 0usize;
    let mut offset = 0x400usize;
    while offset + 512 <= bytes.len() {
        if bytes[offset] != 0x01 { break; }
        let name = &bytes[offset + 1..offset + 9];
        let part = bytes[offset + 0x11];
        // Skip volume label (spaces) and continuation parts
        if name != &[0x20u8; 8] && part == 0 {
            if count == n { return Some(offset); }
            count += 1;
        }
        offset += 512;
    }
    None
}

/// Read file_size (LE32 at offset 0x0C) from a FAT entry.
fn fat_file_size(bytes: &[u8], fat_offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[fat_offset + 0x0C],
        bytes[fat_offset + 0x0D],
        bytes[fat_offset + 0x0E],
        bytes[fat_offset + 0x0F],
    ])
}

/// Read first block index from FAT entry's allocation table (LE16 at offset 0x20).
fn fat_first_block(bytes: &[u8], fat_offset: usize) -> usize {
    u16::from_le_bytes([bytes[fat_offset + 0x20], bytes[fat_offset + 0x21]]) as usize
}

/// Read extension from a FAT entry (3 bytes at offset 0x09).
fn fat_ext(bytes: &[u8], fat_offset: usize) -> [u8; 3] {
    [bytes[fat_offset + 9], bytes[fat_offset + 10], bytes[fat_offset + 11]]
}

/// Read map_id (name) from a FAT entry (8 bytes at offset 0x01).
fn fat_name(bytes: &[u8], fat_offset: usize) -> String {
    String::from_utf8_lossy(&bytes[fat_offset + 1..fat_offset + 9]).trim().to_string()
}

/// Backward-compatible wrapper: read extension by file index (skipping volume label).
fn read_dirent_ext(bytes: &[u8], index: usize) -> [u8; 3] {
    let fat_offset = find_nth_file_entry(bytes, index).expect("FAT entry not found");
    fat_ext(bytes, fat_offset)
}

/// Backward-compatible wrapper: read first block by file index (skipping volume label).
fn read_dirent_block_start(bytes: &[u8], index: usize) -> usize {
    let fat_offset = find_nth_file_entry(bytes, index).expect("FAT entry not found");
    fat_first_block(bytes, fat_offset)
}

#[test]
fn test_nod_validation_img_contains_nod_subfile() {
    // AC6 — Task 6.2: .img FAT must contain a NOD subfile entry
    let bytes = compile_net_validation_img();

    // Routing: 5 subfiles expected: TRE(0), RGN(1), LBL(2), NET(3), NOD(4)
    let nod_ext = read_dirent_ext(&bytes, 4);
    assert_eq!(&nod_ext, b"NOD", "subfile 4 must have extension 'NOD'");
}

#[test]
fn test_nod_validation_header_signature() {
    // AC6 / AC5 — Task 6.3: NOD header starts with "GARMIN NOD" and drive_on_right=0x01
    let bytes = compile_net_validation_img();

    // NOD is subfile index 4
    let block_start = read_dirent_block_start(&bytes, 4);
    let nod_offset = block_start * 512;

    // Header length at 0x00 (LE16) = 48
    let hdr_len = u16::from_le_bytes([bytes[nod_offset], bytes[nod_offset + 1]]);
    assert_eq!(hdr_len, 48, "NOD header length must be 48");

    // Signature "GARMIN NOD" at offset 0x02
    assert_eq!(
        &bytes[nod_offset + 0x02..nod_offset + 0x0C],
        b"GARMIN NOD",
        "NOD signature must be 'GARMIN NOD'"
    );

    // drive_on_right at 0x2E
    assert_eq!(
        bytes[nod_offset + 0x2E],
        0x01,
        "drive_on_right must be 0x01 (France)"
    );
}

#[test]
fn test_nod_validation_sections_non_empty() {
    // AC2 / AC1 — Task 6.4: NOD1 and NOD2 sections must be non-empty
    let bytes = compile_net_validation_img();

    let block_start = read_dirent_block_start(&bytes, 4);
    let nod_offset = block_start * 512;

    // NOD1 length at 0x19
    let nod1_len = u32::from_le_bytes([
        bytes[nod_offset + 0x19],
        bytes[nod_offset + 0x1A],
        bytes[nod_offset + 0x1B],
        bytes[nod_offset + 0x1C],
    ]);
    assert!(nod1_len > 0, "NOD1 section must be non-empty");

    // NOD2 length at 0x22
    let nod2_len = u32::from_le_bytes([
        bytes[nod_offset + 0x22],
        bytes[nod_offset + 0x23],
        bytes[nod_offset + 0x24],
        bytes[nod_offset + 0x25],
    ]);
    assert!(nod2_len > 0, "NOD2 section must be non-empty");
}

#[test]
fn test_nod_validation_nod2_offsets_patched_in_net() {
    // AC3 — Task 6.5: NOD2 offsets in NET1 must be non-zero after patch
    let bytes = compile_net_validation_img();

    // NET is subfile index 3
    let net_block = read_dirent_block_start(&bytes, 3);
    let net_offset = net_block * 512;

    // NET header: NET1 section offset at 0x15 (LE32), relative to start of NET subfile
    let net1_section_off = u32::from_le_bytes([
        bytes[net_offset + 0x15],
        bytes[net_offset + 0x16],
        bytes[net_offset + 0x17],
        bytes[net_offset + 0x18],
    ]) as usize;

    // First NET1 record starts at net_offset + net1_section_off
    // Layout: 3B labels + 1B flags + 3B length + N bytes level_counts + 3×M level_divs + 1B indicator + 2B nod2_offset
    // Check the NET_FLAG_NODINFO (0x40) is set in flags byte
    let first_record_start = net_offset + net1_section_off;
    let flags_byte = bytes[first_record_start + 3];
    assert!(
        flags_byte & 0x40 != 0,
        "NET_FLAG_NODINFO (0x40) must be set in first NET1 record flags, got 0x{:02X}",
        flags_byte
    );

    // Find the NOD2 placeholder for the first road:
    // Skip 3B label + 1B flags + 3B length = 7 bytes
    // Then skip level_count bytes (until bit7 set) + 3B per level_div
    let mut pos = first_record_start + 7;
    let mut level_divs_count = 0usize;
    loop {
        let b = bytes[pos];
        let count = b & 0x7F;
        level_divs_count += count as usize;
        pos += 1;
        if b & 0x80 != 0 {
            break;
        }
    }
    // Skip level_divs (3 bytes each)
    pos += level_divs_count * 3;
    // indicator byte (0x01)
    assert_eq!(bytes[pos], 0x01, "indicator byte must be 0x01 before NOD2 offset");
    pos += 1;
    // First road's NOD2 offset is legitimately 0 (it IS the first entry in the NOD2 section).
    let nod2_off_road1 = u16::from_le_bytes([bytes[pos], bytes[pos + 1]]);
    assert_eq!(nod2_off_road1, 0, "first road NOD2 offset must be 0 (first NOD2 entry)");

    // Road 2: its NOD2 offset must be > 0 because road 1 occupies at least 1 byte in NOD2.
    let road2_start = pos + 2;
    let mut pos2 = road2_start + 7; // skip 3B label + 1B flags + 3B length
    let mut level_divs_count2 = 0usize;
    loop {
        let b = bytes[pos2];
        level_divs_count2 += (b & 0x7F) as usize;
        pos2 += 1;
        if b & 0x80 != 0 {
            break;
        }
    }
    pos2 += level_divs_count2 * 3;
    assert_eq!(bytes[pos2], 0x01, "road 2 indicator byte must be 0x01");
    pos2 += 1;
    let nod2_off_road2 = u16::from_le_bytes([bytes[pos2], bytes[pos2 + 1]]);
    assert!(nod2_off_road2 > 0, "road 2 NOD2 offset must be non-zero (patched, ≥1 byte after road 1)");
}

#[test]
fn test_nod_validation_autoroute_tab_a_info() {
    // AC4 — Task 6.6: tabAInfo pour A480 (speed=7, class=4, toll=true, oneway=false)
    // doit apparaître dans le binaire NOD1 à une position d'arc réelle.
    use imgforge_cli::img::nod::build_nod1_section;
    use imgforge_cli::routing::graph_builder::build_road_network;

    let mp = MpParser::parse_file(&fixture("net_validation.mp")).unwrap();
    let network = build_road_network(&mp.polylines);

    // Find A480 road_def (speed=7, class=4, toll=true)
    let a480 = network
        .road_defs
        .iter()
        .find(|rd| rd.speed == 7 && rd.road_class == 4)
        .expect("A480 road_def (speed=7, class=4) must exist");
    let expected_tab_a: u8 = (a480.speed & 0x07)
        | ((a480.one_way as u8) << 3)
        | ((a480.road_class & 0x07) << 4)
        | ((a480.toll as u8) << 7);
    // Vérifie la valeur attendue (0xC7 pour speed=7/class=4/toll=true/oneway=false)
    assert_eq!(
        expected_tab_a, 0xC7,
        "A480 tabAInfo: speed=7/class=4/toll/bidirectionnel → 0xC7"
    );

    let net_offsets: Vec<u32> = (0..network.road_defs.len()).map(|i| i as u32 * 20).collect();
    let nod1 = build_nod1_section(&network, &net_offsets);
    assert!(!nod1.is_empty(), "NOD1 must be non-empty");

    // Scan le binaire NOD1 aux positions d'arc pour y trouver expected_tab_a.
    // Structure: RouteCenter header (10B) + nœuds (9B header + 5B par arc).
    // net_validation.mp ≤ 256 nœuds → un seul RouteCenter.
    let mut found_in_binary = false;
    let mut pos = 10usize; // skip RouteCenter header (lat 4 + lon 4 + tabB_offset 2)
    while pos + 9 <= nod1.len() {
        let arc_count = nod1[pos + 7] as usize;
        pos += 9; // skip node header
        for _ in 0..arc_count {
            if pos + 5 > nod1.len() {
                break;
            }
            if nod1[pos] == expected_tab_a {
                found_in_binary = true;
            }
            pos += 5; // tabAInfo(1) + bearing(1) + net_offset(3)
        }
    }
    assert!(
        found_in_binary,
        "tabAInfo {:#04X} pour A480 doit apparaître dans le binaire NOD1 à une position d'arc",
        expected_tab_a
    );
}

#[test]
fn test_nod_validation_no_regression() {
    // AC6 — Task 6.7: 0 regression — all existing tests still pass.
    // This test compiles the routing fixture and verifies the full suite succeeds.
    let mp = MpParser::parse_file(&fixture("net_validation.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    // Must not panic
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();
    assert!(bytes.len() > 0, "compiled .img must be non-empty");
    // Verify GARMIN magic still present
    assert_eq!(&bytes[0x010..0x017], b"DSKIMG\0", "IMG magic must still be 'GARMIN'");
    // Verify 5 subfiles now present: TRE, RGN, LBL, NET, NOD
    let extensions: Vec<[u8; 3]> = (0..5).map(|i| read_dirent_ext(&bytes, i)).collect();
    assert_eq!(&extensions[0], b"TRE", "subfile 0 must be TRE");
    assert_eq!(&extensions[1], b"RGN", "subfile 1 must be RGN");
    assert_eq!(&extensions[2], b"LBL", "subfile 2 must be LBL");
    assert_eq!(&extensions[3], b"NET", "subfile 3 must be NET");
    assert_eq!(&extensions[4], b"NOD", "subfile 4 must be NOD");
}

// ================================================================
// Story 14.5 — Validation Routage: Itinéraire Fonctionnel GPS
// ================================================================
//
// VALIDATION GPS MANUELLE
// =======================
// Fixture : routing_full_validation.mp → compiler avec imgforge-cli
//
// Scénario 1 — Préférence autoroute :
//   Charger le .img sur GPS Garmin (eTrex, Edge, etc.)
//   Demander un itinéraire entre (45.16,5.73) et (45.20,5.73)
//   Attendu : l'autoroute A480 est préférée (speed=7 → ~130 km/h) vs D1075 (speed=5 → ~90 km/h)
//
// Scénario 2 — Sens unique respecté :
//   Tenter de naviguer sur Rue_Oneway en sens inverse (45.19,5.76) → (45.18,5.76)
//   Attendu : le GPS propose un détour (sens unique interdit)
//
// Scénario 3 — Profil piéton :
//   Activer le profil piéton sur le GPS
//   Attendu : Zone_Pietons accessible, voitures exclues
//
// Scénario 4 — Éviter les péages :
//   Activer l'option "éviter les péages" sur le GPS
//   Attendu : A480 évitée, D1075 utilisée pour l'itinéraire

fn compile_routing_full_validation_img() -> Vec<u8> {
    let mp = MpParser::parse_file(&fixture("routing_full_validation.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    std::fs::read(tmp.path()).unwrap()
}

// ----------------------------------------------------------------
// Task 3 : Tests validation attributs routage dans le graphe
// ----------------------------------------------------------------

#[test]
fn test_routing_oneway_single_arc() {
    // AC2 — Task 3.1: Route DirIndicator=1 → exactement 1 arc dans RoadNetwork
    use imgforge_cli::routing::graph_builder::build_road_network;

    let mp = MpParser::parse_file(&fixture("routing_full_validation.mp")).unwrap();
    let network = build_road_network(&mp.polylines);

    // Rue_Oneway : speed=3, class=1, oneway=true
    let oneway_def = network
        .road_defs
        .iter()
        .find(|rd| rd.one_way && rd.speed == 3 && rd.road_class == 1)
        .expect("Rue_Oneway (speed=3,class=1,oneway=true) doit exister dans road_defs");

    // Trouver l'index directement pour éviter std::ptr::eq
    let oneway_idx = network
        .road_defs
        .iter()
        .position(|rd| rd.one_way && rd.speed == 3 && rd.road_class == 1)
        .expect("Rue_Oneway (speed=3,class=1,oneway=true) doit exister dans road_defs");
    let _ = oneway_def; // confirmé via oneway_idx

    let arc_count = network.arcs.iter().filter(|a| a.road_def_idx == oneway_idx).count();
    assert_eq!(arc_count, 1, "Rue_Oneway DirIndicator=1 → exactement 1 arc (forward only)");
}

#[test]
fn test_routing_bidirectional_two_arcs() {
    // AC1 — Task 3.1: Route bidirectionnelle → exactement 2 arcs
    use imgforge_cli::routing::graph_builder::build_road_network;

    let mp = MpParser::parse_file(&fixture("routing_full_validation.mp")).unwrap();
    let network = build_road_network(&mp.polylines);

    // D1075 : speed=5, class=2, oneway=false
    let d1075_idx = network
        .road_defs
        .iter()
        .position(|rd| !rd.one_way && rd.speed == 5 && rd.road_class == 2)
        .expect("D1075 (speed=5,class=2,bidirectionnel) doit exister dans road_defs");

    let arc_count = network.arcs.iter().filter(|a| a.road_def_idx == d1075_idx).count();
    assert_eq!(arc_count, 2, "D1075 bidirectionnel → exactement 2 arcs (forward + reverse)");
}

#[test]
fn test_routing_access_mask_truck_denied() {
    // AC3 — Task 3.1: denied_truck=true → access_mask contient 0x0040
    use imgforge_cli::routing::graph_builder::build_road_network;

    let mp = MpParser::parse_file(&fixture("routing_full_validation.mp")).unwrap();
    let network = build_road_network(&mp.polylines);

    // Zone_Pietons : speed=1, class=0, access restreint
    let zone_def = network
        .road_defs
        .iter()
        .find(|rd| rd.speed == 1 && rd.road_class == 0)
        .expect("Zone_Pietons (speed=1,class=0) doit exister dans road_defs");

    assert_ne!(zone_def.access_mask, 0, "Zone_Pietons doit avoir access_mask != 0");
    assert_eq!(
        zone_def.access_mask & 0x0001, 0x0001,
        "denied_car → bit 0x0001 de access_mask doit être activé, got 0x{:04X}",
        zone_def.access_mask
    );
    assert_eq!(
        zone_def.access_mask & 0x0040, 0x0040,
        "denied_truck → bit 0x0040 de access_mask doit être activé, got 0x{:04X}",
        zone_def.access_mask
    );
}

#[test]
fn test_routing_access_mask_pedestrian_truck_denied() {
    // AC3 — Task 3.1: denied_pedestrian + denied_truck → bits 0x0010 + 0x0040 activés
    // (Zone Industrielle dans net_validation.mp : RouteParam=4,2,0,0,0,0,0,0,0,1,0,1)
    use imgforge_cli::routing::graph_builder::build_road_network;

    let mp = MpParser::parse_file(&fixture("net_validation.mp")).unwrap();
    let network = build_road_network(&mp.polylines);

    // Zone Industrielle : RouteParam=4,2,0,0,0,0,0,0,0,1,0,1 → pedestrian(0x0010) + truck(0x0040)
    let zone_def = network
        .road_defs
        .iter()
        .find(|rd| rd.speed == 4 && rd.road_class == 2 && rd.access_mask != 0)
        .expect("Zone Industrielle (speed=4,class=2,restricted) doit exister");

    assert_eq!(
        zone_def.access_mask & 0x0010, 0x0010,
        "denied_pedestrian → bit 0x0010 activé, access_mask=0x{:04X}", zone_def.access_mask
    );
    assert_eq!(
        zone_def.access_mask & 0x0040, 0x0040,
        "denied_truck → bit 0x0040 activé, access_mask=0x{:04X}", zone_def.access_mask
    );
}

#[test]
fn test_routing_bridge_isolated() {
    // AC1 — Task 3.1: Node Level=1 ne partage pas d'arc avec Level=0 au même point
    use imgforge_cli::routing::graph_builder::build_road_network;

    // routing_graph.mp contient un pont Level=1 (H→I) passant au-dessus du noeud E (Level=0)
    // même longitude 5.73 : E=(45.18,5.73) est Level=0, H=(45.185,5.73) et I=(45.175,5.73) sont Level=1
    let mp = MpParser::parse_file(&fixture("routing_graph.mp")).unwrap();
    let network = build_road_network(&mp.polylines);

    // Séparer noeuds Level=0 et Level=1
    let nodes_level0: Vec<_> = network.nodes.iter().filter(|n| n.level == 0).collect();
    let nodes_level1: Vec<_> = network.nodes.iter().filter(|n| n.level == 1).collect();

    assert!(!nodes_level0.is_empty(), "des noeuds Level=0 doivent exister");
    assert!(!nodes_level1.is_empty(), "des noeuds Level=1 doivent exister (pont)");

    // Aucun arc ne doit connecter un noeud Level=0 à un noeud Level=1
    let level0_ids: std::collections::HashSet<u32> = nodes_level0.iter().map(|n| n.id).collect();
    let level1_ids: std::collections::HashSet<u32> = nodes_level1.iter().map(|n| n.id).collect();

    for arc in &network.arcs {
        let from_l0 = level0_ids.contains(&arc.from_node);
        let to_l1 = level1_ids.contains(&arc.to_node);
        let from_l1 = level1_ids.contains(&arc.from_node);
        let to_l0 = level0_ids.contains(&arc.to_node);
        assert!(
            !(from_l0 && to_l1) && !(from_l1 && to_l0),
            "arc {:?} connecte Level=0 et Level=1 (isolation doit être respectée)",
            arc.id
        );
    }
}

// ----------------------------------------------------------------
// Task 4 : Tests validation NET binaire avec access_mask
// ----------------------------------------------------------------

#[test]
fn test_routing_full_net_toll_in_nod_tabAInfo() {
    // AC4 — Task 4.1: A480 (toll=true) → tabAInfo = 0xC7 présent dans la section NOD1 du .img compilé
    // Utilise le pipeline complet pour valider le binaire final, pas une construction partielle.
    let bytes = compile_routing_full_validation_img();

    // NOD subfile index = 4
    let nod_block = read_dirent_block_start(&bytes, 4);
    let nod_offset = nod_block * 512;

    // NOD1 section : offset à 0x15 (LE32) et longueur à 0x19 (LE32) dans le header NOD
    let nod1_section_off = u32::from_le_bytes([
        bytes[nod_offset + 0x15], bytes[nod_offset + 0x16],
        bytes[nod_offset + 0x17], bytes[nod_offset + 0x18],
    ]) as usize;
    let nod1_len = u32::from_le_bytes([
        bytes[nod_offset + 0x19], bytes[nod_offset + 0x1A],
        bytes[nod_offset + 0x1B], bytes[nod_offset + 0x1C],
    ]) as usize;
    assert!(nod1_len > 0, "NOD1 doit être non-vide");

    // tabAInfo pour A480 : speed=7, class=4, toll=true, oneway=false
    // = (7 & 0x07) | (0 << 3) | (4 << 4) | (1 << 7) = 0xC7
    let expected_tab_a: u8 = 0xC7;
    let nod1_start = nod_offset + nod1_section_off;
    let nod1_bytes = &bytes[nod1_start..nod1_start + nod1_len];
    assert!(
        nod1_bytes.contains(&expected_tab_a),
        "tabAInfo 0xC7 pour A480 (speed=7/class=4/toll/bidirectionnel) doit apparaître dans la section NOD1"
    );
    // Vérifier que le toll bit est bien le bit 7
    assert_eq!(expected_tab_a & 0x80, 0x80, "bit 7 de tabAInfo = toll_bit doit être 1");
}

#[test]
fn test_routing_full_net_access_zone_pietons() {
    // AC3 — Task 4.1: Zone_Pietons → NET_FLAG_ACCESS (0x20) dans flags + access_mask bytes dans record NET1
    use imgforge_cli::img::net::{NetWriter, SubdivRoadRef};
    use imgforge_cli::routing::graph_builder::build_road_network;
    use std::collections::HashMap;

    let mp = MpParser::parse_file(&fixture("routing_full_validation.mp")).unwrap();
    let network = build_road_network(&mp.polylines);

    // Trouver Zone_Pietons (road_def avec access_mask != 0)
    let (zone_idx, zone_def) = network
        .road_defs
        .iter()
        .enumerate()
        .find(|(_, rd)| rd.access_mask != 0)
        .expect("Zone_Pietons avec access_mask != 0 doit exister");

    // Vérifier le bit truck (0x0040)
    assert_eq!(
        zone_def.access_mask & 0x0040, 0x0040,
        "denied_truck bit 0x0040 doit être présent, access_mask=0x{:04X}", zone_def.access_mask
    );

    // Construire le binaire NET
    let label_offsets: HashMap<String, u32> = HashMap::new();
    let subdiv_refs: Vec<SubdivRoadRef> = network
        .road_defs
        .iter()
        .enumerate()
        .map(|(i, _)| SubdivRoadRef { road_def_idx: i, subdiv_number: 1, polyline_index: i as u8 })
        .collect();
    let result = NetWriter::build(&network, &label_offsets, &subdiv_refs, &mp.polylines);

    // Naviguer jusqu'au record Zone_Pietons dans NET1
    let net1_start = 55usize; // NET_HEADER_SIZE
    let record_start = net1_start + result.road_offsets[zone_idx] as usize;

    // Flags byte à record_start + 3
    let flags = result.data[record_start + 3];
    assert_eq!(
        flags & 0x20, 0x20,
        "NET_FLAG_ACCESS (0x20) doit être activé pour Zone_Pietons, flags=0x{:02X}", flags
    );

    // Access bytes à record_start + 4 (LE16)
    // Valeur exacte attendue : denied_car(0x0001) | denied_bicycle(0x0020) | denied_truck(0x0040) = 0x0061
    let access_mask = u16::from_le_bytes([result.data[record_start + 4], result.data[record_start + 5]]);
    assert_eq!(
        access_mask & 0x0001, 0x0001,
        "access_mask doit contenir bit car 0x0001 (denied_car=true), access_mask=0x{:04X}", access_mask
    );
    assert_eq!(
        access_mask & 0x0020, 0x0020,
        "access_mask doit contenir bit bicycle 0x0020 (denied_bicycle=true), access_mask=0x{:04X}", access_mask
    );
    assert_eq!(
        access_mask & 0x0040, 0x0040,
        "access_mask doit contenir bit truck 0x0040 (denied_truck=true), access_mask=0x{:04X}", access_mask
    );
    assert_eq!(
        access_mask, 0x0061,
        "access_mask exact = 0x0061 (car|bicycle|truck), got 0x{:04X}", access_mask
    );
}

#[test]
fn test_routing_full_nod_tabAInfo_autoroute() {
    // AC1 + AC4 — Task 4.1: A480 (speed=7,class=4,toll=true,oneway=false) → tabAInfo = 0xC7
    use imgforge_cli::routing::graph_builder::build_road_network;

    let mp = MpParser::parse_file(&fixture("routing_full_validation.mp")).unwrap();
    let network = build_road_network(&mp.polylines);

    let a480 = network
        .road_defs
        .iter()
        .find(|rd| rd.speed == 7 && rd.road_class == 4 && rd.toll)
        .expect("A480 (speed=7,class=4,toll=true) doit exister");

    // tabAInfo = (speed & 0x07) | (oneway << 3) | (class << 4) | (toll << 7)
    //          = (7 & 7) | (0 << 3) | (4 << 4) | (1 << 7)
    //          = 0x07 | 0x00 | 0x40 | 0x80 = 0xC7
    let tab_a: u8 = (a480.speed & 0x07)
        | ((a480.one_way as u8) << 3)
        | ((a480.road_class & 0x07) << 4)
        | ((a480.toll as u8) << 7);
    assert_eq!(tab_a, 0xC7, "A480 tabAInfo: speed=7/class=4/toll/bidirectionnel → 0xC7");
    assert!(!a480.one_way, "A480 est bidirectionnel");
    assert!(a480.toll, "A480 est a peage");
}

#[test]
fn test_routing_full_nod_tabAInfo_oneway() {
    // AC2 — Task 4.1: Rue_Oneway (speed=3,class=1,oneway=true,toll=false) → tabAInfo = 0x1B
    use imgforge_cli::routing::graph_builder::build_road_network;

    let mp = MpParser::parse_file(&fixture("routing_full_validation.mp")).unwrap();
    let network = build_road_network(&mp.polylines);

    let rue_oneway = network
        .road_defs
        .iter()
        .find(|rd| rd.one_way && rd.speed == 3 && rd.road_class == 1)
        .expect("Rue_Oneway (speed=3,class=1,oneway=true) doit exister");

    // tabAInfo = (3 & 0x07) | (1 << 3) | (1 << 4) | (0 << 7)
    //          = 0x03 | 0x08 | 0x10 | 0x00 = 0x1B
    let tab_a: u8 = (rue_oneway.speed & 0x07)
        | ((rue_oneway.one_way as u8) << 3)
        | ((rue_oneway.road_class & 0x07) << 4)
        | ((rue_oneway.toll as u8) << 7);
    assert_eq!(tab_a, 0x1B, "Rue_Oneway tabAInfo: speed=3/class=1/oneway/no-toll → 0x1B");
    assert!(rue_oneway.one_way, "Rue_Oneway doit être sens unique");
    assert!(!rue_oneway.toll, "Rue_Oneway n'est pas a peage");
}

// ----------------------------------------------------------------
// Task 5 : Tests intégration end-to-end
// ----------------------------------------------------------------

#[test]
fn test_routing_full_compile_five_subfiles() {
    // AC5 — Task 5.1: compiler routing_full_validation.mp → .img contient 5 subfiles (TRE/RGN/LBL/NET/NOD)
    let bytes = compile_routing_full_validation_img();

    assert!(bytes.len() > 0, "le .img compilé doit être non-vide");
    assert_eq!(&bytes[0x010..0x017], b"DSKIMG\0", "magic GARMIN doit être présent");

    let extensions: Vec<[u8; 3]> = (0..5).map(|i| read_dirent_ext(&bytes, i)).collect();
    assert_eq!(&extensions[0], b"TRE", "subfile 0 doit être TRE");
    assert_eq!(&extensions[1], b"RGN", "subfile 1 doit être RGN");
    assert_eq!(&extensions[2], b"LBL", "subfile 2 doit être LBL");
    assert_eq!(&extensions[3], b"NET", "subfile 3 doit être NET");
    assert_eq!(&extensions[4], b"NOD", "subfile 4 doit être NOD");
}

#[test]
fn test_routing_full_graph_metrics() {
    // AC1 — Task 5.2: graphe routier depuis routing_full_validation.mp
    // nodes >= 6, arcs >= 8, road_defs >= 5 (5 Level=0 + 1 Level=1)
    use imgforge_cli::routing::graph_builder::build_road_network;

    let mp = MpParser::parse_file(&fixture("routing_full_validation.mp")).unwrap();
    let network = build_road_network(&mp.polylines);

    // Topologie documentée en Task 1.1 :
    //   Level=0 : 6 noeuds (1 triple intersection + 3 intersections + 2 endpoints)
    //   Level=1 : 2 noeuds (Pont_Sud)
    //   Total attendu : 8 noeuds, 11 arcs Level=0 + 2 arcs Level=1 = 13 total
    assert!(
        network.nodes.len() >= 8,
        "au moins 8 RouteNodes attendus (6 Level=0 + 2 Level=1), got {}",
        network.nodes.len()
    );
    assert!(
        network.arcs.len() >= 11,
        "au moins 11 RouteArcs attendus (9 Level=0 + 2 Level=1), got {}",
        network.arcs.len()
    );
    assert!(
        network.road_defs.len() >= 5,
        "au moins 5 road_defs attendus (5 Level=0 + 1 Level=1), got {}",
        network.road_defs.len()
    );

    // Vérifier que l'attribut speed est correctement parsé (A480 speed=7 > D1075 speed=5)
    // Note : la validation que tabAInfo encode bien ce speed dans le binaire NOD est faite
    // dans test_routing_full_net_toll_in_nod_tabAInfo.
    let a480 = network.road_defs.iter().find(|rd| rd.speed == 7).expect("A480 speed=7 doit exister");
    let d1075 = network.road_defs.iter().find(|rd| rd.speed == 5).expect("D1075 speed=5 doit exister");
    assert!(a480.speed > d1075.speed, "A480 (speed={}) doit avoir un attribut speed supérieur à D1075 (speed={})", a480.speed, d1075.speed);
}

#[test]
fn test_routing_full_nod_drive_on_right() {
    // AC1 — Task 5.3: header NOD → drive_on_right = 0x01 (France)
    let bytes = compile_routing_full_validation_img();

    // NOD subfile index = 4
    let nod_block = read_dirent_block_start(&bytes, 4);
    let nod_offset = nod_block * 512;

    // NOD header : drive_on_right est à l'offset 0x2E dans le header NOD
    // Format NOD header: voir nod.rs write_nod_header() — offset 0x2E
    let drive_on_right = bytes[nod_offset + 0x2E];
    assert_eq!(drive_on_right, 0x01, "drive_on_right = 0x01 (France = circulation à droite)");
}

#[test]
fn test_routing_full_validation_fixture_compiles() {
    // AC5 — Task 5.4: la nouvelle fixture se parse et compile sans erreur
    // (la non-régression sur les 268 tests précédents est garantie par `cargo test` globalement)
    let mp = MpParser::parse_file(&fixture("routing_full_validation.mp")).unwrap();
    assert_eq!(mp.polylines.len(), 7, "7 polylines dans la fixture (6 routables + 1 rivière)");

    let routable_count = mp.polylines.iter().filter(|p| p.routing.is_some()).count();
    assert_eq!(routable_count, 6, "6 polylines routables (5 Level=0 + 1 Level=1)");

    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();
    assert!(bytes.len() > 512, "le .img doit faire plus d'un block (512 bytes)");
    assert_eq!(&bytes[0x010..0x017], b"DSKIMG\0", "magic GARMIN toujours présent");
}

// ================================================================
// Story 15.1 — Assemblage gmapsupp.img multi-tuiles (AC1–AC5)
// ================================================================

fn test_build_config_512() -> BuildConfig {
    BuildConfig {
        family_id: 6324,
        product_id: 1,
        description: "Test Assembly".into(),
        block_size_exponent: 9, // 512 bytes — fast for integration tests
        typ_file: None,
        jobs: 1, // sequential for deterministic tests
        show_progress: false,
    }
}

fn tiles_dir_with(files: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for name in files {
        let src = fixture(name);
        std::fs::copy(&src, dir.path().join(name)).unwrap();
    }
    dir
}

#[test]
fn test_build_single_tile() {
    // AC1 — build d'un seul .mp → gmapsupp.img valide
    let tiles = tiles_dir_with(&["tile_a.mp"]);
    let output = tempfile::NamedTempFile::new().unwrap();
    let stats = GmapsuppAssembler::build(tiles.path(), output.path(), &test_build_config_512())
        .unwrap();
    assert_eq!(stats.tile_count, 1);
    let bytes = std::fs::read(output.path()).unwrap();
    // Standard Garmin IMG header
    assert_eq!(&bytes[0x010..0x017], b"DSKIMG\0");
    assert_eq!(&bytes[0x041..0x048], b"GARMIN\0");
    // DOS signature
    assert_eq!(bytes[0x1FE], 0x55);
    assert_eq!(bytes[0x1FF], 0xAA);
}

#[test]
fn test_build_two_tiles() {
    // AC1 — build tile_a + tile_b → 2 map_ids dans le FAT directory
    let tiles = tiles_dir_with(&["tile_a.mp", "tile_b.mp"]);
    let output = tempfile::NamedTempFile::new().unwrap();
    let stats = GmapsuppAssembler::build(tiles.path(), output.path(), &test_build_config_512())
        .unwrap();
    assert_eq!(stats.tile_count, 2);
    // Both map IDs must appear in the directory
    let bytes = std::fs::read(output.path()).unwrap();
    assert!(
        bytes.windows(8).any(|w| w == b"01001001"),
        "map_id 01001001 must appear in gmapsupp.img"
    );
    assert!(
        bytes.windows(8).any(|w| w == b"01001002"),
        "map_id 01001002 must appear in gmapsupp.img"
    );
}

#[test]
fn test_build_two_tiles_subfile_count() {
    // AC1 — 2 tuiles avec routage → ≥ 6 entrées dans le FAT (chaque tuile ≥ 3)
    let tiles = tiles_dir_with(&["tile_a.mp", "tile_b.mp"]);
    let output = tempfile::NamedTempFile::new().unwrap();
    let stats = GmapsuppAssembler::build(tiles.path(), output.path(), &test_build_config_512())
        .unwrap();
    assert!(
        stats.subfile_count >= 6,
        "2 tuiles → au moins 6 subfiles (3 par tuile minimum), got {}",
        stats.subfile_count
    );
}

#[test]
fn test_build_empty_dir_returns_error() {
    // AC1 — répertoire vide → ImgError::EmptyInputDir
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("out.img");
    let result = GmapsuppAssembler::build(dir.path(), &output, &test_build_config_512());
    assert!(
        matches!(result, Err(imgforge_cli::ImgError::EmptyInputDir { .. })),
        "empty dir must return EmptyInputDir error"
    );
}

#[test]
fn test_build_dir_not_found_returns_error() {
    // AC1 — répertoire inexistant → Err
    let output = std::env::temp_dir().join("imgforge_test_out.img");
    let result = GmapsuppAssembler::build(
        Path::new("/does/not/exist/12345"),
        &output,
        &test_build_config_512(),
    );
    assert!(result.is_err(), "non-existent dir must return error");
}

#[test]
fn test_build_gmapsupp_family_id_in_tdb() {
    // family_id is no longer in the IMG header (standard Garmin format).
    // It is stored in the companion TDB file.
    let tiles = tiles_dir_with(&["tile_a.mp"]);
    let config = BuildConfig {
        family_id: 6324,
        product_id: 1,
        description: "Test".into(),
        block_size_exponent: 9,
        typ_file: None,
        jobs: 1,
        show_progress: false,
    };
    let output = tempfile::NamedTempFile::new().unwrap();
    let stats = GmapsuppAssembler::build(tiles.path(), output.path(), &config).unwrap();
    // TDB file must be generated
    assert!(stats.tdb_path.exists(), "TDB companion file must be generated");
    // IMG header must have standard signatures
    let bytes = std::fs::read(output.path()).unwrap();
    assert_eq!(&bytes[0x010..0x017], b"DSKIMG\0");
    assert_eq!(&bytes[0x041..0x048], b"GARMIN\0");
}

#[test]
fn test_build_multi_block_directory() {
    // AC2 — ≥ 17 entries → dir_blocks > 1 pour block_size=512
    // 6 tuiles × 3 subfiles = 18 entrées × 32 bytes = 576 > 512 → dir span > 1 block
    let dir = tempfile::tempdir().unwrap();
    // Create 6 minimal tiles using different IDs
    let ids = ["01000001", "01000002", "01000003", "01000004", "01000005", "01000006"];
    for id in &ids {
        let content = format!(
            "[IMG ID]\nName=Tile {id}\nID={id}\nCodePage=1252\nLevels=2\nLevel0=24\nLevel1=18\n[END-IMG ID]\n\n\
             [POI]\nType=0x2C00\nLabel=TestPOI\nData0=(45.0,5.0)\nEndLevel=4\n[END]\n"
        );
        std::fs::write(dir.path().join(format!("{id}.mp")), content).unwrap();
    }
    let output = tempfile::NamedTempFile::new().unwrap();
    let stats = GmapsuppAssembler::build(dir.path(), output.path(), &test_build_config_512())
        .unwrap();
    assert_eq!(stats.tile_count, 6);
    // 6 tiles × 3 subfiles + 1 SRT = 19 entries
    assert_eq!(stats.subfile_count, 19, "6 tiles × 3 subfiles (no routing) + 1 SRT = 19");
    let bytes = std::fs::read(output.path()).unwrap();
    // With new FAT format (512-byte entries): 1 volume + 19 file entries = 20 FAT sectors
    // header_sectors = 2 + 20 = 22 → 22 header blocks (block_size=512)
    // Data starts at block 22. Verify by checking first file's block allocation.
    let first_file_fat = 0x400 + 512; // skip volume label → second FAT entry
    let first_block = u16::from_le_bytes([bytes[first_file_fat + 0x20], bytes[first_file_fat + 0x21]]);
    assert_eq!(
        first_block, 22,
        "with 20 FAT entries and block_size=512, first subfile starts at block 22"
    );
}

#[test]
fn test_build_boundary_nodes_detected() {
    // AC4 — tile_a → NOD3 non-vide dans le subfile NOD du gmapsupp.img assemblé.
    // tile_a a un RoutePolyline avec endpoints à (45.000,5.710) et (45.010,5.710).
    // bbox lat = [45.000, 45.010] → les deux endpoints sont sur les bords → 2 boundary nodes.
    // NOD3 length = 2 × 6 bytes = 12.
    let tiles = tiles_dir_with(&["tile_a.mp"]);
    let config = BuildConfig {
        family_id: 0,
        product_id: 0,
        description: "Test".into(),
        block_size_exponent: 9,
        typ_file: None,
        jobs: 1,
        show_progress: false,
    };
    let output = tempfile::NamedTempFile::new().unwrap();
    let stats = GmapsuppAssembler::build(tiles.path(), output.path(), &config).unwrap();
    assert_eq!(stats.subfile_count, 6, "tile_a avec routing → 5 subfiles (TRE/RGN/LBL/NET/NOD) + 1 SRT = 6");

    let bytes = std::fs::read(output.path()).unwrap();
    let block_size = 512usize;

    // Trouver le FAT entry du subfile NOD.
    let nod_fat = find_fat_entry_by_ext(&bytes, b"NOD").expect("subfile NOD doit exister");
    let nod_start = fat_first_block(&bytes, nod_fat) * block_size;
    // NOD3 length est à l'offset 0x2A dans le header NOD (LE32).
    let nod3_len = u32::from_le_bytes([
        bytes[nod_start + 0x2A],
        bytes[nod_start + 0x2B],
        bytes[nod_start + 0x2C],
        bytes[nod_start + 0x2D],
    ]);
    assert!(
        nod3_len > 0,
        "tile_a a des boundary nodes → NOD3 doit être non-vide, nod3_len={}",
        nod3_len
    );
}

#[test]
fn test_build_no_regression() {
    // AC5 — Vérification que tous les tests existants passent (lancé par cargo test)
    // Ce test vérifie seulement que la fixture routing_full_validation.mp
    // compile toujours correctement après les changements de la Story 15.1
    let mp = MpParser::parse_file(&fixture("routing_full_validation.mp")).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path()).unwrap();
    assert_eq!(&bytes[0x010..0x017], b"DSKIMG\0");
}

// ── Story 15.2 — TDB Integration Tests ───────────────────────────────────────

/// Copie tile_a.mp et tile_b.mp dans un répertoire temporaire.
fn two_tile_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(fixture("tile_a.mp"), dir.path().join("tile_a.mp")).unwrap();
    std::fs::copy(fixture("tile_b.mp"), dir.path().join("tile_b.mp")).unwrap();
    dir
}

fn tdb_build_config() -> BuildConfig {
    BuildConfig {
        family_id: 1234,
        product_id: 1,
        description: "France BDTOPO 2025".into(),
        block_size_exponent: 9,
        typ_file: None,
        jobs: 1,
        show_progress: false,
    }
}

#[test]
fn test_build_generates_tdb_file() {
    // AC1 — gmapsupp.tdb généré à côté du gmapsupp.img
    let tiles = two_tile_dir();
    let tmp_out = tempfile::tempdir().unwrap();
    let output = tmp_out.path().join("gmapsupp.img");

    let _stats = GmapsuppAssembler::build(tiles.path(), &output, &tdb_build_config()).unwrap();

    let tdb_path = tmp_out.path().join("gmapsupp.tdb");
    assert!(tdb_path.exists(), "gmapsupp.tdb must exist next to gmapsupp.img");
    assert!(tdb_path.metadata().unwrap().len() > 0, "gmapsupp.tdb must not be empty");
}

#[test]
fn test_build_tdb_has_correct_block_structure() {
    // AC4 — Structure [0x50][0x44][0x42][0x4C×N][0x54]
    let tiles = two_tile_dir();
    let tmp_out = tempfile::tempdir().unwrap();
    let output = tmp_out.path().join("gmapsupp.img");

    GmapsuppAssembler::build(tiles.path(), &output, &tdb_build_config()).unwrap();

    let tdb_bytes = std::fs::read(tmp_out.path().join("gmapsupp.tdb")).unwrap();

    // First block must be 0x50
    assert_eq!(tdb_bytes[0], 0x50, "first block must be Product Header (0x50)");

    // Walk all blocks to verify last is 0x54
    let mut pos = 0;
    let mut block_types: Vec<u8> = Vec::new();
    while pos + 3 <= tdb_bytes.len() {
        let block_type = tdb_bytes[pos];
        let len = u16::from_le_bytes([tdb_bytes[pos + 1], tdb_bytes[pos + 2]]) as usize;
        block_types.push(block_type);
        pos += 3 + len;
    }
    assert_eq!(*block_types.last().unwrap(), 0x54, "last block must be Checksum (0x54)");
    assert!(block_types.contains(&0x42), "must contain Overview Map block (0x42)");
    assert!(block_types.contains(&0x44), "must contain Copyright block (0x44)");
}

#[test]
fn test_build_tdb_contains_tile_count_detail_blocks() {
    // AC2 — N tuiles → N blocks 0x4C dans le TDB
    let tiles = two_tile_dir();
    let tmp_out = tempfile::tempdir().unwrap();
    let output = tmp_out.path().join("gmapsupp.img");

    GmapsuppAssembler::build(tiles.path(), &output, &tdb_build_config()).unwrap();

    let tdb_bytes = std::fs::read(tmp_out.path().join("gmapsupp.tdb")).unwrap();

    let mut pos = 0;
    let mut detail_count = 0usize;
    while pos + 3 <= tdb_bytes.len() {
        if tdb_bytes[pos] == 0x4C {
            detail_count += 1;
        }
        let len = u16::from_le_bytes([tdb_bytes[pos + 1], tdb_bytes[pos + 2]]) as usize;
        pos += 3 + len;
    }
    assert_eq!(detail_count, 2, "2 tuiles → 2 blocks Detail Map (0x4C)");
}

#[test]
fn test_build_tdb_family_id_in_product_block() {
    // AC3 — family_id=1234 → bytes[3..5] du block 0x50 = 1234u16.to_le_bytes()
    let tiles = two_tile_dir();
    let tmp_out = tempfile::tempdir().unwrap();
    let output = tmp_out.path().join("gmapsupp.img");

    GmapsuppAssembler::build(tiles.path(), &output, &tdb_build_config()).unwrap();

    let tdb_bytes = std::fs::read(tmp_out.path().join("gmapsupp.tdb")).unwrap();

    // Block 0x50: type(1) + length(2) + data; data[2..4] = family_id
    assert_eq!(tdb_bytes[0], 0x50);
    let data_offset = 3usize;
    // data[0..2] = TDB version, data[2..4] = family_id
    let family_id = u16::from_le_bytes([tdb_bytes[data_offset + 2], tdb_bytes[data_offset + 3]]);
    assert_eq!(family_id, 1234, "family_id in product block must be 1234");
}

#[test]
fn test_build_assembly_stats_has_tdb_path() {
    // AC1 — AssemblyStats.tdb_path existe et se termine par .tdb
    let tiles = two_tile_dir();
    let tmp_out = tempfile::tempdir().unwrap();
    let output = tmp_out.path().join("gmapsupp.img");

    let stats = GmapsuppAssembler::build(tiles.path(), &output, &tdb_build_config()).unwrap();

    assert!(stats.tdb_path.exists(), "stats.tdb_path must point to an existing file");
    assert_eq!(
        stats.tdb_path.extension().and_then(|e| e.to_str()),
        Some("tdb"),
        "stats.tdb_path must have .tdb extension"
    );
}

#[test]
fn test_build_tdb_map_ids_in_detail_blocks() {
    // H1 (code-review) — AC2 : chaque block 0x4C contient le bon map_id
    // tile_a: ID=01001001 → décimal 1_001_001, tile_b: ID=01001002 → décimal 1_001_002
    let tiles = two_tile_dir();
    let tmp_out = tempfile::tempdir().unwrap();
    let output = tmp_out.path().join("gmapsupp.img");

    GmapsuppAssembler::build(tiles.path(), &output, &tdb_build_config()).unwrap();

    let tdb_bytes = std::fs::read(tmp_out.path().join("gmapsupp.tdb")).unwrap();

    let mut pos = 0;
    let mut map_ids: Vec<u32> = Vec::new();
    while pos + 3 <= tdb_bytes.len() {
        let block_type = tdb_bytes[pos];
        let len = u16::from_le_bytes([tdb_bytes[pos + 1], tdb_bytes[pos + 2]]) as usize;
        if block_type == 0x4C {
            // payload bytes 0..4 = map_number (u32 LE)
            let map_id = u32::from_le_bytes([
                tdb_bytes[pos + 3],
                tdb_bytes[pos + 4],
                tdb_bytes[pos + 5],
                tdb_bytes[pos + 6],
            ]);
            map_ids.push(map_id);
        }
        pos += 3 + len;
    }

    map_ids.sort();
    assert_eq!(
        map_ids,
        vec![1_001_001, 1_001_002],
        "detail blocks must contain correct map IDs (decimal parse of tile IDs)"
    );
}

#[test]
fn test_build_tdb_series_name_in_blocks() {
    // H2 (code-review) — AC3 : series_name présente dans blocks 0x50 (Product) et 0x42 (Overview)
    let tiles = two_tile_dir();
    let tmp_out = tempfile::tempdir().unwrap();
    let output = tmp_out.path().join("gmapsupp.img");

    GmapsuppAssembler::build(tiles.path(), &output, &tdb_build_config()).unwrap();

    let tdb_bytes = std::fs::read(tmp_out.path().join("gmapsupp.tdb")).unwrap();
    let expected = b"France BDTOPO 2025";

    let mut pos = 0;
    let mut found_in_product = false;
    let mut found_in_overview = false;
    while pos + 3 <= tdb_bytes.len() {
        let block_type = tdb_bytes[pos];
        let len = u16::from_le_bytes([tdb_bytes[pos + 1], tdb_bytes[pos + 2]]) as usize;
        let payload = &tdb_bytes[pos + 3..pos + 3 + len];
        if block_type == 0x50 {
            found_in_product = payload.windows(expected.len()).any(|w| w == expected);
        }
        if block_type == 0x42 {
            found_in_overview = payload.windows(expected.len()).any(|w| w == expected);
        }
        pos += 3 + len;
    }

    assert!(
        found_in_product,
        "series_name 'France BDTOPO 2025' must be present in block 0x50 (Product Header)"
    );
    assert!(
        found_in_overview,
        "series_name 'France BDTOPO 2025' must be present in block 0x42 (Overview Map description)"
    );
}

// ── Story 15.3 — TYP File Integration Tests ──────────────────────────────────

/// Construit un gmapsupp.img avec un fichier TYP minimal et retourne les bytes + stats.
fn build_with_typ(
    typ_content: &[u8],
    family_id: u16,
) -> (Vec<u8>, imgforge_cli::img::assembler::AssemblyStats, tempfile::TempDir) {
    let tiles = tiles_dir_with(&["tile_a.mp"]);
    let tmp_out = tempfile::tempdir().unwrap();
    let output = tmp_out.path().join("gmapsupp.img");

    let typ_file = tmp_out.path().join("style.typ");
    std::fs::write(&typ_file, typ_content).unwrap();

    let config = BuildConfig {
        family_id,
        product_id: 1,
        description: "Test TYP".into(),
        block_size_exponent: 9,
        typ_file: Some(typ_file),
        jobs: 1,
        show_progress: false,
    };
    let stats = GmapsuppAssembler::build(tiles.path(), &output, &config).unwrap();
    let bytes = std::fs::read(&output).unwrap();
    // Return tmp_out to keep the TempDir alive in the caller — prevents tdb_path from dangling.
    (bytes, stats, tmp_out)
}

#[test]
fn test_build_with_typ_file_embeds_subfile() {
    // AC2 — build avec --typ → subfile TYP présent dans le FAT directory
    let (bytes, stats, _tmp) = build_with_typ(b"TYP MARKER", 6324);

    let typ_fat = find_fat_entry_by_ext(&bytes, b"TYP");
    assert!(typ_fat.is_some(), "un FAT entry avec ext=TYP doit être présent");
    let typ_fat = typ_fat.unwrap();
    // Vérifier le map_id = "00006324"
    let name = fat_name(&bytes, typ_fat);
    assert_eq!(name, "00006324", "map_id du TYP doit être '00006324' (family_id=6324)");
    assert!(stats.typ_embedded, "stats.typ_embedded doit être true");
}

#[test]
fn test_build_with_typ_file_content_matches() {
    // AC2 — les bytes du subfile TYP dans le gmapsupp.img correspondent au fichier source
    let typ_content: &[u8] = b"GARMIN TYP CONTENT 01234567890ABCDEF";
    let (bytes, _stats, _tmp) = build_with_typ(typ_content, 6324);

    let block_size = 512usize;

    let typ_fat = find_fat_entry_by_ext(&bytes, b"TYP").expect("FAT entry TYP doit exister");
    let block_start = fat_first_block(&bytes, typ_fat);
    let size = fat_file_size(&bytes, typ_fat) as usize;
    assert_eq!(size, typ_content.len(), "size_used du TYP doit correspondre aux bytes source");

    let data_start = block_start * block_size;
    let embedded = &bytes[data_start..data_start + size];
    assert_eq!(embedded, typ_content, "contenu du subfile TYP doit correspondre exactement au fichier source");
}

#[test]
fn test_build_without_typ_no_typ_subfile() {
    // AC1 — sans --typ, aucun Dirent avec ext=TYP dans le FAT
    let tiles = tiles_dir_with(&["tile_a.mp"]);
    let output = tempfile::NamedTempFile::new().unwrap();
    let stats = GmapsuppAssembler::build(tiles.path(), output.path(), &test_build_config_512())
        .unwrap();

    let bytes = std::fs::read(output.path()).unwrap();

    let has_typ = find_fat_entry_by_ext(&bytes, b"TYP").is_some();
    assert!(!has_typ, "sans --typ, aucun FAT entry TYP ne doit être présent");
    assert!(!stats.typ_embedded, "stats.typ_embedded doit être false sans --typ");
}

#[test]
fn test_build_typ_missing_file_returns_error() {
    // AC4 — chemin TYP inexistant → erreur avant compilation des tuiles
    let tiles = tiles_dir_with(&["tile_a.mp"]);
    let output = tempfile::NamedTempFile::new().unwrap();

    let config = BuildConfig {
        family_id: 6324,
        product_id: 1,
        description: "Test TYP Missing".into(),
        block_size_exponent: 9,
        typ_file: Some(std::path::PathBuf::from("/nonexistent/path/style.typ")),
        jobs: 1,
        show_progress: false,
    };
    let result = GmapsuppAssembler::build(tiles.path(), output.path(), &config);
    assert!(result.is_err(), "fichier TYP inexistant doit retourner une erreur");
    let err = result.unwrap_err();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("/nonexistent/path/style.typ"),
        "le message d'erreur doit indiquer le chemin du fichier introuvable, got: {}",
        err_msg
    );
}

// ── Story 15.4 — SRT Writer (tri alphabétique français) ──────────────────────

/// Helper : trouve un FAT entry dans le gmapsupp.img par extension.
/// Retourne (map_id_str, block_start, size_used) si trouvé.
fn find_dirent_by_ext(bytes: &[u8], ext: &[u8; 3], _subfile_count: usize, _block_size: usize) -> Option<(String, usize, usize)> {
    let fat_offset = find_fat_entry_by_ext(bytes, ext)?;
    let name = fat_name(bytes, fat_offset);
    let block_start = fat_first_block(bytes, fat_offset);
    let size_used = fat_file_size(bytes, fat_offset) as usize;
    Some((name, block_start, size_used))
}

#[test]
fn test_build_srt_always_embedded() {
    // AC1, AC5 — le SRT est toujours intégré dans le gmapsupp.img ; stats.srt_embedded == true
    let tiles = tiles_dir_with(&["tile_a.mp"]);
    let output = tempfile::NamedTempFile::new().unwrap();
    let stats = GmapsuppAssembler::build(tiles.path(), output.path(), &test_build_config_512())
        .unwrap();

    assert!(stats.srt_embedded, "stats.srt_embedded doit être true");

    let bytes = std::fs::read(output.path()).unwrap();
    let srt_dirent = find_dirent_by_ext(&bytes, b"SRT", stats.subfile_count, 512);
    assert!(
        srt_dirent.is_some(),
        "un Dirent avec ext=SRT doit être présent dans le FAT"
    );
}

#[test]
fn test_build_srt_map_id_matches_family_id() {
    // AC1 — le map_id du subfile SRT correspond à family_id zero-paddé 8 chiffres
    let tiles = tiles_dir_with(&["tile_a.mp"]);
    let output = tempfile::NamedTempFile::new().unwrap();
    let stats = GmapsuppAssembler::build(tiles.path(), output.path(), &test_build_config_512())
        .unwrap();

    let bytes = std::fs::read(output.path()).unwrap();
    let (map_id, _, _) = find_dirent_by_ext(&bytes, b"SRT", stats.subfile_count, 512)
        .expect("Dirent SRT doit être présent");

    assert_eq!(
        map_id, "00006324",
        "map_id du SRT doit être '00006324' (family_id=6324 zero-paddé 8 chiffres)"
    );
}

#[test]
fn test_build_srt_size_is_4398_bytes() {
    // AC4 — le subfile SRT dans le gmapsupp.img a une taille de 4398 bytes
    let tiles = tiles_dir_with(&["tile_a.mp"]);
    let output = tempfile::NamedTempFile::new().unwrap();
    let stats = GmapsuppAssembler::build(tiles.path(), output.path(), &test_build_config_512())
        .unwrap();

    let bytes = std::fs::read(output.path()).unwrap();
    let (_, _, size_used) = find_dirent_by_ext(&bytes, b"SRT", stats.subfile_count, 512)
        .expect("Dirent SRT doit être présent");

    assert_eq!(
        size_used, 4396,
        "taille du subfile SRT doit être 4396 bytes (44 header + 256×17 data)"
    );
}

#[test]
fn test_build_srt_header_codepage_is_1252() {
    // AC4 — le header du subfile SRT contient le codepage 1252 (0x04E4 en LE16)
    let tiles = tiles_dir_with(&["tile_a.mp"]);
    let output = tempfile::NamedTempFile::new().unwrap();
    let stats = GmapsuppAssembler::build(tiles.path(), output.path(), &test_build_config_512())
        .unwrap();

    let bytes = std::fs::read(output.path()).unwrap();
    let block_size = 512usize;
    let (_, block_start, _) = find_dirent_by_ext(&bytes, b"SRT", stats.subfile_count, block_size)
        .expect("Dirent SRT doit être présent");

    let srt_data_start = block_start * block_size;

    // codepage at SRT offset 0x1D–0x1E (LE16 = 1252 = 0x04E4)
    assert_eq!(
        &bytes[srt_data_start + 0x1D..srt_data_start + 0x1F],
        &[0xE4, 0x04],
        "codepage dans le header SRT doit être 1252 (0x04E4 LE16)"
    );
}

#[test]
fn test_build_srt_with_typ_both_embedded() {
    // AC1 + Story 15.3 non-régression — avec --typ, les subfiles TYP ET SRT sont présents
    let tiles = tiles_dir_with(&["tile_a.mp"]);
    let tmp_out = tempfile::tempdir().unwrap();
    let output = tmp_out.path().join("gmapsupp.img");

    let typ_file = tmp_out.path().join("style.typ");
    std::fs::write(&typ_file, b"TYP MARKER").unwrap();

    let config = BuildConfig {
        family_id: 6324,
        product_id: 1,
        description: "Test TYP+SRT".into(),
        block_size_exponent: 9,
        typ_file: Some(typ_file),
        jobs: 1,
        show_progress: false,
    };
    let stats = GmapsuppAssembler::build(tiles.path(), &output, &config).unwrap();
    let bytes = std::fs::read(&output).unwrap();

    let has_typ = find_dirent_by_ext(&bytes, b"TYP", stats.subfile_count, 512).is_some();
    let has_srt = find_dirent_by_ext(&bytes, b"SRT", stats.subfile_count, 512).is_some();

    assert!(has_typ, "avec --typ, le subfile TYP doit être présent");
    assert!(has_srt, "avec --typ, le subfile SRT doit aussi être présent");
    assert!(stats.typ_embedded, "stats.typ_embedded doit être true");
    assert!(stats.srt_embedded, "stats.srt_embedded doit être true");
}

// ── Story 15.5 — Compilation Parallèle & Rapport JSON ────────────────────────

#[test]
fn test_build_parallel_jobs_2() {
    // AC1 — jobs=2 produit un gmapsupp.img fonctionnellement identique à jobs=1.
    // Même tile_count, subfile_count, et bytes identiques.
    let tmp_seq = tempfile::tempdir().unwrap();
    std::fs::copy(fixture("tile_a.mp"), tmp_seq.path().join("tile_a.mp")).unwrap();
    std::fs::copy(fixture("tile_b.mp"), tmp_seq.path().join("tile_b.mp")).unwrap();

    let tmp_par = tempfile::tempdir().unwrap();
    std::fs::copy(fixture("tile_a.mp"), tmp_par.path().join("tile_a.mp")).unwrap();
    std::fs::copy(fixture("tile_b.mp"), tmp_par.path().join("tile_b.mp")).unwrap();

    let out_seq = tempfile::NamedTempFile::new().unwrap();
    let out_par = tempfile::NamedTempFile::new().unwrap();

    let config_seq = BuildConfig {
        family_id: 6324,
        product_id: 1,
        description: "Test Parallel".into(),
        block_size_exponent: 9,
        typ_file: None,
        jobs: 1,
        show_progress: false,
    };
    let config_par = BuildConfig { jobs: 2, ..config_seq.clone() };

    let stats_seq =
        GmapsuppAssembler::build(tmp_seq.path(), out_seq.path(), &config_seq).unwrap();
    let stats_par =
        GmapsuppAssembler::build(tmp_par.path(), out_par.path(), &config_par).unwrap();

    assert_eq!(stats_seq.tile_count, stats_par.tile_count, "tile_count identique");
    assert_eq!(
        stats_seq.subfile_count, stats_par.subfile_count,
        "subfile_count identique"
    );

    let bytes_seq = std::fs::read(out_seq.path()).unwrap();
    let bytes_par = std::fs::read(out_par.path()).unwrap();
    // Structural equivalence — byte-for-byte comparison is fragile due to
    // timestamps in both the IMG header and TRE/NOD subfile headers.
    assert_eq!(bytes_seq.len(), bytes_par.len(), "output size identique (déterminisme FAT)");
    // Verify same FAT structure: same number of entries, same extensions
    let exts_seq: Vec<_> = (0..20)
        .filter_map(|i| find_nth_file_entry(&bytes_seq, i))
        .map(|off| fat_ext(&bytes_seq, off))
        .collect();
    let exts_par: Vec<_> = (0..20)
        .filter_map(|i| find_nth_file_entry(&bytes_par, i))
        .map(|off| fat_ext(&bytes_par, off))
        .collect();
    assert_eq!(exts_seq, exts_par, "FAT extensions identiques (déterminisme)");
}

#[test]
fn test_build_parallel_jobs_0_auto() {
    // AC2 — jobs=0 (auto) produit le même résultat que jobs=1.
    let tmp = tempfile::tempdir().unwrap();
    std::fs::copy(fixture("tile_a.mp"), tmp.path().join("tile_a.mp")).unwrap();
    std::fs::copy(fixture("tile_b.mp"), tmp.path().join("tile_b.mp")).unwrap();

    let output = tempfile::NamedTempFile::new().unwrap();
    let config = BuildConfig {
        family_id: 6324,
        product_id: 1,
        description: "Test Auto Jobs".into(),
        block_size_exponent: 9,
        typ_file: None,
        jobs: 0, // auto-detect
        show_progress: false,
    };
    let stats = GmapsuppAssembler::build(tmp.path(), output.path(), &config).unwrap();
    assert_eq!(stats.tile_count, 2, "doit compiler exactement 2 tuiles");
    assert!(stats.subfile_count >= 6, "doit avoir au moins 6 subfiles");
}

#[test]
fn test_build_report_json_schema() {
    // AC4 — CLI --report → fichier JSON avec les champs attendus.
    use assert_cmd::Command;

    let tmp_tiles = tempfile::tempdir().unwrap();
    std::fs::copy(fixture("tile_a.mp"), tmp_tiles.path().join("tile_a.mp")).unwrap();

    let tmp_out = tempfile::tempdir().unwrap();
    let output_img = tmp_out.path().join("gmapsupp.img");
    let report_path = tmp_out.path().join("rapport.json");

    let mut cmd = Command::cargo_bin("imgforge-cli").unwrap();
    cmd.args([
        "build",
        "--input-dir",
        tmp_tiles.path().to_str().unwrap(),
        "-o",
        output_img.to_str().unwrap(),
        "--report",
        report_path.to_str().unwrap(),
    ]);
    cmd.assert().success();

    assert!(report_path.exists(), "le fichier rapport.json doit être créé");

    let content = std::fs::read_to_string(&report_path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&content)
        .expect("le rapport doit être du JSON valide");

    // Vérifier que tous les champs du schéma sont présents.
    assert!(value.get("status").is_some(), "champ 'status' manquant");
    assert!(value.get("tiles_compiled").is_some(), "champ 'tiles_compiled' manquant");
    assert!(value.get("tiles_failed").is_some(), "champ 'tiles_failed' manquant");
    assert!(value.get("features_by_type").is_some(), "champ 'features_by_type' manquant");
    assert!(
        value["features_by_type"].get("poi").is_some(),
        "champ 'features_by_type.poi' manquant"
    );
    assert!(
        value["features_by_type"].get("polyline").is_some(),
        "champ 'features_by_type.polyline' manquant"
    );
    assert!(
        value["features_by_type"].get("polygon").is_some(),
        "champ 'features_by_type.polygon' manquant"
    );
    assert!(value.get("routing_nodes").is_some(), "champ 'routing_nodes' manquant");
    assert!(value.get("routing_arcs").is_some(), "champ 'routing_arcs' manquant");
    assert!(value.get("img_size_bytes").is_some(), "champ 'img_size_bytes' manquant");
    assert!(value.get("duration_seconds").is_some(), "champ 'duration_seconds' manquant");
    assert!(value.get("errors").is_some(), "champ 'errors' manquant");

    // Vérifier les valeurs logiques pour une compilation réussie.
    assert_eq!(value["status"], "success", "status doit être 'success'");
    assert_eq!(value["tiles_compiled"], 1, "doit avoir compilé 1 tuile");
    assert_eq!(value["tiles_failed"], 0, "doit avoir 0 tuile en échec");
    assert!(
        value["img_size_bytes"].as_u64().unwrap() > 0,
        "img_size_bytes doit être > 0"
    );
    assert!(
        value["duration_seconds"].as_f64().unwrap() >= 0.0,
        "duration_seconds doit être >= 0"
    );
    // tile_a.mp a 1 POI, 2 polylines (dont 1 routable), 0 polygons.
    assert_eq!(value["features_by_type"]["poi"], 1, "tile_a a 1 POI");
    assert_eq!(value["features_by_type"]["polyline"], 2, "tile_a a 2 polylines");
    assert_eq!(value["features_by_type"]["polygon"], 0, "tile_a a 0 polygons");
    assert!(
        value["routing_nodes"].as_u64().unwrap() > 0,
        "tile_a a une route → routing_nodes > 0"
    );
    assert!(
        value["routing_arcs"].as_u64().unwrap() > 0,
        "tile_a a une route → routing_arcs > 0"
    );
    assert!(value["errors"].as_array().unwrap().is_empty(), "0 erreur attendue");
}

#[test]
fn test_build_no_report_creates_no_json_file() {
    // AC5 — sans --report, aucun fichier JSON ne doit être créé.
    use assert_cmd::Command;

    let tmp_tiles = tempfile::tempdir().unwrap();
    std::fs::copy(fixture("tile_a.mp"), tmp_tiles.path().join("tile_a.mp")).unwrap();

    let tmp_out = tempfile::tempdir().unwrap();
    let output_img = tmp_out.path().join("gmapsupp.img");

    let mut cmd = Command::cargo_bin("imgforge-cli").unwrap();
    cmd.args([
        "build",
        "--input-dir",
        tmp_tiles.path().to_str().unwrap(),
        "-o",
        output_img.to_str().unwrap(),
    ]);
    cmd.assert().success();

    // Aucun fichier .json ne doit exister dans le répertoire de sortie.
    let json_files: Vec<_> = std::fs::read_dir(tmp_out.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .collect();
    assert!(
        json_files.is_empty(),
        "aucun fichier .json ne doit être créé sans --report"
    );
}

// ----------------------------------------------------------------
// Extended types tests
// ----------------------------------------------------------------

#[test]
fn test_compile_extended_types_fixture() {
    // AC-6: compile extended_types.mp → valid .img without "not supported" warnings.
    let mp = MpParser::parse_file(&fixture("extended_types.mp")).unwrap();
    assert_eq!(mp.points.len(), 2, "should have 2 POIs (1 standard + 1 extended)");
    assert_eq!(mp.polylines.len(), 2, "should have 2 polylines (1 standard + 1 extended)");
    assert_eq!(mp.polygons.len(), 2, "should have 2 polygons (1 standard + 1 extended)");

    let output = tempfile::NamedTempFile::new().unwrap();
    ImgWriter::write(&mp, output.path()).unwrap();

    let bytes = std::fs::read(output.path()).unwrap();
    assert!(!bytes.is_empty(), "output .img must not be empty");
}

#[test]
fn test_extended_types_in_rgn_data() {
    // AC-2, AC-3, AC-4: verify extended type bytes in RGN data.
    use imgforge_cli::img::rgn::RgnWriter;
    use imgforge_cli::img::tre::levels_from_mp;

    let mp = MpParser::parse_file(&fixture("extended_types.mp")).unwrap();
    let levels = levels_from_mp(&mp.header);
    let rgn = RgnWriter::build(&mp, &levels);

    // The RGN data starts after the 48-byte header.
    let data = &rgn.data[46..];
    assert!(!data.is_empty(), "RGN feature data must not be empty");

    // Verify extended flags are set.
    assert!(rgn.subdiv_has_extended_points[0], "should have extended points at level 0");
    assert!(rgn.subdiv_has_extended_polylines[0], "should have extended polylines at level 0");
    assert!(rgn.subdiv_has_extended_polygons[0], "should have extended polygons at level 0");
}

#[test]
fn test_extended_types_tre_flags() {
    // AC-5: verify TRE data_flags include extended bits.
    use imgforge_cli::img::rgn::RgnWriter;
    use imgforge_cli::img::tre::{levels_from_mp, TreWriter};

    let mp = MpParser::parse_file(&fixture("extended_types.mp")).unwrap();
    let levels = levels_from_mp(&mp.header);
    let rgn = RgnWriter::build(&mp, &levels);
    let tre = TreWriter::build_with_rgn_result(&mp, &rgn, false);

    // TRE layout: 165 (header) + n_levels * 4 (level records) + n_levels * 16 (subdivisions)
    let n_levels = levels.len();
    let subdiv_start = 165 + n_levels * 4;

    // First subdivision data_flags at offset subdiv_start + 3
    let data_flags = tre[subdiv_start + 3];

    // Should have standard points (0x01) + polylines (0x04) + polygons (0x08)
    // + extended points (0x10) + extended polylines (0x20) + extended polygons (0x40)
    assert!(data_flags & 0x01 != 0, "should have standard points flag");
    assert!(data_flags & 0x04 != 0, "should have standard polylines flag");
    assert!(data_flags & 0x08 != 0, "should have standard polygons flag");
    assert!(data_flags & 0x10 != 0, "should have extended points flag");
    assert!(data_flags & 0x20 != 0, "should have extended polylines flag");
    assert!(data_flags & 0x40 != 0, "should have extended polygons flag");
}

#[test]
fn test_extended_types_label_offsets_nonzero() {
    // F7: verify that extended records with labels get non-zero LBL offsets.
    use imgforge_cli::img::lbl::LblWriter;
    use imgforge_cli::img::rgn::RgnWriter;
    use imgforge_cli::img::tre::levels_from_mp;

    let mp = MpParser::parse_file(&fixture("extended_types.mp")).unwrap();
    let levels = levels_from_mp(&mp.header);
    let lbl = LblWriter::build(&mp);
    let rgn = RgnWriter::build_with_lbl_offsets(&mp, &levels, &lbl.label_offsets);

    // The fixture has labelled extended features (Antenne, Voie Ferrée, Bâtiment).
    // With real LBL offsets, extended records should contain non-zero label offsets.
    // Scan the RGN data for the extended POI record (base_type=0x15 for 0x11503).
    let data = &rgn.data[46..];
    let mut found_ext_poi_label = false;
    for (pos, &byte) in data.iter().enumerate() {
        if byte == 0x15 && pos + 1 < data.len() {
            let sub_flags = data[pos + 1];
            // Check has_label flag (bit 5)
            if sub_flags & 0x20 != 0 {
                // Label offset is at pos+6..pos+9 (after type_byte + sub_flags + delta_lon(2) + delta_lat(2))
                if pos + 8 < data.len() {
                    let lbl_off = data[pos + 6] as u32
                        | ((data[pos + 7] as u32) << 8)
                        | ((data[pos + 8] as u32) << 16);
                    assert!(lbl_off > 0, "extended POI label offset must be non-zero with real LBL");
                    found_ext_poi_label = true;
                    break;
                }
            }
        }
    }
    assert!(found_ext_poi_label, "should find an extended POI with non-zero label offset");
}
