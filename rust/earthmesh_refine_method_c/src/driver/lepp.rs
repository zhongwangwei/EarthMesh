//! LEPP-Delaunay's AdaptiveHybrid refinement on a Method-C base mesh, and the
//! repair that takes what it was allowed to overshoot back into Method-C's
//! limits.
//!
//! The demands arrive planned -- named regions and the criteria's circles,
//! with the boundaries to protect already found on the base mesh -- so nothing
//! here reads a raster, and what leaves is a repaired `MeshState`: the caller
//! builds the Voronoi cells and the gridfile from it.

use std::io;

use earthmesh_mesh::{AngleWindowReport, DualShapeReport, MeshState, RefinementRegion};

use crate::{
    refine_adaptive_hybrid, refine_adaptive_hybrid_constrained, AdaptiveHybridConfig,
    AdaptiveHybridDemand, AdaptiveHybridReport, AdaptiveHybridStopReason,
    AdaptiveHybridUnresolvedDemand, LeppInsertionGates,
};

/// The gates LEPP inserts under on a Method-C mesh: the twelve pentagons kept,
/// and every vertex held to the degrees Method-C's tables address (5..=7 for a
/// hex grid).
pub fn method_c_lepp_insertion_gates(
    protected_pentagons: [usize; 12],
    hex: bool,
) -> LeppInsertionGates {
    let mut gates = LeppInsertionGates::for_method_c(protected_pentagons);
    if hex {
        gates.minimum_vertex_degree = 5;
    }
    gates
}

/// The gates the AdaptiveHybrid refinement inserts under.
///
/// Held to Method-C's 5..=7 at every insertion, LEPP refused most of them
/// near the demand's edge -- 892 of the rejections on a global 1000 km circle
/// at two levels, which it left a third unmet. It inserts under 4..=8 and the
/// window repair takes the degrees back afterwards (`repair_lepp_state`),
/// before any table that addresses at most 7. The post-quality pass has no
/// such repair after it and keeps the strict gates.
pub fn method_c_lepp_adaptive_insertion_gates(
    protected_pentagons: [usize; 12],
    hex: bool,
) -> LeppInsertionGates {
    let mut gates = method_c_lepp_insertion_gates(protected_pentagons, hex);
    gates.maximum_vertex_degree = 8;
    gates.minimum_vertex_degree = gates.minimum_vertex_degree.min(4);
    gates
}

/// Everything a LEPP refinement of a Method-C base mesh reads.
pub struct LeppRequest<'a> {
    pub demands: &'a [AdaptiveHybridDemand],
    /// Demands the planning could not turn into a region, reported beside the
    /// ones the refinement leaves unresolved.
    pub pre_unresolved: Vec<AdaptiveHybridUnresolvedDemand>,
    /// Region boundaries on the base mesh that refinement must not cross.
    pub boundary_segments: earthmesh_boundary::SegmentList,
    /// A regional mother's domain, as regions at the requested resolution;
    /// empty without one.
    pub mother_domain: &'a [RefinementRegion],
    pub mother_levels: usize,
    /// The refinement's settings, with the relaxed insertion gates.
    pub config: AdaptiveHybridConfig,
    pub pentagons: [usize; 12],
    pub hex: bool,
}

/// The refined, repaired mesh and the account of how it got there.
pub struct LeppRun {
    pub state: MeshState,
    pub report: AdaptiveHybridReport,
    pub window: AngleWindowReport,
    pub dual: Option<DualShapeReport>,
}

