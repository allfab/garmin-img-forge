// GmpWriter — emballe les 6 sous-sections TRE/RGN/LBL/NET/NOD/DEM dans un conteneur
// .GMP Garmin ("NT format"), consolidant 6 fichiers FAT en 1 seul par tuile.
//
// Spec binaire : docs/implementation-artifacts/imgforge-gmp-format.md
// Source RE : gimgtools garmin_struct.h (GPL, lecture-spec uniquement, pas de recopie)

use super::common_header::{now_secs, unix_to_calendar};

pub const GMP_HEADER_LEN: u16 = 0x3D; // 61 bytes

/// Copyright block exactly as emitted by Garmin TopoFrance v6 Pro (2021-05).
/// Deux C-strings NUL-terminées concaténées ; 179 bytes au total (0xB3).
pub const GMP_COPYRIGHT: &[u8] = b"Copyright Garmin Ltd. or its subsidiaries.  All rights reserved.\x00Copying is expressly prohibited and may result in criminal charges and/or civil action being brought against you.\x00";

/// GmpWriter — emballe les 6 Vec<u8> sous-sections dans un blob .GMP.
pub struct GmpWriter {
    pub tre: Vec<u8>,
    pub rgn: Vec<u8>,
    pub lbl: Vec<u8>,
    pub net: Option<Vec<u8>>,
    pub nod: Option<Vec<u8>>,
    pub dem: Option<Vec<u8>>,
    /// Optionnel : override la date de build (sinon UTC now ou SOURCE_DATE_EPOCH).
    pub fixed_date: Option<(u16, u8, u8, u8, u8, u8)>,
}

impl GmpWriter {
    pub fn new(
        tre: Vec<u8>,
        rgn: Vec<u8>,
        lbl: Vec<u8>,
        net: Option<Vec<u8>>,
        nod: Option<Vec<u8>>,
        dem: Option<Vec<u8>>,
    ) -> Self {
        Self { tre, rgn, lbl, net, nod, dem, fixed_date: None }
    }

    pub fn with_date(mut self, year: u16, month: u8, day: u8, hour: u8, min: u8, sec: u8) -> Self {
        self.fixed_date = Some((year, month, day, hour, min, sec));
        self
    }

