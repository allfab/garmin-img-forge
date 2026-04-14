//! Tests d'intégration CLI pour la sous-commande `imgforge typ`.
//!
//! Utilise `assert_cmd` pour invoquer le binaire compilé et `tempfile` pour
//! isoler les fichiers de sortie.

use assert_cmd::Command;
use std::path::PathBuf;
use tempfile::tempdir;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../pipeline/resources/typfiles")
}

#[test]
fn compile_real_fixture_produces_valid_header() {
    let input = fixtures_dir().join("I2023100.txt");
    if !input.exists() {
        eprintln!("fixture absente : skip");
        return;
    }
    let dir = tempdir().unwrap();
    let output = dir.path().join("out.typ");

    Command::cargo_bin("imgforge")
        .unwrap()
        .arg("typ")
        .arg("compile")
        .arg(input.to_str().unwrap())
        .arg("-o")
        .arg(output.to_str().unwrap())
        .assert()
        .success();

    let bytes = std::fs::read(&output).expect("output exists");
    assert!(bytes.len() > 156, "binary trop court");
    assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), 0x9C);
    assert_eq!(&bytes[2..12], b"GARMIN TYP");
    // Codepage à l'offset 0x15 = 1252.
    assert_eq!(u16::from_le_bytes([bytes[0x15], bytes[0x16]]), 1252);
    // FID à l'offset 0x2F = 1100.
    assert_eq!(u16::from_le_bytes([bytes[0x2F], bytes[0x30]]), 1100);
}

/// Round-trip complet sur un TYP minimal (pas de bitmaps complexes). Valide
/// que `compile → decompile` préserve les params et le drawOrder.
#[test]
fn compile_decompile_roundtrip_minimal() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("min.txt");
    let content = r#"[_id]
ProductCode=1
FID=1100
CodePage=1252
[end]

[_drawOrder]
Type=0x01,1
Type=0x02,1
Type=0x03,2
[end]
"#;
    std::fs::write(&input, content).unwrap();

    let typ_path = dir.path().join("mid.typ");
    let txt_path = dir.path().join("out.txt");

    Command::cargo_bin("imgforge")
        .unwrap()
        .args([
            "typ", "compile",
            input.to_str().unwrap(),
            "-o", typ_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    Command::cargo_bin("imgforge")
        .unwrap()
        .args([
            "typ", "decompile",
            typ_path.to_str().unwrap(),
            "-o", txt_path.to_str().unwrap(),
            "--encoding", "utf8",
        ])
        .assert()
        .success();

    let text = std::fs::read_to_string(&txt_path).expect("output text");
    assert!(text.contains("[_id]"));
    assert!(text.contains("FID=1100"));
    assert!(text.contains("ProductCode=1"));
    assert!(text.contains("CodePage=1252"));
    assert!(text.contains("[_drawOrder]"));
    // Vérifier qu'au moins les 3 entrées drawOrder sont préservées.
    assert!(text.contains("Type=0x1"));
    assert!(text.contains("Type=0x2"));
    assert!(text.contains("Type=0x3"));
}

#[test]
fn default_output_extension_swap() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("sample.txt");
    let content = "[_id]\nProductCode=1\nFID=1234\nCodePage=1252\n[end]\n";
    std::fs::write(&input, content).unwrap();

    Command::cargo_bin("imgforge")
        .unwrap()
        .args(["typ", "compile", input.to_str().unwrap()])
        .assert()
        .success();

    let default_out = dir.path().join("sample.typ");
    assert!(default_out.exists(), "sortie par défaut manquante");
    let bytes = std::fs::read(&default_out).unwrap();
    assert_eq!(&bytes[2..12], b"GARMIN TYP");
}

#[test]
fn invalid_binary_exits_nonzero() {
    let dir = tempdir().unwrap();
    let bad = dir.path().join("bad.typ");
    std::fs::write(&bad, b"NOT A TYP FILE AT ALL").unwrap();

    Command::cargo_bin("imgforge")
        .unwrap()
        .args(["typ", "decompile", bad.to_str().unwrap()])
        .assert()
        .failure();
}

#[test]
fn cp1252_roundtrip_preserves_accent_byte() {
    // Source : texte CP1252 avec `é` = 0xE9.
    let dir = tempdir().unwrap();
    let input = dir.path().join("cp.txt");
    let content: Vec<u8> = [
        b"[_id]\nFID=1\nProductCode=1\nCodePage=1252\n[end]\n".as_ref(),
        b"[_polygon]\nType=0x01\n".as_ref(),
        b"Xpm=\"0 0 2 0\"\n\"1 c #E0E0E0\"\n\"2 c #101010\"\n".as_ref(),
        b"String1=0x04,caf\xE9\n".as_ref(),
        b"[end]\n".as_ref(),
    ]
    .concat();
    std::fs::write(&input, &content).unwrap();

    let typ_path = dir.path().join("out.typ");
    Command::cargo_bin("imgforge")
        .unwrap()
        .args([
            "typ",
            "compile",
            input.to_str().unwrap(),
            "-o",
            typ_path.to_str().unwrap(),
            "--encoding",
            "cp1252",
        ])
        .assert()
        .success();

    let typ_bytes = std::fs::read(&typ_path).unwrap();
    // Le label "café" doit apparaître en CP1252 (0x63 0x61 0x66 0xE9).
    let needle: &[u8] = b"caf\xE9";
    assert!(
        typ_bytes.windows(needle.len()).any(|w| w == needle),
        "label CP1252 non trouvé dans le binaire"
    );
}
