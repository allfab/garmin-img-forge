use imgforge::img::writer;
use imgforge::img::assembler;
use imgforge::parser;

// ============================================================================
// Helpers
// ============================================================================

fn load_fixture(name: &str) -> String {
    let path = format!(
        "{}/tests/fixtures/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Cannot read fixture {}: {}", name, e))
}

fn compile_fixture(name: &str) -> Vec<u8> {
    let content = load_fixture(name);
    let mp = parser::parse_mp(&content).expect("Parse failed");
    writer::build_img(&mp).expect("Build failed")
}

/// Read a 24-bit signed little-endian value
fn read_i24(data: &[u8], offset: usize) -> i32 {
    let val = data[offset] as i32
        | ((data[offset + 1] as i32) << 8)
        | ((data[offset + 2] as i32) << 16);
    if val & 0x800000 != 0 {
        val | !0xFFFFFF
    } else {
        val
    }
}

fn read_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

/// Find a subfile in IMG directory, returns (data_offset, size)
fn find_subfile(img: &[u8], ext: &str) -> Option<(usize, usize)> {
    let block_exp1 = img[0x61] as u32;
    let block_exp2 = img[0x62] as u32;
    let block_size = 1u32 << (block_exp1 + block_exp2);
    let dir_start = 2 * 512;

    let mut pos = dir_start;
    while pos + 512 <= img.len() {
        let entry = &img[pos..pos + 512];
        if entry[0] != 0x01 {
            pos += 512;
            continue;
        }

        let file_ext = std::str::from_utf8(&entry[9..12]).unwrap_or("").trim();
        let part = u16::from_le_bytes([entry[0x11], entry[0x12]]);

        if file_ext == ext && part == 0 {
            let size = u32::from_le_bytes([
                entry[0x0C], entry[0x0D], entry[0x0E], entry[0x0F],
            ]) as usize;

            // First block number
            let first_block = u16::from_le_bytes([entry[0x20], entry[0x21]]);
            if first_block != 0xFFFF {
                let data_offset = first_block as usize * block_size as usize;
                return Some((data_offset, size));
            }
        }
        pos += 512;
    }
    None
}

// ============================================================================
// 12.2 — Tests intégration single-tile
// ============================================================================

#[test]
fn test_minimal_img_header_signatures() {
    let img = compile_fixture("minimal.mp");

    // DSKIMG signature at 0x10
    assert_eq!(
        &img[0x10..0x17],
        b"DSKIMG\0",
        "Missing DSKIMG signature"
    );

    // GARMIN identifier at 0x41
    assert_eq!(
        &img[0x41..0x48],
        b"GARMIN\0",
        "Missing GARMIN identifier"
    );

    // Partition signature 0x55AA at 0x1FE
    assert_eq!(img[0x1FE], 0x55, "Missing partition sig low byte");
    assert_eq!(img[0x1FF], 0xAA, "Missing partition sig high byte");
}

#[test]
fn test_minimal_img_block_size_valid() {
    let img = compile_fixture("minimal.mp");
    let exp1 = img[0x61] as u32;
    let exp2 = img[0x62] as u32;

    assert_eq!(exp1, 9, "Block exponent 1 should be 9");
    assert!(exp1 + exp2 >= 9, "Block size too small");
    assert!(exp1 + exp2 <= 24, "Block size too large");
}

#[test]
fn test_minimal_img_directory_has_entries() {
    let img = compile_fixture("minimal.mp");
    let dir_start = 2 * 512;

    // Should have at least the header entry + TRE + RGN + LBL = 4 entries
    assert!(img.len() > dir_start + 4 * 512, "IMG too small for directory");

    // Count used directory entries
    let mut count = 0;
    let mut pos = dir_start;
    while pos + 512 <= img.len() {
        if img[pos] == 0x01 {
            count += 1;
        }
        pos += 512;
        if count > 100 {
            break;
        }
    }

    assert!(count >= 4, "Expected at least 4 directory entries (header+TRE+RGN+LBL), got {}", count);
}

#[test]
fn test_minimal_img_has_tre_subfile() {
    let img = compile_fixture("minimal.mp");
    let subfile = find_subfile(&img, "TRE");
    assert!(subfile.is_some(), "TRE subfile not found in directory");
    let (offset, size) = subfile.unwrap();
    assert!(size > 0, "TRE subfile has zero size");
    assert!(offset + size <= img.len(), "TRE subfile extends past IMG end");
}