    /// Produit le blob `.GMP` complet.
    ///
    /// Layout : `[GMP hdr 61B][copyright 179B][TRE blob][RGN blob][LBL blob][NET?][NOD?][DEM?]`
    ///
    /// Les offsets internes de chaque sous-header (ex. `tre.map_levels_offset`) sont
    /// relocalisés de « absolu dans le standalone subfile » vers « absolu dans le GMP »
    /// en ajoutant la position de départ de chaque blob dans le GMP.
    pub fn write(&self) -> Vec<u8> {
        let header_len = GMP_HEADER_LEN as usize;
        let copyright_len = GMP_COPYRIGHT.len();
        let body_base = header_len + copyright_len; // = 240 = 0xF0

        // Clones mutables pour la relocalisation des offsets internes
        let mut tre = self.tre.clone();
        // Le TRE est conservé avec son hlen standard (188 B, format_marker 0x00110301).
        // Extension NT (hlen=309) non appliquée : les section descriptors 0xD0..0x134 à
        // zéro empêchent le firmware Alpha 100 d'enregistrer la tuile (validé GC5 2026-04-25).

        let mut rgn = self.rgn.clone();
        let mut lbl = self.lbl.clone();
        let mut net = self.net.clone();
        let mut nod = self.nod.clone();
        let mut dem = self.dem.clone();

        // Positions de départ de chaque blob dans le GMP
        let tre_start = body_base as u32;
        let rgn_start = tre_start + tre.len() as u32;
        let lbl_start = rgn_start + rgn.len() as u32;
        let mut cursor = lbl_start + lbl.len() as u32;

        let net_start: u32;
        let nod_start: u32;
        let dem_start: u32;
        if let Some(ref n) = net { net_start = cursor; cursor += n.len() as u32; } else { net_start = 0; }
        if let Some(ref n) = nod { nod_start = cursor; cursor += n.len() as u32; } else { nod_start = 0; }
        if let Some(ref d) = dem { dem_start = cursor; cursor += d.len() as u32; } else { dem_start = 0; }
        // Relocalisation : chaque offset standalone-relatif devient GMP-absolu
        relocate_tre(&mut tre, tre_start);
        relocate_rgn(&mut rgn, rgn_start);
        relocate_lbl(&mut lbl, lbl_start);
        if let Some(ref mut n) = net { relocate_net(n, net_start); }
        if let Some(ref mut n) = nod { relocate_nod(n, nod_start); }
        if let Some(ref mut d) = dem { relocate_dem(d, dem_start); }

        let extra_cap = net.as_ref().map_or(0, |v| v.len())
            + nod.as_ref().map_or(0, |v| v.len())
            + dem.as_ref().map_or(0, |v| v.len());
        let mut out = Vec::with_capacity(body_base + tre.len() + rgn.len() + lbl.len() + extra_cap);

        // === Header (0x3D bytes) ===
        out.extend_from_slice(&GMP_HEADER_LEN.to_le_bytes());        // 0x00 : hlen
        out.extend_from_slice(b"GARMIN GMP");                        // 0x02..0x0B : magic
        out.push(0x01);                                              // 0x0C : unknown_00c (constant = 1)
        out.push(0x00);                                              // 0x0D : locked (0 = libre)

        let (year, month, day, hour, min, sec) = self.fixed_date.unwrap_or_else(|| {
            let (y, mo, d, h, mi, s) = unix_to_calendar(now_secs());
            (y as u16, mo as u8, d as u8, h as u8, mi as u8, s as u8)
        });
        out.extend_from_slice(&year.to_le_bytes());                  // 0x0E..0x0F : year
        out.push(month);                                             // 0x10
        out.push(day);                                               // 0x11
        out.push(hour);                                              // 0x12
        out.push(min);                                               // 0x13
        out.push(sec);                                               // 0x14

        out.extend_from_slice(&0u32.to_le_bytes());                  // 0x15..0x18 : unknown_015 = 0
        out.extend_from_slice(&tre_start.to_le_bytes());             // 0x19..0x1C
        out.extend_from_slice(&rgn_start.to_le_bytes());             // 0x1D..0x20
        out.extend_from_slice(&lbl_start.to_le_bytes());             // 0x21..0x24
        out.extend_from_slice(&net_start.to_le_bytes());             // 0x25..0x28
        out.extend_from_slice(&nod_start.to_le_bytes());             // 0x29..0x2C
        out.extend_from_slice(&dem_start.to_le_bytes());             // 0x2D..0x30
        out.extend_from_slice(&0u32.to_le_bytes());                  // 0x31..0x34 : mar_offset = 0

        // 0x35..0x3C : 8 bytes reserved (zéros)
        while out.len() < header_len {
            out.push(0x00);
        }
        debug_assert_eq!(out.len(), header_len);

        // === Copyright ===
        out.extend_from_slice(GMP_COPYRIGHT);
        debug_assert_eq!(out.len(), body_base);

        // === Blobs relocalisés ===
        out.extend_from_slice(&tre);
        out.extend_from_slice(&rgn);
        out.extend_from_slice(&lbl);
        if let Some(ref n) = net { out.extend_from_slice(n); }
        if let Some(ref n) = nod { out.extend_from_slice(n); }
        if let Some(ref d) = dem { out.extend_from_slice(d); }

        debug_assert_eq!(out.len(), cursor as usize, "taille GMP finale");
        out
    }
}

