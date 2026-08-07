use std::{collections::HashMap, io};

use super::*;

#[derive(Clone, Debug)]
pub(crate) struct MethodCHfieldDemandCoverage {
    anchors: Vec<(usize, Vec<usize>)>,
    /// Anchors sampled from the field before any legality clipping.
    requested_anchor_count: usize,
    demanded_face_count: usize,
    unmet_face_count: usize,
    /// Anchors dropped for lacking a complete rad3 footprint. `validate` only
    /// ever sees the survivors, so without this count a partly-honoured pass is
    /// indistinguishable from a fully-honoured one.
    clipped_anchor_count: usize,
}

/// Per-run tally of what the h-field asked for versus what survived Method-C's
/// legality rules, accumulated over every pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MethodCHfieldSpawnDiagnostics {
    pub requested_anchor_count: usize,
    pub covered_anchor_count: usize,
    pub boundary_clipped_anchor_count: usize,
    /// Faces the field demanded at this level, and those the selection does not
    /// cover. Reported on success too: a run well short of the error threshold
    /// is still losing demand, and the ratio is the only way to see it.
    pub demanded_face_count: usize,
    pub unmet_face_count: usize,
}

enum MethodCHfieldRad3Footprint {
    Materializable(Vec<usize>),
    PeriodicSeam,
}

impl MethodCHfieldDemandCoverage {
    #[cfg(test)]
    pub(crate) fn from_anchors(anchors: Vec<(usize, Vec<usize>)>) -> Self {
        let requested_anchor_count = anchors.len();
        Self {
            anchors,
            requested_anchor_count,
            demanded_face_count: 0,
            unmet_face_count: 0,
            clipped_anchor_count: 0,
        }
    }

    pub(crate) fn requested_anchor_count(&self) -> usize {
        self.requested_anchor_count
    }

    pub(crate) fn covered_anchor_count(&self) -> usize {
        self.anchors.len()
    }

    pub(crate) fn clipped_anchor_count(&self) -> usize {
        self.clipped_anchor_count
    }

    pub(crate) fn demanded_face_count(&self) -> usize {
        self.demanded_face_count
    }

    pub(crate) fn unmet_face_count(&self) -> usize {
        self.unmet_face_count
    }

    pub(crate) fn validate(&self, selected: &[bool]) -> io::Result<()> {
        for (im, faces) in &self.anchors {
            if !faces
                .iter()
                .any(|&iw| selected.get(iw).copied().unwrap_or(false))
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Method-C h-field aligned demand anchor at M point {im} is not covered by the refinement mask"
                    ),
                ));
            }
        }
        Ok(())
    }
}

/// Method-C spawning driven by a quantized target-level field instead of
/// geometric regions ("M4" of the h-field integration).
///
/// The selection seam is the same `Vec<bool>` over Canonical-indexed W faces
/// that `selected_regions_faces` produces; everything downstream — the
/// Method-C pass, perimeter mrow construction, and the optional per-pass nest
/// spring — is reused verbatim. Invariants mirrored
/// from the region path: only faces of the current generation
/// (`mrlw == pass`) are selectable, and passes run shallow-to-deep so a
/// gradient-limited field (whose level sets are nested rings with bounded
/// shrink) always presents legal nesting to the discrete machinery.
///
/// Differences from the region path, by design: an empty pass-1 selection
/// returns the mesh unchanged (a field that demands nothing is a no-op, not
/// an error), an empty deeper pass simply stops descending, and the
/// region-specific parent-erosion retry is not applicable.
impl MethodCMesh {
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

