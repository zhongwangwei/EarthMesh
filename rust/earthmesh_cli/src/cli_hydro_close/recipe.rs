use std::collections::BTreeMap;
use std::path::PathBuf;

use super::super::cli_args::{
    parse_key_usize_pair, parse_nonnegative_f64, parse_usize_f64_pair, usage,
};

pub(crate) fn run_hydro_close_recipe(args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut args = args.collect::<Vec<_>>().into_iter();
    let input_geojson = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--hydro-close-recipe requires an input GeoJSON"))?,
    );
    let output_prefix = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--hydro-close-recipe requires an output prefix"))?,
    );
    let output_json = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--hydro-close-recipe requires an output recipe JSON"))?,
    );
    let rest = args.collect::<Vec<_>>();
    let mut class_refine: Option<BTreeMap<String, usize>> = None;
    let mut buffer_deg_by_refine_degree = BTreeMap::<usize, f64>::new();
    let mut simplify_tolerance_deg = 0.0_f64;
    let mut example_namelist: Option<String> = None;

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
            "--buffer-deg-by-refine-degree" => {
                index += 1;
                let start = index;
                while index < rest.len() && !rest[index].starts_with("--") {
                    let (degree, buffer) =
                        parse_usize_f64_pair("--buffer-deg-by-refine-degree", &rest[index])?;
                    if buffer < 0.0 {
                        return Err(usage(
                            "--buffer-deg-by-refine-degree buffers must be non-negative",
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
            "--example-namelist" => {
                index += 1;
                example_namelist = Some(
                    rest.get(index)
                        .ok_or_else(|| usage("--example-namelist requires a value"))?
                        .clone(),
                );
                index += 1;
            }
            "-h" | "--help" => return Err(usage("")),
            other => {
                return Err(usage(&format!(
                    "unknown hydro close recipe argument {other}"
                )));
            }
        }
    }

    let report = earthmesh_cli::write_hydro_close_refinement_recipe_json(
        &output_json,
        earthmesh_cli::HydroCloseRefinementRecipeOptions {
            input_geojson,
            output_prefix,
            class_refine: class_refine
                .unwrap_or_else(earthmesh_cli::default_hydro_close_class_refine),
            buffer_deg_by_refine_degree,
            simplify_tolerance_deg,
            example_namelist,
        },
    )
    .map_err(|err| err.to_string())?;
    println!("hydro_close_recipe={}", report.output_json.display());
    println!("hydro_close_max_iter_spc={}", report.max_iter_spc);
    println!("hydro_close_class_count={}", report.class_count);
    println!("hydro_close_buffer_count={}", report.buffer_count);
    Ok(())
}
