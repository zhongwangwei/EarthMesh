use std::collections::BTreeMap;
use std::path::PathBuf;

use super::super::cli_args::{
    parse_key_nonnegative_usize_pair, parse_key_usize_pair, parse_nonnegative_f64,
    parse_nonnegative_usize, parse_usize_f64_pair, usage,
};

pub(crate) fn run_hydro_close_mask_nmls(args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut args = args.collect::<Vec<_>>().into_iter();
    let input_geojson = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--hydro-close-mask-nmls requires an input GeoJSON"))?,
    );
    let output_prefix = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--hydro-close-mask-nmls requires an output prefix"))?,
    );
    let rest = args.collect::<Vec<_>>();
    let mut class_refine: Option<BTreeMap<String, usize>> = None;
    let mut max_rings_per_class: Option<usize> = None;
    let mut max_rings_by_class = BTreeMap::<String, usize>::new();
    let mut max_masks_per_refine_degree = Some(999_usize);
    let mut min_ring_separation_deg = 0.0_f64;
    let mut buffer_deg_by_refine_degree = BTreeMap::<usize, f64>::new();
    let mut simplify_tolerance_deg = 0.0_f64;
    let mut dissolve_overlapping_envelopes = false;
    let mut cumulative_refine = true;

    let mut index = 0_usize;
    while index < rest.len() {
        match rest[index].as_str() {
            "--class-refine" => {
                index += 1;
                let start = index;
                let mut parsed = BTreeMap::<String, usize>::new();
                while index < rest.len() && !rest[index].starts_with("--") {
                    let (class, degree) = parse_key_usize_pair("--class-refine", &rest[index])?;
                    parsed.insert(class, degree);
                    index += 1;
                }
                if index == start {
                    return Err(usage("--class-refine requires at least one CLASS=DEGREE"));
                }
                class_refine = Some(parsed);
            }
            "--max-rings-per-class" => {
                index += 1;
                let value = rest
                    .get(index)
                    .ok_or_else(|| usage("--max-rings-per-class requires a value"))?;
                max_rings_per_class =
                    Some(parse_nonnegative_usize("--max-rings-per-class", value)?);
                index += 1;
            }
            "--max-rings-by-class" => {
                index += 1;
                let start = index;
                while index < rest.len() && !rest[index].starts_with("--") {
                    let (class, cap) =
                        parse_key_nonnegative_usize_pair("--max-rings-by-class", &rest[index])?;
                    max_rings_by_class.insert(class, cap);
                    index += 1;
                }
                if index == start {
                    return Err(usage(
                        "--max-rings-by-class requires at least one CLASS=COUNT",
                    ));
                }
            }
            "--max-masks-per-refine-degree" => {
                index += 1;
                let value = rest
                    .get(index)
                    .ok_or_else(|| usage("--max-masks-per-refine-degree requires a value"))?;
                max_masks_per_refine_degree = Some(parse_nonnegative_usize(
                    "--max-masks-per-refine-degree",
                    value,
                )?);
                index += 1;
            }
            "--no-max-masks-per-refine-degree" => {
                max_masks_per_refine_degree = None;
                index += 1;
            }
            "--min-ring-separation-deg" => {
                index += 1;
                let value = rest
                    .get(index)
                    .ok_or_else(|| usage("--min-ring-separation-deg requires a value"))?;
                min_ring_separation_deg =
                    parse_nonnegative_f64("--min-ring-separation-deg", value)?;
                index += 1;
            }
            "--buffer-deg-by-refine-degree" => {
                index += 1;
                let start = index;
                while index < rest.len() && !rest[index].starts_with("--") {
                    let (degree, buffer) =
                        parse_usize_f64_pair("--buffer-deg-by-refine-degree", &rest[index])?;
                    if degree == 0 || buffer < 0.0 {
                        return Err(usage(
                            "--buffer-deg-by-refine-degree requires positive degrees and non-negative buffers",
                        ));
                    }
                    buffer_deg_by_refine_degree.insert(degree, buffer);
                    index += 1;
                }
                if index == start {
                    return Err(usage(
                        "--buffer-deg-by-refine-degree requires at least one DEGREE=BUFFER",
                    ));
                }
            }
            "--simplify-tolerance-deg" => {
                index += 1;
                let value = rest
                    .get(index)
                    .ok_or_else(|| usage("--simplify-tolerance-deg requires a value"))?;
                simplify_tolerance_deg = parse_nonnegative_f64("--simplify-tolerance-deg", value)?;
                index += 1;
            }
            "--dissolve-overlapping-envelopes" => {
                dissolve_overlapping_envelopes = true;
                index += 1;
            }
            "--non-cumulative-refine" => {
                cumulative_refine = false;
                index += 1;
            }
            "-h" | "--help" => return Err(usage("")),
            other => {
                return Err(usage(&format!(
                    "unknown hydro close-mask NML argument {other}"
                )));
            }
        }
    }

    let report = earthmesh_cli::hydro_close_masks::write_hydro_close_mask_nmls(
        &input_geojson,
        &output_prefix,
        earthmesh_cli::hydro_close_types::HydroCloseMaskNmlOptions {
            class_refine: class_refine.unwrap_or_else(
                earthmesh_cli::hydro_close_recipe::default_hydro_close_class_refine,
            ),
            max_rings_per_class,
            max_rings_by_class,
            max_masks_per_refine_degree,
            min_ring_separation_deg,
            buffer_deg_by_refine_degree,
            simplify_tolerance_deg,
            dissolve_overlapping_envelopes,
            cumulative_refine,
        },
    )
    .map_err(|err| err.to_string())?;
    println!("hydro_close_mask_prefix={}", report.output_prefix.display());
    println!("hydro_close_mask_files={}", report.files.len());
    println!("hydro_close_mask_specs={}", report.spec_count);
    for file in &report.files {
        println!("hydro_close_mask_file={}", file.display());
    }
    Ok(())
}
