use std::path::PathBuf;

use super::util::next_required_arg;
use super::{parse_f64_arg, parse_positive_f64, usage};

pub(crate) fn run_cama_reach_export(
    command: &str,
    mut args: impl Iterator<Item = String>,
) -> Result<(), String> {
    let output_label = match command {
        "--cama-reach-jsonl" => "JSONL",
        "--cama-reach-geojson" => "GeoJSON",
        _ => {
            return Err(usage(&format!(
                "unknown CaMa reach export command {command}"
            )));
        }
    };
    let map_dir = PathBuf::from(
        args.next()
            .ok_or_else(|| usage(&format!("{command} requires a map_dir")))?,
    );
    let output = PathBuf::from(
        args.next()
            .ok_or_else(|| usage(&format!("{command} requires an output {output_label} path")))?,
    );
    let mut bbox: Option<earthmesh_cli::CamaLonLatBbox> = None;
    let mut target_dx_km: Option<f64> = None;
    let mut uparea_to_km2 = 1.0e-6_f64;
    let mut y_reversed_storage = true;

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
                bbox = Some(earthmesh_cli::CamaLonLatBbox {
                    west,
                    east,
                    south,
                    north,
                });
            }
            "--target-dx-km" => {
                let value = next_required_arg(&mut args, "--target-dx-km")?;
                target_dx_km = Some(parse_positive_f64("--target-dx-km", &value)?);
            }
            "--uparea-to-km2" => {
                let value = next_required_arg(&mut args, "--uparea-to-km2")?;
                uparea_to_km2 = parse_positive_f64("--uparea-to-km2", &value)?;
            }
            "--no-yrev" => {
                y_reversed_storage = false;
            }
            "-h" | "--help" => return Err(usage("")),
            other => {
                return Err(usage(&format!(
                    "unknown CaMa reach export argument {other}"
                )));
            }
        }
    }

    let bbox = bbox.ok_or_else(|| usage(&format!("{command} requires --bbox W S E N")))?;
    let target_dx_km =
        target_dx_km.ok_or_else(|| usage(&format!("{command} requires --target-dx-km")))?;
    let inventory = earthmesh_cli::read_cama_reach_inventory_from_map_dir(
        &map_dir,
        bbox,
        target_dx_km,
        uparea_to_km2,
        y_reversed_storage,
    )
    .map_err(|err| err.to_string())?;
    match command {
        "--cama-reach-jsonl" => {
            let report = earthmesh_cli::write_cama_reach_inventory_jsonl(&inventory, &output)
                .map_err(|err| err.to_string())?;
            println!("cama_reach_jsonl={}", report.output.display());
            println!("cama_reach_records={}", report.record_count);
        }
        "--cama-reach-geojson" => {
            let report =
                earthmesh_cli::write_cama_reach_inventory_point_geojson(&inventory, &output)
                    .map_err(|err| err.to_string())?;
            println!("cama_reach_geojson={}", report.output.display());
            println!("cama_reach_features={}", report.feature_count);
        }
        _ => {
            return Err(usage(&format!(
                "unknown CaMa reach export command {command}"
            )))
        }
    }
    Ok(())
}
