//! A whole red-green run: the levels, then the passes that finish them.
//!
//! The demand arrives planned -- named regions, every level's criteria circles
//! or an h-field -- so nothing here reads a raster, and what leaves is a
//! [`RedGreenMesh`]: the caller converts it to the gridfile's tables and
//! decides what to publish. The one question about the published form that
//! the finishing passes have to ask, whether a hex ring folds, is asked of the
//! caller (`folded_dual_cells`), so it is answered on exactly the mesh that
//! will be written.

use std::io;

use earthmesh_core::RefineConfig;
use earthmesh_mesh::{LonLatDegrees, RefinementRegion, TriangularMesh};
use earthmesh_refine::nest::{nested_criteria_regions, LevelCircles, NestPassReport};
use earthmesh_refine::{HfieldTargets, RegionTargets, TargetLevelField};

use super::{
    polish_redgreen_mesh, redgreen_settings_for_level, refine_redgreen_level,
    repair_redgreen_angle_window,
};
use crate::{redgreen_mesh_from_triangular, triangle_balance_marks, RedGreenMesh};

/// HEX publication needs 5..=7 polygon sides; TRI keeps variable-width W fans.
pub const REDGREEN_MAX_CELL_DEGREE: usize = 7;

/// The criteria half of a red-green request, planned before any level is
/// marked: the circles read only the source raster, never the mesh.
#[derive(Clone, Copy)]
pub struct RedGreenCriteria<'a> {
    /// Every level's circles, level 1 first. Each level's marking also takes
    /// the deeper levels' circles, so a deeper demand always has the level
    /// above it underneath (guide 11.71, 11.78).
    pub planned: &'a [LevelCircles],
    /// The cell the criteria judge at level 1; level `l` judges
    /// `base / 2^(l-1)`.
    pub base_cell_meters: f64,
}

/// Everything a red-green run refines from.
pub struct RedGreenRequest<'a> {
    pub mesh: &'a TriangularMesh,
    /// The named regions, with a regional mother's levels already added and
    /// its domain among them.
    pub named_regions: &'a [RefinementRegion],
    pub refine: &'a RefineConfig,
    pub max_level: usize,
    /// Levels a regional mother refines the domain by before the requested
    /// resolution; zero without one.
    pub mother_levels: usize,
    pub criteria: Option<RedGreenCriteria<'a>>,
    /// With an h-field the named regions are already composed into it, as on
    /// Method-C's h-field route, so each level marks from the field alone.
    pub hfield: Option<&'a HfieldTargets<'a>>,
    /// The base cell the halo margins are counted in.
    pub base_cell_meters: f64,
    /// Triangle output: closure keeps the triangles' shape and leaves cell
    /// degree free. Hex output needs the canonical dual and its degree cap.
    pub preserve_locality: bool,
}

/// What the levels built, before the finishing passes.
pub struct RedGreenLevels {
    pub mesh: RedGreenMesh,
    pub passes: Vec<NestPassReport>,
    pub split_triangles: usize,
    /// Level the run stopped at, zero if nothing was ever split.
    pub deepest_level: usize,
    /// True when a level found nothing asking at its depth.
    pub stopped_on_empty_demand: bool,
    /// Every region a level marked from: the named regions and each level's
    /// criteria circles.
    pub spring_regions: Vec<RefinementRegion>,
}

/// The finished triangulation.
pub struct RedGreenFinish {
    pub mesh: RedGreenMesh,
    /// Faces from a green closure or retriangulated by the polish.
    pub transition_faces: usize,
    /// Hex output kept the polish and the angle window; a caller's spring has
    /// nothing left to do.
    pub hex_repaired: bool,
}

