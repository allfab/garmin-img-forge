//! Tech-spec #2 AC16 (M7 code review) — intersections partagées Autoroute +
//! Nationale après simplification DP.
//!
//! Fixture synthétique : deux TRONCON_DE_ROUTE avec `CL_ADMIN=Autoroute` et
//! `CL_ADMIN=Nationale` partageant un point d'intersection (même coord aux
//! endpoints). Un profil routable applique `simplify: 0.00002` au bucket
//! `n:0`.
//!
//! Assertions :
//!   (a) après DP, les deux TRONCON conservent la coordonnée d'intersection
//!       byte-identique (propriété mathématique du DP sur endpoints).
//!   (b) dans l'IMG produit, les deux arcs NOD partagent exactement le même
//!       nœud (extraction `gmt -i -v` + comparaison coord).
//!
//! **Statut** : placeholder. Requiert un `imgforge build` chaîné dans le
//! test + parsing `gmt -iv` output — dépendance lourde. Fait bouger la
//! matrix CI.

#[test]
#[ignore = "AC16 fixture not yet wired — see tech-spec §Implementation Record"]
fn test_shared_intersection_preserved_across_simplify() {
    todo!(
        "1) créer SHP avec 2 LineString, endpoint commun exact\n\
         2) sources.yaml avec profil TRONCON_DE_ROUTE autoroute/nationale\n\
         3) assert Data0 des 2 features après run : coord endpoint\n\
            strictement byte-identique (format %.6f)\n\
         4) chaîner imgforge::build_img sur le .mp produit\n\
         5) extraction NOD via gmt -iv (ou decodeur interne imgforge)\n\
         6) assert nœud unique partagé par les 2 arcs NET"
    );
}
