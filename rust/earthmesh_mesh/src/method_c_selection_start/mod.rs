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

    /// The canonical start point.
    ///
    /// It is *not* corrected when it lands on ground an earlier tile in this
    /// same pass already refined, and that was tried twice and measured twice.
    ///
    /// The fault is real: the walk takes its generation from its start point,
    /// so a start on already-refined ground runs a generation too fine and
    /// reads every ordinary unrefined edge beside it as coarser -- its test for
    /// stepping off the parent. That the refusals are false follows from the
    /// test itself, since `crosses` is `edge < mrlo` over edges of generation
    /// one or more and so needs `mrlo >= 2`, which a first-level pass over
    /// unrefined ground cannot have.
    ///
    /// Correcting it costs more than it saves. Stepping to the nearest point of
    /// the right generation, and then walking the stride-3 lattice to the
    /// nearest such point instead, were both measured on the globe against the
    /// uncorrected run: refusals fell from 25 of 59 to 20, and the mesh lost
    /// 10,956 faces -- 196,548 down to 185,592. The tiles that stopped refusing
    /// refine a fraction of what they were asked for, because moving the start
    /// moves which ninth of the M points the stride-3 walk can seed and how far
    /// into the regions it can reach.
    ///
    /// So the refusals stay, and they now say which of the two faults they are
    /// (see the walk's own message). What this wants is a start that keeps both
    /// the generation and the coverage, which is a different design rather than
    /// a correction applied to this one.
    pub(crate) fn method_c_refinement_start_point_for_regions_with_neighbors(
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