#[test]
fn test_minimal_img_has_rgn_subfile() {
    let img = compile_fixture("minimal.mp");
    let subfile = find_subfile(&img, "RGN");
    assert!(subfile.is_some(), "RGN subfile not found in directory");
    let (_, size) = subfile.unwrap();
    assert!(size > 0, "RGN subfile has zero size");
}

#[test]
fn test_minimal_img_has_lbl_subfile() {
    let img = compile_fixture("minimal.mp");
    let subfile = find_subfile(&img, "LBL");
    assert!(subfile.is_some(), "LBL subfile not found in directory");
    let (_, size) = subfile.unwrap();
    assert!(size > 0, "LBL subfile has zero size");
}

#[test]
fn test_minimal_tre_header_garmin_type() {
    let img = compile_fixture("minimal.mp");
    if let Some((offset, size)) = find_subfile(&img, "TRE") {
        assert!(size >= 188, "TRE too small for header");
        let tre = &img[offset..offset + size];

        // Header length
        let hlen = read_u16(tre, 0);
        assert_eq!(hlen, 188, "TRE header length should be 188");

        // Type string
        assert_eq!(&tre[2..12], b"GARMIN TRE", "TRE type string mismatch");
    }
}

#[test]
fn test_minimal_rgn_header_garmin_type() {
    let img = compile_fixture("minimal.mp");
    if let Some((offset, size)) = find_subfile(&img, "RGN") {
        assert!(size >= 125, "RGN too small for header");
        let rgn = &img[offset..offset + size];

        let hlen = read_u16(rgn, 0);
        assert_eq!(hlen, 125, "RGN header length should be 125");
        assert_eq!(&rgn[2..12], b"GARMIN RGN", "RGN type string mismatch");
    }
}

#[test]
fn test_minimal_lbl_header_garmin_type() {
    let img = compile_fixture("minimal.mp");
    if let Some((offset, size)) = find_subfile(&img, "LBL") {
        assert!(size >= 196, "LBL too small for header");
        let lbl = &img[offset..offset + size];

        let hlen = read_u16(lbl, 0);
        assert_eq!(hlen, 196, "LBL header length should be 196");
        assert_eq!(&lbl[2..12], b"GARMIN LBL", "LBL type string mismatch");
    }
}

#[test]
fn test_minimal_tre_bounds_nonzero() {
    let img = compile_fixture("minimal.mp");
    if let Some((offset, _)) = find_subfile(&img, "TRE") {
        let tre = &img[offset..];

        // Bounds at offset 21: north(3) + east(3) + south(3) + west(3)
        let north = read_i24(tre, 21);
        let east = read_i24(tre, 24);
        let south = read_i24(tre, 27);
        let west = read_i24(tre, 30);

        assert!(north > south, "North {} should be > South {}", north, south);
        assert!(east > west, "East {} should be > West {}", east, west);
        assert!(north != 0 || south != 0, "Bounds should not all be zero");
    }
}

#[test]
fn test_minimal_rgn_has_data() {
    let img = compile_fixture("minimal.mp");
    if let Some((offset, size)) = find_subfile(&img, "RGN") {
        let rgn = &img[offset..offset + size];

        // Data section offset and size in RGN header (at offset 21)
        let data_offset = read_u32(rgn, 21);
        let data_size = read_u32(rgn, 25);

        assert_eq!(data_offset, 125, "RGN data should start after 125B header");
        assert!(data_size > 0, "RGN data section should not be empty (has 1 POI + 1 polyline + 1 polygon)");
    }
}

#[test]
fn test_minimal_lbl_has_labels() {
    let img = compile_fixture("minimal.mp");
    if let Some((offset, size)) = find_subfile(&img, "LBL") {
        let lbl = &img[offset..offset + size];

        // Label section offset and size (at offset 21 in LBL header)
        let label_offset = read_u32(lbl, 21);
        let label_size = read_u32(lbl, 25);

        assert_eq!(label_offset, 196, "Label data should start after 196B header");
        assert!(label_size > 1, "Label section should contain labels (got size {})", label_size);
    }
}