    fn cartesian_hfield_rad3_failure_is_periodic_seam(
        &self,
        im: usize,
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<bool> {
        if self.impent.iter().any(|&pentagon| pentagon != 1) {
            return Ok(false);
        }
        require_method_c_id("Method-C Cartesian h-field seed M point", im, self.nmd)?;
        require_method_c_len(
            "Method-C Cartesian h-field M-neighbors",
            m_neighbors.len(),
            self.nmd + 1,
        )?;
        require_method_c_len(
            "Method-C Cartesian h-field M prognostic map",
            self.m_prognostic.len(),
            self.nmd + 1,
        )?;
        require_method_c_len(
            "Method-C Cartesian h-field W prognostic map",
            self.w_prognostic.len(),
            self.nwd + 1,
        )?;
        require_method_c_len(
            "Method-C Cartesian h-field W faces",
            self.w_faces.len(),
            self.nwd + 1,
        )?;
        require_method_c_len(
            "Method-C Cartesian h-field U edges",
            self.u_edges.len(),
            self.nud + 1,
        )?;

        let m_is_periodic_copy = |point: usize| -> io::Result<bool> {
            require_method_c_id("Method-C Cartesian h-field seam M point", point, self.nmd)?;
            let owner = self.m_prognostic[point];
            require_method_c_id("Method-C Cartesian h-field seam M owner", owner, self.nmd)?;
            if self.m_prognostic[owner] != owner {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Method-C Cartesian h-field M prognostic owner {owner} for point {point} is not canonical"
                    ),
                ));
            }
            Ok(owner != point)
        };
        let w_is_periodic_copy = |face: usize| -> io::Result<bool> {
            require_method_c_id("Method-C Cartesian h-field seam W face", face, self.nwd)?;
            let owner = self.w_prognostic[face];
            require_method_c_id("Method-C Cartesian h-field seam W owner", owner, self.nwd)?;
            if self.w_prognostic[owner] != owner {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Method-C Cartesian h-field W prognostic owner {owner} for face {face} is not canonical"
                    ),
                ));
            }
            Ok(owner != face)
        };
        let reciprocal_w_neighbors = |face_iw: usize| -> io::Result<[usize; 3]> {
            require_method_c_id(
                "Method-C Cartesian h-field reciprocal W face",
                face_iw,
                self.nwd,
            )?;
            let face = self.w_faces[face_iw];
            let mut result = [1usize; 3];
            for (slot, result_iw) in result.iter_mut().enumerate() {
                let iu = face.iu[slot];
                require_method_c_id("Method-C Cartesian h-field reciprocal U edge", iu, self.nud)?;
                let edge = self.u_edges[iu];
                let other_iw = if edge.iw[0] == face_iw {
                    edge.iw[1]
                } else if edge.iw[1] == face_iw {
                    edge.iw[0]
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Method-C Cartesian h-field W face {face_iw} edge slot {slot} points at U edge {iu}, but the edge does not point back"
                        ),
                    ));
                };
                require_method_c_id(
                    "Method-C Cartesian h-field reciprocal W neighbor",
                    other_iw,
                    self.nwd,
                )?;
                *result_iw = other_iw;
            }
            Ok(result)
        };

        let neighbors = m_neighbors[im];
        for &iw in neighbors.iw.iter().take(neighbors.npoly) {
            require_method_c_id("Method-C Cartesian h-field sector W face", iw, self.nwd)?;
            let sector = self.w_faces[iw];
            let (imx, iwx, iwy, inner_slot) = if im == sector.im[0] {
                (sector.im[1], sector.iw[3], sector.iw[4], 0)
            } else if im == sector.im[1] {
                (sector.im[2], sector.iw[5], sector.iw[6], 1)
            } else if im == sector.im[2] {
                (sector.im[0], sector.iw[7], sector.iw[8], 2)
            } else {
                return Ok(false);
            };
            require_method_c_id("Method-C Cartesian h-field sector M point", imx, self.nmd)?;
            require_method_c_id("Method-C Cartesian h-field outer W face", iwx, self.nwd)?;
            require_method_c_id("Method-C Cartesian h-field outer W face", iwy, self.nwd)?;

            let (im1, im2) = match face_following_two_vertices(self.w_faces[iwx], imx, iwx) {
                Ok(points) => points,
                Err(_) => {
                    // `iw[3..9]` is not covered by the general topology
                    // validator. Re-derive this exact pair from the validated
                    // first-ring adjacency before accepting the known cart_hex
                    // periodic representation gap. An arbitrary ghost pointer
                    // therefore remains a fatal rad3 error.
                    let sector_neighbors = reciprocal_w_neighbors(iw)?;
                    let inner_iw = sector_neighbors[inner_slot];
                    let inner_neighbors = reciprocal_w_neighbors(inner_iw)?;
                    let canonical_pair = tri_neighbors_outer_w_pair(iw, inner_neighbors);
                    for &outer_iw in &canonical_pair {
                        require_method_c_id(
                            "Method-C Cartesian h-field canonical outer W face",
                            outer_iw,
                            self.nwd,
                        )?;
                    }
                    if [iwx, iwy] != canonical_pair
                        || self.w_faces[canonical_pair[0]].im.contains(&imx)
                        || self.w_faces[canonical_pair[1]].im.contains(&imx)
                    {
                        return Ok(false);
                    }
                    let mut touches_periodic_copy = m_is_periodic_copy(imx)?;
                    for &outer_iw in &canonical_pair {
                        touches_periodic_copy |= w_is_periodic_copy(outer_iw)?;
                        for &face_im in &self.w_faces[outer_iw].im {
                            touches_periodic_copy |= m_is_periodic_copy(face_im)?;
                        }
                    }
                    return Ok(touches_periodic_copy);
                }
            };
            require_method_c_id("Method-C Cartesian h-field distant M point", im1, self.nmd)?;
            require_method_c_id("Method-C Cartesian h-field distant M point", im2, self.nmd)?;
            let im3 = match face_following_vertex(self.w_faces[iwy], im2, iwy) {
                Ok(point) => point,
                Err(_) => return Ok(false),
            };
            require_method_c_id("Method-C Cartesian h-field distant M point", im3, self.nmd)?;
            for far_im in [im1, im2, im3] {
                for &far_iw in m_neighbors[far_im].iw.iter().take(6) {
                    if far_iw > self.nwd {
                        return Ok(false);
                    }
                }
            }
        }
        Ok(false)
    }

    fn hfield_rad3_footprint(
        &self,
        im: usize,
        m_neighbors: &[IcosahedronMPointNeighbors],
        use_cartesian_xy: bool,
    ) -> io::Result<MethodCHfieldRad3Footprint> {
        match self.method_c_rad3_faces_with_neighbors(im, m_neighbors) {
            Ok(faces) => Ok(MethodCHfieldRad3Footprint::Materializable(faces)),
            Err(_error)
                if use_cartesian_xy
                    && self.cartesian_hfield_rad3_failure_is_periodic_seam(im, m_neighbors)? =>
            {
                Ok(MethodCHfieldRad3Footprint::PeriodicSeam)
            }
            Err(error) => Err(error),
        }
    }

    #[cfg(test)]
    pub(crate) fn hfield_rad3_faces_for_test(
        &self,
        im: usize,
        m_neighbors: &[IcosahedronMPointNeighbors],
        use_cartesian_xy: bool,
    ) -> io::Result<Option<Vec<usize>>> {
        self.hfield_rad3_footprint(im, m_neighbors, use_cartesian_xy)
            .map(|footprint| match footprint {
                MethodCHfieldRad3Footprint::Materializable(faces) => Some(faces),
                MethodCHfieldRad3Footprint::PeriodicSeam => None,
            })
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
    #[cfg(test)]
    pub(crate) fn selected_faces_from_target_levels<F: Fn(f64, f64) -> u8>(
        &self,
        target_level: &F,
        pass: usize,
        use_cartesian_xy: bool,
    ) -> io::Result<Vec<bool>> {
        self.selected_faces_and_coverage_from_target_levels_with_policy(
            target_level,
            pass,
            use_cartesian_xy,
            true,
        )
        .map(|(selected, _)| selected)
    }

    fn selected_faces_and_coverage_from_target_levels_with_policy<F: Fn(f64, f64) -> u8>(
        &self,
        target_level: &F,
        pass: usize,
        use_cartesian_xy: bool,
        preserve_all_demands: bool,
    ) -> io::Result<(Vec<bool>, MethodCHfieldDemandCoverage)> {
        require_method_c_len("m_points", self.m_points.len(), self.nmd + 1)?;
        require_method_c_len("w_faces", self.w_faces.len(), self.nwd + 1)?;
        if use_cartesian_xy {
            require_method_c_len(
                "Method-C Cartesian h-field M prognostic map",
                self.m_prognostic.len(),
                self.nmd + 1,
            )?;
        }
        let method_c_m_neighbors = self.method_c_m_neighbors()?;
        let mut selected = vec![false; self.nwd + 1];
        let mut anchors = Vec::new();
        let mut alignable_faces = vec![false; self.nwd + 1];

        // A deeper H-field level may touch the transition apron produced by
        // the previous pass. Only current-parent interior M points can seed a
        // legal Method-C perimeter; clipping that boundary row preserves the
        // valid demand instead of aborting the whole refinement.
        let mut parent_interior = vec![false; self.nmd + 1];
        for im in 2..=self.nmd {
            if use_cartesian_xy {
                let owner = self.m_prognostic[im];
                require_method_c_id("Method-C Cartesian h-field M owner", owner, self.nmd)?;
                if self.m_prognostic[owner] != owner {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Method-C Cartesian h-field M prognostic owner {owner} for point {im} is not canonical"
                        ),
                    ));
                }
                if owner != im {
                    continue;
                }
            }
            let mrlo = self.m_metadata[im].mrlm;
            if mrlo != pass {
                continue;
            }
            let neighbors = method_c_m_neighbors[im];
            let mut is_parent_interior = true;
            for &iu in neighbors.iu.iter().take(neighbors.npoly) {
                require_method_c_id("Method-C h-field eligibility U edge", iu, self.nud)?;
                if self.u_edges[iu].mrlu != mrlo {
                    is_parent_interior = false;
                    break;
                }
            }
            parent_interior[im] = is_parent_interior;
        }

        // Record every sampled point/edge demand separately. This prevents a
        // repair from preserving one face of a large component while silently
        // eroding the rest of the requested threshold footprint.
        let mut demand_at_m = vec![false; self.nmd + 1];
        let mut point_demand_at_m = vec![false; self.nmd + 1];
        for im in 2..=self.nmd {
            if !parent_interior[im]
                || !self.m_point_demands_pass(im, target_level, pass, use_cartesian_xy)
            {
                continue;
            }
            let neighbors = method_c_m_neighbors[im];
            let mut faces = Vec::new();
            for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                require_method_c_id("Method-C h-field point-demand W face", iw, self.nwd)?;
                if self.w_faces[iw].mrlw == pass {
                    faces.push(iw);
                }
            }
            if !faces.is_empty() {
                demand_at_m[im] = true;
                point_demand_at_m[im] = true;
                anchors.push((im, faces));
            }
        }
        for iu in 2..=self.nud {
            let edge = self.u_edges[iu];
            if edge.mrlu != pass
                || self.u_edge_midpoint_target_level(iu, target_level, use_cartesian_xy) < pass
            {
                continue;
            }
            let Some(anchor_im) = edge
                .im
                .iter()
                .copied()
                .find(|&im| im <= self.nmd && parent_interior[im])
            else {
                continue;
            };
            let mut faces = Vec::new();
            for &iw in edge.iw.iter().take(2) {
                require_method_c_id("Method-C h-field edge-demand W face", iw, self.nwd)?;
                if self.w_faces[iw].mrlw == pass {
                    faces.push(iw);
                }
            }
            if !faces.is_empty() {
                for &im in &edge.im {
                    if im <= self.nmd && parent_interior[im] {
                        demand_at_m[im] = true;
                    }
                }
                anchors.push((anchor_im, faces));
            }
        }

        // Nearby islands must share one Canonical phase because their rad3
        // footprints and transition aprons can meet. A six-edge support halo
        // joins only potentially interacting islands; keeping the traversal
        // local avoids carrying one phase around a pentagon or parent seam.
        let mut phase_support = demand_at_m.clone();
        if preserve_all_demands {
            for _ in 0..6 {
                let mut expanded = phase_support.clone();
                for im in 2..=self.nmd {
                    if !phase_support[im] {
                        continue;
                    }
                    let neighbors = method_c_m_neighbors[im];
                    for &iu in neighbors.iu.iter().take(neighbors.npoly) {
                        require_method_c_id("Method-C h-field phase-support U edge", iu, self.nud)?;
                        for &next in &self.u_edges[iu].im {
                            if next > 1 && next <= self.nmd && parent_interior[next] {
                                expanded[next] = true;
                            }
                        }
                    }
                }
                phase_support = expanded;
            }
        }

        // Method-C has one globally valid thirdm congruence class on the base
        // icosahedron. Anchoring separate pass-1 demand islands to arbitrary
        // local M points can shift that phase and create an invalid transition
        // even when every individual rad3 footprint is legal. Build the phase
        // membership once; components still select only their local demand.
        // cart_hex has no spherical pentagon; like the geometric Cartesian
        // path, its local stride phase is anchored directly in the demand.
        let use_global_canonical_phase = pass == 1 && !use_cartesian_xy;
        let mut canonical_phase = vec![false; self.nmd + 1];
        if use_global_canonical_phase {
            if let Some(global_start) = self.impent.iter().copied().find(|&im| im > 1) {
                let mut phase_done = vec![[false; 6]; self.nmd + 1];
                let mut stack = vec![global_start];
                canonical_phase[global_start] = true;
                while let Some(im) = stack.pop() {
                    for next in self.method_c_thirdm_neighbors_canonical_with_neighbors(
                        im,
                        &mut phase_done,
                        &method_c_m_neighbors,
                    )? {
                        if !canonical_phase[next] {
                            canonical_phase[next] = true;
                            stack.push(next);
                        }
                    }
                }
            }
        }

        // Reuse pass-wide indexed scratch. Fragmented fields can contain many
        // components; allocating/scanning nmd/nwd-sized buffers per island
        // made selection O(components * mesh size) before any mesh work.
        // The component root is a unique non-zero stamp, so touched entries
        // need neither clearing nor a second membership bitmap.
        let mut component_stamp = vec![0usize; self.nmd + 1];
        let mut seed_seen = vec![0usize; self.nmd + 1];
        let mut legal_seed = vec![0usize; self.nmd + 1];
        let mut lattice_neighbors = vec![Vec::new(); self.nmd + 1];
        let mut jdone = vec![[false; 6]; self.nmd + 1];
        let mut jdone_touched = Vec::new();
        let mut seed_demand_reachable = vec![0usize; self.nmd + 1];
        let mut selected_seeds = vec![0usize; self.nmd + 1];
        let mut footprint_index = vec![usize::MAX; self.nmd + 1];
        let mut owner = HashMap::new();
        let mut anchor_indices_by_m = vec![Vec::new(); self.nmd + 1];
        for (index, (im, _)) in anchors.iter().enumerate() {
            anchor_indices_by_m[*im].push(index);
        }

        for root in 2..=self.nmd {
            if component_stamp[root] != 0 || !phase_support[root] {
                continue;
            }
            let component_mrl = self.m_metadata[root].mrlm;
            let mut component = Vec::new();
            let mut queue = std::collections::VecDeque::from([root]);
            component_stamp[root] = root;
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
                        && component_stamp[next] == 0
                        && phase_support[next]
                        && self.m_metadata[next].mrlm == component_mrl
                    {
                        component_stamp[next] = root;
                        queue.push_back(next);
                    }
                }
            }
            if !component.iter().any(|&im| demand_at_m[im]) {
                continue;
            }
            let has_point_demand = component.iter().any(|&im| point_demand_at_m[im]);
            // Anchor the Canonical phase inside the demand, as the geometric
            // region path does. All demand islands in this parent then share
            // one phase without forcing a globe-spanning pentagon phase.
            let demand_start = component
                .iter()
                .copied()
                .filter(|&im| demand_at_m[im])
                .find(|im| self.impent.contains(im))
                .or_else(|| {
                    let demanded = component.iter().copied().filter(|&im| demand_at_m[im]);
                    if preserve_all_demands {
                        demanded.min()
                    } else {
                        demanded.max_by(|a, b| {
                            let a_level = self.m_point_or_edge_target_level(
                                *a,
                                &method_c_m_neighbors[*a],
                                target_level,
                                use_cartesian_xy,
                            );
                            let b_level = self.m_point_or_edge_target_level(
                                *b,
                                &method_c_m_neighbors[*b],
                                target_level,
                                use_cartesian_xy,
                            );
                            a_level.cmp(&b_level).then_with(|| b.cmp(a))
                        })
                    }
                })
                .expect("demanded parent component has an anchor");
            let start = if use_global_canonical_phase {
                let anchor = self.m_points[demand_start];
                component
                    .iter()
                    .copied()
                    .filter(|&im| canonical_phase[im])
                    .min_by(|&a, &b| {
                        let distance = |im: usize| {
                            let point = self.m_points[im];
                            (point.x - anchor.x).powi(2)
                                + (point.y - anchor.y).powi(2)
                                + (point.z - anchor.z).powi(2)
                        };
                        distance(a).total_cmp(&distance(b)).then_with(|| a.cmp(&b))
                    })
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "Method-C h-field pass {pass} demand component has no canonical stride-3 seed"
                            ),
                        )
                    })?
            } else if use_cartesian_xy {
                let (sum_x, sum_y, count) = component
                    .iter()
                    .copied()
                    .filter(|&im| demand_at_m[im])
                    .fold((0.0, 0.0, 0usize), |(sum_x, sum_y, count), im| {
                        (
                            sum_x + self.m_points[im].x,
                            sum_y + self.m_points[im].y,
                            count + 1,
                        )
                    });
                let centroid = CartesianPoint::new(sum_x / count as f64, sum_y / count as f64, 0.0);
                let mut candidates = component.clone();
                candidates.sort_by(|&a, &b| {
                    let distance = |im: usize| {
                        let point = self.m_points[im];
                        (point.x - centroid.x).powi(2) + (point.y - centroid.y).powi(2)
                    };
                    distance(a).total_cmp(&distance(b)).then_with(|| a.cmp(&b))
                });
                let mut legal_start = None;
                for im in candidates {
                    let MethodCHfieldRad3Footprint::Materializable(footprint) =
                        self.hfield_rad3_footprint(im, &method_c_m_neighbors, use_cartesian_xy)?
                    else {
                        continue;
                    };
                    if footprint.iter().any(|&iw| iw >= 2)
                        && footprint
                            .iter()
                            .copied()
                            .filter(|&iw| iw >= 2)
                            .all(|iw| iw <= self.nwd && self.w_faces[iw].mrlw == component_mrl)
                    {
                        legal_start = Some(im);
                        break;
                    }
                }
                legal_start.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Method-C Cartesian h-field pass {pass} demand component has no legal local stride-3 seed"
                        ),
                    )
                })?
            } else {
                demand_start
            };
            let mut lattice_seeds = Vec::new();
            let mut lista = vec![start];
            seed_seen[start] = root;
            while let Some(im) = lista.pop() {
                // cart_hex's outer seam contains valid traversal points whose
                // periodic face representation cannot materialize a complete
                // rad3 footprint. Skip only that explicitly classified case;
                // malformed topology and every other rad3 error remain fatal.
                let footprint = match self.hfield_rad3_footprint(
                    im,
                    &method_c_m_neighbors,
                    use_cartesian_xy,
                )? {
                    MethodCHfieldRad3Footprint::Materializable(footprint) => footprint,
                    MethodCHfieldRad3Footprint::PeriodicSeam => Vec::new(),
                };
                let footprint_is_legal = footprint.iter().any(|&iw| iw >= 2)
                    && !footprint
                        .iter()
                        .copied()
                        .filter(|&iw| iw >= 2)
                        .any(|iw| iw > self.nwd || self.w_faces[iw].mrlw != component_mrl);
                if footprint_is_legal {
                    for &iw in &footprint {
                        if iw >= 2 && iw <= self.nwd && self.w_faces[iw].mrlw == component_mrl {
                            alignable_faces[iw] = true;
                        }
                    }
                    legal_seed[im] = root;
                    lattice_seeds.push((im, footprint));
                }
                // An atomic footprint can straddle the parent transition
                // boundary even though legal seeds exist one stride farther
                // inside. Keep traversing the Canonical lattice through that
                // non-materializable seed; otherwise the boundary seed cuts
                // off the entire legal interior and valid deeper demand is
                // reported as uncovered.
                jdone_touched.push(im);
                for neighbor in self.method_c_thirdm_neighbors_canonical_with_neighbors(
                    im,
                    &mut jdone,
                    &method_c_m_neighbors,
                )? {
                    jdone_touched.push(neighbor);
                    let traversed_count = jdone[neighbor].iter().filter(|&&done| done).count();
                    if component_stamp[neighbor] != root
                        || self.m_metadata[neighbor].mrlm != component_mrl
                    {
                        continue;
                    }
                    lattice_neighbors[im].push(neighbor);
                    lattice_neighbors[neighbor].push(im);
                    let follows_intermediate_demand = preserve_all_demands
                        || point_demand_at_m[neighbor]
                        || (!has_point_demand && demand_at_m[neighbor]);
                    if traversed_count < 2
                        && seed_seen[neighbor] != root
                        && follows_intermediate_demand
                    {
                        seed_seen[neighbor] = root;
                        lista.push(neighbor);
                    }
                }
            }
            for im in jdone_touched.drain(..) {
                jdone[im] = [false; 6];
            }
            for (im, _) in &lattice_seeds {
                let neighbors = &mut lattice_neighbors[*im];
                neighbors.retain(|&neighbor| legal_seed[neighbor] == root);
                neighbors.sort_unstable();
                neighbors.dedup();
            }
            if legal_seed[start] == root {
                let mut queue = std::collections::VecDeque::from([start]);
                seed_demand_reachable[start] = root;
                while let Some(im) = queue.pop_front() {
                    for &next in &lattice_neighbors[im] {
                        let follows_demand = if has_point_demand {
                            point_demand_at_m[next]
                        } else {
                            demand_at_m[next]
                        };
                        if follows_demand && seed_demand_reachable[next] != root {
                            seed_demand_reachable[next] = root;
                            queue.push_back(next);
                        }
                    }
                }
            }

            // Assign each parent face to its nearest seed. Selecting the owner
            // of each demand anchor applies one aligned rad3 footprint instead
            // of dilating the demand once while finding seeds and again while
            // materializing their footprints.
            owner.clear();
            for (im, footprint) in &lattice_seeds {
                let seed = self.m_points[*im];
                for &iw in footprint {
                    if iw < 2 || iw > self.nwd || self.w_faces[iw].mrlw != component_mrl {
                        continue;
                    }
                    let face = self.w_faces[iw];
                    let center = CartesianPoint::new(
                        (self.m_points[face.im[0]].x
                            + self.m_points[face.im[1]].x
                            + self.m_points[face.im[2]].x)
                            / 3.0,
                        (self.m_points[face.im[0]].y
                            + self.m_points[face.im[1]].y
                            + self.m_points[face.im[2]].y)
                            / 3.0,
                        (self.m_points[face.im[0]].z
                            + self.m_points[face.im[1]].z
                            + self.m_points[face.im[2]].z)
                            / 3.0,
                    );
                    let distance = (seed.x - center.x).powi(2)
                        + (seed.y - center.y).powi(2)
                        + (seed.z - center.z).powi(2);
                    let (current_owner, current_distance) =
                        owner.get(&iw).copied().unwrap_or((0, f64::INFINITY));
                    if distance < current_distance
                        || (distance == current_distance && *im < current_owner)
                    {
                        owner.insert(iw, (*im, distance));
                    }
                }
            }

            for (index, (im, _)) in lattice_seeds.iter().enumerate() {
                footprint_index[*im] = index;
            }
            for (im, _) in &lattice_seeds {
                if seed_demand_reachable[*im] == root {
                    selected_seeds[*im] = root;
                }
            }
            selected_seeds[start] = root;
            for (im, footprint) in &lattice_seeds {
                if selected_seeds[*im] == root {
                    for &iw in footprint {
                        if iw >= 2 && iw <= self.nwd && self.w_faces[iw].mrlw == component_mrl {
                            selected[iw] = true;
                        }
                    }
                }
            }
            // Vertex sampling can miss a thin edge-only tail. Add exactly one
            // nearest aligned owner only when an individual demand anchor is
            // still uncovered by the center-selected footprints.
            let mut component_anchor_indices = component
                .iter()
                .flat_map(|&im| anchor_indices_by_m[im].iter().copied())
                .collect::<Vec<_>>();
            component_anchor_indices.sort_unstable();
            for anchor_index in component_anchor_indices {
                let (anchor_im, faces) = &anchors[anchor_index];
                if faces.iter().any(|&iw| selected[iw]) {
                    continue;
                }
                let anchor = self.m_points[*anchor_im];
                let mut best = None;
                for &iw in faces {
                    let im = owner.get(&iw).map(|&(im, _)| im).unwrap_or(0);
                    if im <= 1 {
                        continue;
                    }
                    let seed = self.m_points[im];
                    let distance = (seed.x - anchor.x).powi(2)
                        + (seed.y - anchor.y).powi(2)
                        + (seed.z - anchor.z).powi(2);
                    if best.is_none_or(|(best_distance, best_im)| {
                        distance < best_distance || (distance == best_distance && im < best_im)
                    }) {
                        best = Some((distance, im));
                    }
                }
                if let Some((_, im)) = best {
                    if selected_seeds[im] != root {
                        selected_seeds[im] = root;
                        let index = footprint_index[im];
                        if index != usize::MAX {
                            for &iw in &lattice_seeds[index].1 {
                                if iw >= 2
                                    && iw <= self.nwd
                                    && self.w_faces[iw].mrlw == component_mrl
                                {
                                    selected[iw] = true;
                                }
                            }
                        }
                    }
                }
            }
            if preserve_all_demands {
                loop {
                    let mut bridges = Vec::new();
                    for (mid, _) in &lattice_seeds {
                        if selected_seeds[*mid] == root {
                            continue;
                        }
                        let selected_neighbors = lattice_neighbors[*mid]
                            .iter()
                            .copied()
                            .filter(|&im| selected_seeds[im] == root)
                            .collect::<Vec<_>>();
                        'pairs: for a_index in 0..selected_neighbors.len() {
                            for b_index in (a_index + 1)..selected_neighbors.len() {
                                let a = selected_neighbors[a_index];
                                let b = selected_neighbors[b_index];
                                if lattice_neighbors[a].binary_search(&b).is_ok() {
                                    continue;
                                }
                                let common = lattice_neighbors[a]
                                    .iter()
                                    .copied()
                                    .filter(|candidate| {
                                        lattice_neighbors[b].binary_search(candidate).is_ok()
                                    })
                                    .collect::<Vec<_>>();
                                if common.as_slice() == [*mid] {
                                    bridges.push(*mid);
                                    break 'pairs;
                                }
                            }
                        }
                    }
                    if bridges.is_empty() {
                        break;
                    }
                    for im in bridges {
                        selected_seeds[im] = root;
                    }
                }
            }
            for (im, footprint) in lattice_seeds {
                if selected_seeds[im] == root {
                    for iw in footprint {
                        if iw >= 2 && iw <= self.nwd && self.w_faces[iw].mrlw == component_mrl {
                            selected[iw] = true;
                        }
                    }
                }
            }
        }
        // The previous pass's transition apron can contain current-generation
        // M points while still being too close to the parent seam for any
        // complete rad3 footprint. Those samples are not legal Method-C
        // anchors: clip them based on the existence of an atomic aligned
        // footprint, rather than letting a partial footprint cross the seam or
        // failing an otherwise valid deeper interior pass.
        let requested_anchor_count = anchors.len();
        let anchors_before_clip = anchors.clone();
        anchors.retain(|(_, faces)| {
            faces
                .iter()
                .any(|&iw| alignable_faces.get(iw).copied().unwrap_or(false))
        });
        let clipped_anchor_count = requested_anchor_count - anchors.len();
        // Not every clipped anchor is a loss. Clipping the apron row is
        // deliberate, and an anchor there still gets refined when its own
        // demand component hosts a legal footprint somewhere. What is a real
        // loss is a whole demand component with no alignable face at all: it is
        // narrower than one rad3 footprint, so nothing in it can ever be
        // materialized at this generation. Judging by the clipped *fraction*
        // instead only correlates with that; this measures it.
        // Ask the direct question: of the faces the field demanded at this
        // level, how many actually end up refined? A component-level test is
        // too permissive — a coastline strip is one big connected component, so
        // a handful of legal seeds inside it satisfies "has an alignable face"
        // while the rest of the strip is still dropped. Measuring unmet demand
        // catches that; measuring the clipped anchor fraction only correlates
        // with it.
        let (demanded_faces, unmet_faces) =
            self.hfield_unmet_demand(&anchors_before_clip, &selected)?;
        if unmet_faces * 2 > demanded_faces {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Method-C h-field pass {pass} can refine only {} of {demanded_faces} demanded faces; \
                     {unmet_faces} lie in demand narrower than one rad3 footprint and would be dropped \
                     silently. Raise NXP so a footprint fits, coarsen the h-field raster \
                     (hfield_nlon/nlat) so sub-footprint speckle is not resolved, or lower the \
                     requested level.",
                    demanded_faces - unmet_faces
                ),
            ));
        }
        let coverage = MethodCHfieldDemandCoverage {
            anchors,
            requested_anchor_count,
            demanded_face_count: demanded_faces,
            unmet_face_count: unmet_faces,
            clipped_anchor_count,
        };
        coverage.validate(&selected)?;
        Ok((selected, coverage))
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
    ) -> io::Result<(Self, usize, MethodCHfieldSpawnDiagnostics)> {
        self.validate_topology()?;
        let mut diagnostics = MethodCHfieldSpawnDiagnostics::default();
        if max_level == 0 {
            return Ok((self.clone(), 0, diagnostics));
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
            let has_deeper_demand = pass < max_level
                && mesh.hfield_has_demand_at_or_above(target_level, pass + 1, use_cartesian_xy)?;
            let (selected_faces, coverage) = mesh
                .selected_faces_and_coverage_from_target_levels_with_policy(
                    target_level,
                    pass,
                    use_cartesian_xy,
                    !has_deeper_demand,
                )?;
            diagnostics.requested_anchor_count += coverage.requested_anchor_count();
            diagnostics.covered_anchor_count += coverage.covered_anchor_count();
            diagnostics.boundary_clipped_anchor_count += coverage.clipped_anchor_count();
            diagnostics.demanded_face_count += coverage.demanded_face_count();
            diagnostics.unmet_face_count += coverage.unmet_face_count();
            if selected_faces.iter().skip(2).all(|selected| !*selected) {
                if mesh.hfield_has_current_parent_demand(target_level, pass, use_cartesian_xy)? {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Method-C h-field pass {pass} demand is entirely on the parent transition boundary"
                        ),
                    ));
                }
                if has_deeper_demand {
                    continue;
                }
                break;
            }

            mesh = mesh
                .spawn_nest_pass_method_c_preserving_demands(
                    &selected_faces,
                    grid_number,
                    max_mrows,
                    true,
                    &coverage,
                )
                .map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!("Method-C h-field spawn_nest pass {pass} failed: {error}"),
                    )
                })?;

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

        Ok((mesh, spring_passes, diagnostics))
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
        .map(|(mesh, _, _)| mesh)
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
    ) -> io::Result<(Self, usize, MethodCHfieldSpawnDiagnostics)> {
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
    ) -> io::Result<(Self, usize, MethodCHfieldSpawnDiagnostics)> {
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

/// Demanded faces versus those the selection actually refines.
impl MethodCMesh {
    /// `(demanded, unmet)` face counts for one pass.
    ///
    /// Demand narrower than a rad3 footprint cannot be selected: only the spots
    /// where a footprint happens to fit get refined, and the rest of the region
    /// silently disappears. Comparing the two counts states that directly,
    /// rather than inferring it from how many anchors were clipped.
    fn hfield_unmet_demand(
        &self,
        anchors: &[(usize, Vec<usize>)],
        selected: &[bool],
    ) -> io::Result<(usize, usize)> {
        let mut demanded = vec![false; self.nwd + 1];
        for (_, faces) in anchors {
            for &iw in faces {
                if iw >= 2 && iw <= self.nwd {
                    demanded[iw] = true;
                }
            }
        }
        let demanded_faces = demanded.iter().filter(|d| **d).count();
        let unmet_faces = (2..=self.nwd)
            .filter(|&iw| demanded[iw] && !selected.get(iw).copied().unwrap_or(false))
            .count();
        Ok((demanded_faces, unmet_faces))
    }
}
