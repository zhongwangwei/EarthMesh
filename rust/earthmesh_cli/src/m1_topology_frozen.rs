use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

use earthmesh_core::{EarthmeshConfig, RefineConfig};
use earthmesh_hfield::HField;
use earthmesh_mesh::{
    grid_xyz2lonlat_one_based_state, method_c_edge_target_lengths_from_field,
    pcvt_adjust_voronoi_grid_state, voronoi_grid_from_method_c_delaunay_mesh, MethodCDelaunayMesh,
    MethodCHfieldNestSpringFailure, MethodCHfieldSpringTrace,
};
use serde_json::{json, Value};

use crate::source_demand_artifact::PreparedHfieldDemand;
use crate::{grid_quality_pipeline, mesh_conversion_gridfile_state, MethodCGridfileMetadataSlices};

pub(crate) fn run_if_requested(
    mesh: &MethodCDelaunayMesh,
    field: &HField,
    demand: Option<&PreparedHfieldDemand>,
    namelist: &str,
    nxp: usize,
) -> io::Result<()> {
    let Some(output) = std::env::var_os("EARTHMESH_M1_DIAGNOSTICS_PATH").map(PathBuf::from) else {
        return Ok(());
    };
    let demand = demand.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "M1 diagnostics require persisted HField demand",
        )
    })?;
    let config = EarthmeshConfig::from_mkgrd_namelist(namelist)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let refine = RefineConfig::from_mkrefine_namelist_with_external_field(
        namelist,
        config.mesh_type.trim(),
        config.mode_grid.trim(),
        true,
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let hfield = crate::hfield_refine::read_hfield_refine_options(namelist)?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "M1 diagnostics require an active HField configuration",
        )
    })?;
    let base_m = hfield.base_m.unwrap_or_else(|| {
        2.0 * std::f64::consts::PI * earthmesh_hfield::EARTH_RADIUS_METERS / (5.0 * nxp as f64)
    });
    let extra_iterations = std::env::var("EARTHMESH_M1_EXTRA_ITERS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(4_500);
    let generations = mesh
        .m_point_metadata()
        .iter()
        .map(|metadata| metadata.ngr)
        .filter(|ngr| *ngr > 1)
        .collect::<BTreeSet<_>>();
    if generations.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "M1 diagnostics require at least one refined generation",
        ));
    }

    let branch_dir = output
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("m1_topology_frozen_branches");
    if branch_dir.exists() {
        fs::remove_dir_all(&branch_dir)?;
    }
    fs::create_dir_all(&branch_dir)?;
    let base = measure_branch("base", mesh, mesh, demand, namelist, &branch_dir, 0.0)?;
    let mut branches = Vec::new();
    for branch in ["A", "B", "C", "D"] {
        let started = Instant::now();
        let measured = (|| -> io::Result<Value> {
            let mut adjusted = mesh.clone();
            let mut hfield_failure = None;
            let mut guard_generations = Vec::new();
            let mut guard_backtracked_iterations = 0usize;
            let mut guard_total_halvings = 0usize;
            let mut guard_max_halvings = 0usize;
            for &ngr in &generations {
                if branch == "A" {
                    let (next, diagnostics) =
                        adjusted.spring_nest_guarded(nxp, extra_iterations, ngr, false)?;
                    guard_backtracked_iterations += diagnostics.backtracked_iterations;
                    guard_total_halvings += diagnostics.total_halvings;
                    guard_max_halvings = guard_max_halvings.max(diagnostics.max_halvings);
                    guard_generations.push(json!({
                        "ngr": ngr,
                        "backtracked_iterations": diagnostics.backtracked_iterations,
                        "total_halvings": diagnostics.total_halvings,
                        "max_halvings": diagnostics.max_halvings,
                    }));
                    adjusted = next;
                    continue;
                }
                if branch == "B" || branch == "C" {
                    let targets =
                        method_c_edge_target_lengths_from_field(&adjusted, |lon, lat| {
                            field.sample(lon, lat)
                        })?;
                    let result = adjusted.spring_nest_with_edge_targets_guarded(
                        extra_iterations,
                        ngr,
                        false,
                        true,
                        &targets,
                        branch == "B",
                    );
                    match result {
                        Ok((next, diagnostics)) => {
                            guard_backtracked_iterations += diagnostics.backtracked_iterations;
                            guard_total_halvings += diagnostics.total_halvings;
                            guard_max_halvings = guard_max_halvings.max(diagnostics.max_halvings);
                            guard_generations.push(json!({
                                "ngr": ngr,
                                "backtracked_iterations": diagnostics.backtracked_iterations,
                                "total_halvings": diagnostics.total_halvings,
                                "max_halvings": diagnostics.max_halvings,
                            }));
                            adjusted = next;
                        }
                        Err(error) => {
                            hfield_failure = Some(hfield_failure_json(
                                branch,
                                started.elapsed().as_secs_f64(),
                                &adjusted,
                                &targets,
                                base_m,
                                &error,
                            ));
                            break;
                        }
                    }
                    continue;
                }
                adjusted = adjusted.spring_nest(nxp, extra_iterations, ngr, true)?;
            }
            let guard = json!({
                "enabled": branch != "D",
                "generations": guard_generations,
                "backtracked_iterations": guard_backtracked_iterations,
                "total_halvings": guard_total_halvings,
                "max_halvings": guard_max_halvings,
            });
            if let Some(mut failure) = hfield_failure {
                failure
                    .as_object_mut()
                    .expect("M1 branch failure is a JSON object")
                    .insert("step_guard".to_string(), guard);
                return Ok(failure);
            }
            let mut measured = measure_branch(
                branch,
                mesh,
                &adjusted,
                demand,
                namelist,
                &branch_dir,
                started.elapsed().as_secs_f64(),
            )?;
            measured
                .as_object_mut()
                .expect("M1 branch measurement is a JSON object")
                .insert("step_guard".to_string(), guard);
            Ok(measured)
        })();
        branches.push(measured.unwrap_or_else(|error| {
            json!({
                "branch": branch,
                "status": "failed",
                "wall_time_seconds": started.elapsed().as_secs_f64(),
                "error": error.to_string(),
            })
        }));
    }

    let base_edge = metric(&base, "edge_cv_max")?;
    let a_edge = metric(&branches[0], "edge_cv_max").ok();
    let b_edge = metric(&branches[1], "edge_cv_max").ok();
    let result = json!({
        "kind": "earthmesh_m1_topology_frozen",
        "case": config.experiment_name.trim(),
        "nxp": nxp,
        "base_niter_refine": refine.niter_refine,
        "extra_iterations_per_generation": extra_iterations,
        "generations": generations,
        "field": {
            "nlon": field.nlon(),
            "nlat": field.nlat(),
        },
        "base": base,
        "branches": branches,
        "checks": {
            "all_branches_completed": branches.iter()
                .all(|branch| branch.get("status").and_then(Value::as_str) == Some("ok")),
            "a_reproduces_edge_max_reversal": a_edge.map(|edge| edge > base_edge),
            "b_reduces_a_edge_max_delta": a_edge.zip(b_edge)
                .map(|(a, b)| b - base_edge < a - base_edge),
            "all_successful_branches_topology_frozen": branches.iter()
                .filter(|branch| branch.get("status").and_then(Value::as_str) == Some("ok"))
                .all(|branch| branch.get("topology_frozen").and_then(Value::as_bool) == Some(true)),
        },
    });
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, serde_json::to_vec_pretty(&result)?)?;
    Ok(())
}

