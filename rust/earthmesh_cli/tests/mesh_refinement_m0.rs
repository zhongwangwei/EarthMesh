use earthmesh_cli::{
    bbox_mask_io::{write_bbox_mask_netcdf, BBoxMask, BBoxPoint},
    circle_close_mask_io::{write_circle_mask_netcdf, CircleMask},
    coordinate_types::LonLatPoint,
};
use earthmesh_quality::{PercentileTail, QualityThresholds, Stat5};
use serde_json::{json, Value};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};

struct MeasureFailure {
    error: String,
    diagnostics: Option<Value>,
    requested_g: f64,
    effective_g: Option<f64>,
    wall_time_seconds: f64,
    stage_trace: PathBuf,
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn env_f64(name: &str, default: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn env_usizes(name: &str, default: &[usize]) -> Vec<usize> {
    std::env::var(name)
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(|item| item.trim().parse().expect("M0 integer list"))
                .collect()
        })
        .unwrap_or_else(|| default.to_vec())
}

fn env_strings(name: &str, default: &[&str]) -> Vec<String> {
    std::env::var(name)
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(|item| item.trim().to_string())
                .collect()
        })
        .unwrap_or_else(|| default.iter().map(|item| (*item).to_string()).collect())
}

fn topology_g_cap_enabled(value: &str) -> bool {
    match value {
        "on" => true,
        "off" => false,
        other => panic!("M0 topology-g cap variant must be 'on' or 'off', got {other}"),
    }
}

fn effective_g_from_diagnostics(diagnostics: Option<&Value>) -> Option<f64> {
    diagnostics?
        .get("topology_gradation")?
        .get("effective_g")?
        .as_f64()
}

fn assert_successful_m0_valence_census(diagnostics: Option<&Value>) {
    let passes = diagnostics
        .and_then(|diagnostics| diagnostics.get("passes"))
        .and_then(Value::as_array)
        .expect("successful M0 run did not record pass diagnostics");
    assert!(!passes.is_empty(), "successful M0 run recorded no passes");
    for pass in passes {
        let pass_id = pass.get("pass").and_then(Value::as_u64).unwrap_or(0);
        let validation = pass
            .get("candidate_validation")
            .expect("successful M0 pass did not record candidate validation");
        assert_eq!(
            validation
                .get("materialized_m_valence_census_available")
                .and_then(Value::as_bool),
            Some(true),
            "M0 pass {pass_id} did not complete the materialized M-valence census"
        );
        assert_eq!(
            validation
                .get("materialized_m_valence_violation_count")
                .and_then(Value::as_u64),
            Some(0),
            "M0 pass {pass_id} materialized with an overfull M ring"
        );
    }
}

fn measurement_status_counts(records: &[Value]) -> (usize, usize, usize) {
    let ok = records
        .iter()
        .filter(|record| record.get("status").and_then(Value::as_str) == Some("ok"))
        .count();
    let failed = records
        .iter()
        .filter(|record| record.get("status").and_then(Value::as_str) == Some("failed"))
        .count();
    let skipped = records
        .iter()
        .filter(|record| record.get("status").and_then(Value::as_str) == Some("skipped"))
        .count();
    (ok, failed, skipped)
}

fn delaunay_proxy_json(report: &earthmesh_quality::MeshQualityReport) -> Value {
    json!({
        "cell_view": "tri",
        "triangle_count": report.geometry.cell_count,
        "cell_area_min_km2": report.geometry.cell_area.min,
        "aspect_ratio_max": report.geometry.aspect_ratio.max,
        "edge_cv_max": report.geometry.cell_edge_length_cv.max,
        "edge_cv_p95": report.geometry.cell_edge_length_cv_percentiles.p95,
        "edge_cv_p99": report.geometry.cell_edge_length_cv_percentiles.p99,
        "triangle_eta_min": report.geometry.triangle_eta.min,
        "triangle_nsr_min": report.geometry.triangle_nsr.min,
        "zero_area_count": report.geometry.zero_area_cell_count,
        "negative_area_count": report.geometry.negative_area_cell_count,
        "self_intersection_count": report.geometry.self_intersection_count,
        "invalid_polygon_count": report.geometry.invalid_polygon_count,
    })
}

