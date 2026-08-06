use std::io;

use super::*;

/// Parent radii to retry with, nearest to the asked-for size first, and the
/// larger of each pair before the smaller.
///
/// The sweep used to run downward only, 0.95 to 0.40, on the reading that a
/// smaller parent is a looser constraint. It is not: legality here is not upward
/// closed, which `ladder.rs` records from its own sweep, so a factor either side
/// of one is a different alignment rather than a tighter or looser one. Measured
/// over sixty three-level cases at NXP 21, fourteen were refused, a larger
/// parent rescued all fourteen and a smaller parent rescued none.
///
/// Both directions are load bearing. Two NXP 7 cases in `method_c_boundary_repair`
/// are rescued only by shrinking, so dropping that half trades one set of
/// refusals for another.
///
/// Larger first at each magnitude, because the two are not equally safe. The
/// error this rescues says the child sits too close to the parent's boundary,
/// and growing the parent is what moves that boundary away; growing refines more
/// than was asked at the parent level, which costs cells, while shrinking
/// refines less, which loses what the run asked for. Nearest first so the parent
/// moves as little as it can.
fn scaled_parent_retry_factors() -> impl Iterator<Item = f64> {
    (1..=12).flat_map(|step| {
        let delta = step as f64 * 0.05;
        [1.0 + delta, 1.0 - delta]
    })
}

impl TriangularMesh {
    pub(crate) fn retry_child_with_scaled_parent_region(
        &self,
        parent_regions: &[RefinementRegion],
        parent_grid_number: usize,
        child_regions: &[RefinementRegion],
        child_grid_number: usize,
        max_mrows: usize,
        project_to_radius: bool,
        use_cartesian_xy: bool,
    ) -> io::Result<Option<Self>> {
        for factor in scaled_parent_retry_factors() {
            let Some(scaled_parent_regions) =
                scale_refinement_regions_radius(parent_regions, factor)
            else {
                // A factor this one cannot express says nothing about the next
                // one, which may be on the other side of 1.0.
                continue;
            };
            let parent_selected = self.selected_regions_faces(
                &scaled_parent_regions,
                scaled_parent_regions[0].level(),
                use_cartesian_xy,
            )?;
            if parent_selected.iter().skip(2).all(|selected| !*selected) {
                continue;
            }
            let Ok(parent_mesh) = self.spawn_nest_pass_with_max_mrows(
                &parent_selected,
                parent_grid_number,
                max_mrows,
                project_to_radius,
            ) else {
                continue;
            };
            let child_selected = parent_mesh.selected_regions_faces(
                child_regions,
                child_regions[0].level(),
                use_cartesian_xy,
            )?;
            if child_selected.iter().skip(2).all(|selected| !*selected) {
                continue;
            }
            // A rescue that hands back a mesh which does not validate is not a
            // rescue, and moving the parent is exactly the kind of change that
            // can produce one. Checking here rather than trusting the pass keeps
            // the search going instead of returning the first thing that built.
            if let Ok(refined) = parent_mesh.spawn_nest_pass_with_max_mrows(
                &child_selected,
                child_grid_number,
                max_mrows,
                project_to_radius,
            ) {
                if refined.validate_topology().is_ok() {
                    return Ok(Some(refined));
                }
            }
            if let Some(refined) = parent_mesh.spawn_nest_pass_with_mask_annealing(
                &child_selected,
                child_grid_number,
                max_mrows,
                project_to_radius,
                true,
            )? {
                if refined.validate_topology().is_ok() {
                    return Ok(Some(refined));
                }
            }
        }
        Ok(None)
    }
}