/// Refine `state` toward the demands, then repair it into Method-C's degrees
/// and the angle window -- refining again under the strict gates when a
/// degree is left that the repair cannot take back.
pub fn refine_lepp(mut state: MeshState, request: LeppRequest<'_>) -> io::Result<LeppRun> {
    let LeppRequest {
        demands,
        pre_unresolved,
        mut boundary_segments,
        mother_domain,
        mother_levels,
        config: adaptive_config,
        pentagons,
        hex,
    } = request;
    let refinement_started = std::time::Instant::now();
    eprintln!(
        "earthmesh_cli: LEPP AdaptiveHybrid mesh refinement started: {} demands, {} protected boundary segments, at most {} cycles",
        demands.len(),
        boundary_segments.len(),
        adaptive_config.max_cycles
    );
    // Over a regional mother the domain is refined to the requested
    // resolution first, on its own, and the criteria start from there -- the
    // mesh they start from without a mother. Refined toward 5 km demand
    // straight from 157 km cells, LEPP's paths widened every transition: the
    // Heihe land run delivered 28% more cells, half of its background at the
    // transition size.
    if !mother_domain.is_empty() {
        // Not protected, and not among the run's own demands: every cell of
        // the domain would be both, and resolved again against the refined
        // mesh it would ask 2^k finer still.
        let domain_demands = mother_domain
            .iter()
            .enumerate()
            .map(|(index, region)| {
                AdaptiveHybridDemand::user_region(
                    format!("regional-mother-domain-{index}"),
                    region.clone(),
                )
            })
            .collect::<Vec<_>>();
        let domain_config = AdaptiveHybridConfig {
            // About two bisections of every edge per level.
            max_cycles: 3 * mother_levels,
            ..adaptive_config.clone()
        };
        let domain_report = if boundary_segments.is_empty() {
            refine_adaptive_hybrid(&mut state, &domain_demands, &domain_config)
        } else {
            refine_adaptive_hybrid_constrained(
                &mut state,
                &mut boundary_segments,
                &domain_demands,
                &domain_config,
            )
        }
        .map_err(|error| io::Error::other(error.to_string()))?;
        eprintln!(
            "earthmesh_cli: LEPP regional mother: the domain refined to the requested resolution \
             in {} cycles, {} committed insertions, {} -> {} faces, stop={:?}",
            domain_report.cycles,
            domain_report.path_stats.committed,
            domain_report.initial_faces,
            domain_report.final_faces,
            domain_report.stop_reason,
        );
    }
    // Kept for a second pass under the strict gates, should the relaxed one
    // leave a degree the repair cannot take back.
    let unrefined = (state.clone(), boundary_segments.clone());
    let refine = |state: &mut MeshState,
                  segments: &mut earthmesh_boundary::SegmentList,
                  config: &AdaptiveHybridConfig| {
        if segments.is_empty() {
            refine_adaptive_hybrid(state, demands, config)
        } else {
            refine_adaptive_hybrid_constrained(state, segments, demands, config)
        }
        .map_err(|error| io::Error::other(error.to_string()))
    };
    let mut report = refine(&mut state, &mut boundary_segments, &adaptive_config)?;
    eprintln!(
        "earthmesh_cli: LEPP AdaptiveHybrid mesh refinement complete: {} cycles, {} committed insertions, {} -> {} faces, stop={:?}, {:.1}s",
        report.cycles,
        report.path_stats.committed,
        report.initial_faces,
        report.final_faces,
        report.stop_reason,
        refinement_started.elapsed().as_secs_f64()
    );
    if !pre_unresolved.is_empty() {
        for unresolved in pre_unresolved {
            report.add_unresolved_demand(unresolved);
        }
        if matches!(report.stop_reason, AdaptiveHybridStopReason::Satisfied) {
            report.stop_reason = AdaptiveHybridStopReason::NoCommittableInsertion;
        }
    }
    if hex && report.path_stats.committed == 0 && report.unresolved_demand_count > 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "LEPP AdaptiveHybrid HEX refinement committed no insertions while demands remain unresolved; refusing unchanged 5..=7 publication",
        ));
    }
    let (state, window, dual) = match repair_lepp_state(
        &state,
        report.initial_vertices,
        pentagons,
        hex,
    ) {
        Err(error) if error.to_string().contains(LEPP_OVER_DEGREE) => {
            // A base vertex boxed in by neighbours at the limit (a regional
            // basin run left one at degree 8 among five at 7): no flip lowers
            // it. Refine again under Method-C's own gates, which never let a
            // degree past 7 -- a little less refinement where that binds.
            eprintln!(
                "earthmesh_cli: warning: {error}; refining again under the strict 5..=7 gates"
            );
            let (mut strict_state, mut strict_segments) = unrefined;
            let strict_config = AdaptiveHybridConfig {
                gates: method_c_lepp_insertion_gates(pentagons, hex),
                ..adaptive_config.clone()
            };
            let strict_report = refine(&mut strict_state, &mut strict_segments, &strict_config)?;
            eprintln!(
                    "earthmesh_cli: LEPP AdaptiveHybrid strict refinement complete: {} cycles, {} committed insertions, {} -> {} faces, stop={:?}",
                    strict_report.cycles,
                    strict_report.path_stats.committed,
                    strict_report.initial_faces,
                    strict_report.final_faces,
                    strict_report.stop_reason,
                );
            report = strict_report;
            repair_lepp_state(&strict_state, report.initial_vertices, pentagons, hex)?
        }
        other => other?,
    };
    eprintln!(
        "earthmesh_cli: LEPP angle window: {} -> {} triangles outside, angles {:.2}..{:.2} -> \
         {:.2}..{:.2} degrees ({} flips, {} moves, {} vertices removed)",
        window.outside_before,
        window.outside_after,
        window.min_angle_before,
        window.max_angle_before,
        window.min_angle_after,
        window.max_angle_after,
        window.flips,
        window.moves,
        window.removed_vertices,
    );
    if let Some(dual) = &dual {
        eprintln!(
            "earthmesh_cli: LEPP hex cells: {} -> {} over the aspect/edge-CV limits, aspect \
             {:.3} -> {:.3}, edge CV {:.3} -> {:.3} ({} moves)",
            dual.over_limit_before,
            dual.over_limit_after,
            dual.max_aspect_before,
            dual.max_aspect_after,
            dual.max_edge_cv_before,
            dual.max_edge_cv_after,
            dual.moves,
        );
    }
    Ok(LeppRun {
        state,
        report,
        window,
        dual,
    })
}