fn write_threshold_matrix(path: &Path, nlon: usize, nlat: usize) {
    let mut values = vec![0.0; nlon * nlat];
    for i in 0..nlon {
        let lon = -180.0 + (i as f64 + 0.5) * 360.0 / nlon as f64;
        for j in 0..nlat {
            let lat = -90.0 + (j as f64 + 0.5) * 180.0 / nlat as f64;
            let fragmented = (105.0..=125.0).contains(&lon)
                && (15.0..=35.0).contains(&lat)
                && ((i + 2 * j) % 3 == 0);
            if fragmented {
                values[i * nlat + j] = 10.0;
            }
        }
    }
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create M0 threshold");
    file.add_dimension("lon", nlon).unwrap();
    file.add_dimension("lat", nlat).unwrap();
    file.add_variable::<f64>("lai", &["lon", "lat"])
        .unwrap()
        .put_values(&values, (.., ..))
        .unwrap();
}

fn write_case(
    root: &Path,
    case_id: &str,
    nxp: usize,
    global_niter: usize,
    niter: usize,
) -> PathBuf {
    let sources = root.join("sources");
    fs::create_dir_all(&sources).unwrap();
    let (mesh_type, output_format, domain, refine, calculated) = match case_id {
        "G-CIRCLE" => {
            write_circle_mask_netcdf(
                sources.join("refine_circle_002.nc4"),
                &CircleMask {
                    refine_degree: 2,
                    points: vec![LonLatPoint {
                        lon: 115.0,
                        lat: 25.0,
                    }],
                    radius_km: vec![2_500.0],
                },
            )
            .unwrap();
            (
                "atmosmesh",
                "MPAS",
                "  NL%mask_domain_global=.true.\n".to_string(),
                format!(
                    "  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=2\n  RL%max_iter_cal=0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{}'\n",
                    sources.join("refine_circle").display()
                ),
                String::new(),
            )
        }
        "G-FRAGMENT" => {
            let threshold = root.join("threshold");
            fs::create_dir_all(&threshold).unwrap();
            write_threshold_matrix(&threshold.join("lai.nc"), 72, 36);
            (
                "landmesh",
                "CoLM",
                "  NL%mask_domain_global=.true.\n".to_string(),
                format!(
                    "  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=2\n  RL%threshold_dir='{}'\n",
                    threshold.display()
                ),
                "  RL%refine_lai_s=.true.\n  RL%th_lai_s=2.0\n".to_string(),
            )
        }
        "R-BBOX" => {
            write_bbox_mask_netcdf(
                sources.join("refine_bbox_002.nc4"),
                &BBoxMask {
                    refine_degree: 2,
                    points: vec![BBoxPoint {
                        west: 105.0,
                        east: 125.0,
                        south: 15.0,
                        north: 35.0,
                    }],
                },
            )
            .unwrap();
            write_bbox_mask_netcdf(
                sources.join("domain_bbox_000.nc4"),
                &BBoxMask {
                    refine_degree: 0,
                    points: vec![BBoxPoint {
                        west: 95.0,
                        east: 135.0,
                        south: 5.0,
                        north: 45.0,
                    }],
                },
            )
            .unwrap();
            (
                "atmosmesh",
                "MPAS",
                format!(
                    "  NL%mask_domain_global=.false.\n  NL%mask_domain_type='bbox'\n  NL%mask_domain_fprefix='{}'\n",
                    sources.join("domain_bbox").display()
                ),
                format!(
                    "  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=2\n  RL%max_iter_cal=0\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{}'\n",
                    sources.join("refine_bbox").display()
                ),
                String::new(),
            )
        }
        other => panic!("unknown M0 case {other}"),
    };

    let namelist = root.join("project.nml");
    let hfield_g = env_f64("EARTHMESH_M0_HFIELD_G", 0.2);
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='m0_{case_id}'\n  NL%base_dir='{}/'\n  NL%NXP={nxp}\n  NL%mesh_type='{mesh_type}'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter={global_niter}\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n{domain}  NL%mask_patch_on=.false.\n  NL%output_format='{output_format}'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%niter_refine={niter}\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n{refine}{calculated}/\n&hfield\n  NL%hfield_on=.true.\n  NL%hfield_g={hfield_g}\n  NL%hfield_max_level=2\n  NL%hfield_nlon=180\n  NL%hfield_nlat=90\n/\n",
            root.display()
        ),
    )
    .unwrap();
    namelist
}

