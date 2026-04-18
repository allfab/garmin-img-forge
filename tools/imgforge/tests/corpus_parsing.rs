//! Test corpus FRANCE-SE — robustesse du parser sur 460 `.mp` (~31 GB).
//!
//! Lance avec :
//!   IMGFORGE_FRANCE_SE_CORPUS=/chemin cargo test --test corpus_parsing -- --ignored
//!
//! Sans la variable d'environnement, le test est silencieusement skip.
//! Filet de sécurité local non-CI : volumineux, ne tourne pas sur la CI.

use imgforge::parser;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[test]
#[ignore]
fn parse_full_france_se_corpus() {
    let Ok(corpus_dir) = std::env::var("IMGFORGE_FRANCE_SE_CORPUS") else {
        eprintln!("IMGFORGE_FRANCE_SE_CORPUS non défini — skip");
        return;
    };
    let mp_dir = PathBuf::from(corpus_dir);
    if !mp_dir.exists() {
        panic!("corpus introuvable : {}", mp_dir.display());
    }

    let mut total_files = 0usize;
    let mut total_bytes = 0u64;
    let mut errors: Vec<(String, String)> = Vec::new();
    let mut counts_by_type: HashMap<u32, (usize, usize)> = HashMap::new(); // type → (features, vertices_l0)

    for entry in fs::read_dir(&mp_dir).expect("read corpus dir") {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("mp") {
            continue;
        }
        total_files += 1;
        let bytes = fs::read(&path).unwrap();
        total_bytes += bytes.len() as u64;
        let content = String::from_utf8_lossy(&bytes);
        match parser::parse_mp(&content) {
            Ok(mp) => {
                for pl in &mp.polylines {
                    let entry = counts_by_type.entry(pl.type_code).or_default();
                    entry.0 += 1;
                    entry.1 += pl.geometry_for_level(0).len();
                }
                for pg in &mp.polygons {
                    let entry = counts_by_type.entry(pg.type_code).or_default();
                    entry.0 += 1;
                    entry.1 += pg.geometry_for_level(0).len();
                }
            }
            Err(e) => errors.push((path.display().to_string(), e.to_string())),
        }
    }

    eprintln!("Corpus parcouru : {} fichiers, {} MB", total_files, total_bytes / 1024 / 1024);
    eprintln!("Types distincts : {}", counts_by_type.len());
    for (type_code, (n, vtx)) in counts_by_type.iter().take(15) {
        eprintln!("  type 0x{:04X} : {} features, {} vertices L0", type_code, n, vtx);
    }
    assert!(errors.is_empty(), "parser errors:\n{:#?}", errors);
}
