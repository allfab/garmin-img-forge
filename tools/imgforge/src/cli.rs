// CLI definitions (clap)

use std::path::PathBuf;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "imgforge")]
#[command(about = "Garmin IMG map compiler based on mkgmap")]
#[command(version = env!("GIT_VERSION"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Verbosity level (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Compile a single .mp file to .img
    Compile {
        /// Input .mp file
        input: String,

        /// Output .img file
        #[arg(short, long)]
        output: Option<String>,

        /// Map description
        #[arg(long)]
        description: Option<String>,

        /// Codepage number (e.g. 1252, 65001)
        #[arg(long, value_name = "CODEPAGE")]
        code_page: Option<u16>,

        /// Use Unicode encoding (shortcut for --code-page 65001)
        #[arg(long, conflicts_with = "code_page")]
        unicode: bool,

        /// Use Latin-1 encoding (shortcut for --code-page 1252)
        #[arg(long, conflicts_with = "code_page")]
        latin1: bool,

        /// Allow lowercase in labels (forces Format9/10 instead of Format6)
        #[arg(long)]
        lower_case: bool,

        /// Transparent map (overlay)
        #[arg(long)]
        transparent: bool,

        /// Display priority (default: 25)
        #[arg(long, value_name = "N")]
        draw_priority: Option<u32>,

        /// Zoom levels (e.g. "24,22,20,18,16" or "0:24,1:22,2:20")
        #[arg(long, value_name = "LEVELS")]
        levels: Option<String>,

        /// Sort polygons by decreasing area
        #[arg(long)]
        order_by_decreasing_area: bool,

        /// Douglas-Peucker distance threshold for line simplification
        #[arg(long, value_name = "NUM")]
        reduce_point_density: Option<f64>,

        /// Douglas-Peucker by resolution (e.g. "24:12,18:10,16:8")
        #[arg(long, value_name = "SPEC")]
        simplify_polygons: Option<String>,

        /// Filter polygons smaller than this area (in map units, default mkgmap: 8)
        #[arg(long, value_name = "NUM")]
        min_size_polygon: Option<i32>,

        /// Merge adjacent polylines with same type/label
        #[arg(long)]
        merge_lines: bool,

        /// Force NET+NOD routing generation
        #[arg(long, conflicts_with = "no_route")]
        route: bool,

        /// Generate NET only (address search without turn-by-turn routing)
        #[arg(long, conflicts_with_all = ["route", "no_route"])]
        net: bool,

        /// Disable routing even if roads are present
        #[arg(long, conflicts_with = "route")]
        no_route: bool,

        /// Copyright message
        #[arg(long)]
        copyright_message: Option<String>,

        /// TYP file for custom map styling/symbology
        #[arg(long, value_name = "FILE")]
        typ_file: Option<PathBuf>,
    },

    /// Build multi-tile gmapsupp.img from a directory of .mp files
    Build {
        /// Input directory containing .mp files
        input: String,

        /// Output gmapsupp.img file
        #[arg(short, long, default_value = "gmapsupp.img")]
        output: String,

        /// Number of parallel jobs
        #[arg(short, long)]
        jobs: Option<usize>,

        /// Family ID
        #[arg(long)]
        family_id: Option<u16>,

        /// Product ID
        #[arg(long)]
        product_id: Option<u16>,

        /// Series name
        #[arg(long)]
        series_name: Option<String>,

        /// Family name
        #[arg(long)]
        family_name: Option<String>,

        /// Codepage number (e.g. 1252, 65001)
        #[arg(long, value_name = "CODEPAGE")]
        code_page: Option<u16>,

        /// Use Unicode encoding (shortcut for --code-page 65001)
        #[arg(long, conflicts_with = "code_page")]
        unicode: bool,

        /// Use Latin-1 encoding (shortcut for --code-page 1252)
        #[arg(long, conflicts_with = "code_page")]
        latin1: bool,

        /// Allow lowercase in labels (forces Format9/10 instead of Format6)
        #[arg(long)]
        lower_case: bool,

        /// Transparent map (overlay)
        #[arg(long)]
        transparent: bool,

        /// Display priority (default: 25)
        #[arg(long, value_name = "N")]
        draw_priority: Option<u32>,

        /// Zoom levels (e.g. "24,22,20,18,16" or "0:24,1:22,2:20")
        #[arg(long, value_name = "LEVELS")]
        levels: Option<String>,

        /// Sort polygons by decreasing area
        #[arg(long)]
        order_by_decreasing_area: bool,

        /// Douglas-Peucker distance threshold for line simplification
        #[arg(long, value_name = "NUM")]
        reduce_point_density: Option<f64>,

        /// Douglas-Peucker by resolution (e.g. "24:12,18:10,16:8")
        #[arg(long, value_name = "SPEC")]
        simplify_polygons: Option<String>,

        /// Filter polygons smaller than this area (in map units, default mkgmap: 8)
        #[arg(long, value_name = "NUM")]
        min_size_polygon: Option<i32>,

        /// Merge adjacent polylines with same type/label
        #[arg(long)]
        merge_lines: bool,

        /// Force NET+NOD routing generation
        #[arg(long, conflicts_with = "no_route")]
        route: bool,

        /// Generate NET only (address search without turn-by-turn routing)
        #[arg(long, conflicts_with_all = ["route", "no_route"])]
        net: bool,

        /// Disable routing even if roads are present
        #[arg(long, conflicts_with = "route")]
        no_route: bool,

        /// Copyright message
        #[arg(long)]
        copyright_message: Option<String>,

        /// Map name (8-digit identifier)
        #[arg(long, value_name = "NAME")]
        mapname: Option<String>,

        /// Country name
        #[arg(long)]
        country_name: Option<String>,

        /// Country abbreviation
        #[arg(long)]
        country_abbr: Option<String>,

        /// Region name
        #[arg(long)]
        region_name: Option<String>,

        /// Region abbreviation
        #[arg(long)]
        region_abbr: Option<String>,

        /// Area name
        #[arg(long)]
        area_name: Option<String>,

        /// Product version (100 = v1.00)
        #[arg(long)]
        product_version: Option<u16>,

        /// Continue building if a tile fails
        #[arg(long)]
        keep_going: bool,

        /// TYP file for custom map styling/symbology
        #[arg(long, value_name = "FILE")]
        typ_file: Option<PathBuf>,
    },
}
