// GmpWriter — emballe les 6 sous-sections TRE/RGN/LBL/NET/NOD/DEM dans un conteneur
// .GMP Garmin ("NT format"), consolidant 6 fichiers FAT en 1 seul par tuile.
//
// Spec binaire : docs/implementation-artifacts/imgforge-gmp-format.md
// Source RE : gimgtools garmin_struct.h (GPL, lecture-spec uniquement, pas de recopie)

use super::common_header::{now_secs, unix_to_calendar};

pub const GMP_HEADER_LEN: u16 = 0x3D; // 61 bytes

/// Copyright block exactly as emitted by Garmin TopoFrance v6 Pro (2021-05).
/// Deux C-strings NUL-terminées concaténées ; 179 bytes au total (0xB3).
/// C'est une donnée de format (équivalent magic number) — requise pour byte-exact
/// match avec les samples officiels et probablement attendue par certains firmwares.
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

    /// Produit le blob `.GMP` complet : header(0x3D) + copyright(181) + TRE + RGN + LBL [+ NET] [+ NOD] [+ DEM].
    pub fn write(&self) -> Vec<u8> {
        let header_len = GMP_HEADER_LEN as usize;
        let copyright_len = GMP_COPYRIGHT.len();
        let body_base = header_len + copyright_len;

        // Offsets pointent sur le début du header de chaque sous-section (2 B avant
        // le magic `GARMIN XXX` inline, cf. imgforge-gmp-format.md).
        let tre_offset = body_base as u32;
        let rgn_offset = tre_offset + self.tre.len() as u32;
        let lbl_offset = rgn_offset + self.rgn.len() as u32;

        let mut cursor = lbl_offset + self.lbl.len() as u32;
        let net_offset = if let Some(ref net) = self.net {
            let o = cursor; cursor += net.len() as u32; o
        } else { 0 };
        let nod_offset = if let Some(ref nod) = self.nod {
            let o = cursor; cursor += nod.len() as u32; o
        } else { 0 };
        let dem_offset = if let Some(ref dem) = self.dem {
            let o = cursor; cursor += dem.len() as u32; o
        } else { 0 };
        let _ = cursor;

        let mut out = Vec::with_capacity(body_base + self.tre.len() + self.rgn.len() + self.lbl.len());

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
        out.extend_from_slice(&tre_offset.to_le_bytes());            // 0x19..0x1C
        out.extend_from_slice(&rgn_offset.to_le_bytes());            // 0x1D..0x20
        out.extend_from_slice(&lbl_offset.to_le_bytes());            // 0x21..0x24
        out.extend_from_slice(&net_offset.to_le_bytes());            // 0x25..0x28
        out.extend_from_slice(&nod_offset.to_le_bytes());            // 0x29..0x2C
        out.extend_from_slice(&dem_offset.to_le_bytes());            // 0x2D..0x30
        out.extend_from_slice(&0u32.to_le_bytes());                  // 0x31..0x34 : mar_offset = 0

        // 0x35..0x3C : 8 bytes reserved (zéros) — hlen=0x3D donc on remplit jusqu'à 0x3D exclusif
        while out.len() < header_len {
            out.push(0x00);
        }
        debug_assert_eq!(out.len(), header_len);

        // === Copyright ===
        out.extend_from_slice(GMP_COPYRIGHT);
        debug_assert_eq!(out.len(), body_base);

        // === Sous-sections concaténées ===
        out.extend_from_slice(&self.tre);
        out.extend_from_slice(&self.rgn);
        out.extend_from_slice(&self.lbl);
        if let Some(ref net) = self.net { out.extend_from_slice(net); }
        if let Some(ref nod) = self.nod { out.extend_from_slice(nod); }
        if let Some(ref dem) = self.dem { out.extend_from_slice(dem); }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let dem = vec![0xFF; 42];

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

        // Sous-sections concaténées à partir de 0xF0 (= tre_offset).
        assert_eq!(gmp[0xF0], 0xAA, "TRE commence à 0xF0");
        assert_eq!(gmp[0x263], 0xBB, "RGN commence à 0x263");
        assert_eq!(gmp[0x3D4], 0xCC, "LBL commence à 0x3D4");
        assert_eq!(gmp[0x67D], 0xDD, "NET commence à 0x67D");
        assert_eq!(gmp[0x6E1], 0xEE, "NOD commence à 0x6E1");
        assert_eq!(gmp[0x7E6], 0xFF, "DEM commence à 0x7E6");
    }

    #[test]
    fn optional_sections_get_offset_zero() {
        let tre = vec![0x01; 10];
        let rgn = vec![0x02; 10];
        let lbl = vec![0x03; 10];
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
        assert_eq!(gmp.len(), 0x3D + 0xB3 + 30);
    }

    #[test]
    fn roundtrip_subsections_recover_original_bytes() {
        let tre = vec![0x11; 100];
        let rgn = vec![0x22; 200];
        let lbl = vec![0x33; 150];
        let net = vec![0x44; 50];
        let nod = vec![0x55; 75];
        let dem = vec![0x66; 123];
        let gmp = GmpWriter::new(tre.clone(), rgn.clone(), lbl.clone(), Some(net.clone()), Some(nod.clone()), Some(dem.clone()))
            .with_date(2026, 4, 21, 12, 0, 0)
            .write();

        let tre_off = u32::from_le_bytes([gmp[0x19], gmp[0x1A], gmp[0x1B], gmp[0x1C]]) as usize;
        let rgn_off = u32::from_le_bytes([gmp[0x1D], gmp[0x1E], gmp[0x1F], gmp[0x20]]) as usize;
        let lbl_off = u32::from_le_bytes([gmp[0x21], gmp[0x22], gmp[0x23], gmp[0x24]]) as usize;
        let net_off = u32::from_le_bytes([gmp[0x25], gmp[0x26], gmp[0x27], gmp[0x28]]) as usize;
        let nod_off = u32::from_le_bytes([gmp[0x29], gmp[0x2A], gmp[0x2B], gmp[0x2C]]) as usize;
        let dem_off = u32::from_le_bytes([gmp[0x2D], gmp[0x2E], gmp[0x2F], gmp[0x30]]) as usize;

        assert_eq!(&gmp[tre_off..tre_off + tre.len()], &tre[..]);
        assert_eq!(&gmp[rgn_off..rgn_off + rgn.len()], &rgn[..]);
        assert_eq!(&gmp[lbl_off..lbl_off + lbl.len()], &lbl[..]);
        assert_eq!(&gmp[net_off..net_off + net.len()], &net[..]);
        assert_eq!(&gmp[nod_off..nod_off + nod.len()], &nod[..]);
        assert_eq!(&gmp[dem_off..dem_off + dem.len()], &dem[..]);
    }
}