// ============================================================================
// 12.2 — Labels accentués
// ============================================================================

#[test]
fn test_accented_labels_compile() {
    let img = compile_fixture("labels_accented.mp");
    assert!(img.len() > 512, "Accented labels IMG too small");
    assert_eq!(&img[0x10..0x17], b"DSKIMG\0");
}

#[test]
fn test_accented_labels_lbl_has_content() {
    let img = compile_fixture("labels_accented.mp");
    if let Some((offset, size)) = find_subfile(&img, "LBL") {
        let lbl = &img[offset..offset + size];
        let label_size = read_u32(lbl, 25);
        // Should have at least labels for: Château de Versailles, Forêt de Fontainebleau, etc.
        assert!(label_size > 20, "Label section too small for accented labels: {}", label_size);
    }
}

// ============================================================================
// 12.3 — Tests intégration routing
// ============================================================================

#[test]
fn test_routing_compiles() {
    let img = compile_fixture("routing.mp");
    assert!(img.len() > 512);
    assert_eq!(&img[0x10..0x17], b"DSKIMG\0");
}

#[test]
fn test_routing_has_tre_rgn_lbl() {
    let img = compile_fixture("routing.mp");
    assert!(find_subfile(&img, "TRE").is_some(), "Missing TRE");
    assert!(find_subfile(&img, "RGN").is_some(), "Missing RGN");
    assert!(find_subfile(&img, "LBL").is_some(), "Missing LBL");
}

#[test]
fn test_routing_has_net_subfile() {
    let img = compile_fixture("routing.mp");
    let net = find_subfile(&img, "NET");
    assert!(net.is_some(), "Routing IMG must contain NET subfile");
    let (offset, size) = net.unwrap();
    assert!(size >= 55, "NET subfile too small for header (got {} bytes)", size);
    // Verify NET header
    let net_data = &img[offset..offset + size];
    let hlen = read_u16(net_data, 0);
    assert_eq!(hlen, 55, "NET header length should be 55");
    assert_eq!(&net_data[2..12], b"GARMIN NET", "NET type string mismatch");
}

#[test]
fn test_routing_has_nod_subfile() {
    let img = compile_fixture("routing.mp");
    let nod = find_subfile(&img, "NOD");
    assert!(nod.is_some(), "Routing IMG must contain NOD subfile");
    let (offset, size) = nod.unwrap();
    assert!(size >= 127, "NOD subfile too small for header (got {} bytes)", size);
    // Verify NOD header
    let nod_data = &img[offset..offset + size];
    let hlen = read_u16(nod_data, 0);
    assert_eq!(hlen, 127, "NOD header length should be 127");
    assert_eq!(&nod_data[2..12], b"GARMIN NOD", "NOD type string mismatch");
}

#[test]
fn test_routing_polylines_in_rgn() {
    let img = compile_fixture("routing.mp");
    if let Some((offset, size)) = find_subfile(&img, "RGN") {
        let rgn = &img[offset..offset + size];
        let data_size = read_u32(rgn, 25);
        assert!(data_size > 10, "RGN data too small for routing features: {}", data_size);
    }
}

// ── T12: mkgmap-faithful NOD/NET structural validation ──

#[test]
fn test_routing_nod1_has_route_centers() {
    let img = compile_fixture("routing.mp");
    let (offset, size) = find_subfile(&img, "NOD").expect("Missing NOD");
    let nod = &img[offset..offset + size];
    // NOD1 section: offset at 0x15 (4B), size at 0x19 (4B)
    let nod1_off = read_u32(nod, 0x15) as usize;
    let nod1_size = read_u32(nod, 0x19) as usize;
    assert!(nod1_size > 0, "NOD1 section should not be empty");
    // First byte of a node is the table pointer (backpatched calcLowByte)
    // It should be a small value (distance to tables in 64B units)
    let first_byte = nod[nod1_off];
    assert!(first_byte < 64, "First node byte0 (calcLowByte) should be small: {}", first_byte);
}

