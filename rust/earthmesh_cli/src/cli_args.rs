pub(crate) fn parse_f64_arg(flag: &str, value: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .map_err(|_| usage(&format!("{flag} must be a finite number")))
        .and_then(|parsed| {
            if parsed.is_finite() {
                Ok(parsed)
            } else {
                Err(usage(&format!("{flag} must be a finite number")))
            }
        })
}

pub(crate) fn parse_positive_f64(flag: &str, value: &str) -> Result<f64, String> {
    let parsed = parse_f64_arg(flag, value)?;
    if parsed <= 0.0 {
        return Err(usage(&format!("{flag} must be positive")));
    }
    Ok(parsed)
}

pub(crate) fn parse_nonnegative_f64(flag: &str, value: &str) -> Result<f64, String> {
    let parsed = parse_f64_arg(flag, value)?;
    if parsed < 0.0 {
        return Err(usage(&format!("{flag} must be non-negative")));
    }
    Ok(parsed)
}

pub(crate) fn parse_positive_usize(flag: &str, value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| usage(&format!("{flag} must be a positive integer")))?;
    if parsed == 0 {
        return Err(usage(&format!("{flag} must be a positive integer")));
    }
    Ok(parsed)
}

pub(crate) fn parse_nonnegative_usize(flag: &str, value: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|_| usage(&format!("{flag} must be a non-negative integer")))
}

pub(crate) fn parse_key_usize_pair(flag: &str, value: &str) -> Result<(String, usize), String> {
    let (key, raw_value) = value
        .split_once('=')
        .ok_or_else(|| usage(&format!("{flag} values must use KEY=VALUE syntax")))?;
    let key = key.trim();
    if key.is_empty() {
        return Err(usage(&format!("{flag} keys must not be empty")));
    }
    Ok((
        key.to_string(),
        parse_positive_usize(flag, raw_value.trim())?,
    ))
}

pub(crate) fn parse_key_nonnegative_usize_pair(
    flag: &str,
    value: &str,
) -> Result<(String, usize), String> {
    let (key, raw_value) = value
        .split_once('=')
        .ok_or_else(|| usage(&format!("{flag} values must use KEY=VALUE syntax")))?;
    let key = key.trim();
    if key.is_empty() {
        return Err(usage(&format!("{flag} keys must not be empty")));
    }
    Ok((
        key.to_string(),
        parse_nonnegative_usize(flag, raw_value.trim())?,
    ))
}

pub(crate) fn parse_usize_f64_pair(flag: &str, value: &str) -> Result<(usize, f64), String> {
    let (raw_key, raw_value) = value
        .split_once('=')
        .ok_or_else(|| usage(&format!("{flag} values must use DEGREE=VALUE syntax")))?;
    Ok((
        parse_positive_usize(flag, raw_key.trim())?,
        parse_f64_arg(flag, raw_value.trim())?,
    ))
}

pub(crate) fn parse_nonnegative_i32(flag: &str, value: &str) -> Result<i32, String> {
    let parsed = value
        .parse::<i32>()
        .map_err(|_| usage(&format!("{flag} must be a non-negative integer")))?;
    if parsed < 0 {
        return Err(usage(&format!("{flag} must be a non-negative integer")));
    }
    Ok(parsed)
}

pub(crate) fn usage(message: &str) -> String {
    let prefix = if message.is_empty() {
        String::new()
    } else {
        format!("{message}\n")
    };
    format!(
        "{prefix}usage: earthmesh_cli --cama-reach-jsonl <map_dir> <output.jsonl> --bbox W S E N --target-dx-km KM [--uparea-to-km2 SCALE] [--no-yrev]
       earthmesh_cli --cama-reach-geojson <map_dir> <output.geojson> --bbox W S E N --target-dx-km KM [--uparea-to-km2 SCALE] [--no-yrev]
       earthmesh_cli --merit-hydro-geojson <merit_root> <output_dir> --bbox W S E N [--stride N] [--r2-width-m M] [--r3-width-m M] [--r2-upa-km2 KM2] [--r3-upa-km2 KM2] [--skip-surface-mask]
       earthmesh_cli --landtype-cell-mask <cells.geojson> <landtype.nc> <out.geojson> [--gridnum-perdegree N]
       earthmesh_cli --hydro-complete-cell-mask <background.geojson> <out.geojson> [--river-geojson PATH] [--coast-geojson PATH] [--surface-geojson PATH]
       earthmesh_cli --hydro-close-recipe <input.geojson> <output_prefix> <recipe.json> [--class-refine CLASS=DEGREE ...] [--buffer-deg-by-refine-degree DEGREE=BUFFER ...] [--simplify-tolerance-deg DEG] [--example-namelist FILE]
       earthmesh_cli --hydro-close-mask-nmls <input.geojson> <output_prefix> [--class-refine CLASS=DEGREE ...] [--max-rings-per-class N] [--max-rings-by-class CLASS=COUNT ...] [--max-masks-per-refine-degree N | --no-max-masks-per-refine-degree] [--min-ring-separation-deg DEG] [--buffer-deg-by-refine-degree DEGREE=BUFFER ...] [--simplify-tolerance-deg DEG] [--dissolve-overlapping-envelopes] [--non-cumulative-refine]
       earthmesh_cli --hydro-composite-close-mask-nmls <recipe.json> <output_prefix> [--summary-json PATH]
       earthmesh_cli --colm-coupling-csv-to-netcdf <colm_coupling_cells.csv> <colm_coupling_cells.nc> [--case-name NAME] [--delivery-manifest PATH] [--restart-template-netcdf PATH] [--forcing-template-netcdf PATH]
       earthmesh_cli --mesh-quality <gridfile.nc4> [out_dir] [quality.nml] [--kind tri|hex|hex-delaunay]
       earthmesh_cli --project-quality <project.yaml|json> <gridfile.nc4> <out_dir>
       earthmesh_cli --project-hydro-postprocess <project.yaml|json> <gridfile.nc4> <out_dir> [compiled.nml] [unmasked_parent.nc4]
       earthmesh_cli <mkgrd.nml> [--quiet] [--max-tris N] [--run-refine-passthrough --source-gridnum-perdegree N --source-nlons N --source-nlats N [--source-first-triangle-id N] | --run-refine-landtype-source [--source-gridnum-perdegree N] [--source-first-triangle-id N] | --run-mask-restart-ocean [--mask-postproc-num-vertex N] [--mask-restart-max-iter N] | --run-mask-restart-patch [--mask-restart-max-iter N] | --run-mask-restart-area-judge [--source-gridnum-perdegree N --source-nlons N --source-nlats N] [--mask-restart-max-iter N] | --run-mask-restart-area-judge-refine-landtype-source [--restart-refine-initial-gridfile PATH] [--source-gridnum-perdegree N] [--source-first-triangle-id N] | --restart-refine-initial-gridfile PATH [--source-gridnum-perdegree N] [--source-first-triangle-id N]]"
    )
}
