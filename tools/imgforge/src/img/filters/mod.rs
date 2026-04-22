//! Filtres de polylignes/polygones — parité mkgmap r4924 `filters/*.java`.
//!
//! Chaque sous-module réplique un filtre mkgmap dédié, branché par
//! `writer.rs::filter_features_for_level` après la DP et avant le splitter.
//! Tous les filtres opèrent sur des `Vec<Coord>` (coordonnées bits-world
//! 24-bit) et sont gated sur `level > 0` pour protéger le niveau détail.
//!
//! Correspondances mkgmap :
//!
//! | Filtre Rust                 | mkgmap r4924                                                  |
//! |----------------------------|---------------------------------------------------------------|
//! | `round_coords`             | `filters/RoundCoordsFilter.java`                              |
//! | `remove_obsolete_points`   | `filters/RemoveObsoletePointsFilter.java`                     |
//! | `passes_size_filter`       | `filters/SizeFilter.java` (MIN_SIZE_LINE=1 dans MapBuilder)   |
//! | `passes_remove_empty`      | `filters/RemoveEmpty.java`                                    |

pub mod round_coords;
pub mod remove_obsolete_points;
pub mod remove_empty;
pub mod size;

pub use round_coords::round_coords;
pub use remove_obsolete_points::remove_obsolete_points;
pub use remove_empty::passes_remove_empty;
pub use size::passes_size_filter;