fn hfield_failure_json(
    branch: &str,
    wall_time_seconds: f64,
    mesh: &MethodCDelaunayMesh,
    targets: &[f64],
    base_m: f64,
    error: &io::Error,
) -> Value {
    let Some(failure) = error
        .get_ref()
        .and_then(|source| source.downcast_ref::<MethodCHfieldNestSpringFailure>())
    else {
        return json!({
            "branch": branch,
            "status": "failed",
            "wall_time_seconds": wall_time_seconds,
            "error": error.to_string(),
            "failure_trace": {
                "status": "not_available",
                "reason": "unstructured HField spring error",
            },
        });
    };
    let trace_started = Instant::now();
    let trace = mesh
        .trace_hfield_nest_spring_failure(failure, false, true, targets, base_m, 32)
        .map(|trace| hfield_trace_json(&trace, trace_started.elapsed().as_secs_f64()))
        .unwrap_or_else(|trace_error| {
            json!({
                "status": "failed",
                "replay_wall_time_seconds": trace_started.elapsed().as_secs_f64(),
                "error": trace_error.to_string(),
            })
        });
    json!({
        "branch": branch,
        "status": "failed",
        "wall_time_seconds": wall_time_seconds,
        "error": error.to_string(),
        "failure": {
            "iteration": failure.iteration,
            "niter": failure.niter,
            "ngr": failure.ngr,
            "preserve_mrow": failure.preserve_mrow,
            "reason": failure.reason,
            "edge_id": failure.edge_id,
            "adjacent_area_squared": failure.adjacent_area_squared,
            "target_min_distance": failure.target_min_distance,
            "min_area_squared": failure.min_area_squared,
        },
        "failure_trace": trace,
    })
}

