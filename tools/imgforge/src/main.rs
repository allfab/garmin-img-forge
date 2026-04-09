use std::path::Path;
use std::time::Instant;
use anyhow::{Context, Result};
use clap::Parser;
use tracing_subscriber::EnvFilter;

use imgforge::cli::{Cli, Commands};
use imgforge::dem;
use imgforge::img::writer;
use imgforge::parser;
use imgforge::report::BuildReport;

/// Read .mp file with UTF-8 first, Latin-1 (ISO-8859-1) fallback for BDTOPO accents
fn read_mp_file(path: impl AsRef<Path>) -> Result<String> {
    let bytes = std::fs::read(path.as_ref())
        .with_context(|| format!("Cannot read file: {}", path.as_ref().display()))?;
    match String::from_utf8(bytes.clone()) {
        Ok(s) => Ok(s),
        Err(_) => {
            // CP1252 fallback for BDTOPO/French map data
            tracing::debug!("File is not UTF-8, using CP1252 fallback: {}", path.as_ref().display());
            Ok(bytes.iter().map(|&b| imgforge::img::labelenc::format9::cp1252_to_unicode(b)).collect())
        }
    }
}

/// Read optional TYP file from disk, validating it is non-empty
fn read_typ_file(path: impl AsRef<Path>) -> Result<Vec<u8>> {
    let path = path.as_ref();
    let data = std::fs::read(path)
        .with_context(|| format!("Failed to read TYP file: {}", path.display()))?;
    anyhow::ensure!(!data.is_empty(), "TYP file is empty: {}", path.display());
    Ok(data)
}

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
        Commands::Compile {
            input, output, description,
            code_page, unicode, latin1, lower_case,
            transparent, draw_priority, levels, order_by_decreasing_area,
            reduce_point_density, simplify_polygons, min_size_polygon, merge_lines,
            route, net, no_route, copyright_message, typ_file,
            dem, dem_dists, dem_interpolation, dem_source_srs,
        } => {
            let start = Instant::now();
            let mut report = BuildReport::new();

            let content = read_mp_file(&input)
                .with_context(|| format!("Failed to read {}", input))?;

            let mut mp = parser::parse_mp(&content)
                .with_context(|| format!("Failed to parse {}", input))?;

            // Apply CLI overrides
            apply_tile_overrides(
                &mut mp, description.as_deref(),
                code_page, unicode, latin1, lower_case,
                transparent, draw_priority, levels.as_deref(),
                order_by_decreasing_area,
                reduce_point_density, simplify_polygons.as_deref(), min_size_polygon, merge_lines,
                route, net, no_route, copyright_message.as_deref(),
            );

            report.total_points = mp.points.len();
            report.total_polylines = mp.polylines.len();
            report.total_polygons = mp.polygons.len();

            let typ_data = typ_file.as_ref().map(read_typ_file).transpose()?;

            // Build DEM if --dem provided
            let dem_config = dem.as_ref().map(|paths| {
                imgforge::dem::DemConfig {
                    paths: paths.clone(),
                    dists: dem_dists.clone().unwrap_or_default(),
                    interpolation: imgforge::dem::InterpolationMethod::from_str(&dem_interpolation),
                    source_srs: dem_source_srs.clone(),
                }
            });

            let mut result = writer::build_subfiles(&mp)
                .with_context(|| "Failed to build subfiles")?;

            // Add DEM subfile if configured
            if let Some(ref config) = dem_config {
                match build_dem_subfile(&mp, config) {
                    Ok(dem_data) => { result.dem = Some(dem_data); }
                    Err(e) => { tracing::warn!("DEM generation failed: {:#}", e); }
                }
            }

            let img_data = writer::build_img_with_typ_from_result(&result, typ_data.as_deref())
                .with_context(|| "Failed to build IMG")?;

            let out_path = output.unwrap_or_else(|| {
                let stem = Path::new(&input).file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("output");
                format!("{}.img", stem)
            });

            std::fs::write(&out_path, &img_data)
                .with_context(|| format!("Failed to write {}", out_path))?;

            report.tiles_compiled = 1;
            report.output_file = out_path.clone();
            report.output_size_bytes = img_data.len() as u64;
            report.set_duration(start.elapsed());

            println!("{}", report.to_json());
        }

        Commands::Build {
            input, output, jobs, family_id, product_id, series_name, family_name,
            code_page, unicode, latin1, lower_case,
            transparent, draw_priority, levels, order_by_decreasing_area,
            reduce_point_density, simplify_polygons, min_size_polygon, merge_lines,
            route, net, no_route, copyright_message,
            mapname, country_name, country_abbr, region_name, region_abbr,
            area_name, product_version, keep_going, typ_file,
            dem, dem_dists, dem_interpolation, dem_source_srs,
        } => {
            if let Some(j) = jobs {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(j)
                    .build_global()
                    .ok();
            }
            let start = Instant::now();
            let mut report = BuildReport::new();

            let input_path = Path::new(&input);
            let mp_files: Vec<_> = std::fs::read_dir(input_path)
                .with_context(|| format!("Failed to read directory {}", input))?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|ext| ext == "mp").unwrap_or(false))
                .collect();

            if mp_files.is_empty() {
                anyhow::bail!("No .mp files found in {}", input);
            }

            // Build DEM config if --dem provided
            let dem_config = dem.as_ref().map(|paths| {
                dem::DemConfig {
                    paths: paths.clone(),
                    dists: dem_dists.clone().unwrap_or_default(),
                    interpolation: dem::InterpolationMethod::from_str(&dem_interpolation),
                    source_srs: dem_source_srs.clone(),
                }
            });

            // Pre-load DEM grids once, share via Arc (F6 fix)
            let shared_dem = dem_config.as_ref().map(|config| {
                match dem::load_elevation_sources(&config.paths, config.source_srs.as_deref()) {
                    Ok(grids) => Some(std::sync::Arc::new(grids)),
                    Err(e) => {
                        tracing::warn!("DEM loading failed: {:#}", e);
                        None
                    }
                }
            }).flatten();

            // Clone CLI values for parallel closure
            let levels_clone = levels.clone();
            let simplify_clone = simplify_polygons.clone();
            let copyright_clone = copyright_message.clone();
            let dem_config_clone = dem_config.clone();
            let shared_dem_clone = shared_dem.clone();

            use rayon::prelude::*;

            let results: Vec<Result<_, anyhow::Error>> = mp_files.par_iter().map(|entry| {
                let path = entry.path();
                let content = read_mp_file(&path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                let mut mp = parser::parse_mp(&content)
                    .with_context(|| format!("Failed to parse {}", path.display()))?;

                apply_tile_overrides(
                    &mut mp, None,
                    code_page, unicode, latin1, lower_case,
                    transparent, draw_priority, levels_clone.as_deref(),
                    order_by_decreasing_area,
                    reduce_point_density, simplify_clone.as_deref(), min_size_polygon, merge_lines,
                    route, net, no_route, copyright_clone.as_deref(),
                );

                let mut tile = writer::build_subfiles(&mp)
                    .with_context(|| format!("Failed to build {}", path.display()))?;

                // Add DEM subfile using pre-loaded grids
                if let (Some(ref config), Some(ref grids)) = (&dem_config_clone, &shared_dem_clone) {
                    match build_dem_subfile_with_grids(&mp, config, grids) {
                        Ok(dem_data) => { tile.dem = Some(dem_data); }
                        Err(e) => { tracing::warn!("DEM generation failed for {}: {:#}", path.display(), e); }
                    }
                }

                let counts = (mp.points.len(), mp.polylines.len(), mp.polygons.len());
                Ok((tile, counts, path.display().to_string()))
            }).collect();

            // Handle keep-going: collect successes, log failures
            use imgforge::img::assembler::TileSubfiles;
            let mut tile_subfiles = Vec::with_capacity(results.len());
            let mut errors = 0usize;

            for result in results {
                match result {
                    Ok((tile, (pts, lines, polys), _path)) => {
                        report.total_points += pts;
                        report.total_polylines += lines;
                        report.total_polygons += polys;
                        report.tiles_compiled += 1;
                        tile_subfiles.push(TileSubfiles {
                            map_number: tile.map_number,
                            description: tile.description,
                            tre: tile.tre,
                            rgn: tile.rgn,
                            lbl: tile.lbl,
                            net: tile.net,
                            nod: tile.nod,
                            dem: tile.dem,
                        });
                    }
                    Err(e) => {
                        if keep_going {
                            eprintln!("WARNING: {:#}", e);
                            errors += 1;
                        } else {
                            return Err(e);
                        }
                    }
                }
            }

            if tile_subfiles.is_empty() {
                anyhow::bail!("All tiles failed to compile");
            }

            if errors > 0 {
                eprintln!("{} tiles compiled, {} errors", tile_subfiles.len(), errors);
            }

            let fid = family_id.unwrap_or(1);
            let pid = product_id.unwrap_or(1);
            let map_desc = mapname.as_deref()
                .or(family_name.as_deref())
                .unwrap_or("Map");

            // Effective codepage: CLI flags > .mp default
            let effective_codepage = if unicode { 65001 }
                else if latin1 { 1252 }
                else { code_page.unwrap_or(0) };

            let gmapsupp_meta = imgforge::img::assembler::GmapsuppMeta {
                family_id: fid,
                product_id: pid,
                family_name: family_name.as_deref().unwrap_or("Map").to_string(),
                area_name: area_name.as_deref().unwrap_or("").to_string(),
                codepage: effective_codepage,
                typ_basename: typ_file.as_ref().map(|p| {
                    p.file_stem().and_then(|s| s.to_str()).unwrap_or("00000001").to_string()
                }),
            };
            let typ_data = typ_file.as_ref().map(read_typ_file).transpose()?;

            // Build TDB data — needed both as companion file and embedded in gmapsupp
            let tdb_data = {
                use imgforge::img::tdb::{TdbWriter, TdbTile};
                let overview_map_id = imgforge::img::assembler::compute_overview_map_id(fid);
                let mut tdb = TdbWriter::new(fid, pid);
                tdb.overview_map_number = overview_map_id;
                tdb.codepage = effective_codepage;
                tdb.series_name = series_name.as_deref().unwrap_or("imgforge").to_string();
                tdb.family_name = family_name.as_deref().unwrap_or("Map").to_string();
                if let Some(ref an) = area_name { tdb.area_name = an.clone(); }
                if let Some(ref cm) = copyright_message { tdb.copyright = cm.clone(); }
                if let Some(pv) = product_version { tdb.product_version = pv; }
                if let Some(ref cn) = country_name { tdb.country_name = cn.clone(); }
                if let Some(ref ca) = country_abbr { tdb.country_abbr = ca.clone(); }
                if let Some(ref rn) = region_name { tdb.region_name = rn.clone(); }
                if let Some(ref ra) = region_abbr { tdb.region_abbr = ra.clone(); }

                // Enable profile/elevation display if any tile has DEM data
                if tile_subfiles.iter().any(|t| t.dem.is_some()) {
                    tdb.enable_profile = true;
                }

                for tile in &tile_subfiles {
                    let map_num: u32 = tile.map_number.parse().unwrap_or(0);
                    let (north, east, south, west) = imgforge::img::common_header::read_tre_bounds(&tile.tre);

                    // Build subfile list with sizes (IMG subfile naming convention)
                    let img_name = |ext: &str| format!("I{:08}.{}", map_num, ext);
                    let mut subfiles = vec![
                        (img_name("TRE"), tile.tre.len() as u32),
                        (img_name("RGN"), tile.rgn.len() as u32),
                        (img_name("LBL"), tile.lbl.len() as u32),
                    ];
                    if let Some(ref net) = tile.net {
                        subfiles.push((img_name("NET"), net.len() as u32));
                    }
                    if let Some(ref nod) = tile.nod {
                        subfiles.push((img_name("NOD"), nod.len() as u32));
                    }
                    if let Some(ref dem) = tile.dem {
                        subfiles.push((img_name("DEM"), dem.len() as u32));
                    }

                    tdb.add_tile(TdbTile {
                        map_number: map_num,
                        parent_map_number: tdb.overview_map_number,
                        description: tile.map_number.clone(),
                        north,
                        south,
                        east,
                        west,
                        subfiles,
                    });
                }

                tdb.build()
            };

            // Build gmapsupp with TDB + overview map embedded
            let gmapsupp = imgforge::img::assembler::build_gmapsupp_with_meta_and_typ(
                &tile_subfiles, map_desc, &gmapsupp_meta,
                typ_data.as_deref(), Some(&tdb_data),
            )?;
            std::fs::write(&output, &gmapsupp)?;

            // Also write TDB as companion file (for desktop software like BaseCamp)
            {
                let tdb_path = Path::new(&output).with_extension("tdb");
                std::fs::write(&tdb_path, &tdb_data)
                    .with_context(|| format!("Failed to write {}", tdb_path.display()))?;
            }

            report.output_file = output;
            report.output_size_bytes = gmapsupp.len() as u64;
            report.set_duration(start.elapsed());

            println!("{}", report.to_json());
        }
    }

    Ok(())
}