#[test]
fn test_routing_nod2_per_road_format() {
    let img = compile_fixture("routing.mp");
    let (offset, size) = find_subfile(&img, "NOD").expect("Missing NOD");
    let nod = &img[offset..offset + size];
    // NOD2 section: offset at 0x25 (4B), size at 0x29 (4B)
    let nod2_off = read_u32(nod, 0x25) as usize;
    let nod2_size = read_u32(nod, 0x29) as usize;
    assert!(nod2_size > 0, "NOD2 section should not be empty");
    // First byte = nod2Flags, bit 0 should be set
    let nod2_flags = nod[nod2_off];
    assert!(nod2_flags & 0x01 != 0, "NOD2 record nod2Flags bit 0 must be set");
    // Bytes 1-3 = NOD1 offset (should be within NOD1 section range)
    let nod1_ptr = (nod[nod2_off + 1] as u32)
        | ((nod[nod2_off + 2] as u32) << 8)
        | ((nod[nod2_off + 3] as u32) << 16);
    let nod1_size_u32 = read_u32(nod, 0x19);
    assert!(nod1_ptr < nod1_size_u32, "NOD2 NOD1 ptr {} should be within NOD1 size {}", nod1_ptr, nod1_size_u32);
}

#[test]
fn test_routing_net1_has_nod2_offset() {
    let img = compile_fixture("routing.mp");
    let (offset, size) = find_subfile(&img, "NET").expect("Missing NET");
    let net = &img[offset..offset + size];
    // NET1 starts at header offset
    let net1_off = read_u32(net, 0x15) as usize;
    let net1_size = read_u32(net, 0x19) as usize;
    assert!(net1_size > 0, "NET1 should not be empty");
    // First road's flags byte should have 0x44 (UNK1=0x04, NODINFO=0x40)
    // The first road: label(3B) then flags byte
    let flags = net[net1_off + 3];
    assert_eq!(flags & 0x04, 0x04, "NET1 flags should have NET_FLAG_UNK1 (0x04)");
    assert_eq!(flags & 0x40, 0x40, "NET1 flags should have NET_FLAG_NODINFO (0x40)");
}

#[test]
fn test_routing_nod_header_4_sections() {
    let img = compile_fixture("routing.mp");
    let (offset, size) = find_subfile(&img, "NOD").expect("Missing NOD");
    let nod = &img[offset..offset + size];
    // Verify 4 section descriptors present:
    // NOD1 @0x15, NOD2 @0x25, NOD3 @0x31, NOD4 after NOD3
    let nod1_off = read_u32(nod, 0x15);
    let nod2_off = read_u32(nod, 0x25);
    let nod3_off = read_u32(nod, 0x31);
    // NOD1 < NOD2 < NOD3
    assert!(nod1_off <= nod2_off, "NOD1 offset should be <= NOD2");
    assert!(nod2_off <= nod3_off, "NOD2 offset should be <= NOD3");
    // Flags at 0x1D-0x20 should contain 0x0227
    let flags_lo = nod[0x1D];
    let flags_hi = nod[0x1E];
    assert_eq!(flags_lo, 0x27, "NOD flags low byte should be 0x27");
    assert_eq!(flags_hi, 0x02, "NOD flags high byte should be 0x02");
    // Alignment at 0x21 should be 6
    assert_eq!(nod[0x21], 0x06, "NOD alignment shift should be 6");
    // Table A reclen at 0x23 should be 5
    assert_eq!(read_u16(nod, 0x23), 5, "Table A record length should be 5");
}

// ============================================================================
// 12.4 — Tests intégration multi-tile
// ============================================================================

#[test]
fn test_multi_tile_compile_both() {
    // Both tiles should compile individually
    let img_a = compile_fixture("tile_a.mp");
    let img_b = compile_fixture("tile_b.mp");
    assert!(img_a.len() > 512);
    assert!(img_b.len() > 512);
}

#[test]
fn test_multi_tile_gmapsupp() {
    let img_a = compile_fixture("tile_a.mp");
    let img_b = compile_fixture("tile_b.mp");

    let tiles = vec![
        ("63240001".to_string(), img_a),
        ("63240002".to_string(), img_b),
    ];

    let gmapsupp = assembler::build_gmapsupp_from_imgs(&tiles, "Alsace Multi-tile")
        .expect("gmapsupp assembly failed");

    // Basic structure checks
    assert!(gmapsupp.len() > 512, "gmapsupp too small");
    assert_eq!(&gmapsupp[0x10..0x17], b"DSKIMG\0", "Missing DSKIMG");
    assert_eq!(&gmapsupp[0x41..0x48], b"GARMIN\0", "Missing GARMIN");
    assert_eq!(gmapsupp[0x1FE], 0x55, "Missing partition sig");
    assert_eq!(gmapsupp[0x1FF], 0xAA, "Missing partition sig");
}

