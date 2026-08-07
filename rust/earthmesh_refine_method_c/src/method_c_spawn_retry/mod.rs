use std::io;

use super::*;

impl MethodCMesh {
    pub(crate) fn retry_child_with_eroded_parent_mask(
        &self,
        parent_selected_faces: &[bool],
        parent_grid_number: usize,
        parent_regions: &[RefinementRegion],
        child_regions: &[RefinementRegion],
        child_grid_number: usize,
        max_mrows: usize,
        project_to_radius: bool,
        use_cartesian_xy: bool,
    ) -> io::Result<Option<Self>> {
        if let Some(refined) = self.retry_child_with_scaled_parent_region(
            parent_regions,
            parent_grid_number,
            child_regions,
            child_grid_number,
            max_mrows,
            project_to_radius,
            use_cartesian_xy,
        )? {
            return Ok(Some(refined));
        }
        if let Some(refined) = self.retry_child_with_parent_mask_sequence(
            parent_selected_faces.to_vec(),
            true,
            parent_grid_number,
            child_regions,
            child_grid_number,
            max_mrows,
            project_to_radius,
            use_cartesian_xy,
        )? {
            return Ok(Some(refined));
        }
        self.retry_child_with_parent_mask_sequence(
            parent_selected_faces.to_vec(),
            false,
            parent_grid_number,
            child_regions,
            child_grid_number,
            max_mrows,
            project_to_radius,
            use_cartesian_xy,
        )
    }

    pub(crate) fn retry_child_with_parent_mask_sequence(
        &self,
        mut parent_selected: Vec<bool>,
        grow_parent: bool,
        parent_grid_number: usize,
        child_regions: &[RefinementRegion],
        child_grid_number: usize,
        max_mrows: usize,
        project_to_radius: bool,
        use_cartesian_xy: bool,
    ) -> io::Result<Option<Self>> {
        for _ in 0..32 {
            let next_parent = if grow_parent {
                self.grow_method_c_selected_boundary(&parent_selected)?
            } else {
                self.erode_method_c_selected_boundary(&parent_selected)?
            };
            let Some(next_parent) = next_parent else {
                return Ok(None);
            };
            parent_selected = next_parent;
            if parent_selected.iter().skip(2).all(|selected| !*selected) {
                return Ok(None);
            }

            let Ok(parent_mesh) = self.spawn_nest_pass_method_c_without_mask_repair(
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