/// Apply tile-level CLI overrides to a parsed MpFile
fn apply_tile_overrides(
    mp: &mut imgforge::parser::mp_types::MpFile,
    description: Option<&str>,
    code_page: Option<u16>, unicode: bool, latin1: bool, lower_case: bool,
    transparent: bool, draw_priority: Option<u32>, levels: Option<&str>,
    order_by_decreasing_area: bool,
    reduce_point_density: Option<f64>, simplify_polygons: Option<&str>,
    min_size_polygon: Option<i32>, merge_lines: bool,
    route: bool, net: bool, no_route: bool,
    copyright_message: Option<&str>,
) {
    use imgforge::parser::mp_types::RoutingMode;

    if let Some(desc) = description {
        mp.header.name = desc.to_string();
    }

    // Encoding overrides
    if unicode { mp.header.codepage = 65001; }
    else if latin1 { mp.header.codepage = 1252; }
    else if let Some(cp) = code_page { mp.header.codepage = cp; }
    mp.header.lower_case = lower_case;

    // Rendering overrides
    if transparent { mp.header.transparent = true; }
    if let Some(dp) = draw_priority { mp.header.draw_priority = dp; }
    if let Some(lvl) = levels {
        if let Some(parsed) = parse_levels(lvl) {
            mp.header.levels = parsed;
        }
    }
    mp.header.order_by_decreasing_area = order_by_decreasing_area;

    // Geometry overrides
    mp.header.reduce_point_density = reduce_point_density;
    mp.header.simplify_polygons = simplify_polygons.map(|s| s.to_string());
    mp.header.min_size_polygon = min_size_polygon;
    mp.header.merge_lines = merge_lines;

    // Routing overrides
    if no_route {
        mp.header.routing_mode = RoutingMode::Disabled;
    } else if net {
        mp.header.routing_mode = RoutingMode::NetOnly;
    } else if route {
        mp.header.routing_mode = RoutingMode::Route;
    }

    // Copyright override
    if let Some(cm) = copyright_message {
        mp.header.copyright = cm.to_string();
    }
}