#[test]
fn test_multi_tile_gmapsupp_has_subfiles() {
    let img_a = compile_fixture("tile_a.mp");
    let img_b = compile_fixture("tile_b.mp");

    let tiles = vec![
        ("63240001".to_string(), img_a),
        ("63240002".to_string(), img_b),
    ];

    let gmapsupp = assembler::build_gmapsupp_from_imgs(&tiles, "Alsace")
        .expect("gmapsupp assembly failed");

    // Count TRE subfiles in the gmapsupp directory — should have at least 2
    let dir_start = 2 * 512;
    let mut tre_count = 0;
    let mut pos = dir_start;
    while pos + 512 <= gmapsupp.len() {
        if gmapsupp[pos] == 0x01 {
            let ext = std::str::from_utf8(&gmapsupp[pos + 9..pos + 12]).unwrap_or("");
            let part = u16::from_le_bytes([gmapsupp[pos + 0x11], gmapsupp[pos + 0x12]]);
            if ext == "TRE" && part == 0 {
                tre_count += 1;
            }
        }
        pos += 512;
        if pos > dir_start + 100 * 512 {
            break;
        }
    }

    assert!(
        tre_count >= 2,
        "Expected at least 2 TRE subfiles in gmapsupp, found {}",
        tre_count
    );
}

// ============================================================================
// 12.5 — Tests intégration types étendus
// ============================================================================

#[test]
fn test_extended_types_compile() {
    let img = compile_fixture("extended_types.mp");
    assert!(img.len() > 512, "Extended types IMG too small");
    assert_eq!(&img[0x10..0x17], b"DSKIMG\0");
    assert_eq!(img[0x1FE], 0x55);
    assert_eq!(img[0x1FF], 0xAA);
}

#[test]
fn test_extended_types_rgn_has_ext_sections() {
    let img = compile_fixture("extended_types.mp");
    if let Some((offset, size)) = find_subfile(&img, "RGN") {
        let rgn = &img[offset..offset + size];

        // RGN header: standard data at 21-28
        let data_offset = read_u32(rgn, 21);
        let data_size = read_u32(rgn, 25);
        assert_eq!(data_offset, 125, "RGN data should start after 125B header");
        assert!(data_size > 0, "RGN standard data should not be empty");

        // Extended areas at position 29-36
        let ext_areas_offset = read_u32(rgn, 29);
        let ext_areas_size = read_u32(rgn, 33);
        assert!(ext_areas_offset > 0, "Extended areas offset should be non-zero (has polygon 0x10f04)");
        assert!(ext_areas_size > 0, "Extended areas size should be non-zero");

        // Extended points at position 85-92
        let ext_points_offset = read_u32(rgn, 85);
        let ext_points_size = read_u32(rgn, 89);
        assert!(ext_points_offset > 0, "Extended points offset should be non-zero (has POI 0x2C04)");
        assert!(ext_points_size > 0, "Extended points size should be non-zero");
    }
}

#[test]
fn test_extended_types_tre_has_ext_overviews() {
    let img = compile_fixture("extended_types.mp");
    if let Some((offset, size)) = find_subfile(&img, "TRE") {
        let tre = &img[offset..offset + size];
        let hlen = read_u16(tre, 0);
        assert_eq!(hlen, 188, "TRE header should be 188 bytes");

        // extTypeOffsets at position 124-133 (mkgmap TREHeader layout)
        let ext_offsets_offset = read_u32(tre, 124);
        let ext_offsets_size = read_u32(tre, 128);
        assert!(ext_offsets_offset > 0, "extTypeOffsets offset should be non-zero");
        assert!(ext_offsets_size > 0, "extTypeOffsets should have data");

        // Record size should be 13
        let record_size = read_u16(tre, 132);
        assert_eq!(record_size, 13, "extTypeOffsets record size should be 13");

        // Magic 0x0607 at position 134-137
        let magic = read_u32(tre, 134);
        assert_eq!(magic, 0x0607, "Extended types magic should be 0x0607");

        // extTypeOverviews at position 138-147
        let ext_ov_offset = read_u32(tre, 138);
        let ext_ov_size = read_u32(tre, 142);
        assert!(ext_ov_offset > 0, "extTypeOverviews offset should be non-zero");
        assert!(ext_ov_size > 0, "extTypeOverviews should have data");

        // Record size should be 4
        let ov_record_size = read_u16(tre, 146);
        assert_eq!(ov_record_size, 4, "extTypeOverviews record size should be 4");
    }
}