fn fnv1a64(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    format!("{hash:016x}")
}

fn sample_group(values: impl Iterator<Item = (Option<f64>, Option<f64>)>) -> Value {
    let values = values.collect::<Vec<_>>();
    let cell_count = values.len();
    let (edges, areas): (Vec<_>, Vec<_>) = values.into_iter().unzip();
    let edges = edges.into_iter().flatten().collect::<Vec<_>>();
    let areas = areas.into_iter().flatten().collect::<Vec<_>>();
    let edge_stat = Stat5::from_slice(&edges);
    let edge_tail = PercentileTail::from_slice(&edges);
    json!({
        "cell_count": cell_count,
        "edge_cv_max": edge_stat.max,
        "edge_cv_p95": edge_tail.p95,
        "edge_cv_p99": edge_tail.p99,
        "normalized_area_cv": Stat5::from_slice(&areas).cv,
    })
}

#[test]
fn sample_group_counts_members_without_quality_values() {
    let group = sample_group([(Some(0.2), Some(0.3)), (None, None)].into_iter());

    assert_eq!(group["cell_count"], 2);
}

fn reachable_cell_count(
    cell_indices: impl Iterator<Item = usize>,
    lineages: Option<&[i64]>,
    reachable: &BTreeSet<i64>,
) -> Option<usize> {
    lineages.map(|lineages| {
        cell_indices
            .filter(|&cell_index| {
                lineages
                    .get(cell_index)
                    .is_some_and(|lineage| reachable.contains(lineage))
            })
            .count()
    })
}

#[test]
fn reachable_cell_count_uses_quality_aligned_lineages() {
    let reachable = BTreeSet::from([20_i64, 30]);

    assert_eq!(
        reachable_cell_count([0, 1, 2].into_iter(), Some(&[10, 20, 30]), &reachable),
        Some(2)
    );
    assert_eq!(
        reachable_cell_count([0].into_iter(), None, &reachable),
        None
    );
}

