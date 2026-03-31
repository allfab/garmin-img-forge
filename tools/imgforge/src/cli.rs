// CLI definitions (clap)

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
    },
}
