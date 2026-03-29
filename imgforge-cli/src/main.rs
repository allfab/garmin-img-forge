use std::path::Path;
use std::time::Instant;
use anyhow::{Context, Result};
use clap::Parser;
use tracing_subscriber::EnvFilter;

use imgforge_cli::cli::{Cli, Commands};
use imgforge_cli::img::writer;
use imgforge_cli::parser;
use imgforge_cli::report::BuildReport;

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Setup tracing
    let filter = match cli.verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(filter))
        .init();

    match cli.command {
        Commands::Compile { input, output, description } => {
            let start = Instant::now();
            let mut report = BuildReport::new();

            // Read input .mp file
            let content = std::fs::read_to_string(&input)
                .with_context(|| format!("Failed to read {}", input))?;

            // Parse
            let mut mp = parser::parse_mp(&content)
                .with_context(|| format!("Failed to parse {}", input))?;

            // Override description from CLI if provided
            if let Some(ref desc) = description {
                mp.header.name = desc.clone();
            }

            report.total_points = mp.points.len();
            report.total_polylines = mp.polylines.len();
            report.total_polygons = mp.polygons.len();

            // Build IMG
            let img_data = writer::build_img(&mp)
                .with_context(|| "Failed to build IMG")?;

            // Determine output path
            let out_path = output.unwrap_or_else(|| {
                let stem = Path::new(&input).file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("output");
                format!("{}.img", stem)
            });

            // Write output
            std::fs::write(&out_path, &img_data)
                .with_context(|| format!("Failed to write {}", out_path))?;

            report.tiles_compiled = 1;
            report.output_file = out_path.clone();
            report.output_size_bytes = img_data.len() as u64;
            report.set_duration(start.elapsed());

            println!("{}", report.to_json());
        }

        Commands::Build { input, output, jobs, family_id: _, product_id: _, series_name: _, family_name: _ } => {
            // Configure rayon thread pool if --jobs specified
            if let Some(j) = jobs {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(j)
                    .build_global()
                    .ok();
            }
            let start = Instant::now();
            let mut report = BuildReport::new();

            // Find all .mp files in directory
            let input_path = Path::new(&input);
            let mp_files: Vec<_> = std::fs::read_dir(input_path)
                .with_context(|| format!("Failed to read directory {}", input))?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|ext| ext == "mp").unwrap_or(false))
                .collect();

            if mp_files.is_empty() {
                anyhow::bail!("No .mp files found in {}", input);
            }

            // Compile each tile
            let mut tiles = Vec::new();
            for entry in &mp_files {
                let path = entry.path();
                let content = std::fs::read_to_string(&path)?;
                let mp = parser::parse_mp(&content)?;

                report.total_points += mp.points.len();
                report.total_polylines += mp.polylines.len();
                report.total_polygons += mp.polygons.len();

                let img_data = writer::build_img(&mp)?;
                let name = path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("00000000")
                    .to_string();
                tiles.push((name, img_data));
                report.tiles_compiled += 1;
            }

            // Assemble gmapsupp from pre-built tile IMGs
            let gmapsupp = imgforge_cli::img::assembler::build_gmapsupp_from_imgs(&tiles, "Map")?;
            std::fs::write(&output, &gmapsupp)?;

            report.output_file = output;
            report.output_size_bytes = gmapsupp.len() as u64;
            report.set_duration(start.elapsed());

            println!("{}", report.to_json());
        }
    }

    Ok(())
}