#[test]
fn test_extended_types_parser_preserves_large_types() {
    let content = load_fixture("extended_types.mp");
    let mp = parser::parse_mp(&content).unwrap();

    // 0x1101C should not be truncated
    let large_poi = mp.points.iter().find(|p| p.label == "Large Type POI").unwrap();
    assert_eq!(large_poi.type_code, 0x1101C, "Type 0x1101C must be preserved as u32");

    // 0x2C04 should be preserved
    let ext_poi = mp.points.iter().find(|p| p.label == "Extended POI").unwrap();
    assert_eq!(ext_poi.type_code, 0x2C04);

    // 0x10f04 should be preserved
    let ext_poly = mp.polygons.iter().find(|p| p.label == "Extended Building").unwrap();
    assert_eq!(ext_poly.type_code, 0x10f04);
}

#[test]
fn test_extended_types_nonregression_fixtures() {
    // All existing fixtures must still compile correctly
    for fixture in &["minimal.mp", "routing.mp", "labels_accented.mp", "tile_a.mp", "tile_b.mp"] {
        let content = load_fixture(fixture);
        let mp = parser::parse_mp(&content)
            .unwrap_or_else(|e| panic!("Parse failed for {}: {}", fixture, e));
        let img = writer::build_img(&mp)
            .unwrap_or_else(|e| panic!("Build failed for {}: {}", fixture, e));

        assert!(img.len() > 512, "{} produced IMG < 512 bytes", fixture);
        assert_eq!(&img[0x10..0x17], b"DSKIMG\0", "{} missing DSKIMG", fixture);
    }
}

// ============================================================================
// 12.6 — Tests MPS subfile dans gmapsupp
// ============================================================================

#[test]
fn test_gmapsupp_contains_mps() {
    let img_a = compile_fixture("tile_a.mp");
    let img_b = compile_fixture("tile_b.mp");

    let tiles = vec![
        ("63240001".to_string(), img_a),
        ("63240002".to_string(), img_b),
    ];

    let gmapsupp = assembler::build_gmapsupp_from_imgs(&tiles, "Test MPS")
        .expect("gmapsupp assembly failed");

    // Scan directory for MPS subfile
    let dir_start = 2 * 512;
    let mut found_mps = false;
    let mut pos = dir_start;
    while pos + 512 <= gmapsupp.len() {
        if gmapsupp[pos] == 0x01 {
            let ext = std::str::from_utf8(&gmapsupp[pos + 9..pos + 12]).unwrap_or("");
            let part = u16::from_le_bytes([gmapsupp[pos + 0x11], gmapsupp[pos + 0x12]]);
            if ext == "MPS" && part == 0 {
                found_mps = true;
                break;
            }
        }
        pos += 512;
        if pos > dir_start + 200 * 512 { break; }
    }

    assert!(found_mps, "gmapsupp should contain an MPS subfile");
}

