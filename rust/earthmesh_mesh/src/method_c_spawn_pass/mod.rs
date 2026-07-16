use super::*;

impl MethodCDelaunayMesh {
    pub(crate) fn spawn_nest_pass_with_max_mrows(
        &self,
        selected_faces: &[bool],
        child_level: usize,
        max_mrows: usize,
        project_to_radius: bool,
    ) -> io::Result<Self> {
        self.spawn_nest_pass_method_c(selected_faces, child_level, max_mrows, project_to_radius)
    }

    pub(crate) fn spawn_nest_pass_method_c(
        &self,
        selected_faces: &[bool],
        child_level: usize,
        max_mrows: usize,
        project_to_radius: bool,
    ) -> io::Result<Self> {
        self.validate_topology()?;
        require_method_c_len("selected_faces", selected_faces.len(), self.nwd + 1)?;
        if child_level <= 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C child level must be greater than one",
            ));
        }

        let mut selected = selected_faces.to_vec();
        let method_c_m_neighbors = self.method_c_m_neighbors()?;
        self.close_method_c_concavities_for_level_with_neighbors(
            &mut selected,
            &method_c_m_neighbors,
        )?;
        self.ensure_method_c_selected_faces_share_parent_mrlw(&selected, child_level)?;

        let mut last_repairable_error = None;
        for _ in 0..64 {
            let perimeter = self.repair_method_c_non_triplet_perimeter(
                &mut selected,
                &method_c_m_neighbors,
                child_level,
            )?;
            let mut nest_wd =
                self.method_c_nest_wd_from_selected_and_perimeter(&selected, &perimeter)?;
            match self.emit_method_c_tables(
                &perimeter,
                &method_c_m_neighbors,
                &mut nest_wd,
                child_level,
                max_mrows,
                project_to_radius,
            ) {
                Ok(mesh) => return Ok(mesh),
                Err(error) if Self::is_repairable_method_c_transition_error(&error) => {
                    let valence_m = Self::method_c_valence_error_m_point(&error);
                    let mut repaired = if valence_m.is_some() {
                        self.try_shrink_method_c_perimeter_once(
                            &selected,
                            &method_c_m_neighbors,
                            child_level,
                            Some(&perimeter),
                        )?
                    } else {
                        None
                    };
                    if repaired.is_none() {
                        repaired = if let Some(im) = valence_m {
                            if im <= self.nmd {
                                self.try_fill_method_c_specific_m_point(
                                    &selected,
                                    &method_c_m_neighbors,
                                    child_level,
                                    im,
                                )?
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                    }
                    if repaired.is_none() {
                        repaired = self.try_fill_method_c_perimeter_boundary(
                            &selected,
                            &method_c_m_neighbors,
                            child_level,
                            Some(&perimeter),
                        )?;
                    }
                    if repaired.is_none() {
                        repaired = self.try_grow_method_c_non_triplet_perimeter_once(
                            &selected,
                            &method_c_m_neighbors,
                            child_level,
                            Some(&perimeter),
                        )?;
                    }
                    let Some((repaired, _)) = repaired else {
                        return Err(error);
                    };
                    selected.clone_from_slice(&repaired);
                    last_repairable_error = Some(error);
                }
                Err(error) => return Err(error),
            }
        }

        Err(last_repairable_error.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Method-C automatic perimeter repair exceeded its iteration limit",
            )
        }))
    }

    pub(crate) fn spawn_nest_pass_method_c_without_mask_repair(
        &self,
        selected_faces: &[bool],
        child_level: usize,
        max_mrows: usize,
        project_to_radius: bool,
    ) -> io::Result<Self> {
        self.validate_topology()?;
        require_method_c_len("selected_faces", selected_faces.len(), self.nwd + 1)?;
        if child_level <= 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C child level must be greater than one",
            ));
        }

        let mut selected = selected_faces.to_vec();
        let method_c_m_neighbors = self.method_c_m_neighbors()?;
        self.close_method_c_concavities_for_level_with_neighbors(
            &mut selected,
            &method_c_m_neighbors,
        )?;
        self.ensure_method_c_selected_faces_share_parent_mrlw(&selected, child_level)?;

        let perimeters =
            self.method_c_perimeters_from_selected_faces(&selected, &method_c_m_neighbors)?;
        if !Self::method_c_perimeters_are_triplets(&perimeters) {
            return Err(method_c_repairable_error(
                MethodCRepairableKind::NonTripletPerimeter,
                None,
                format!(
                    "Method-C perimeter length invalid: perimeter lengths {:?} cannot be grouped into transition triples",
                    perimeters.iter().map(Vec::len).collect::<Vec<_>>()
                ),
            ));
        }
        let perimeter = perimeters.into_iter().flatten().collect::<Vec<_>>();
        let mut nest_wd =
            self.method_c_nest_wd_from_selected_and_perimeter(&selected, &perimeter)?;
        self.emit_method_c_tables(
            &perimeter,
            &method_c_m_neighbors,
            &mut nest_wd,
            child_level,
            max_mrows,
            project_to_radius,
        )
    }
}
