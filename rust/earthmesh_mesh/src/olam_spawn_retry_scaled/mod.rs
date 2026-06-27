use std::io;

use super::*;

impl OlamDelaunayMesh {
    pub(crate) fn retry_child_with_scaled_parent_region(
        &self,
        parent_regions: &[OlamRefinementRegion],
        parent_grid_number: usize,
        child_regions: &[OlamRefinementRegion],
        child_grid_number: usize,
        max_mrows: usize,
        project_to_radius: bool,
        use_cartesian_xy: bool,
    ) -> io::Result<Option<Self>> {
        for step in 1..=12 {
            let factor = 1.0 - (step as f64 * 0.05);
            let Some(scaled_parent_regions) =
                scale_olam_refinement_regions_radius(parent_regions, factor)
            else {
                return Ok(None);
            };
            let parent_selected = self.selected_regions_faces(
                &scaled_parent_regions,
                scaled_parent_regions[0].level(),
                use_cartesian_xy,
            )?;
            if parent_selected.iter().skip(2).all(|selected| !*selected) {
                return Ok(None);
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
            if let Ok(refined) = parent_mesh.spawn_nest_pass_with_max_mrows(
                &child_selected,
                child_grid_number,
                max_mrows,
                project_to_radius,
            ) {
                return Ok(Some(refined));
            }
            if let Some(refined) = parent_mesh.spawn_nest_pass_with_mask_annealing(
                &child_selected,
                child_grid_number,
                max_mrows,
                project_to_radius,
                true,
            )? {
                return Ok(Some(refined));
            }
        }
        Ok(None)
    }
}