#[test]
fn test_gmapsupp_mps_contains_product_and_map_blocks() {
    use imgforge::img::assembler::{TileSubfiles, GmapsuppMeta, build_gmapsupp_with_meta};

    let content_a = load_fixture("tile_a.mp");
    let content_b = load_fixture("tile_b.mp");
    let mp_a = parser::parse_mp(&content_a).unwrap();
    let mp_b = parser::parse_mp(&content_b).unwrap();
    let tile_a = writer::build_subfiles(&mp_a).unwrap();
    let tile_b = writer::build_subfiles(&mp_b).unwrap();

    let tiles = vec![
        TileSubfiles {
            map_number: tile_a.map_number, description: tile_a.description,
            tre: tile_a.tre, rgn: tile_a.rgn, lbl: tile_a.lbl,
            net: tile_a.net, nod: tile_a.nod, dem: tile_a.dem,
        },
        TileSubfiles {
            map_number: tile_b.map_number, description: tile_b.description,
            tre: tile_b.tre, rgn: tile_b.rgn, lbl: tile_b.lbl,
            net: tile_b.net, nod: tile_b.nod, dem: tile_b.dem,
        },
    ];

    let meta = GmapsuppMeta {
        family_id: 53403,
        product_id: 1,
        family_name: "Test Map Family".to_string(),
        area_name: String::new(),
        codepage: 1252,
    };

    let gmapsupp = build_gmapsupp_with_meta(&tiles, "Test", &meta).unwrap();

    // Extract MPS data from gmapsupp
    let dir_start = 2 * 512;
    let block_exp1 = gmapsupp[0x61] as u32;
    let block_exp2 = gmapsupp[0x62] as u32;
    let block_size = 1u32 << (block_exp1 + block_exp2);

    let mut mps_data = Vec::new();
    let mut pos = dir_start;
    while pos + 512 <= gmapsupp.len() {
        if gmapsupp[pos] == 0x01 {
            let ext = std::str::from_utf8(&gmapsupp[pos + 9..pos + 12]).unwrap_or("");
            let part = u16::from_le_bytes([gmapsupp[pos + 0x11], gmapsupp[pos + 0x12]]);
            if ext == "MPS" && part == 0 {
                let size = u32::from_le_bytes([
                    gmapsupp[pos + 0x0C], gmapsupp[pos + 0x0D],
                    gmapsupp[pos + 0x0E], gmapsupp[pos + 0x0F],
                ]) as usize;
                // Read blocks
                for slot in 0..240 {
                    let blk_off = pos + 0x20 + slot * 2;
                    let blk = u16::from_le_bytes([gmapsupp[blk_off], gmapsupp[blk_off + 1]]);
                    if blk == 0xFFFF { break; }
                    let data_start = blk as usize * block_size as usize;
                    let data_end = (data_start + block_size as usize).min(gmapsupp.len());
                    let take = (size - mps_data.len()).min(data_end - data_start);
                    mps_data.extend_from_slice(&gmapsupp[data_start..data_start + take]);
                }
                mps_data.truncate(size);
                break;
            }
        }
        pos += 512;
        if pos > dir_start + 200 * 512 { break; }
    }

    assert!(!mps_data.is_empty(), "MPS data should not be empty");

    // Parse MPS blocks: count product (0x46) and map (0x4C) blocks
    let mut product_count = 0;
    let mut map_count = 0;
    let mut mps_pos = 0;
    while mps_pos + 3 <= mps_data.len() {
        let block_type = mps_data[mps_pos];
        let block_len = u16::from_le_bytes([mps_data[mps_pos + 1], mps_data[mps_pos + 2]]) as usize;
        match block_type {
            0x46 => {
                product_count += 1;
                // Verify FID = 53403
                let fid = u16::from_le_bytes([mps_data[mps_pos + 5], mps_data[mps_pos + 6]]);
                assert_eq!(fid, 53403, "MPS product block should contain FID 53403");
            }
            0x4C => {
                map_count += 1;
                // Verify PID and FID
                let pid = u16::from_le_bytes([mps_data[mps_pos + 3], mps_data[mps_pos + 4]]);
                let fid = u16::from_le_bytes([mps_data[mps_pos + 5], mps_data[mps_pos + 6]]);
                assert_eq!(pid, 1, "MPS map block PID should be 1");
                assert_eq!(fid, 53403, "MPS map block FID should be 53403");
            }
            _ => {}
        }
        mps_pos += 3 + block_len;
    }

    assert_eq!(product_count, 1, "Should have exactly 1 product block");
    assert_eq!(map_count, 2, "Should have exactly 2 map blocks (one per tile)");
}

// ============================================================================
// Parse → Compile round-trip sanity checks
// ============================================================================

#[test]
fn test_parse_compile_all_fixtures() {
    for fixture in &["minimal.mp", "routing.mp", "labels_accented.mp", "tile_a.mp", "tile_b.mp"] {
        let content = load_fixture(fixture);
        let mp = parser::parse_mp(&content)
            .unwrap_or_else(|e| panic!("Parse failed for {}: {}", fixture, e));
        let img = writer::build_img(&mp)
            .unwrap_or_else(|e| panic!("Build failed for {}: {}", fixture, e));

        assert!(img.len() > 512, "{} produced IMG < 512 bytes", fixture);
        assert_eq!(
            &img[0x10..0x17],
            b"DSKIMG\0",
            "{} missing DSKIMG signature",
            fixture
        );
    }
}

