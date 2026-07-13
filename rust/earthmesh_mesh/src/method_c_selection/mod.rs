use super::*;

impl MethodCDelaunayMesh {
    #[cfg(test)]
    pub(crate) fn selected_region_faces(
        &self,
        region: &MethodCRefinementRegion,
        pass: usize,
        use_cartesian_xy: bool,
    ) -> io::Result<Vec<bool>> {
        self.selected_regions_faces(std::slice::from_ref(region), pass, use_cartesian_xy)
    }

    pub(crate) fn selected_regions_faces(
        &self,
        regions: &[MethodCRefinementRegion],
        pass: usize,
        use_cartesian_xy: bool,
    ) -> io::Result<Vec<bool>> {
        let radius = active_mesh_radius(self)?;
        require_method_c_len("m_points", self.m_points.len(), self.nmd + 1)?;
        let method_c_m_neighbors = self.method_c_m_neighbors()?;
        let mut selected = vec![false; self.nwd + 1];
        if regions.is_empty() {
            return Ok(selected);
        }
        let seed_points = self.selected_region_thirdm_seed_points_with_neighbors(
            regions,
            pass,
            radius,
            &method_c_m_neighbors,
            use_cartesian_xy,
        )?;
        for im in seed_points {
            let mrlo = self.m_metadata[im].mrlm;
            let mut footprint = vec![false; self.nwd + 1];
            self.mark_fill_rad3_faces_with_neighbors(im, &mut footprint, &method_c_m_neighbors)?;
            for iw in 2..=self.nwd {
                if footprint[iw] && self.w_faces[iw].mrlw == mrlo {
                    selected[iw] = true;
                }
            }
        }
        if selected.iter().skip(2).all(|selected| !*selected) {
            return Ok(selected);
        }
        Ok(selected)
    }

    pub(crate) fn method_c_m_neighbors(&self) -> io::Result<Vec<IcosahedronMPointNeighbors>> {
        require_method_c_len(
            "Method-C M-neighbor table",
            self.m_neighbors.len(),
            self.nmd + 1,
        )?;
        Ok(self.m_neighbors.clone())
    }

    #[cfg(test)]
    pub(crate) fn derive_icosahedron_m_neighbors_canonical(
        &self,
    ) -> io::Result<Vec<IcosahedronMPointNeighbors>> {
        derive_icosahedron_m_neighbors_canonical_checked(self.nmd, &self.u_edges, &self.w_faces)
    }

    pub(crate) fn selected_region_thirdm_seed_points_with_neighbors(
        &self,
        regions: &[MethodCRefinementRegion],
        pass: usize,
        radius: f64,
        m_neighbors: &[IcosahedronMPointNeighbors],
        use_cartesian_xy: bool,
    ) -> io::Result<BTreeSet<usize>> {
        require_method_c_len(
            "Method-C perim M-neighbors",
            m_neighbors.len(),
            self.nmd + 1,
        )?;
        let mut seeds = BTreeSet::new();
        let active_regions = regions
            .iter()
            .filter(|region| region.level() >= pass)
            .cloned()
            .collect::<Vec<_>>();
        if active_regions.is_empty() {
            return Ok(seeds);
        }
        let start = self.method_c_refinement_start_point_for_regions_with_neighbors(
            &active_regions,
            radius,
            m_neighbors,
            use_cartesian_xy,
        )?;
        let mrlo = self.m_metadata[start].mrlm;

        let mut jdone = vec![[false; 6]; self.nmd + 1];
        let mut lista = vec![start];
        while let Some(im) = lista.pop() {
            let neighbors = m_neighbors[im];
            for &iu in neighbors.iu.iter().take(neighbors.npoly) {
                require_method_c_id("Method-C refinement boundary U edge", iu, self.nud)?;
                if self.u_edges[iu].mrlu != mrlo {
                    return Err(method_c_repairable_error(
                        MethodCRepairableKind::NonTripletPerimeter,
                        Some(im),
                        format!(
                            "Method-C perimeter length invalid: Current nested grid crosses the parent boundary / next coarser grid boundary at M point {im}"
                        ),
                    ));
                }
            }
            seeds.insert(im);

            for neighbor in self.method_c_thirdm_neighbors_canonical_with_neighbors(
                im,
                &mut jdone,
                m_neighbors,
            )? {
                let point = self.m_points[neighbor];
                let traversed_count = jdone[neighbor].iter().filter(|&&done| done).count();
                if traversed_count < 2
                    && refine_regions_contain_method_c(
                        &active_regions,
                        point,
                        radius,
                        use_cartesian_xy,
                    )
                {
                    lista.push(neighbor);
                }
            }
        }
        Ok(seeds)
    }
}
