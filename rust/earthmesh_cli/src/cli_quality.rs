use std::fs;
use std::path::PathBuf;

use crate::cli_args::usage;

/// `--mesh-quality <gridfile.nc4> [out_dir] [quality.nml] [--kind tri|hex|hex-delaunay]`:
/// read a gridfile, build the selected cell-view quality input, and write
/// quality_summary.json /.csv, worst_cells.geojson and quality_report.md. The
/// optional third arg is a namelist whose `&quality` block configures the gate
/// thresholds and the on-violation policy; with `on_violation = 'block'` a Fail
/// verdict exits non-zero (CI gate). Absent ⇒ default thresholds, warn-only
/// (unchanged compatibility behavior).
pub(crate) fn run_mesh_quality(args: impl Iterator<Item = String>) -> Result<(), String> {
    // Optional `--kind` selects the production tri/hex view or the diagnostic
    // interior-Delaunay view of a hex gridfile.
    let mut kind = String::from("tri");
    let mut positional: Vec<String> = Vec::new();
    let mut args = args;
    while let Some(a) = args.next() {
        if a == "--kind" {
            kind = args
                .next()
                .ok_or_else(|| usage("--kind needs a value (tri|hex|hex-delaunay)"))?;
        } else if a.starts_with('-') {
            return Err(usage(&format!("unknown --mesh-quality option `{a}`")));
        } else {
            positional.push(a);
        }
    }
    let gridfile = PathBuf::from(
        positional
            .first()
            .ok_or_else(|| usage("--mesh-quality needs a gridfile path"))?,
    );
    if positional.len() > 3 {
        return Err(usage("--mesh-quality accepts at most 3 positional args"));
    }
    let out_dir = positional.get(1).map(PathBuf::from).unwrap_or_else(|| {
        gridfile
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    });
    let quality_cfg_path = positional.get(2).map(PathBuf::from);
    let kind = parse_quality_kind(&kind)?;

    let mesh = earthmesh_cli::grid_quality_pipeline::read_gridfile_mesh_points(&gridfile)
        .map_err(|e| format!("read gridfile {}: {e}", gridfile.display()))?;
    let (input, delaunay_rows) = match kind {
        "hex" => earthmesh_cli::grid_quality_pipeline::quality_input_from_gridfile_hex(&mesh)
            .map(|input| (input, None)),
        "hex-delaunay" => {
            earthmesh_cli::grid_quality_pipeline::quality_input_from_gridfile_hex_delaunay_interior(
                &mesh,
            )
            .map(|(input, rows)| (input, Some(rows)))
        }
        _ => earthmesh_cli::grid_quality_pipeline::quality_input_from_gridfile(&mesh)
            .map(|input| (input, None)),
    }
    .map_err(|e| format!("validate gridfile {} for quality: {e}", gridfile.display()))?;

    // Optional &quality block → thresholds + policy; absent ⇒ defaults + warn.
    let quality_cfg_text = match &quality_cfg_path {
        Some(p) => Some(
            fs::read_to_string(p)
                .map_err(|e| format!("read quality config {}: {e}", p.display()))?,
        ),
        None => None,
    };
    let (thresholds, on_violation) = match &quality_cfg_text {
        Some(p) => {
            let q = earthmesh_core::QualityNamelist::from_quality_namelist(p)?;
            (quality_thresholds_from_namelist(&q), q.on_violation)
        }
        None => (
            earthmesh_quality::QualityThresholds::default(),
            "warn".to_string(),
        ),
    };
    if on_violation.eq_ignore_ascii_case("auto_refine") {
        return Err(
            "on_violation=auto_refine requires `mkgrd.x --project <project.yaml>` so EarthMesh can change refinement and rerun; standalone --mesh-quality is read-only"
                .to_string(),
        );
    }

    let mut report = earthmesh_quality::compute(&input, &thresholds);
    report.mesh_name = gridfile.display().to_string();
    report.cell_view = kind.to_string();
    if let Some(text) = &quality_cfg_text {
        let attached =
            earthmesh_cli::grid_quality_pipeline::attach_hfield_diagnostics_from_namelist_for_gridfile(
                &mut report,
                &input,
                &mesh,
                &gridfile,
                if kind == "hex-delaunay" {
                    "tri"
                } else {
                    kind
                },
                text,
            )
            .map_err(|e| format!("attach h-field diagnostics: {e}"))?;
        if attached {
            println!("mesh_quality_hfield=1");
        }
    }
    let written = earthmesh_quality::io::write_all(&report, &out_dir)
        .map_err(|e| format!("write quality report to {}: {e}", out_dir.display()))?;
    println!("mesh_quality_kind={kind}");
    if let Some(rows) = delaunay_rows {
        println!(
            "mesh_quality_delaunay_rows=placeholder:{} interior:{} boundary_dual:{} invalid:0",
            rows.placeholder_rows, rows.interior_triangle_rows, rows.boundary_dual_rows
        );
    }
    println!("mesh_quality_verdict={}", report.verdict.as_str());
    let calibration = earthmesh_quality::io::gate_calibration(&report);
    println!(
        "mesh_quality_gate_calibration={} scope={} reference_set={}",
        calibration.status,
        calibration.scope,
        calibration.reference_set.unwrap_or("none")
    );
    if !calibration.triggered_uncalibrated_gates.is_empty() {
        println!(
            "mesh_quality_triggered_uncalibrated_gates={}",
            calibration.triggered_uncalibrated_gates.join(",")
        );
    }
    eprintln!(
        "earthmesh_cli: quality gate calibration={} scope={}: {}",
        calibration.status, calibration.scope, calibration.caveat
    );
    println!("mesh_quality_cells={}", report.geometry.cell_count);
    println!(
        "mesh_quality_min_angle_deg={}",
        report.geometry.min_angle_deg
    );
    println!(
        "mesh_quality_edge_cv_max={}",
        report.geometry.cell_edge_length_cv.max
    );
    println!(
        "mesh_quality_edge_cv_p95={}",
        report.geometry.cell_edge_length_cv_percentiles.p95
    );
    println!(
        "mesh_quality_edge_cv_p99={}",
        report.geometry.cell_edge_length_cv_percentiles.p99
    );
    println!(
        "mesh_quality_edge_cv_above_warn={}/{}",
        report.geometry.cell_edge_length_cv_above_warn.count,
        report.geometry.cell_edge_length_cv_above_warn.sample_count
    );
    println!(
        "mesh_quality_aspect_ratio=max:{} p95:{} p99:{} above_warn:{}/{} above_fail:{}/{}",
        report.geometry.aspect_ratio.max,
        report.geometry.aspect_ratio_percentiles.p95,
        report.geometry.aspect_ratio_percentiles.p99,
        report.geometry.aspect_ratio_above_warn.count,
        report.geometry.aspect_ratio_above_warn.sample_count,
        report.geometry.aspect_ratio_above_fail.count,
        report.geometry.aspect_ratio_above_fail.sample_count
    );
    println!(
        "mesh_quality_angle_deviation_deg_max={}",
        report.geometry.angle_deviation_deg.max
    );
    println!(
        "mesh_quality_triangle_eta_local_min={}",
        report.geometry.triangle_eta.min
    );
    println!(
        "mesh_quality_triangle_nsr_local_min={}",
        report.geometry.triangle_nsr.min
    );
    println!(
        "mesh_quality_cell_sides=tri:{} quad:{} pent:{} hex:{} hept:{} other:{}",
        report.topology.triangle_cell_count,
        report.topology.quadrilateral_cell_count,
        report.topology.pentagon_cell_count,
        report.topology.hexagon_cell_count,
        report.topology.heptagon_cell_count,
        report.topology.other_polygon_cell_count
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

fn parse_quality_kind(kind: &str) -> Result<&'static str, String> {
    match kind.trim() {
        "tri" => Ok("tri"),
        "hex" => Ok("hex"),
        "hex-delaunay" => Ok("hex-delaunay"),
        other => Err(usage(&format!(
            "--kind must be `tri`, `hex`, or `hex-delaunay`, got `{other}`"
        ))),
    }
}

/// Map a parsed `&quality` namelist block to `earthmesh_quality::QualityThresholds`
/// (field-for-field; the namelist's i32 worst-cell limit becomes usize).
fn quality_thresholds_from_namelist(
    q: &earthmesh_core::QualityNamelist,
) -> earthmesh_quality::QualityThresholds {
    earthmesh_quality::QualityThresholds {
        min_angle_warn_deg: q.min_angle_warn_deg,
        min_angle_fail_deg: q.min_angle_fail_deg,
        angle_deviation_warn_deg: q.angle_deviation_warn_deg,
        aspect_ratio_warn: q.aspect_ratio_warn,
        aspect_ratio_fail: q.aspect_ratio_fail,
        cell_edge_cv_warn: q.cell_edge_cv_warn,
        area_cv_warn: q.area_cv_warn,
        normalized_area_cv_warn: q.normalized_area_cv_warn,
        max_adjacent_resolution_ratio_warn: q.max_adjacent_resolution_ratio_warn,
        worst_cells_limit: q.worst_cells_limit.max(0) as usize,
        repair_batch_limit: q.repair_batch_limit.max(0) as usize,
        repair_level_cap: None,
    }
}