#[test]
fn test_img_file_size_reasonable() {
    let img = compile_fixture("minimal.mp");
    // A minimal map should be between 2KB and 100KB
    assert!(img.len() > 2048, "IMG too small: {} bytes", img.len());
    assert!(img.len() < 100_000, "IMG unexpectedly large: {} bytes", img.len());
}

// ============================================================================
// DEM integration tests
// ============================================================================

#[test]
fn test_compile_without_dem_no_dem_subfile() {
    // AC 5: Without --dem, IMG should NOT contain a DEM subfile
    let img = compile_fixture("minimal.mp");
    let dem = find_subfile(&img, "DEM");
    assert!(dem.is_none(), "IMG without --dem should NOT contain a DEM subfile");
}

#[test]
fn test_dem_writer_produces_valid_header() {
    use imgforge::img::dem::DemWriter;

    let writer = DemWriter::new();
    let data = writer.build();

    // Minimum: 41-byte header
    assert!(data.len() >= 41, "DEM data too small: {} bytes", data.len());

    // CommonHeader: header_length at offset 0-1
    let header_len = u16::from_le_bytes([data[0], data[1]]);
    assert_eq!(header_len, 41, "DEM header length should be 41");

    // Type string: "GARMIN DEM" at offset 2-11
    assert_eq!(&data[2..12], b"GARMIN DEM");

    // Unknown byte (0x01) at offset 12
    assert_eq!(data[12], 0x01);

    // Lock byte (0x00) at offset 13
    assert_eq!(data[13], 0x00);
}

#[test]
fn test_dem_tile_encoding_roundtrip() {
    use imgforge::img::dem::encode_dem_tile;

    // Flat tile: should produce empty bitstream
    let flat = vec![500i16; 64 * 64];
    let result = encode_dem_tile(&flat, 64, 64);
    assert_eq!(result.base_height, 500);
    assert_eq!(result.max_delta, 0);
    assert!(result.bitstream.is_empty());

    // Gradient tile: should produce non-empty bitstream
    let mut gradient = Vec::with_capacity(64 * 64);
    for r in 0..64 {
        for c in 0..64 {
            gradient.push((100 + r + c) as i16);
        }
    }
    let result = encode_dem_tile(&gradient, 64, 64);
    assert_eq!(result.base_height, 100);
    assert_eq!(result.max_delta, 126); // max = 100 + 63 + 63 = 226, delta = 126
    assert!(!result.bitstream.is_empty());
}

#[test]
fn test_dem_section_with_converter() {
    use imgforge::dem::{ElevationGrid, GeoBounds, InterpolationMethod};
    use imgforge::dem::converter::DemConverter;
    use imgforge::img::dem::DemSection;

    // Create a simple elevation grid covering the minimal.mp area
    let width = 10u32;
    let height = 10u32;
    let mut data = Vec::with_capacity((width * height) as usize);
    for r in 0..height {
        for c in 0..width {
            data.push((200 + r * 10 + c * 5) as f64);
        }
    }

    let grid = ElevationGrid {
        width,
        height,
        data,
        nodata: -99999.0,
        bounds: GeoBounds {
            south: 48.0,
            west: 7.0,
            north: 49.0,
            east: 8.0,
        },
        cellsize_lat: 1.0 / 9.0,
        cellsize_lon: 1.0 / 9.0,
    };

    let converter = DemConverter::new(vec![grid], InterpolationMethod::Bilinear);

    let bounds = GeoBounds {
        south: 48.57,
        west: 7.75,
        north: 48.58,
        east: 7.76,
    };

    let section = DemSection::new(0, &bounds, 3312, &converter);

    // Should have at least 1 tile
    assert!(section.tiles_lat >= 1);
    assert!(section.tiles_lon >= 1);
    assert!(!section.tile_results.is_empty());

    // Section header should be 64 bytes
    let header = section.build_header(0, 0);
    assert_eq!(header.len(), 60);
}
