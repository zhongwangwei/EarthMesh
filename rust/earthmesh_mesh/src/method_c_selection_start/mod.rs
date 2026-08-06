use super::*;

impl MethodCDelaunayMesh {
    #[cfg(test)]
    pub(crate) fn method_c_refinement_start_point_with_neighbors(
        &self,
        region: &MethodCRefinementRegion,
        radius: f64,
        m_neighbors: &[IcosahedronMPointNeighbors],
        use_cartesian_xy: bool,
    ) -> io::Result<usize> {
        self.method_c_refinement_start_point_for_regions_with_neighbors(
            std::slice::from_ref(region),
            radius,
            m_neighbors,
            use_cartesian_xy,
        )
    }

    /// The canonical start point, corrected when it lands off the generation
    /// this pass is refining.
    ///
    /// Everything about how the start is *found* is left alone -- the pentagon
    /// containment, the generation-matched nearby pentagon, the march. Only the
    /// answer is checked, and only against the generation the regions
    /// themselves are standing on.
    pub(crate) fn method_c_refinement_start_point_for_regions_with_neighbors(
        &self,
        regions: &[MethodCRefinementRegion],
        radius: f64,
        m_neighbors: &[IcosahedronMPointNeighbors],
        use_cartesian_xy: bool,
    ) -> io::Result<usize> {
        let start = self.method_c_refinement_start_point_for_regions_unadjusted(
            regions,
            radius,
            m_neighbors,
            use_cartesian_xy,
        )?;
        if use_cartesian_xy {
            return Ok(start);
        }
        let Some(target_generation) =
            self.method_c_generation_to_refine(regions, radius, use_cartesian_xy)
        else {
            return Ok(start);
        };
        if self.m_metadata[start].mrlm == target_generation {
            return Ok(start);
        }
        // The canonical search answered with ground an earlier tile in this
        // same pass had already refined. Its generation would become the
        // walk's, and every ordinary unrefined edge beside it would then read
        // as coarser -- the test for stepping off the parent -- so the tile
        // would be refused for touching ground nobody had touched.
        //
        // Stepping to the geometrically nearest point of the right generation
        // is not the way out. The selection walk moves three hops at a time,
        // so only one M point in nine is ever a seed, and *which* ninth is
        // fixed by where the walk starts. The canonical search puts the start
        // on the lattice the pentagons define; a jump straight to the nearest
        // point lands on whatever phase happens to be there, and the tile then
        // refines a fraction of what it was asked to. Measured on the globe:
        // no refusals at all, and eight thousand fewer faces by group ten than
        // the run that refused.
        //
        // So walk the same stride-3 lattice the selection will use, and stop at
        // the first point of the generation this pass refines. The phase is
        // whatever the canonical start had, which is the point.
        // Breadth first, so the answer is the *closest* lattice point of that
        // generation. Depth first would find one too, on the far side of an
        // ocean if the lattice led that way.
        let mut jdone = vec![[false; 6]; self.nmd + 1];
        let mut visited = vec![false; self.nmd + 1];
        let mut frontier = std::collections::VecDeque::from([start]);
        visited[start] = true;
        while let Some(im) = frontier.pop_front() {
            if self.m_metadata[im].mrlm == target_generation {
                return Ok(im);
            }
            for neighbor in self.method_c_thirdm_neighbors_canonical_with_neighbors(
                im,
                &mut jdone,
                m_neighbors,
            )? {
                if !visited[neighbor] {
                    visited[neighbor] = true;
                    frontier.push_back(neighbor);
                }
            }
        }
        // The lattice never reaches that generation from here. The canonical
        // answer is still the canonical answer, and the walk will judge it on
        // its own terms.
        Ok(start)
    }