fn hfield_trace_json(trace: &MethodCHfieldSpringTrace, replay_wall_time_seconds: f64) -> Value {
    json!({
        "status": "ok",
        "replay_wall_time_seconds": replay_wall_time_seconds,
        "geometry_phase": "before_iteration",
        "failure_iteration": trace.failure_iteration,
        "failure_edge_id": trace.failure_edge_id,
        "triangle_side": trace.triangle_side,
        "triangle_edge_ids": trace.triangle_edge_ids,
        "triangle_m_point_ids": trace.triangle_m_point_ids,
        "samples": trace.samples.iter().map(|sample| {
            json!({
                "iteration": sample.iteration,
                "heron_area_squared": sample.heron_area_squared,
                "applied_vertex_step_m": sample.applied_vertex_step_m,
                "edges": sample.edges.iter().map(|edge| {
                    json!({
                        "edge_id": edge.edge_id,
                        "mrlu": edge.mrlu,
                        "mrow": edge.mrow,
                        "mrow_multiplier": edge.mrow_multiplier,
                        "raw_target_m": edge.raw_target_m,
                        "nominal_target_m": edge.nominal_target_m,
                        "current_length_m": edge.current_length_m,
                        "target_over_nominal": edge.target_over_nominal,
                        "current_over_target": edge.current_over_target,
                        "angle_ratio": edge.angle_ratio,
                        "adjacent_area_squared": edge.adjacent_area_squared,
                        "min_area_over_floor": edge.min_area_over_floor,
                        "area_ratio": edge.area_ratio,
                        "solver_target_before_area_m": edge.solver_target_before_area_m,
                        "solver_target_m": edge.solver_target_m,
                        "current_over_solver_target": edge.current_over_solver_target,
                    })
                }).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
    })
}

fn measure_branch(
    name: &str,
    base: &MethodCDelaunayMesh,
    mesh: &MethodCDelaunayMesh,
    demand: &PreparedHfieldDemand,
    namelist: &str,
    branch_dir: &Path,
    wall_time_seconds: f64,
) -> io::Result<Value> {
    ensure_topology_frozen(base, mesh)?;
    let mut state =
        voronoi_grid_from_method_c_delaunay_mesh(mesh, earthmesh_core::EARTH_RADIUS_METERS)?;
    pcvt_adjust_voronoi_grid_state(&mut state)?;
    grid_xyz2lonlat_one_based_state(&mut state.grid)?;
    let output_mesh = mesh_conversion_gridfile_state::gridfile_mesh_from_one_based_state(
        &state.grid,
        &state.tabs,
    )?;
    let m_lineage = mesh.gridfile_m_cell_lineages()?;
    let w_lineage = mesh.gridfile_w_cell_lineages()?;
    let m_level = levels((1..=state.grid.nma).map(|id| state.tabs.m[id].mrlm), "M")?;
    let m_level_orig = levels(
        (1..=state.grid.nma).map(|id| state.tabs.m[id].mrlm_orig),
        "M orig",
    )?;
    let w_level = levels((1..=state.grid.nwa).map(|id| state.tabs.w[id].mrlw), "W")?;
    let w_level_orig = levels(
        (1..=state.grid.nwa).map(|id| state.tabs.w[id].mrlw_orig),
        "W orig",
    )?;
    let m_ngr = ngrs((1..=state.grid.nma).map(|id| state.tabs.m[id].ngr), "M")?;
    let w_ngr = ngrs((1..=state.grid.nwa).map(|id| state.tabs.w[id].ngr), "W")?;
    let gridfile = branch_dir.join(format!("{name}.nc4"));
    crate::write_unstructured_mesh_netcdf_with_method_c_metadata(
        &gridfile,
        &output_mesh,
        MethodCGridfileMetadataSlices {
            m_lineage: Some(&m_lineage),
            m_refine_level: Some(&m_level),
            m_refine_level_orig: Some(&m_level_orig),
            m_ngr: Some(&m_ngr),
            w_lineage: Some(&w_lineage),
            w_refine_level: Some(&w_level),
            w_refine_level_orig: Some(&w_level_orig),
            w_ngr: Some(&w_ngr),
        },
    )?;
    demand.persist_for_gridfile(&gridfile)?;
    let points = grid_quality_pipeline::read_gridfile_mesh_points(&gridfile)?;
    let input = grid_quality_pipeline::quality_input_from_gridfile_hex(&points)?;
    let mut quality =
        earthmesh_quality::compute(&input, &earthmesh_quality::QualityThresholds::default());
    quality.mesh_name = gridfile.display().to_string();
    quality.cell_view = "hex".to_string();
    grid_quality_pipeline::attach_hfield_diagnostics_from_namelist_for_gridfile(
        &mut quality,
        &input,
        &points,
        &gridfile,
        "hex",
        namelist,
    )?;
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
    Ok(json!({
        "branch": name,
        "status": "ok",
        "wall_time_seconds": wall_time_seconds,
        "gridfile": gridfile,
        "gridfile_sha256": earthmesh_project::file_content_hash(&gridfile)?,
        "topology_frozen": true,
        "quality_verdict": quality.verdict.as_str(),
        "cell_count": quality.geometry.cell_count,
        "edge_cv_max": quality.geometry.cell_edge_length_cv.max,
        "edge_cv_p95": quality.geometry.cell_edge_length_cv_percentiles.p95,
        "edge_cv_p99": quality.geometry.cell_edge_length_cv_percentiles.p99,
        "normalized_area_cv": area_cv,
        "aspect_ratio_max": quality.geometry.aspect_ratio.max,
        "self_intersection_count": quality.geometry.self_intersection_count,
        "invalid_polygon_count": quality.geometry.invalid_polygon_count,
        "topology_issue_count": quality.topology_issues.len(),
        "uncovered_hard_support_bins": uncovered,
        "delaunay": delaunay_quality_json(mesh)?,
    }))
}

fn delaunay_quality_json(mesh: &MethodCDelaunayMesh) -> io::Result<Value> {
    let mut min_area_squared = f64::INFINITY;
    let mut max_edge_ratio = 0.0_f64;
    let mut shape_quality = Vec::with_capacity(mesh.nwd.saturating_sub(1));
    let mut nonpositive_area_count = 0usize;

    for iw in 2..=mesh.nwd {
        let face = mesh.w_faces[iw];
        let [im1, im2, im3] = face.im;
        let distance = |a: usize, b: usize| {
            let p1 = mesh.m_points[a];
            let p2 = mesh.m_points[b];
            let dx = (p2.x - p1.x) as f32;
            let dy = (p2.y - p1.y) as f32;
            let dz = (p2.z - p1.z) as f32;
            (dx * dx + dy * dy + dz * dz).sqrt() as f64
        };
        let [a, b, c] = [distance(im1, im2), distance(im2, im3), distance(im3, im1)];
        if !a.is_finite() || !b.is_finite() || !c.is_finite() || a <= 0.0 || b <= 0.0 || c <= 0.0 {
            nonpositive_area_count += 1;
            continue;
        }
        let s = 0.5 * (a + b + c);
        let area_squared = s * (s - a) * (s - b) * (s - c);
        if !area_squared.is_finite() || area_squared <= 0.0 {
            nonpositive_area_count += 1;
            continue;
        }
        min_area_squared = min_area_squared.min(area_squared);
        let shortest = a.min(b).min(c);
        max_edge_ratio = max_edge_ratio.max(a.max(b).max(c) / shortest);
        shape_quality.push(4.0 * 3.0_f64.sqrt() * area_squared.sqrt() / (a * a + b * b + c * c));
    }
    shape_quality.sort_by(f64::total_cmp);
    let percentile = |fraction: f64| {
        (!shape_quality.is_empty())
            .then(|| shape_quality[((shape_quality.len() - 1) as f64 * fraction).round() as usize])
    };
    Ok(json!({
        "triangle_count": shape_quality.len(),
        "nonpositive_area_count": nonpositive_area_count,
        "min_area_squared": min_area_squared.is_finite().then_some(min_area_squared),
        "max_edge_ratio": max_edge_ratio,
        "shape_quality_min": shape_quality.first(),
        "shape_quality_p01": percentile(0.01),
        "shape_quality_p05": percentile(0.05),
    }))
}

fn ensure_topology_frozen(
    base: &MethodCDelaunayMesh,
    candidate: &MethodCDelaunayMesh,
) -> io::Result<()> {
    let same = base.nmd == candidate.nmd
        && base.nud == candidate.nud
        && base.nwd == candidate.nwd
        && base.impent == candidate.impent
        && base.m_point_metadata() == candidate.m_point_metadata()
        && base.u_edges == candidate.u_edges
        && base.w_faces == candidate.w_faces
        && base.m_neighbors == candidate.m_neighbors
        && base.m_prognostic == candidate.m_prognostic
        && base.u_prognostic == candidate.u_prognostic
        && base.w_prognostic == candidate.w_prognostic
        && base.boundary_rows() == candidate.boundary_rows()
        && base.gridfile_m_cell_lineages()? == candidate.gridfile_m_cell_lineages()?
        && base.gridfile_w_cell_lineages()? == candidate.gridfile_w_cell_lineages()?;
    if !same {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "M1 branch changed frozen topology or ownership metadata",
        ));
    }
    candidate.validate_topology()?;
    Ok(())
}

fn levels(values: impl Iterator<Item = i32>, role: &str) -> io::Result<Vec<i32>> {
    values
        .enumerate()
        .map(|(row, value)| {
            if row == 0 && value <= 0 {
                Ok(0)
            } else if value > 0 {
                Ok(value - 1)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{role} level at row {} is {value}", row + 1),
                ))
            }
        })
        .collect()
}

fn ngrs(values: impl Iterator<Item = i32>, role: &str) -> io::Result<Vec<i32>> {
    values
        .enumerate()
        .map(|(row, value)| {
            if row == 0 && value <= 0 || value > 0 {
                Ok(value.max(0))
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{role} ngr at row {} is {value}", row + 1),
                ))
            }
        })
        .collect()
}

fn metric(value: &Value, name: &str) -> io::Result<f64> {
    value.get(name).and_then(Value::as_f64).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("M1 measurement is missing {name}"),
        )
    })
}
