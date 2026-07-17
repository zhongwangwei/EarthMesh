use std::path::PathBuf;

use super::util::next_required_arg;
use super::{parse_f64_arg, parse_positive_f64, parse_positive_usize, usage};

pub(crate) fn run_merit_hydro_geojson(
    mut args: impl Iterator<Item = String>,
) -> Result<(), String> {
    let merit_root = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--merit-hydro-geojson requires a MERIT root directory"))?,
    );
    let output_dir = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--merit-hydro-geojson requires an output directory"))?,
    );
    let mut bbox: Option<earthmesh_cli::merit_tile_selection::MeritLonLatBbox> = None;
    let mut stride = 1_usize;
    let mut thresholds = earthmesh_cli::merit_hydro_io::MeritMaskThresholds::default();
    let mut include_surface_masks = true;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bbox" => {
                let west =
                    parse_f64_arg("--bbox west", &next_required_arg(&mut args, "--bbox west")?)?;
                let south = parse_f64_arg(
                    "--bbox south",
                    &next_required_arg(&mut args, "--bbox south")?,
                )?;
                let east =
                    parse_f64_arg("--bbox east", &next_required_arg(&mut args, "--bbox east")?)?;
                let north = parse_f64_arg(
                    "--bbox north",
                    &next_required_arg(&mut args, "--bbox north")?,
                )?;
                bbox = Some(earthmesh_cli::merit_tile_selection::MeritLonLatBbox {
                    west,
                    east,
                    south,
                    north,
                });
            }
            "--stride" => {
                let value = next_required_arg(&mut args, "--stride")?;
                stride = parse_positive_usize("--stride", &value)?;
            }
            "--r2-width-m" => {
                let value = next_required_arg(&mut args, "--r2-width-m")?;
                thresholds.r2_width_m = parse_positive_f64("--r2-width-m", &value)?;
            }
            "--r3-width-m" => {
                let value = next_required_arg(&mut args, "--r3-width-m")?;
                thresholds.r3_width_m = parse_positive_f64("--r3-width-m", &value)?;
                thresholds.river_width_refinement_m = thresholds.r3_width_m;
            }
            "--r2-upa-km2" => {
                let value = next_required_arg(&mut args, "--r2-upa-km2")?;
                thresholds.r2_upa_km2 = parse_positive_f64("--r2-upa-km2", &value)?;
            }
            "--r3-upa-km2" => {
                let value = next_required_arg(&mut args, "--r3-upa-km2")?;
                thresholds.r3_upa_km2 = parse_positive_f64("--r3-upa-km2", &value)?;
                thresholds.river_upstream_area_refinement_km2 = thresholds.r3_upa_km2;
            }
            "--skip-surface-mask" => {
                include_surface_masks = false;
            }
            "-h" | "--help" => return Err(usage("")),
            other => return Err(usage(&format!("unknown MERIT-Hydro argument {other}"))),
        }
    }

    let bbox = bbox.ok_or_else(|| usage("--merit-hydro-geojson requires --bbox W S E N"))?;
    let query_windows = earthmesh_cli::merit_tile_selection::split_merit_query_bbox(bbox)
        .map_err(|err| err.to_string())?;
    let mut windows = Vec::new();
    for query in query_windows {
        let tile_paths =
            earthmesh_cli::merit_tile_selection::select_merit_hydro_tiles(&merit_root, query)
                .map_err(|err| err.to_string())?;
        for tile in tile_paths {
            let tile_bounds = earthmesh_cli::merit_tile_selection::merit_tile_bounds_from_name(
                tile.file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| format!("invalid MERIT-Hydro tile path {}", tile.display()))?,
            )
            .map_err(|err| err.to_string())?;
            let Some(clipped) =
                earthmesh_cli::merit_tile_selection::clip_merit_bbox_to_tile(query, tile_bounds)
                    .map_err(|err| err.to_string())?
            else {
                continue;
            };
            windows.push(
                earthmesh_cli::merit_hydro_io::read_merit_hydro_window(&tile, clipped, stride)
                    .map_err(|err| err.to_string())?,
            );
        }
    }
    if windows.is_empty() {
        return Err(format!(
            "no MERIT-Hydro tiles in {} intersect bbox",
            merit_root.display()
        ));
    }
    let report = earthmesh_cli::merit_hydro_io::write_merit_hydro_mask_geojson_layers(
        &windows,
        thresholds,
        &output_dir,
        include_surface_masks,
        true,
    )
    .map_err(|err| err.to_string())?;
    println!("merit_tile_count={}", report.window_count);
    println!("merit_masks={}", report.combined_geojson.display());
    println!("merit_river_masks={}", report.river_geojson.display());
    println!("merit_coast_masks={}", report.coast_geojson.display());
    if let Some(surface) = &report.surface_geojson {
        println!("merit_surface_masks={}", surface.display());
    }
    println!("merit_summary={}", report.summary_json.display());
    println!("merit_features={}", report.combined_feature_count);
    Ok(())
}