    fn method_c_refinement_start_point_for_regions_unadjusted(
        &self,
        regions: &[MethodCRefinementRegion],
        radius: f64,
        m_neighbors: &[IcosahedronMPointNeighbors],
        use_cartesian_xy: bool,
    ) -> io::Result<usize> {
        require_method_c_len(
            "Method-C perim M-neighbors",
            m_neighbors.len(),
            self.nmd + 1,
        )?;
        let Some(first_region) = regions.first() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C refinement start requires at least one region",
            ));
        };
        let imcent = self.closest_m_point_to_region_anchor(first_region, use_cartesian_xy)?;
        if use_cartesian_xy {
            return Ok(imcent);
        }
        for &pentagon_id in &self.impent {
            if pentagon_id <= 1 {
                continue;
            }
            require_method_c_id(
                "Method-C refinement pentagon M point",
                pentagon_id,
                self.nmd,
            )?;
            if refine_regions_contain_method_c(
                regions,
                self.m_points[pentagon_id],
                radius,
                use_cartesian_xy,
            ) {
                return Ok(pentagon_id);
            }
        }
        let mut nearby_pentagon = None;
        for &pentagon_id in &self.impent {
            if pentagon_id <= 1 {
                continue;
            }
            require_method_c_id(
                "Method-C refinement pentagon M point",
                pentagon_id,
                self.nmd,
            )?;
            if refine_regions_close_to_method_c(
                regions,
                self.m_points[pentagon_id],
                radius,
                use_cartesian_xy,
            ) && self.m_metadata[pentagon_id].mrlm == self.m_metadata[imcent].mrlm
            {
                nearby_pentagon = Some(pentagon_id);
            }
        }
        if let Some(pentagon_id) = nearby_pentagon {
            if let Some(start) = self
                .method_c_march_from_nearby_pentagon_to_regions_with_neighbors(
                    pentagon_id,
                    regions,
                    radius,
                    m_neighbors,
                    use_cartesian_xy,
                )?
            {
                return Ok(start);
            }
        }
        Ok(imcent)
    }

    /// The generation of the ground this pass still has to refine.
    ///
    /// Method-C takes the walk's generation from its start point, and the start
    /// point is just the M point nearest the region -- with no regard for
    /// whether an earlier tile in this same pass already made it finer. When it
    /// has, the walk runs one generation too fine and reads every ordinary
    /// unrefined edge around it as coarser, which is its test for stepping off
    /// the parent.
    ///
    /// That the refusals were false follows from the test itself. `crosses` is
    /// `edge < mrlo` over edges whose generation is at least one, so it can
    /// only fire when `mrlo` is two or more -- a first-level pass over
    /// unrefined ground can never reach it. A global run reported it nine
    /// times at level one, which is nine proofs that the walk had started on
    /// ground it was not refining.
    ///
    /// The coarsest generation the regions themselves contain is the ground the
    /// pass is there to refine, and it keeps the canonical rule that mrlo comes
    /// from the mesh rather than from a pass counter: a nested region sits
    /// wholly inside its parent, so every point it holds carries the parent's
    /// generation and the minimum is that. Only a tile pressed against a
    /// neighbour refined moments earlier sees two generations at once, and
    /// there the coarser one is the half nobody has served yet.
    fn method_c_generation_to_refine(
        &self,
        regions: &[MethodCRefinementRegion],
        radius: f64,
        use_cartesian_xy: bool,
    ) -> Option<usize> {
        let mut coarsest: Option<usize> = None;
        for im in 2..=self.nmd {
            let generation = self.m_metadata[im].mrlm;
            if generation == 0 || coarsest.is_some_and(|best| generation >= best) {
                continue;
            }
            if refine_regions_contain_method_c(regions, self.m_points[im], radius, use_cartesian_xy)
            {
                coarsest = Some(generation);
                // One is the coarsest a mesh can be, so nothing later can win
                // and the containment tests -- the expensive half of this --
                // stop here. Without it a group of several hundred circles
                // costs one stereographic distance per circle per M point.
                if generation == 1 {
                    break;
                }
            }
        }
        coarsest
    }

    pub(crate) fn closest_m_point_to_region_anchor(
        &self,
        region: &MethodCRefinementRegion,
        use_cartesian_xy: bool,
    ) -> io::Result<usize> {
        if use_cartesian_xy {
            let anchor = region.anchor_lonlat();
            let mut best_im = 0usize;
            let mut best_distance = f64::INFINITY;
            for im in 2..=self.nmd {
                let point = self.m_points[im];
                let distance = (point.x - anchor.lon_degrees).hypot(point.y - anchor.lat_degrees);
                if distance < best_distance {
                    best_distance = distance;
                    best_im = im;
                }
            }
            return require_method_c_id("Method-C refinement anchor M point", best_im, self.nmd)
                .map(|_| best_im);
        }
        let anchor = lonlat_degrees_to_unit_xyz(region.anchor_lonlat());
        let mut best_im = 0usize;
        let mut best_score = f64::NEG_INFINITY;
        for im in 2..=self.nmd {
            let point = self.m_points[im];
            let point_radius = magnitude(point);
            if point_radius == 0.0 {
                continue;
            }
            let score = dot(point, anchor) / point_radius;
            if score > best_score {
                best_score = score;
                best_im = im;
            }
        }
        require_method_c_id("Method-C refinement anchor M point", best_im, self.nmd)?;
        Ok(best_im)
    }
}