fn measure(
    root: &Path,
    case_id: &str,
    nxp: usize,
    global_niter: usize,
    niter: usize,
    repeat: usize,
    topology_g_cap_enabled: bool,
    collect_m0_diagnostics: bool,
) -> Result<(Value, Vec<u8>), MeasureFailure> {
    fs::create_dir_all(root).unwrap();
    let namelist = write_case(root, case_id, nxp, global_niter, niter);
    let contents = fs::read_to_string(&namelist).unwrap();
    let requested_g = env_f64("EARTHMESH_M0_HFIELD_G", 0.2);
    let repair_trace_enabled = std::env::var("EARTHMESH_M0_REPAIR_TRACE")
        .is_ok_and(|value| matches!(value.as_str(), "1" | "on" | "true"));
    let started = Instant::now();
    let stage_trace_path = root.join("stage_trace.jsonl");
    let stage_trace_started = Instant::now();
    let stage_trace = RefCell::new(fs::File::create(&stage_trace_path).unwrap());
    earthmesh_core::progress::set(move |phase, done, total| {
        let mut trace = stage_trace.borrow_mut();
        writeln!(
            trace,
            "{}",
            json!({
                "elapsed_seconds": stage_trace_started.elapsed().as_secs_f64(),
                "phase": phase,
                "done": done,
                "total": total,
            })
        )
        .unwrap();
        trace.flush().unwrap();
        true
    });
    let diagnostics_path = root.join("m2a_diagnostics.json");
    if diagnostics_path.exists() {
        fs::remove_file(&diagnostics_path).unwrap();
    }
    std::env::set_var(
        "EARTHMESH_M0_TOPOLOGY_G_CAP",
        if topology_g_cap_enabled { "on" } else { "off" },
    );
    if collect_m0_diagnostics {
        std::env::set_var("EARTHMESH_M0_DIAGNOSTICS", "1");
        std::env::set_var("EARTHMESH_M0_DIAGNOSTICS_PATH", &diagnostics_path);
    } else {
        std::env::remove_var("EARTHMESH_M0_DIAGNOSTICS");
        std::env::remove_var("EARTHMESH_M0_DIAGNOSTICS_PATH");
    }
    let run = earthmesh_cli::run_refine_pipeline_namelist(&namelist, root, 2_000_000, None);
    earthmesh_core::progress::clear();
    std::env::remove_var("EARTHMESH_M0_TOPOLOGY_G_CAP");
    std::env::remove_var("EARTHMESH_M0_DIAGNOSTICS");
    std::env::remove_var("EARTHMESH_M0_DIAGNOSTICS_PATH");
    let diagnostics = fs::read_to_string(&diagnostics_path)
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok());
    let effective_g = effective_g_from_diagnostics(diagnostics.as_ref());
    let run = match run {
        Ok(run) => run,
        Err(error) => {
            return Err(MeasureFailure {
                error: error.to_string(),
                diagnostics,
                requested_g,
                effective_g,
                wall_time_seconds: started.elapsed().as_secs_f64(),
                stage_trace: stage_trace_path,
            })
        }
    };
    if collect_m0_diagnostics {
        assert!(
            effective_g.is_some(),
            "successful M0 run did not record effective topology g"
        );
        assert_successful_m0_valence_census(diagnostics.as_ref());
    }
    let elapsed = started.elapsed().as_secs_f64();

    let mesh = earthmesh_cli::grid_quality_pipeline::read_gridfile_mesh_points(&run.output.output)
        .unwrap();
    let (input, source_rows) =
        earthmesh_cli::grid_quality_pipeline::quality_input_from_gridfile_hex_with_source_rows(
            &mesh,
        )
        .unwrap();
    let mut quality = earthmesh_quality::compute(&input, &QualityThresholds::default());
    quality.mesh_name = run.output.output.display().to_string();
    quality.cell_view = "hex".to_string();
    let delaunay_proxy = match earthmesh_cli::grid_quality_pipeline::quality_input_from_gridfile_hex_delaunay_interior(&mesh) {
            Ok((tri_input, row_counts)) => {
                let mut report =
                    earthmesh_quality::compute(&tri_input, &QualityThresholds::default());
                report.mesh_name = run.output.output.display().to_string();
                report.cell_view = "tri".to_string();
                let mut proxy = delaunay_proxy_json(&report);
                proxy["available"] = json!(true);
                proxy["placeholder_row_count"] = json!(row_counts.placeholder_rows);
                proxy["interior_triangle_row_count"] =
                    json!(row_counts.interior_triangle_rows);
                proxy["excluded_boundary_dual_row_count"] =
                    json!(row_counts.boundary_dual_rows);
                proxy["invalid_row_count"] = json!(0);
                proxy
            }
            Err(error) => json!({
                "available": false,
                "reason": error.to_string(),
            }),
        };
    earthmesh_cli::grid_quality_pipeline::attach_hfield_diagnostics_from_namelist_for_gridfile(
        &mut quality,
        &input,
        &mesh,
        &run.output.output,
        "hex",
        &contents,
    )
    .unwrap();
    let quality_dir = root.join("quality");
    earthmesh_quality::io::write_all(&quality, &quality_dir).unwrap();

    let reachable = run
        .spring_diagnostics
        .iter()
        .flat_map(|item| item.movable_adjacent_hex_cell_lineages.iter().copied())
        .collect::<BTreeSet<_>>();
    let gridfile_lineages =
        earthmesh_cli::grid_quality_pipeline::read_gridfile_cell_lineages(&run.output.output)
            .unwrap()
            .w;
    let lineages = (source_rows.len() == quality.cell_samples.len())
        .then(|| {
            source_rows
                .iter()
                .map(|&wi| gridfile_lineages.get(wi).copied())
                .collect::<Option<Vec<_>>>()
        })
        .flatten();
    let grouped = if let Some(lineages) = lineages.as_ref() {
        let samples = quality
            .cell_samples
            .iter()
            .zip(lineages)
            .map(|(sample, lineage)| (*sample, reachable.contains(lineage)))
            .collect::<Vec<_>>();
        json!({
            "available": true,
            "movable_adjacent": sample_group(samples.iter().filter(|(_, yes)| *yes).map(|(sample, _)| (sample.edge_length_cv, sample.level_normalized_area))),
            "other": sample_group(samples.iter().filter(|(_, yes)| !*yes).map(|(sample, _)| (sample.edge_length_cv, sample.level_normalized_area))),
        })
    } else {
        json!({"available": false, "reason": "lineage/sample row mismatch"})
    };

    let area_cv = quality
        .gates
        .iter()
        .find(|gate| gate.metric == "cell_area_cv_normalized")
        .map(|gate| gate.value);
    let uncovered = quality
        .gates
        .iter()
        .find(|gate| gate.metric == "hfield_uncovered_hard_support_bin_count")
        .map(|gate| gate.value);
    let hfield = quality.hfield.as_ref().unwrap();
    let gridfile_bytes = fs::read(&run.output.output).unwrap();
    let spring = run
        .spring_diagnostics
        .iter()
        .map(|item| {
            json!({
                "ngr": item.ngr,
                "generation_m_points": item.generation_m_points,
                "movable_m_points": item.movable_m_points,
                "movable_edges": item.movable_edges,
                "shaped_movable_edges": item.shaped_movable_edges,
                "movable_adjacent_hex_cells": item.movable_adjacent_hex_cells,
            })
        })
        .collect::<Vec<_>>();
    let worst_reachable = reachable_cell_count(
        quality.worst_cells.iter().map(|cell| cell.cell_index),
        lineages.as_deref(),
        &reachable,
    );
    let repair_reachable = reachable_cell_count(
        quality.repair_cells.iter().map(|cell| cell.cell_index),
        lineages.as_deref(),
        &reachable,
    );
    let mut record = json!({
        "case": case_id,
        "nxp": nxp,
        "global_niter": global_niter,
        "niter_refine": niter,
        "repeat": repeat,
        "cell_view": "hex",
        "topology_g_cap_enabled": topology_g_cap_enabled,
        "m0_diagnostics_enabled": collect_m0_diagnostics,
        "repair_trace_enabled": repair_trace_enabled,
        "requested_g": requested_g,
        "effective_g": effective_g,
        "wall_time_seconds": elapsed,
        "stage_trace": stage_trace_path,
        "peak_memory_bytes": Value::Null,
        "peak_memory_status": "unavailable_in_process_runner",
        "gridfile": run.output.output,
        "gridfile_fnv1a64": fnv1a64(&gridfile_bytes),
        "quality_verdict": quality.verdict.as_str(),
        "cell_count": quality.geometry.cell_count,
        "self_intersection_count": quality.geometry.self_intersection_count,
        "invalid_polygon_count": quality.geometry.invalid_polygon_count,
        "aspect_ratio_max": quality.geometry.aspect_ratio.max,
        "actual_max_level": run.actual_max_level,
        "refined_cells": run.refined_cells,
        "transition_faces": run.transition_faces,
        "edge_cv_max": quality.geometry.cell_edge_length_cv.max,
        "edge_cv_p95": quality.geometry.cell_edge_length_cv_percentiles.p95,
        "edge_cv_p99": quality.geometry.cell_edge_length_cv_percentiles.p99,
        "normalized_area_cv": area_cv,
        "actual_above_target": hfield.actual_above_target_count,
        "target_above_actual": hfield.target_above_actual_count,
        "uncovered_hard_support_bins": uncovered,
        "topology_issue_count": quality.topology_issues.len(),
        "topology_fail_count": quality.topology_issues.iter().filter(|issue| issue.severity == earthmesh_quality::topology::Severity::Fail).count(),
        "worst_cell_count": quality.worst_cells.len(),
        "worst_movable_adjacent_count": worst_reachable,
        "repair_plan_cell_count": quality.repair_cells.len(),
        "repair_plan_movable_adjacent_count": repair_reachable,
        "groups": grouped,
        "spring": spring,
        "m2a": diagnostics,
    });
    record["delaunay_proxy"] = delaunay_proxy;
    Ok((record, gridfile_bytes))
}