// ── Helpers de relocalisation ───────────────────────────────────────────────
//
// Chaque writer imgforge produit un blob standalone dont les offsets de header
// sont absolus depuis le byte 0 du blob (convention « relatif au fichier »).
// Pour intégrer le blob dans un GMP qui commence à `gmp_start`, on ajoute
// `gmp_start` à chaque champ offset du header.
//
// Règle : si la valeur lue est 0 (section absente), on ne touche pas (0 resterait
// à 0 même après addition, mais la sémantique « pas de section » doit être préservée
// pour les sections optionnelles de RGN).

fn add_u32_offset(blob: &mut [u8], pos: usize, delta: u32) {
    debug_assert!(pos + 4 <= blob.len(),
        "offset field @{pos} hors-bornes (blob.len={})", blob.len());
    if pos + 4 > blob.len() { return; }
    let v = u32::from_le_bytes(blob[pos..pos + 4].try_into().unwrap());
    // 0 = section absente (sentinel Garmin) : ne pas relocater.
    if v != 0 {
        let patched = v.wrapping_add(delta);
        blob[pos..pos + 4].copy_from_slice(&patched.to_le_bytes());
    }
}

/// Relocalise les champs offset du header TRE (hlen = 188 = TRE_HEADER_LEN).
/// Positions : map_levels, subdivisions, copyright, polyline_ov, polygon_ov,
///             point_ov, extTypeOffsets, extTypeOverviews — toutes < 188.
fn relocate_tre(blob: &mut Vec<u8>, delta: u32) {
    for &pos in &[33usize, 41, 49, 74, 88, 102, 124, 138] {
        add_u32_offset(blob, pos, delta);
    }
}

/// Relocalise les champs offset du header RGN (hlen = 125 = RGN_HEADER_LEN).
/// data_offset toujours présent ; ext_areas/lines/points = 0 si absents.
fn relocate_rgn(blob: &mut Vec<u8>, delta: u32) {
    for &pos in &[21usize, 29, 57, 85] {
        add_u32_offset(blob, pos, delta);
    }
}

/// Relocalise les champs offset du header LBL (hlen = 196 = LBL_HEADER_LEN).
/// Inclut label_data, toutes les sections PlacesHeader, sort_desc, last_position.
fn relocate_lbl(blob: &mut Vec<u8>, delta: u32) {
    for &pos in &[21usize, 31, 45, 59, 73, 87, 100, 114, 128, 142, 156, 176, 184] {
        add_u32_offset(blob, pos, delta);
    }
}

/// Relocalise les champs offset du header NET (hlen = 55 = NET_HEADER_LEN).
/// net1_offset @21, net2_offset @30, net3_offset @39.
fn relocate_net(blob: &mut Vec<u8>, delta: u32) {
    for &pos in &[21usize, 30, 39] {
        add_u32_offset(blob, pos, delta);
    }
}

/// Relocalise les champs offset du header NOD (hlen = 127 = NOD_HEADER_LEN).
/// nod1_offset @21, nod2_offset @37, nod3_offset @49, nod4_offset @63.
fn relocate_nod(blob: &mut Vec<u8>, delta: u32) {
    for &pos in &[21usize, 37, 49, 63] {
        add_u32_offset(blob, pos, delta);
    }
}