/// What `repair_lepp_state` says when a degree above 7 is left.
const LEPP_OVER_DEGREE: &str = "vertices above degree 7 that the window repair could not flip back";

/// Take the degrees LEPP was allowed to overshoot back into 5..=7 and the
/// angles into the window. Base vertices may move but stay; only LEPP's own
/// sites may be removed, so the twelve pentagons keep their ids. For a hex
/// grid, then even out the cells the triangles leave lopsided.
fn repair_lepp_state(
    state: &MeshState,
    initial_vertices: usize,
    pentagons: [usize; 12],
    hex: bool,
) -> io::Result<(
    MeshState,
    earthmesh_mesh::AngleWindowReport,
    Option<earthmesh_mesh::DualShapeReport>,
)> {
    let first = earthmesh_mesh::MESH_STATE_FIRST_ID;
    let radius = state.sphere_radius();
    let mut points = state
        .vertices()
        .iter()
        .map(|p| {
            let r = (p.x * p.x + p.y * p.y + p.z * p.z).sqrt();
            if r > 0.0 {
                [p.x / r, p.y / r, p.z / r]
            } else {
                [0.0; 3]
            }
        })
        .collect::<Vec<_>>();
    let mut faces = state.triangles()[..first].to_vec();
    faces.extend(
        (first..state.triangles().len())
            .filter(|&triangle| state.is_triangle_live(triangle))
            .map(|triangle| state.triangles()[triangle]),
    );
    let (lo, hi) = earthmesh_quality::TRIANGLE_ANGLE_WINDOW_DEG;
    let mut options = earthmesh_mesh::AngleWindowOptions::new((lo + 0.25, hi - 0.25));
    options.first_vertex = first;
    options.first_face = first;
    options.removable_from = initial_vertices;
    // Toward 60 only where LEPP left more than the base grid's own spread:
    // the icosahedral far field (54-72 degrees) is left where it is.
    options.equilateral_rounds = 4;
    options.equilateral_tolerance_deg = 12.5;
    options.max_valence = 7;
    // The twelve pentagons stay at degree 5: Method-C refuses the mesh
    // otherwise, and LEPP's own gates never changed them either.
    let report = earthmesh_mesh::repair_triangle_angle_window_locked(
        &mut points,
        &mut faces,
        &mut Vec::new(),
        options,
        &pentagons,
    );
    let mut degree = vec![0usize; points.len()];
    for face in &faces[first..] {
        for &vertex in face {
            degree[vertex] += 1;
        }
    }
    let over: Vec<usize> = (0..degree.len()).filter(|&v| degree[v] > 7).collect();
    if !over.is_empty() {
        // Where, and what surrounds them: a flip needs a neighbour below 7.
        let detail = over
            .iter()
            .take(3)
            .map(|&v| {
                let p = earthmesh_mesh::xyz_to_lonlat_degrees(earthmesh_mesh::CartesianPoint::new(
                    points[v][0],
                    points[v][1],
                    points[v][2],
                ));
                let mut ring: Vec<usize> = faces[first..]
                    .iter()
                    .filter(|face| face.contains(&v))
                    .flat_map(|face| face.iter().copied().filter(|&u| u != v))
                    .collect();
                ring.sort_unstable();
                ring.dedup();
                let ring_degrees: Vec<usize> = ring.iter().map(|&u| degree[u]).collect();
                format!(
                    "vertex {v} at ({:.3}, {:.3}) degree {}, neighbours {ring_degrees:?}",
                    p.lon_degrees, p.lat_degrees, degree[v]
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "LEPP left {} vertices above degree 7 that the window repair could not flip \
                 back; Method-C tables address at most 7 ({detail})",
                over.len()
            ),
        ));
    }
    // The triangles can all be in the window while a hex cell is not near a
    // hexagon: at a jump from fine to coarse neighbours one of its edges was
    // a quarter of another (aspect 4.07, edge CV 0.395, the only cell over
    // the quality check's lines in a global 1000 km circle at two levels).
    // Moving generators evens such cells out; the faces stay as they are.
    let dual = hex.then(|| {
        let thresholds = earthmesh_quality::QualityThresholds::default();
        let mut dual_options = earthmesh_mesh::DualShapeOptions::new((lo + 0.25, hi - 0.25));
        dual_options.aspect_limit = thresholds.aspect_ratio_warn;
        dual_options.edge_cv_limit = thresholds.cell_edge_cv_warn;
        dual_options.first_vertex = first;
        dual_options.first_face = first;
        earthmesh_mesh::even_out_dual_cells(&mut points, &faces, dual_options)
    });
    let vertices = points
        .iter()
        .map(|&[x, y, z]| earthmesh_mesh::CartesianPoint::new(x * radius, y * radius, z * radius))
        .collect();
    let repaired = MeshState::from_parts(vertices, faces).map_err(|errors| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "LEPP window repair left an invalid mesh: {}",
                errors
                    .iter()
                    .take(3)
                    .map(|error| format!("{error:?}"))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        )
    })?;
    Ok((repaired, report, dual))
}