#[allow(clippy::too_many_arguments)]
fn run_measurement_record(
    output: &Path,
    case_id: &str,
    topology_g_cap_variant: &str,
    topology_g_cap_enabled: bool,
    nxp: usize,
    global_niter: usize,
    niter: usize,
    repeat: usize,
    first_outputs: &mut BTreeMap<(String, usize, bool), Vec<u8>>,
    records: &mut Vec<Value>,
) {
    let collect_m0_diagnostics =
        std::env::var("EARTHMESH_M0_COLLECT_DIAGNOSTICS").map_or(true, |value| {
            match value.as_str() {
                "on" => true,
                "off" => false,
                _ => panic!("EARTHMESH_M0_COLLECT_DIAGNOSTICS must be 'on' or 'off'"),
            }
        });
    let run_root = output.join(format!(
        "{case_id}-g{topology_g_cap_variant}-n{niter}-r{repeat}"
    ));
    eprintln!("M0 {case_id} topology_g_cap={topology_g_cap_variant} niter={niter} repeat={repeat}");
    match measure(
        &run_root,
        case_id,
        nxp,
        global_niter,
        niter,
        repeat,
        topology_g_cap_enabled,
        collect_m0_diagnostics,
    ) {
        Ok((mut record, bytes)) => {
            let key = (case_id.to_string(), niter, topology_g_cap_enabled);
            let deterministic = first_outputs
                .get(&key)
                .map(|first| first == &bytes)
                .unwrap_or(true);
            first_outputs.entry(key).or_insert(bytes);
            record["status"] = json!("ok");
            record["deterministic_with_first_repeat"] = json!(deterministic);
            assert!(
                deterministic,
                "{case_id} topology_g_cap={topology_g_cap_variant} niter={niter} repeat={repeat} changed gridfile bytes"
            );
            records.push(record);
        }
        Err(failure) => records.push(json!({
            "case": case_id,
            "nxp": nxp,
            "global_niter": global_niter,
            "niter_refine": niter,
            "repeat": repeat,
            "cell_view": "hex",
            "topology_g_cap_enabled": topology_g_cap_enabled,
            "m0_diagnostics_enabled": collect_m0_diagnostics,
            "repair_trace_enabled": std::env::var("EARTHMESH_M0_REPAIR_TRACE")
                .is_ok_and(|value| matches!(value.as_str(), "1" | "on" | "true")),
            "requested_g": failure.requested_g,
            "effective_g": failure.effective_g,
            "wall_time_seconds": failure.wall_time_seconds,
            "stage_trace": failure.stage_trace,
            "peak_memory_bytes": Value::Null,
            "peak_memory_status": "unavailable_in_process_runner",
            "status": "failed",
            "error": failure.error,
            "m2a": failure.diagnostics,
        })),
    }
}