/// Triangles left holding an edge no other triangle owns.
///
/// A mesh of the whole sphere has none: every edge is shared by exactly two
/// triangles. The subdivision steps used to leave them wherever their per
/// triangle antimeridian rotation fired for one of two triangles sharing an
/// edge but not the other, which is fixed -- see
/// `refine_onedivide_four_renew`.
///
/// Kept because of how that failed rather than because one is expected: only
/// the level *after* the one that opened the edges would say so, as "ngrmm row
/// N has invalid neighbor 0", and a single-level run has no next level. It
/// writes the gridfile, and the gridfile opens.
fn redgreen_open_edges(mesh: &RedGreenMesh) -> usize {
    let Some(rows) = earthmesh_mesh::triangle_neighbors_from_cell_membership_one_based(
        &mesh.cells_on_triangle,
        &mesh.triangles_on_cell,
        &mesh.n_triangles_on_cell,
    ) else {
        // Membership that does not resolve at all is worse than an open edge,
        // not better; report it as every triangle being suspect.
        return mesh.triangle_count();
    };
    (mesh.num_vertex + 1..=mesh.triangle_count())
        .filter(|&triangle| rows[triangle].contains(&0))
        .count()
}

/// The widest cell the refinement may change: the most triangles around one.
fn widest_cell(mesh: &RedGreenMesh) -> usize {
    (mesh.num_center + 1..=mesh.cell_count())
        .map(|cell| mesh.n_triangles_on_cell[cell])
        .max()
        .unwrap_or(0)
}

/// The smallest and largest triangle angle, in degrees, over every real face
/// (rows 0 and 1 are placeholders).
fn triangle_angle_range(
    cells_on_triangle: &[[usize; 3]],
    cell_points: &[LonLatDegrees],
) -> io::Result<(f64, f64)> {
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for (row, corners) in cells_on_triangle.iter().enumerate().skip(2) {
        let mut triangle = [LonLatDegrees::new(0.0, 0.0); 3];
        for (slot, &cell) in corners.iter().enumerate() {
            triangle[slot] = *cell_points.get(cell).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "triangle {row} names cell {cell}, but only {} cell rows exist",
                        cell_points.len()
                    ),
                )
            })?;
        }
        let metrics = earthmesh_mesh::polygon_length_angle_metrics(&triangle).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "refinement mesh contains a degenerate triangle",
            )
        })?;
        for angle in metrics.angles_degrees {
            if !angle.is_finite() || angle <= 0.0 || angle >= 180.0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "refinement mesh contains a non-finite or degenerate triangle angle",
                ));
            }
            minimum = minimum.min(angle);
            maximum = maximum.max(angle);
        }
    }
    if !minimum.is_finite() || !maximum.is_finite() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "refinement mesh contains no physical triangles",
        ));
    }
    Ok((minimum, maximum))
}

