use std::path::Path;

// Chemins des fixtures
const SAMPLE_TXT: &str = "tests/fixtures/sample.txt";
const IGNBDTOPO_TXT: &str = "../../pipeline/resources/typfiles/IGNBDTOPO.txt";
const IGNBDTOPO_TYP: &str = "../../pipeline/resources/typfiles/IGNBDTOPO.typ";

fn load_bytes(path: &str) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("Cannot read {}: {}", path, e))
}

/// Round-trip texte : parse sample.txt → write → parse → comparer champs clés.
#[test]
fn round_trip_text() {
    let bytes = load_bytes(SAMPLE_TXT);

    use typforge::typ::text_reader;
    use typforge::typ::text_writer;

    let doc1 = text_reader::parse(&bytes).expect("parse sample.txt");

    // Vérifications sur doc1
    assert_eq!(doc1.param.family_id, 1100);
    assert_eq!(doc1.param.product_id, 1);
    assert_eq!(doc1.param.codepage, 1252);
    assert!(!doc1.polygons.is_empty(), "Pas de polygones");
    assert!(!doc1.lines.is_empty(), "Pas de lignes");
    assert!(!doc1.points.is_empty(), "Pas de points");

    // Écriture puis re-parse
    let out_bytes = text_writer::write(&doc1).expect("write doc1");
    let doc2 = text_reader::parse(&out_bytes).expect("re-parse");

    assert_eq!(doc2.param.family_id, doc1.param.family_id);
    assert_eq!(doc2.param.product_id, doc1.param.product_id);
    assert_eq!(doc2.param.codepage, doc1.param.codepage);
    assert_eq!(doc2.polygons.len(), doc1.polygons.len());
    assert_eq!(doc2.lines.len(), doc1.lines.len());
    assert_eq!(doc2.points.len(), doc1.points.len());

    // Vérifier que type_code est préservé
    assert_eq!(doc2.polygons[0].type_code, doc1.polygons[0].type_code);
    assert_eq!(doc2.lines[0].type_code, doc1.lines[0].type_code);
    assert_eq!(doc2.points[0].type_code, doc1.points[0].type_code);

    // Vérifier les labels du point
    let p1 = &doc1.points[0];
    let p2 = &doc2.points[0];
    assert_eq!(p1.labels.len(), p2.labels.len());
    for (l1, l2) in p1.labels.iter().zip(p2.labels.iter()) {
        assert_eq!(l1.lang, l2.lang);
        assert_eq!(l1.text, l2.text);
    }
}

/// Round-trip binaire : compile sample.txt → decompile → recompile → byte-identical.
#[test]
fn round_trip_binary() {
    let bytes = load_bytes(SAMPLE_TXT);

    use typforge::typ::text_reader;
    use typforge::typ::binary_writer;
    use typforge::typ::binary_reader;

    let doc = text_reader::parse(&bytes).expect("parse sample.txt");
    let bin1 = binary_writer::compile(&doc).expect("compile 1");
    let doc2 = binary_reader::decompile(&bin1).expect("decompile");
    let bin2 = binary_writer::compile(&doc2).expect("compile 2");

    assert_eq!(bin1, bin2, "Round-trip binaire non byte-identical");
}

/// Parse IGNBDTOPO.txt et vérifier les comptages.
#[test]
fn ignbdtopo_parse() {
    if !Path::new(IGNBDTOPO_TXT).exists() {
        eprintln!("SKIP: {} absent", IGNBDTOPO_TXT);
        return;
    }
    let bytes = load_bytes(IGNBDTOPO_TXT);
    use typforge::typ::text_reader;
    let doc = text_reader::parse(&bytes).expect("parse IGNBDTOPO.txt");
    assert_eq!(doc.polygons.len(), 132, "attendu 132 polygones, trouvé {}", doc.polygons.len());
    assert_eq!(doc.lines.len(), 93, "attendu 93 lignes, trouvé {}", doc.lines.len());
    assert_eq!(doc.points.len(), 330, "attendu 330 points, trouvé {}", doc.points.len());
}

/// Compile IGNBDTOPO.txt et compare avec IGNBDTOPO.typ (tolérance padding).
#[test]
fn ignbdtopo_compile() {
    if !Path::new(IGNBDTOPO_TXT).exists() || !Path::new(IGNBDTOPO_TYP).exists() {
        eprintln!("SKIP: fixtures absentes");
        return;
    }
    let txt_bytes = load_bytes(IGNBDTOPO_TXT);
    let ref_typ = load_bytes(IGNBDTOPO_TYP);

    use typforge::typ::text_reader;
    use typforge::typ::binary_writer;
    use typforge::typ::binary_reader;

    let doc = text_reader::parse(&txt_bytes).expect("parse IGNBDTOPO.txt");
    let compiled = binary_writer::compile(&doc).expect("compile IGNBDTOPO");

    // Décompiler les deux et comparer le modèle (pas byte-à-byte à cause des
    // dates et du padding)
    let doc_compiled = binary_reader::decompile(&compiled).expect("decompile compiled");
    let doc_ref = binary_reader::decompile(&ref_typ).expect("decompile reference");

    assert_eq!(doc_compiled.polygons.len(), doc_ref.polygons.len(), "polygones");
    assert_eq!(doc_compiled.lines.len(), doc_ref.lines.len(), "lignes");
    assert_eq!(doc_compiled.points.len(), doc_ref.points.len(), "points");
}

/// XPM import PNG : créer un PNG 32×32 synthétique et vérifier la conversion.
#[test]
fn xpm_import_png() {
    use typforge::typ::xpm;
    use typforge::typ::model::ColorMode;

    // Créer un PNG 4×4 en mémoire
    let mut img_data = Vec::new();
    {
        // Tiny PNG 4×4 blanc
        let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 255, 255, 255]));
        let mut cursor = std::io::Cursor::new(&mut img_data);
        img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();
    }

    let result = xpm::import_image(&img_data).expect("import PNG");
    assert_eq!(result.width, 4);
    assert_eq!(result.height, 4);
    assert!(result.palette.len() <= 16, "palette > 16 couleurs: {}", result.palette.len());
    assert_eq!(result.colour_mode, ColorMode::Indexed);
}

/// XPM round-trip : parse XPM texte → render → régénérer → palette et pixels identiques.
#[test]
fn xpm_round_trip() {
    use typforge::typ::model::{ColorMode, Rgba, Xpm};
    use typforge::typ::xpm::{xpm_to_image, image_to_xpm};

    let original_pixels = vec![
        vec![Rgba::opaque(255, 0, 0), Rgba::opaque(0, 255, 0)],
        vec![Rgba::opaque(0, 0, 255), Rgba::transparent()],
    ];

    let xpm = image_to_xpm(&original_pixels, ColorMode::Indexed);
    let restored = xpm_to_image(&xpm);

    assert_eq!(restored[0][0], Rgba::opaque(255, 0, 0));
    assert_eq!(restored[0][1], Rgba::opaque(0, 255, 0));
    assert_eq!(restored[1][0], Rgba::opaque(0, 0, 255));
    assert_eq!(restored[1][1], Rgba::transparent());
}