/// Relocalise le header DEM (hlen = 41) + les section-headers dans le body.
/// Main header : sections_offset @33.
/// Section headers (60 B chacun) — layout complet :
///   [0]  unknown   [1]  zoom_level
///   [2-5]  points_per_lat (u32)   [6-9]   points_per_lon (u32)
///   [10-13] non_std_height-1 (u32) [14-17] non_std_width-1 (u32)
///   [18-19] flags (u16)
///   [20-23] tiles_lon-1 (u32)     [24-27] tiles_lat-1 (u32)
///   [28-29] record_desc (u16)     [30-31] tile_desc_size (u16)
///   [32-35] data_offset (u32) ← descripteurs de tuiles   ← à relocater
///   [36-39] data_offset2 (u32) ← bitstream data          ← à relocater
fn relocate_dem(blob: &mut Vec<u8>, delta: u32) {
    if blob.len() < 41 { return; }
    // Lire zoom_count et sections_offset AVANT de patcher
    let zoom_count = u16::from_le_bytes([blob[25], blob[26]]) as usize;
    let sections_offset = u32::from_le_bytes(blob[33..37].try_into().unwrap()) as usize;

    // Patcher le header principal
    add_u32_offset(blob, 33, delta);

    // Patcher chaque section header (60 B) dans la zone body
    for i in 0..zoom_count {
        let base = sections_offset + i * 60;
        add_u32_offset(blob, base + 32, delta); // data_offset  (tile descriptor table)
        add_u32_offset(blob, base + 36, delta); // data_offset2 (bitstream data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::img::tre::TreWriter;
    use crate::img::rgn::RgnWriter;
    use crate::img::lbl::{LblWriter};
    use crate::img::labelenc::LabelEncoding;

    #[test]
    fn header_is_61_bytes_and_copyright_179() {
        assert_eq!(GMP_HEADER_LEN, 0x3D);
        // 179 B : de 0x3D à 0xF0 (exclu) — 0xF0 = tre_offset sur les samples Topo France v6 Pro.
        assert_eq!(GMP_COPYRIGHT.len(), 0xB3);
    }

    #[test]
    fn byte_exact_header_matches_topofrance_sample_layout() {
        // Simule les headers inline des 6 sous-sections avec les TAILLES observées sur le sample 5922.
        // Tailles headers+data (pour forcer rgn_offset=0x263, etc.) :
        //   TRE zone intra-GMP : 0x263 - 0xF0 = 0x173 (371 B)
        //   RGN zone intra-GMP : 0x3D4 - 0x263 = 0x171 (369 B)
        //   LBL : 0x67D - 0x3D4 = 0x2A9 (681 B)
        //   NET : 0x6E1 - 0x67D = 0x64 (100 B)
        //   NOD : 0x7E6 - 0x6E1 = 0x105 (261 B)
        //   DEM : quelconque
        let tre = vec![0xAA; 0x173];
        let rgn = vec![0xBB; 0x171];
        let lbl = vec![0xCC; 0x2A9];
        let net = vec![0xDD; 0x64];
        let nod = vec![0xEE; 0x105];
        // DEM : zoom_count=0 (bytes 25-26) pour éviter une boucle sur sections parasites.
        let dem = vec![0x00u8; 42];

        let gmp = GmpWriter::new(tre, rgn, lbl, Some(net), Some(nod), Some(dem))
            .with_date(2021, 4, 16, 11, 59, 27)
            .write();

        // Header byte-exact vs sample 05445922.GMP (hex-dumpé en T1) :
        //   3d 00 47 41 52 4D 49 4E 20 47 4D 50 01 00 E5 07 04 10 0B 3B 1B 00 00 00 00
        //   00 00 00 00 F0 00 00 00 63 02 00 00 D4 03 00 00 7D 06 00 00 E1 06 00 00 E6 07 00 00 00 00
        //   00 00 00 00 00 00 00 00
        let expected_header: [u8; 0x3D] = [
            0x3D, 0x00,
            0x47, 0x41, 0x52, 0x4D, 0x49, 0x4E, 0x20, 0x47, 0x4D, 0x50, // "GARMIN GMP"
            0x01, 0x00,
            0xE5, 0x07,                     // year 2021
            0x04, 0x10, 0x0B, 0x3B, 0x1B,   // 04/16 11:59:27
            0x00, 0x00, 0x00, 0x00,         // unknown_015 = 0
            0xF0, 0x00, 0x00, 0x00,         // tre_offset = 0xF0
            0x63, 0x02, 0x00, 0x00,         // rgn_offset = 0x263
            0xD4, 0x03, 0x00, 0x00,         // lbl_offset = 0x3D4
            0x7D, 0x06, 0x00, 0x00,         // net_offset = 0x67D
            0xE1, 0x06, 0x00, 0x00,         // nod_offset = 0x6E1
            0xE6, 0x07, 0x00, 0x00,         // dem_offset = 0x7E6
            0x00, 0x00, 0x00, 0x00,         // mar_offset = 0
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // reserved
        ];
        assert_eq!(&gmp[0..0x3D], &expected_header, "header GMP doit matcher byte-exact le sample Topo France v6 Pro");

        // Copyright immédiatement après le header, 179 bytes.
        assert_eq!(&gmp[0x3D..0x3D + 0xB3], GMP_COPYRIGHT);
    }

    #[test]
    fn optional_sections_get_offset_zero() {
        // Blobs réels pour satisfaire les debug_assert de relocalisation (hlen >= 188/125/196).
        let tre = TreWriter::new().build();
        let rgn = RgnWriter::new().build();
        let lbl = LblWriter::new(LabelEncoding::Format6).build();
        let total_blobs = tre.len() + rgn.len() + lbl.len();
        let gmp = GmpWriter::new(tre, rgn, lbl, None, None, None)
            .with_date(2026, 4, 21, 12, 0, 0)
            .write();
        // NET/NOD/DEM offsets = 0
        let net_off = u32::from_le_bytes([gmp[0x25], gmp[0x26], gmp[0x27], gmp[0x28]]);
        let nod_off = u32::from_le_bytes([gmp[0x29], gmp[0x2A], gmp[0x2B], gmp[0x2C]]);
        let dem_off = u32::from_le_bytes([gmp[0x2D], gmp[0x2E], gmp[0x2F], gmp[0x30]]);
        assert_eq!(net_off, 0);
        assert_eq!(nod_off, 0);
        assert_eq!(dem_off, 0);
        // TRE non étendu (hlen=188 conservé, pas d'extension NT).
        assert_eq!(gmp.len(), 0x3D + 0xB3 + total_blobs);
    }

    /// Vérifie que les offsets internes du header TRE sont correctement relocalisés
    /// vers des positions absolues dans le GMP.
    ///
    /// TreWriter standalone écrit map_levels_offset = TRE_HEADER_LEN (188).
    /// GmpWriter ne modifie pas hlen (pas d'extension NT), relocalise seulement :
    /// map_levels_offset → TRE_HEADER_LEN (188) + gmp_tre_start (0xF0=240) = 428.
    #[test]
    fn gmp_v2_tre_header_offsets_relocated() {
        use crate::img::tre::TRE_HEADER_LEN;

        let tre_blob = TreWriter::new().build();
        let rgn_blob = RgnWriter::new().build();
        let lbl_blob = LblWriter::new(LabelEncoding::Format6).build();

        let original_ml_offset = u32::from_le_bytes(tre_blob[33..37].try_into().unwrap());
        assert_eq!(original_ml_offset, TRE_HEADER_LEN as u32,
            "TreWriter sans copyright blob : map_levels_offset doit être TRE_HEADER_LEN");

        let gmp = GmpWriter::new(
            tre_blob.clone(), rgn_blob, lbl_blob, None, None, None,
        ).with_date(2026, 4, 24, 0, 0, 0).write();

        let gmp_tre_start = u32::from_le_bytes([gmp[0x19], gmp[0x1A], gmp[0x1B], gmp[0x1C]]);
        assert_eq!(gmp_tre_start, 0xF0);

        let tre_in_gmp = &gmp[gmp_tre_start as usize..];
        let relocated_ml_offset = u32::from_le_bytes(tre_in_gmp[33..37].try_into().unwrap());
        assert_eq!(relocated_ml_offset, original_ml_offset + gmp_tre_start,
            "map_levels_offset doit être relocalisé vers GMP-absolu (GMP base seul, pas d'ext NT)");

        assert_eq!(&tre_in_gmp[2..12], b"GARMIN TRE");
    }

    /// Vérifie que les offsets RGN (data_offset @21) sont relocalisés.
    #[test]
    fn gmp_v2_rgn_data_offset_relocated() {
        use crate::img::rgn::RGN_HEADER_LEN;

        let mut rgn_writer = RgnWriter::new();
        rgn_writer.write_subdivision(&[0xAB; 5], &[], &[], &[]);
        let rgn_blob = rgn_writer.build();

        // data_offset standalone = RGN_HEADER_LEN = 125
        let original_data_offset = u32::from_le_bytes(rgn_blob[21..25].try_into().unwrap());
        assert_eq!(original_data_offset, RGN_HEADER_LEN as u32);

        let tre_blob = TreWriter::new().build();
        let lbl_blob = LblWriter::new(LabelEncoding::Format6).build();

        let gmp = GmpWriter::new(tre_blob.clone(), rgn_blob.clone(), lbl_blob, None, None, None)
            .with_date(2026, 4, 24, 0, 0, 0).write();

        let gmp_rgn_start = 0xF0u32 + tre_blob.len() as u32;

        let rgn_in_gmp = &gmp[gmp_rgn_start as usize..];
        let relocated = u32::from_le_bytes(rgn_in_gmp[21..25].try_into().unwrap());
        assert_eq!(relocated, original_data_offset + gmp_rgn_start,
            "RGN data_offset doit être relocalisé vers GMP-absolu");
        assert_eq!(&rgn_in_gmp[2..12], b"GARMIN RGN");
    }

    /// Vérifie que les offsets LBL (label_data_offset @21) sont relocalisés.
    #[test]
    fn gmp_v2_lbl_label_offset_relocated() {
        use crate::img::lbl::LBL_HEADER_LEN;

        let lbl_blob = LblWriter::new(LabelEncoding::Format6).build();
        // label_data_offset standalone = LBL_HEADER_LEN (pas de sort descriptor pour Format6)
        let original_lbl_offset = u32::from_le_bytes(lbl_blob[21..25].try_into().unwrap());
        assert_eq!(original_lbl_offset, LBL_HEADER_LEN as u32);

        let tre_blob = TreWriter::new().build();
        let rgn_blob = RgnWriter::new().build();

        let gmp = GmpWriter::new(tre_blob.clone(), rgn_blob.clone(), lbl_blob.clone(), None, None, None)
            .with_date(2026, 4, 24, 0, 0, 0).write();

        let gmp_lbl_start = 0xF0u32 + tre_blob.len() as u32 + rgn_blob.len() as u32;

        let lbl_in_gmp = &gmp[gmp_lbl_start as usize..];
        let relocated = u32::from_le_bytes(lbl_in_gmp[21..25].try_into().unwrap());
        assert_eq!(relocated, original_lbl_offset + gmp_lbl_start,
            "LBL label_data_offset doit être relocalisé vers GMP-absolu");
        assert_eq!(&lbl_in_gmp[2..12], b"GARMIN LBL");
    }

    /// Vérifie que les sections optionnelles absentes (offset = 0) ne sont pas touchées par
    /// la relocalisation — un 0 doit rester 0 dans le header RGN.
    #[test]
    fn gmp_v2_zero_offsets_not_relocated() {
        let rgn_blob = RgnWriter::new().build(); // pas de données ext
        let original_ext_areas = u32::from_le_bytes(rgn_blob[29..33].try_into().unwrap());
        assert_eq!(original_ext_areas, 0, "pas de ext_areas → offset = 0");

        let tre_blob = TreWriter::new().build();
        let lbl_blob = LblWriter::new(LabelEncoding::Format6).build();

        let gmp = GmpWriter::new(tre_blob.clone(), rgn_blob.clone(), lbl_blob, None, None, None)
            .with_date(2026, 4, 24, 0, 0, 0).write();

        let gmp_rgn_start = (0xF0u32 + tre_blob.len() as u32) as usize;
        let ext_areas_in_gmp = u32::from_le_bytes(
            gmp[gmp_rgn_start + 29..gmp_rgn_start + 33].try_into().unwrap()
        );
        assert_eq!(ext_areas_in_gmp, 0, "offset 0 (section absente) ne doit pas être relocalisé");
    }
}