/// Refine level by level until `max_level` or until nothing asks.
///
/// Each level marks the triangles whose centres the demand asks at least that
/// deep, splits them, and closes the seams. A level that leaves an open edge
/// fails here: only the next level would otherwise notice, and a run that
/// stops at this level has none.
pub fn refine_levels(request: &RedGreenRequest<'_>) -> io::Result<RedGreenLevels> {
    let RedGreenRequest {
        mesh,
        named_regions,
        refine,
        max_level,
        mother_levels,
        criteria,
        hfield,
        base_cell_meters,
        preserve_locality,
    } = *request;
    if !refine.is_transition {
        // Not only for a second level: the transition rows *are* red-green's
        // closure step, so without them even one level comes out with hanging
        // nodes -- 345 open edges on the shipped atmosphere example in tri mode.
        //
        // The engine allows the setting for `mode_grid = 'tri'` alone, and
        // Method-C closes without it, so this is red-green's limit rather than
        // the configuration's. Said here rather than met later as an open-edge
        // count or, at a second level, as "ngrmm row N has invalid neighbor 0".
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "red-green refinement requires RL%Istransition = .true.: the transition rows are what \
             close the seams a 1-into-4 split leaves, so without them the mesh has hanging nodes \
             at any depth. Method-C closes without them; use it for this run",
        ));
    }
    let planned_circles = criteria.map_or(&[][..], |criteria| criteria.planned);
    let mut redgreen = redgreen_mesh_from_triangular(mesh, &mesh.m_neighbors)?;
    let mut previous_marks: Option<Vec<i32>> = None;
    let mut split_triangles = 0usize;
    let mut passes = Vec::new();
    let mut spring_regions = named_regions.to_vec();
    let mut deepest_level = 0usize;
    let mut stopped_on_empty_demand = false;
    for level in 1..=max_level {
        if level <= mother_levels {
            eprintln!(
                "red-green refine level {level}: the domain, toward the requested resolution \
                 ({level} of {mother_levels})"
            );
        }
        let marked_regions = nested_criteria_regions(
            named_regions,
            planned_circles,
            level,
            base_cell_meters,
            |round| redgreen_settings_for_level(refine, round).halo,
            true,
        );
        let region_targets = RegionTargets::new(&marked_regions);
        let before = redgreen.triangle_count();
        // What this level itself asked for, as the run record reports it; the
        // marking reads `region_targets`, which holds every level's. Only the
        // named regions asked at least this deep: the record keeps a pass's
        // regions without their own levels, and the quality step reads every
        // one as asking for the pass's level -- a level-1 region listed in
        // pass 2 read as level-2 demand and reported cells below a target
        // nobody set (guide 11.113).
        let mut level_regions: Vec<RefinementRegion> = named_regions
            .iter()
            .filter(|region| region.level() >= level)
            .cloned()
            .collect();
        let mut demanded_cells = 0usize;
        if let (Some(criteria), Some(demand)) = (criteria, planned_circles.get(level - 1)) {
            demanded_cells = demand.demanded_cells;
            eprintln!(
                "red-green refine level {level} judging {:.0} m cells: {} circles over {} \
                 demanded source cells",
                criteria.base_cell_meters / 2f64.powi((level - 1) as i32),
                demand.circles.len(),
                demand.demanded_cells,
            );
            spring_regions.extend(demand.circles.iter().cloned());
            level_regions.extend(demand.circles.iter().cloned());
        }
        let targets: &dyn TargetLevelField = match hfield {
            Some(field) => field,
            None => &region_targets,
        };
        // Nothing asks at this depth, and nothing deeper will either: the
        // criteria stopped and the named regions that reach here are gone.
        if !targets.demands_anywhere(level) {
            stopped_on_empty_demand = true;
            break;
        }
        let outcome = refine_redgreen_level(
            &redgreen,
            targets,
            refine,
            level,
            previous_marks.as_deref(),
            preserve_locality,
            // The h-field's check reads every face centre; the halves of a
            // green closure are faces too.
            hfield.is_some(),
        )?;
        eprintln!(
            "red-green refine level {level}: {} triangles split, {} grown by the judges, \
             {} dropped as isolated, {} cancelled outside the halo, {} flipped, {before} -> {} triangles",
            outcome.refined_triangle_count,
            outcome.grown_triangle_count,
            outcome.isolated_dropped_count,
            outcome.halo_cancelled_count,
            outcome.flipped_triangle_count,
            outcome.mesh.triangle_count(),
        );
        if let Some(balance) = &outcome.balance_repair {
            eprintln!("earthmesh_cli: Red-Green physical 2:1 level {level}: {} -> {} violations, {} added triangles",
                balance.initial_warning_count, balance.remaining_warning_count, balance.added_triangle_count);
            if let Some(reason) = &balance.rejection_reason {
                eprintln!(
                    "earthmesh_cli: warning: physical balance candidate rolled back: {reason}"
                );
            }
        }
        // The degree the gridfile's dual and the mask post-process are built
        // for. Method-C guarantees {5, 6, 7} by construction; red-green only
        // reaches it by taking back, with Lawson flips, the degree each
        // transition split adds. Checked rather than trusted because a run
        // without a carve -- an atmosphere mesh -- would otherwise write a cell
        // the readers cannot address and say nothing.
        let widest_cell = widest_cell(&outcome.mesh);
        if !preserve_locality && widest_cell > REDGREEN_MAX_CELL_DEGREE {
            // Refused here, a Tibetan land-type run lost everything to one
            // degree-8 cell. The next level splits around it, and the final
            // repair lowers every cell under the cap and is checked itself.
            eprintln!(
                "earthmesh_cli: warning: red-green level {level} left a cell with {widest_cell} \
                 incident triangles; the final repair takes it down to {REDGREEN_MAX_CELL_DEGREE}"
            );
        }
        // Checked here rather than trusted, because the next level is the only
        // thing that would otherwise notice -- and a run that stops at this
        // level has no next level.
        let open_edges = redgreen_open_edges(&outcome.mesh);
        if open_edges > 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "red-green level {level} left {open_edges} triangle edge(s) with no \
                     neighbouring triangle, so the mesh does not close. Writing it would produce \
                     a gridfile that opens and carries a hole, and only a level after this one \
                     would otherwise notice"
                ),
            ));
        }
        split_triangles += outcome.refined_triangle_count;
        previous_marks = Some(outcome.interior_marks.clone());
        passes.push(NestPassReport {
            level,
            cell_meters: criteria
                .map(|criteria| criteria.base_cell_meters / 2f64.powi((level - 1) as i32))
                .unwrap_or(0.0),
            circle_count: level_regions.len(),
            regions: level_regions,
            demanded_cells,
            faces_before: before,
            faces_after: outcome.mesh.triangle_count(),
        });
        deepest_level = level;
        redgreen = outcome.mesh;
    }
    Ok(RedGreenLevels {
        mesh: redgreen,
        passes,
        split_triangles,
        deepest_level,
        stopped_on_empty_demand,
        spring_regions,
    })
}

