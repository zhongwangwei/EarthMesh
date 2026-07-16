use std::io;

use super::*;

/// Method-C spawning driven by a quantized target-level field instead of
/// geometric regions ("M4" of the h-field integration).
///
/// The selection seam is the same `Vec<bool>` over Canonical-indexed W faces
/// that `selected_regions_faces` produces; everything downstream — the
/// Method-C pass, mask annealing repair, perimeter mrow construction, and the
/// optional per-pass nest spring — is reused verbatim. Invariants mirrored
/// from the region path: only faces of the current generation
/// (`mrlw == pass`) are selectable, and passes run shallow-to-deep so a
/// gradient-limited field (whose level sets are nested rings with bounded
/// shrink) always presents legal nesting to the discrete machinery.
///
/// Differences from the region path, by design: an empty pass-1 selection
/// returns the mesh unchanged (a field that demands nothing is a no-op, not
/// an error), an empty deeper pass simply stops descending, and the
/// region-specific parent-erosion retry is not applicable.
impl MethodCDelaunayMesh {
    fn sample_target_level<F: Fn(f64, f64) -> u8>(
        &self,
        point: CartesianPoint,
        target_level: &F,
        use_cartesian_xy: bool,
    ) -> usize {
        if use_cartesian_xy {
            usize::from(target_level(point.x, point.y))
        } else {
            let lonlat = xyz_to_lonlat_degrees(point);
            usize::from(target_level(lonlat.lon_degrees, lonlat.lat_degrees))
        }
    }

    fn m_point_target_level<F: Fn(f64, f64) -> u8>(
        &self,
        im: usize,
        target_level: &F,
        use_cartesian_xy: bool,
    ) -> usize {
        self.sample_target_level(self.m_points[im], target_level, use_cartesian_xy)
    }

    fn m_point_or_edge_target_level<F: Fn(f64, f64) -> u8>(
        &self,
        im: usize,
        neighbors: &IcosahedronMPointNeighbors,
        target_level: &F,
        use_cartesian_xy: bool,
    ) -> usize {
        let mut level = self.m_point_target_level(im, target_level, use_cartesian_xy);
        for &iu in neighbors.iu.iter().take(neighbors.npoly) {
            level =
                level.max(self.u_edge_midpoint_target_level(iu, target_level, use_cartesian_xy));
        }
        level
    }

    fn u_edge_midpoint_target_level<F: Fn(f64, f64) -> u8>(
        &self,
        iu: usize,
        target_level: &F,
        use_cartesian_xy: bool,
    ) -> usize {
        let [im1, im2] = self.u_edges[iu].im;
        let p1 = self.m_points[im1];
        let p2 = self.m_points[im2];
        let midpoint = CartesianPoint::new(
            0.5 * (p1.x + p2.x),
            0.5 * (p1.y + p2.y),
            0.5 * (p1.z + p2.z),
        );
        self.sample_target_level(midpoint, target_level, use_cartesian_xy)
    }

    /// True when the M point's sampled target level demands refinement at
    /// `pass` or deeper.
    fn m_point_demands_pass<F: Fn(f64, f64) -> u8>(
        &self,
        im: usize,
        target_level: &F,
        pass: usize,
        use_cartesian_xy: bool,
    ) -> bool {
        self.m_point_target_level(im, target_level, use_cartesian_xy) >= pass
    }

