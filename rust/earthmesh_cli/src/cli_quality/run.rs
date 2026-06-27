use std::fs;
use std::path::PathBuf;

use super::super::cli_args::usage;

/// `--mesh-quality <gridfile.nc4> [out_dir] [quality.nml]`: read a gridfile, build
/// the quality input from its triangle (M->W) view, and write quality_summary.json
/// /.csv, worst_cells.geojson and quality_report.md. The optional third arg is a
/// namelist whose `&quality` block configures the gate thresholds and the
/// on-violation policy; with `on_violation = 'block'` a Fail verdict exits non-zero
/// (CI gate). Absent ⇒ default thresholds, warn-only (unchanged legacy behavior).
pub(crate) fn run_mesh_quality(args: impl Iterator<Item = String>) -> Result<(), String> {
    // Optional `--kind hex|tri` selects the cell view: hex/atmos (MPAS) meshes are
    // measured as their W-cell hexagons (≈120° angles); tri/FVCOM meshes as the M
    // triangles. Default tri for backward compatibility. The rest stays positional:
    // <gridfile> [out_dir] [quality.nml].
    let mut kind = String::from("tri");
    let mut positional: Vec<String> = Vec::new();
    let mut args = args;
    while let Some(a) = args.next() {
        if a == "--kind" {
            kind = args
                .next()
                .ok_or_else(|| usage("--kind needs a value (hex|tri)"))?;
        } else {
            positional.push(a);
        }
    }
    let gridfile = PathBuf::from(
        positional
            .first()
            .ok_or_else(|| usage("--mesh-quality needs a gridfile path"))?,
    );
    let out_dir = positional.get(1).map(PathBuf::from).unwrap_or_else(|| {
        gridfile
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    });
    let quality_cfg_path = positional.get(2).map(PathBuf::from);

    let mesh = earthmesh_cli::read_gridfile_mesh_points(&gridfile)
        .map_err(|e| format!("read gridfile {}: {e}", gridfile.display()))?;
    let input = if kind.trim() == "hex" {
        earthmesh_cli::quality_input_from_gridfile_hex(&mesh)
    } else {
        earthmesh_cli::quality_input_from_gridfile(&mesh)
    };

    // Optional &quality block → thresholds + policy; absent ⇒ defaults + warn.
    let (thresholds, on_violation) = match &quality_cfg_path {
        Some(p) => {
            let text = fs::read_to_string(p)
                .map_err(|e| format!("read quality config {}: {e}", p.display()))?;
            let q = earthmesh_core::QualityNamelist::from_quality_namelist(&text)?;
            (quality_thresholds_from_namelist(&q), q.on_violation)
        }
        None => (
            earthmesh_quality::QualityThresholds::default(),
            "warn".to_string(),
        ),
    };

    let report = earthmesh_quality::compute(&input, &thresholds);
    let written = earthmesh_quality::io::write_all(&report, &out_dir)
        .map_err(|e| format!("write quality report to {}: {e}", out_dir.display()))?;
    println!("mesh_quality_verdict={}", report.verdict.as_str());
    println!("mesh_quality_cells={}", report.geometry.cell_count);
    println!(
        "mesh_quality_min_angle_deg={}",
        report.geometry.min_angle_deg
    );
    for path in &written {
        println!("mesh_quality_output={}", path.display());
    }
    // on_violation = block ⇒ a Fail verdict aborts with a non-zero exit code.
    if report.verdict.as_str() == "fail" && on_violation.eq_ignore_ascii_case("block") {
        return Err(format!(
            "quality gate failed (verdict=fail, on_violation=block); reports in {}",
            out_dir.display()
        ));
    }
    Ok(())
}

/// Map a parsed `&quality` namelist block to `earthmesh_quality::QualityThresholds`
/// (field-for-field; the namelist's i32 worst-cell limit becomes usize).
fn quality_thresholds_from_namelist(
    q: &earthmesh_core::QualityNamelist,
) -> earthmesh_quality::QualityThresholds {
    earthmesh_quality::QualityThresholds {
        min_angle_warn_deg: q.min_angle_warn_deg,
        min_angle_fail_deg: q.min_angle_fail_deg,
        aspect_ratio_warn: q.aspect_ratio_warn,
        aspect_ratio_fail: q.aspect_ratio_fail,
        area_cv_warn: q.area_cv_warn,
        max_adjacent_resolution_ratio_warn: q.max_adjacent_resolution_ratio_warn,
        worst_cells_limit: q.worst_cells_limit.max(0) as usize,
    }
}