/// Polish the levels' triangulation and bring it into the angle window.
///
/// Hex output took none of this and was sprung instead, and the regional
/// spring optimises edge lengths: on a global 200 km run it folded 274 hex
/// rings, was discarded, and left the triangles at 26-101 degrees. The polish
/// and the angle window serve the dual as well -- its cells are the triangles'
/// circumcentre rings -- so hex takes them too, and keeps them only while the
/// widest cell stays within what the dual and the mask post-process address
/// and no hex ring folds. `folded_dual_cells` counts the folded rings of a
/// mesh as it will be published.
pub fn finish_levels(
    mut redgreen: RedGreenMesh,
    preserve_locality: bool,
    folded_dual_cells: &dyn Fn(&RedGreenMesh) -> io::Result<usize>,
) -> io::Result<RedGreenFinish> {
    let hex_checkpoint = (!preserve_locality).then(|| redgreen.clone());
    let mut transition_faces;
    {
        // Count final faces derived from green closure or retriangulated by
        // Lawson; zero previously hid every Red-Green transition from the GUI.
        let before = redgreen.cells_on_triangle.clone();
        let mut transitions = vec![false; before.len()];
        for &(_, children) in &redgreen.green_parents {
            for face in children {
                transitions[face] = true;
            }
        }
        // TRI can retain non-Delaunay diagonals; forcing them would undo the
        // closure's angle floor without helping the published triangle mesh.
        let polish = polish_redgreen_mesh(&mut redgreen)?;
        eprintln!("earthmesh_cli: Red-Green Lawson flipped {} edges ({} topology fallback); remaining Delaunay violations={}{}",
            polish.flipped_edges, polish.forced_flips, polish.remaining_illegal_edges,
            if polish.remaining_illegal_edges == 0 { "" } else { "; not Delaunay certified" });
        if polish.flipped_edges > 0 {
            // The polish flips diagonals and moves no point, so the corner
            // table from before it, over the same points, is the mesh it
            // started from.
            let baseline_angles = triangle_angle_range(&before, &redgreen.cell_points)?;
            let candidate_angles =
                triangle_angle_range(&redgreen.cells_on_triangle, &redgreen.cell_points)?;
            if polish.forced_flips == 0
                && (candidate_angles.0 < baseline_angles.0 - 1.0e-4
                    || candidate_angles.1 > baseline_angles.1 + 1.0e-4)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "angle-safe Lawson violated its non-degradation invariant",
                ));
            }
        }
        transition_faces = before
            .iter()
            .zip(&redgreen.cells_on_triangle)
            .enumerate()
            .skip(redgreen.num_vertex + 1)
            .filter(|(i, (old, new))| transitions[*i] || old != new)
            .count();
        let repair = repair_redgreen_angle_window(
            &mut redgreen,
            (!preserve_locality).then_some(REDGREEN_MAX_CELL_DEGREE),
        )?;
        eprintln!(
            "earthmesh_cli: Red-Green angle window: {} -> {} triangles outside, angles {:.2}..{:.2} -> \
             {:.2}..{:.2} degrees, |angle-60| max {:.2} -> {:.2} mean {:.2} -> {:.2} (window \
             phase {:.2}) ({} flips, {} moves, {} vertices removed, {}+{} rounds)",
            repair.outside_before,
            repair.outside_after,
            repair.min_angle_before,
            repair.max_angle_before,
            repair.min_angle_after,
            repair.max_angle_after,
            repair.max_deviation_before,
            repair.max_deviation_after,
            repair.mean_deviation_before,
            repair.mean_deviation_after,
            repair.mean_deviation_window_phase,
            repair.flips,
            repair.moves,
            repair.removed_vertices,
            repair.rounds,
            repair.equilateral_rounds,
        );
        if repair.flips + repair.moves + repair.removed_vertices > 0 {
            let (_, count, ratio) =
                triangle_balance_marks(&redgreen.cell_points, &redgreen.cells_on_triangle, 2)?;
            eprintln!(
                "earthmesh_cli: Red-Green angle window left {count} physical 2:1 violations \
                 (max ratio {ratio:.3})"
            );
        }
    }
    let mut hex_repaired = false;
    if let Some(saved) = hex_checkpoint {
        let widest = widest_cell(&redgreen);
        let (folded_before, folded_after) =
            (folded_dual_cells(&saved)?, folded_dual_cells(&redgreen)?);
        let saved_widest = widest_cell(&saved);
        if widest > REDGREEN_MAX_CELL_DEGREE && saved_widest > REDGREEN_MAX_CELL_DEGREE {
            // The refinement itself left the cell, so the checkpoint has it too.
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "red-green left a cell with {saved_widest} incident triangles that the \
                     repair could not bring down ({widest} after it); the gridfile's dual \
                     and the mask post-process address at most {REDGREEN_MAX_CELL_DEGREE}"
                ),
            ));
        }
        if widest > REDGREEN_MAX_CELL_DEGREE || folded_after > folded_before {
            eprintln!(
                "earthmesh_cli: warning: Red-Green hex repair rolled back (widest cell \
                 {widest}, folded hex rings {folded_before} -> {folded_after}); springing \
                 instead"
            );
            redgreen = saved;
            transition_faces = 0;
        } else {
            hex_repaired = true;
        }
    }
    Ok(RedGreenFinish {
        mesh: redgreen,
        transition_faces,
        hex_repaired,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pass_records_only_the_named_regions_asked_that_deep() {
        // The quality step reads every region of a pass at the pass's level,
        // so a level-1 region listed in pass 2 would read as level-2 demand.
        let mesh = TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25).expect("base mesh");
        let shallow = RefinementRegion::Circle {
            center: LonLatDegrees::new(0.0, 0.0),
            radius_meters: 3_000_000.0,
            level: 1,
        };
        let deep = RefinementRegion::Circle {
            center: LonLatDegrees::new(90.0, 0.0),
            radius_meters: 3_000_000.0,
            level: 2,
        };
        let named = [shallow.clone(), deep.clone()];
        let refine = RefineConfig {
            is_transition: true,
            ..RefineConfig::default()
        };
        let levels = refine_levels(&RedGreenRequest {
            mesh: &mesh,
            named_regions: &named,
            refine: &refine,
            max_level: 2,
            mother_levels: 0,
            criteria: None,
            hfield: None,
            base_cell_meters: 1_000_000.0,
            preserve_locality: true,
        })
        .expect("two red-green levels");
        assert_eq!(levels.passes.len(), 2);
        assert_eq!(levels.passes[0].regions, vec![shallow, deep.clone()]);
        assert_eq!(
            levels.passes[1].regions,
            vec![deep],
            "a level-1 region is not level-2 demand"
        );
    }
}