/// Build DEM subfile from DEM config and map bounds (loads grids from disk)
fn build_dem_subfile(
    mp: &imgforge::parser::mp_types::MpFile,
    config: &dem::DemConfig,
) -> Result<Vec<u8>> {
    let grids = dem::load_elevation_sources(&config.paths, config.source_srs.as_deref())?;
    build_dem_subfile_with_grids(mp, config, &grids)
}

/// Build DEM subfile using pre-loaded grids (for parallel builds — F6 fix)
fn build_dem_subfile_with_grids(
    mp: &imgforge::parser::mp_types::MpFile,
    config: &dem::DemConfig,
    grids: &[dem::ElevationGrid],
) -> Result<Vec<u8>> {
    use imgforge::img::dem::DemWriter;
    use imgforge::img::zoom::Zoom;

    let converter = dem::converter::DemConverter::new(grids.to_vec(), config.interpolation.clone());

    let bounds = compute_mp_bounds(mp);

    let levels: Vec<Zoom> = mp.header.levels.iter().enumerate().map(|(i, &res)| {
        Zoom::new(i as u8, res)
    }).collect();

    let mut writer = DemWriter::new();
    writer.calc(&bounds, config, &converter, &levels);

    Ok(writer.build())
}

/// Compute WGS84 bounds from mp file features
fn compute_mp_bounds(mp: &imgforge::parser::mp_types::MpFile) -> dem::GeoBounds {
    let mut min_lat = f64::INFINITY;
    let mut max_lat = f64::NEG_INFINITY;
    let mut min_lon = f64::INFINITY;
    let mut max_lon = f64::NEG_INFINITY;

    let mut update = |coord: &imgforge::img::coord::Coord| {
        let lat = coord.lat_degrees();
        let lon = coord.lon_degrees();
        if lat < min_lat { min_lat = lat; }
        if lat > max_lat { max_lat = lat; }
        if lon < min_lon { min_lon = lon; }
        if lon > max_lon { max_lon = lon; }
    };

    for pt in &mp.points {
        update(&pt.coord);
    }
    for pl in &mp.polylines {
        for coord in &pl.points {
            update(coord);
        }
    }
    for pg in &mp.polygons {
        for coord in &pg.points {
            update(coord);
        }
    }

    if min_lat > max_lat {
        // No features — use a default 1-degree box
        min_lat = 45.0;
        max_lat = 46.0;
        min_lon = 5.0;
        max_lon = 6.0;
    }

    dem::GeoBounds {
        north: max_lat,
        south: min_lat,
        east: max_lon,
        west: min_lon,
    }
}

/// Parse levels string: "24,22,20" or "0:24,1:22,2:20"
fn parse_levels(s: &str) -> Option<Vec<u8>> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.is_empty() { return None; }

    let mut levels = Vec::new();
    for part in &parts {
        let part = part.trim();
        if let Some((_idx, res)) = part.split_once(':') {
            match res.trim().parse::<u8>() {
                Ok(r) => levels.push(r),
                Err(_) => tracing::warn!("Ignoring malformed level entry: '{}'", part),
            }
        } else {
            match part.parse::<u8>() {
                Ok(r) => levels.push(r),
                Err(_) => tracing::warn!("Ignoring malformed level entry: '{}'", part),
            }
        }
    }

    if levels.is_empty() { None } else { Some(levels) }
}