    /// Mirror of `selected_regions_faces` with the geometric containment test
    /// replaced by the target-level closure: grow thirdm-stride seed M points
    /// from a deterministic anchor (the deepest-demand point, lowest id on
    /// ties), then mark each seed's rad3 face footprint filtered to the seed
    /// generation (`mrlw == mrlo`). Reusing the seed/rad3 machinery — rather
    /// than rasterizing face centroids — is what keeps the mask boundary
    /// smooth and multiple-of-3 aligned, which the Method-C perimeter walker
    /// requires.
    pub(crate) fn selected_faces_from_target_levels<F: Fn(f64, f64) -> u8>(
        &self,
        target_level: &F,
        pass: usize,
        use_cartesian_xy: bool,
    ) -> io::Result<Vec<bool>> {
        require_method_c_len("m_points", self.m_points.len(), self.nmd + 1)?;
        require_method_c_len("w_faces", self.w_faces.len(), self.nwd + 1)?;
        let method_c_m_neighbors = self.method_c_m_neighbors()?;
        let mut selected = vec![false; self.nwd + 1];

        let mut levels = vec![0usize; self.nmd + 1];
        for im in 2..=self.nmd {
            levels[im] = self.m_point_or_edge_target_level(
                im,
                &method_c_m_neighbors[im],
                target_level,
                use_cartesian_xy,
            );
        }
        // A deeper H-field level may touch the transition apron produced by
        // the previous pass. Only current-parent interior M points can seed a
        // legal Method-C perimeter; clipping that boundary row preserves the
        // valid demand instead of aborting the whole refinement.
        let mut eligible = vec![false; self.nmd + 1];
        for im in 2..=self.nmd {
            if levels[im] < pass {
                continue;
            }
            let mrlo = self.m_metadata[im].mrlm;
            if mrlo != pass {
                continue;
            }
            let neighbors = method_c_m_neighbors[im];
            let mut parent_interior = true;
            for &iu in neighbors.iu.iter().take(neighbors.npoly) {
                require_method_c_id("Method-C h-field eligibility U edge", iu, self.nud)?;
                if self.u_edges[iu].mrlu != mrlo {
                    parent_interior = false;
                    break;
                }
            }
            eligible[im] = parent_interior;
        }
        let mut component_seen = vec![false; self.nmd + 1];
        for root in 2..=self.nmd {
            if component_seen[root] || !eligible[root] {
                continue;
            }
            let component_mrl = self.m_metadata[root].mrlm;
            let mut component = Vec::new();
            let mut queue = std::collections::VecDeque::from([root]);
            component_seen[root] = true;
            while let Some(im) = queue.pop_front() {
                component.push(im);
                let neighbors = method_c_m_neighbors[im];
                for &iu in neighbors.iu.iter().take(neighbors.npoly) {
                    require_method_c_id("Method-C h-field component U edge", iu, self.nud)?;
                    let edge = self.u_edges[iu];
                    let next = if edge.im[0] == im {
                        edge.im[1]
                    } else {
                        edge.im[0]
                    };
                    if next > 1
                        && next <= self.nmd
                        && !component_seen[next]
                        && eligible[next]
                        && self.m_metadata[next].mrlm == component_mrl
                    {
                        component_seen[next] = true;
                        queue.push_back(next);
                    }
                }
            }
            // Pentagon anchors retain the Canonical stride lattice; otherwise
            // use deepest demand and lowest id as a deterministic tie-break.
            let start = component
                .iter()
                .copied()
                .find(|im| self.impent.contains(im))
                .unwrap_or_else(|| {
                    component
                        .iter()
                        .copied()
                        .max_by(|a, b| levels[*a].cmp(&levels[*b]).then_with(|| b.cmp(a)))
                        .expect("demand component is non-empty")
                });
            let mrlo = self.m_metadata[start].mrlm;
            let has_point_demand = component
                .iter()
                .copied()
                .any(|im| self.m_point_demands_pass(im, target_level, pass, use_cartesian_xy));
            let mut component_member = vec![false; self.nmd + 1];
            for &im in &component {
                component_member[im] = true;
            }
            let mut seeds = std::collections::BTreeSet::new();
            let mut jdone = vec![[false; 6]; self.nmd + 1];
            let mut lista = vec![start];
            while let Some(im) = lista.pop() {
                seeds.insert(im);
                for neighbor in self.method_c_thirdm_neighbors_canonical_with_neighbors(
                    im,
                    &mut jdone,
                    &method_c_m_neighbors,
                )? {
                    let traversed_count = jdone[neighbor].iter().filter(|&&done| done).count();
                    if traversed_count < 2
                        && component_member[neighbor]
                        && self.m_metadata[neighbor].mrlm == mrlo
                        && (self.m_point_demands_pass(
                            neighbor,
                            target_level,
                            pass,
                            use_cartesian_xy,
                        ) || (!has_point_demand && levels[neighbor] >= pass))
                    {
                        lista.push(neighbor);
                    }
                }
            }
            for im in seeds {
                let mrl_seed = self.m_metadata[im].mrlm;
                let mut footprint = vec![false; self.nwd + 1];
                self.mark_fill_rad3_faces_with_neighbors(
                    im,
                    &mut footprint,
                    &method_c_m_neighbors,
                )?;
                for iw in 2..=self.nwd {
                    if footprint[iw] && self.w_faces[iw].mrlw == mrl_seed {
                        selected[iw] = true;
                    }
                }
            }
            for &im in &component {
                let neighbors = method_c_m_neighbors[im];
                if self.m_point_demands_pass(im, target_level, pass, use_cartesian_xy)
                    && !neighbors
                        .iw
                        .iter()
                        .take(neighbors.npoly)
                        .any(|&iw| selected[iw])
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Method-C h-field point demand at M point {im} is not covered by the aligned refinement footprint"
                        ),
                    ));
                }
                for &iu in neighbors.iu.iter().take(neighbors.npoly) {
                    let edge = self.u_edges[iu];
                    if edge.mrlu == pass
                        && self.u_edge_midpoint_target_level(iu, target_level, use_cartesian_xy)
                            >= pass
                        && !edge.iw[..2].iter().any(|&iw| selected[iw])
                    {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "Method-C h-field edge-midpoint demand on U edge {iu} is not covered by the aligned refinement footprint"
                            ),
                        ));
                    }
                }
            }
        }
        Ok(selected)
    }

    fn hfield_has_demand_at_or_above<F: Fn(f64, f64) -> u8>(
        &self,
        target_level: &F,
        level: usize,
        use_cartesian_xy: bool,
    ) -> io::Result<bool> {
        let m_neighbors = self.method_c_m_neighbors()?;
        Ok((2..=self.nmd).any(|im| {
            self.m_point_or_edge_target_level(im, &m_neighbors[im], target_level, use_cartesian_xy)
                >= level
        }))
    }

    fn hfield_has_current_parent_demand<F: Fn(f64, f64) -> u8>(
        &self,
        target_level: &F,
        pass: usize,
        use_cartesian_xy: bool,
    ) -> io::Result<bool> {
        let m_neighbors = self.method_c_m_neighbors()?;
        Ok((2..=self.nmd).any(|im| {
            self.m_metadata[im].mrlm == pass
                && self.m_point_or_edge_target_level(
                    im,
                    &m_neighbors[im],
                    target_level,
                    use_cartesian_xy,
                ) >= pass
        }))
    }

    pub(crate) fn spawn_nest_from_target_levels_internal<F: Fn(f64, f64) -> u8>(
        &self,
        target_level: &F,
        max_level: usize,
        max_mrows: usize,
        spring: Option<(usize, usize, Option<f64>)>,
        use_cartesian_xy: bool,
    ) -> io::Result<(Self, usize)> {
        self.validate_topology()?;
        if max_level == 0 {
            return Ok((self.clone(), 0));
        }
        if max_mrows == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C spawn_nest max_mrows must be greater than zero",
            ));
        }
        if max_level > 5 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Method-C refinement max_level {max_level} must be in 1..=5"),
            ));
        }

        let mut mesh = self.clone();
        let mut spring_passes = 0usize;
        let first_grid_number = self
            .w_faces
            .iter()
            .skip(2)
            .map(|face| face.ngr)
            .chain(self.m_metadata.iter().skip(2).map(|metadata| metadata.ngr))
            .max()
            .unwrap_or(1)
            .max(1)
            + 1;

        let mut grid_number = first_grid_number;
        for pass in 1..=max_level {
            let selected_faces =
                mesh.selected_faces_from_target_levels(target_level, pass, use_cartesian_xy)?;
            if selected_faces.iter().skip(2).all(|selected| !*selected) {
                if mesh.hfield_has_current_parent_demand(target_level, pass, use_cartesian_xy)? {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Method-C h-field pass {pass} demand is entirely on the parent transition boundary"
                        ),
                    ));
                }
                if pass < max_level
                    && mesh.hfield_has_demand_at_or_above(
                        target_level,
                        pass + 1,
                        use_cartesian_xy,
                    )?
                {
                    continue;
                }
                break;
            }
            match mesh.spawn_nest_pass_with_max_mrows(&selected_faces, grid_number, max_mrows, true)
            {
                Ok(refined) => mesh = refined,
                Err(error) => match mesh.spawn_nest_pass_with_mask_annealing(
                    &selected_faces,
                    grid_number,
                    max_mrows,
                    true,
                    pass > 1,
                )? {
                    Some(refined) => mesh = refined,
                    None => {
                        return Err(io::Error::new(
                            error.kind(),
                            format!("Method-C h-field spawn_nest pass {pass} failed: {error}"),
                        ));
                    }
                },
            }

            if let Some((nxp, niter, cartesian_dist00)) = spring {
                if niter > 0 {
                    mesh = mesh.spring_nest_with_radius_projection(
                        nxp,
                        niter,
                        grid_number,
                        false,
                        !use_cartesian_xy,
                        cartesian_dist00,
                    )?;
                    spring_passes += 1;
                }
            }
            grid_number += 1;
        }

        Ok((mesh, spring_passes))
    }

    /// Spawn Method-C nests from a quantized target-level closure, typically
    /// `|lon, lat| hfield.level_at(lon, lat, h_base, max_level)` built from a
    /// composed and gradient-limited `earthmesh_hfield` cell-width field.
    /// Spherical lon/lat meshes only (the Cartesian-XY native path keeps the
    /// geometric region API).
    pub fn spawn_nest_from_target_levels<F: Fn(f64, f64) -> u8>(
        &self,
        target_level: F,
        max_level: usize,
        max_mrows: usize,
    ) -> io::Result<Self> {
        self.spawn_nest_from_target_levels_internal(
            &target_level,
            max_level,
            max_mrows,
            None,
            false,
        )
        .map(|(mesh, _)| mesh)
    }

    /// Same as [`Self::spawn_nest_from_target_levels`], with the compatibility
    /// per-pass nest spring applied after each refinement pass. Returns the
    /// refined mesh together with the number of spring passes executed
    /// (matching the region-path driver's reporting shape).
    pub fn spawn_nest_from_target_levels_with_spring<F: Fn(f64, f64) -> u8>(
        &self,
        target_level: F,
        max_level: usize,
        max_mrows: usize,
        nxp: usize,
        niter: usize,
    ) -> io::Result<(Self, usize)> {
        self.spawn_nest_from_target_levels_internal(
            &target_level,
            max_level,
            max_mrows,
            Some((nxp, niter, None)),
            false,
        )
    }

    /// Cartesian-XY counterpart of
    /// [`Self::spawn_nest_from_target_levels_with_spring`]. The closure is
    /// sampled with native `(x, y)` meters and nest spring uses the same
    /// `deltax` target spacing as the geometric Cartesian Method-C path.
    pub fn spawn_nest_from_cartesian_xy_target_levels_with_spring_deltax<F: Fn(f64, f64) -> u8>(
        &self,
        target_level: F,
        max_level: usize,
        max_mrows: usize,
        nxp: usize,
        niter: usize,
        deltax_meters: f64,
    ) -> io::Result<(Self, usize)> {
        if !deltax_meters.is_finite() || deltax_meters < 0.001 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C Cartesian h-field nest spring deltax must be at least 0.001",
            ));
        }
        let cartesian_dist00 = deltax_meters * (2.0 / 3.0_f64.sqrt()).sqrt();
        self.spawn_nest_from_target_levels_internal(
            &target_level,
            max_level,
            max_mrows,
            Some((nxp, niter, Some(cartesian_dist00))),
            true,
        )
    }
}
