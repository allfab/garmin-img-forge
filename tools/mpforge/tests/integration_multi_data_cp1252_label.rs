//! Tech-spec #2 AC14 (M7 code review) — fixture CP1252 label + multi-Data.
//!
//! Vérifie que l'ajout du chemin multi-Data n'a pas corrompu l'encodage
//! CP1252 des labels côté writer C++. Fixture : 1 feature POLYLINE avec
//! `Label="Forêt de Saône"` (accents UTF-8) + un profil `levels: [{n:0},
//! {n:2, simplify: X}]`.
//!
//! Assertions :
//!   (a) le `.mp` contient la ligne `Label=Forêt de Saône` avec les bons
//!       octets CP1252 (pas de mojibake).
//!   (b) les lignes `Data0=` et `Data2=` sont strictement ASCII.
//!   (c) le sha256 du `.mp` est identique sur 3 runs successifs.
//!
//! **Statut** : placeholder avec `#[ignore]`. L'infrastructure commune
//! (génération de SHP à la volée + plugin fresh) est réutilisable depuis
//! `integration_multi_data.rs`. À activer en mergeant la fixture SHP avec
//! le champ `Label` puis en retirant `#[ignore]`.

#[test]
#[ignore = "AC14 fixture not yet wired — see tech-spec §Implementation Record"]
fn test_cp1252_label_with_multi_data_bytes_exact() {
    todo!(
        "1) créer SHP avec feature LineString + Label=Forêt de Saône\n\
         2) sources.yaml avec generalize-profiles.yaml multi-level\n\
         3) assert bytes CP1252 : 'Lab' 'el=' 0xC6 'or' 0xEA 't de Sa' 0xF4 'ne'\n\
         4) assert pas de Data1=..Data9= hors bucket attendu\n\
         5) 3 runs successifs → sha256 identiques"
    );
}