fn diagnostics_gridfile_parity(
    output: &Path,
    case_id: &str,
    nxp: usize,
    global_niter: usize,
    niter: usize,
    topology_g_cap_enabled: bool,
) -> Value {
    let root = output.join(format!(
        "diagnostics-gridfile-parity-g{}",
        if topology_g_cap_enabled { "on" } else { "off" }
    ));
    if root.exists() {
        fs::remove_dir_all(&root).unwrap();
    }
    let production = measure(
        &root,
        case_id,
        nxp,
        global_niter,
        niter,
        1,
        topology_g_cap_enabled,
        false,
    );
    fs::remove_dir_all(&root).unwrap();
    let measured = measure(
        &root,
        case_id,
        nxp,
        global_niter,
        niter,
        1,
        topology_g_cap_enabled,
        true,
    );
    match (production, measured) {
        (Ok((_, production_bytes)), Ok((_, measured_bytes))) => json!({
            "case": case_id,
            "niter_refine": niter,
            "topology_g_cap_enabled": topology_g_cap_enabled,
            "outcome": "gridfile",
            "gridfile_fnv1a64": fnv1a64(&measured_bytes),
            "matches": measured_bytes == production_bytes,
        }),
        (Err(production), Err(measured)) => {
            let matches = production.error == measured.error;
            json!({
                "case": case_id,
                "niter_refine": niter,
                "topology_g_cap_enabled": topology_g_cap_enabled,
                "outcome": "failure",
                "production_error": production.error,
                "diagnostics_error": measured.error,
                "matches": matches,
            })
        }
        (Ok(_), Err(measured)) => json!({
            "case": case_id,
            "niter_refine": niter,
            "topology_g_cap_enabled": topology_g_cap_enabled,
            "outcome": "mismatch",
            "production_status": "ok",
            "diagnostics_status": "failed",
            "diagnostics_error": measured.error,
            "matches": false,
        }),
        (Err(production), Ok(_)) => json!({
            "case": case_id,
            "niter_refine": niter,
            "topology_g_cap_enabled": topology_g_cap_enabled,
            "outcome": "mismatch",
            "production_status": "failed",
            "production_error": production.error,
            "diagnostics_status": "ok",
            "matches": false,
        }),
    }
}

#[test]
#[ignore = "explicit M0 measurement matrix"]
fn mesh_refinement_m0_measurements() {
    let nxp = env_usize("EARTHMESH_M0_NXP", 81);
    assert!(
        nxp > 0 && nxp.is_multiple_of(3),
        "M0 NXP must be positive and divisible by 3"
    );
    let global_niter = env_usize("EARTHMESH_M0_GLOBAL_NITER", 5000);
    let repeats = env_usize("EARTHMESH_M0_REPEATS", 2);
    let iterations = env_usizes("EARTHMESH_M0_ITERS", &[0, 50, 500, 5000]);
    let cases = env_strings("EARTHMESH_M0_CASES", &["G-CIRCLE", "G-FRAGMENT", "R-BBOX"]);
    let topology_g_cap_variants = env_strings("EARTHMESH_M0_TOPOLOGY_G_CAPS", &["off", "on"]);
    let diagnostics_parity_enabled = topology_g_cap_enabled(
        &std::env::var("EARTHMESH_M0_DIAGNOSTICS_PARITY").unwrap_or_else(|_| "on".to_string()),
    );
    let output = std::env::var_os("EARTHMESH_M0_OUTPUT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/mesh-refinement-m0"));
    fs::create_dir_all(&output).unwrap();

    let mut first_outputs = BTreeMap::<(String, usize, bool), Vec<u8>>::new();
    let mut records = Vec::new();
    for topology_g_cap_variant in &topology_g_cap_variants {
        let topology_g_cap_enabled = topology_g_cap_enabled(topology_g_cap_variant);
        for case_id in &cases {
            let mut skip_after_failed_cap_off_baseline = false;
            for &niter in &iterations {
                if skip_after_failed_cap_off_baseline {
                    for repeat in 1..=repeats {
                        records.push(json!({
                            "case": case_id,
                            "nxp": nxp,
                            "global_niter": global_niter,
                            "niter_refine": niter,
                            "repeat": repeat,
                            "cell_view": "hex",
                            "topology_g_cap_enabled": false,
                            "requested_g": env_f64("EARTHMESH_M0_HFIELD_G", 0.2),
                            "effective_g": Value::Null,
                            "peak_memory_bytes": Value::Null,
                            "peak_memory_status": "not_run",
                            "status": "skipped",
                            "skipped_reason": "cap_off_niter0_failed",
                        }));
                    }
                    continue;
                }
                for repeat in 1..=repeats {
                    run_measurement_record(
                        &output,
                        case_id,
                        topology_g_cap_variant,
                        topology_g_cap_enabled,
                        nxp,
                        global_niter,
                        niter,
                        repeat,
                        &mut first_outputs,
                        &mut records,
                    );
                }
                if !topology_g_cap_enabled && niter == 0 {
                    skip_after_failed_cap_off_baseline = !records.iter().any(|record| {
                        record.get("status").and_then(Value::as_str) == Some("ok")
                            && record.get("case").and_then(Value::as_str) == Some(case_id)
                            && record
                                .get("topology_g_cap_enabled")
                                .and_then(Value::as_bool)
                                == Some(false)
                            && record.get("niter_refine").and_then(Value::as_u64) == Some(0)
                    });
                }
            }
        }
    }
    let parity_records = if diagnostics_parity_enabled {
        cases
            .iter()
            .map(|case_id| {
                diagnostics_gridfile_parity(&output, case_id, nxp, global_niter, 0, true)
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let parity_ok = parity_records
        .iter()
        .all(|record| record.get("matches").and_then(Value::as_bool) == Some(true));
    let (ok_runs, failed_runs, skipped_runs) = measurement_status_counts(&records);
    let attempted_runs = ok_runs + failed_runs;
    let total_records = records.len();
    fs::write(
        output.join("measurements.json"),
        serde_json::to_string_pretty(&json!({
            "kind": "earthmesh_mesh_refinement_m0",
            "nxp": nxp,
            "global_niter": global_niter,
            "iterations": iterations,
            "iteration_policy": "fixed",
            "cases": cases,
            "repeats": repeats,
            "topology_g_cap_variants": topology_g_cap_variants,
            "diagnostics_parity_enabled": diagnostics_parity_enabled,
            "diagnostics_gridfile_parity": parity_records,
            "summary": {
                "total_records": total_records,
                "attempted_runs": attempted_runs,
                "ok_runs": ok_runs,
                "failed_runs": failed_runs,
                "skipped_runs": skipped_runs,
            },
            "runs": records,
        }))
        .unwrap(),
    )
    .unwrap();
    eprintln!("M0 completed: {ok_runs}/{attempted_runs} attempted runs generated meshes");
    assert!(
        ok_runs > 0,
        "M0 generated no meshes: 0/{attempted_runs} successful"
    );
    assert!(
        parity_ok,
        "M0 diagnostics did not preserve gridfile bytes; see measurements.json"
    );
}

#[test]
fn m0_helpers_report_effective_g_and_status_counts() {
    let diagnostics = json!({
        "topology_gradation": {
            "effective_g": 0.0625
        }
    });
    assert_eq!(
        effective_g_from_diagnostics(Some(&diagnostics)),
        Some(0.0625)
    );
    assert!(topology_g_cap_enabled("on"));
    assert!(!topology_g_cap_enabled("off"));
    assert_eq!(
        measurement_status_counts(&[
            json!({"status": "ok"}),
            json!({"status": "failed"}),
            json!({"status": "failed"}),
        ]),
        (1, 2, 0)
    );
}
