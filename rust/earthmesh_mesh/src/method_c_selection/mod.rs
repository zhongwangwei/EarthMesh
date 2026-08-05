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

    /// Faces selected for `pass`, across regions that do not touch each other.
    ///
    /// Selection marches a stride-3 lattice outward from one start and keeps the
    /// neighbours the regions contain, so one march covers one connected piece
    /// of demand and no more. Handed every region at once it refined whichever
    /// piece its start happened to fall in and left the rest alone -- and said
    /// nothing, because the run still succeeded: a global coastal case emitted
    /// 114566 circles and grew the mesh by 2.2%.
    ///
    /// So each connected group of regions is marched on its own -- its own
    /// start, its own lattice phase, its own parent-boundary validation -- and
    /// the masks are combined. Everything downstream already works on a mask
    /// that holds several blocks: `method_c_perimeters_from_selected_faces`
    /// returns one perimeter per block, and the emit that follows renumbers the
    /// whole mesh once regardless of how many blocks it carries.
    ///
    /// Combining is what makes it affordable. Marching a group costs about a
    /// third of a millisecond and its perimeter another two fifths; the emit
    /// costs 74 ms on a 131k-face mesh, and is 99.5% of a single-group pass.
    /// Refining 23106 groups one call each is half an hour of rebuilding the
    /// same mesh over and over. Marching them all and emitting once is seconds.
    ///
    /// A group that cannot be marched -- a pentagon it would leave with the
    /// wrong degree, a rim it would cross -- is left out of the mask rather than
    /// failing the pass, and the count comes back so the caller can say how much
    /// of the demand went unserved.
    pub(crate) fn selected_regions_faces_over_groups(
        &self,
        regions: &[MethodCRefinementRegion],
        pass: usize,
        use_cartesian_xy: bool,
    ) -> io::Result<Vec<bool>> {
        let groups = method_c_connected_region_groups(regions, use_cartesian_xy);
        if groups.len() <= 1 {
            return self.selected_regions_faces(regions, pass, use_cartesian_xy);
        }
        let mut combined = vec![false; self.nwd + 1];
        let mut served = 0usize;
        let mut first_error = None;
        for group in &groups {
            match self.selected_regions_faces(group, pass, use_cartesian_xy) {
                Ok(mask) => {
                    served += 1;
                    for (slot, selected) in combined.iter_mut().zip(mask) {
                        *slot |= selected;
                    }
                }
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }
        if served == 0 {
            return Err(first_error.unwrap_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Method-C selected no region group for this pass",
                )
            }));
        }
        if served < groups.len() {
            eprintln!(
                "Method-C selection: {} of {} region groups could not be marched at pass {pass}",
                groups.len() - served,
                groups.len()
            );
        }
        Ok(combined)
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
        self.stand_selected_faces_off_finer_ground(&mut selected);
        Ok(selected)
    }

    /// Pull the mask back from ground already refined in this pass.
    ///
    /// A block's machinery consumes faces beyond its own mask: the transition
    /// patch subdivides the suppressed face across each perimeter triple and
    /// rewrites eight faces around it, all just outside the mask. When groups
    /// refine one after another, a later mask flush against an earlier block
    /// hands the patch a face that is already finer, and the whole group is
    /// refused -- chased one ring at a time on a real global run: first the
    /// walk's own edges, then the concavity fill, then the patch, each failure
    /// two rings further out. Two rings of standoff covers the patch's reach in
    /// one place instead.
    ///
    /// The strip given up lies beside a block that is already refined, so the
    /// demand there is served -- by the neighbour.
    fn stand_selected_faces_off_finer_ground(&self, selected: &mut [bool]) {
        // Three rings, not two. The patch consumes two rings of faces beyond
        // the mask, so two rings of face-standoff keeps the *faces* clear --
        // but two faces one ring apart share a vertex, and a transition row
        // adds edges at its vertices. Two passes' rows meeting at one shared
        // vertex push its valence past seven, and the rebuild after emit
        // refuses the whole mesh. Vertex reach is one ring beyond face reach.
        const STANDOFF_RINGS: usize = 3;
        let Some(mask_mrlw) = (2..=self.nwd)
            .find(|&iw| selected[iw])
            .map(|iw| self.w_faces[iw].mrlw)
        else {
            return;
        };
        // The ground an earlier block occupies is more than its subdivided
        // children. Its transition apron keeps the parent generation's mrlw --
        // indistinguishable from ground this mask may take by generation alone
        // -- but its edges are rewired and its vertices carry the 5/7 valences
        // of a transition row, and a later perimeter walk that steps into it
        // goes in circles. The apron is stamped with the pass's grid number
        // (`ngr = child_level` in the patch), so "same generation but a newer
        // grid number than this mask's own ground" identifies it exactly.
        let mask_ngr = (2..=self.nwd)
            .filter(|&iw| selected[iw])
            .map(|iw| self.w_faces[iw].ngr)
            .min()
            .unwrap_or(1);
        let mut near = vec![false; self.nwd + 1];
        let mut frontier: Vec<usize> = (2..=self.nwd)
            .filter(|&iw| {
                let face = &self.w_faces[iw];
                face.mrlw > mask_mrlw || (face.mrlw == mask_mrlw && face.ngr > mask_ngr)
            })
            .collect();
        if frontier.is_empty() {
            return;
        }
        for &iw in &frontier {
            near[iw] = true;
        }
        for _ in 0..STANDOFF_RINGS {
            let mut next = Vec::new();
            for &iw in &frontier {
                for &iu in self.w_faces[iw].iu.iter() {
                    if iu < 2 || iu > self.nud {
                        continue;
                    }
                    for &jw in self.u_edges[iu].iw.iter().take(2) {
                        if (2..=self.nwd).contains(&jw) && !near[jw] {
                            near[jw] = true;
                            next.push(jw);
                        }
                    }
                }
            }
            frontier = next;
        }
        for iw in 2..=self.nwd {
            if selected[iw] && near[iw] {
                selected[iw] = false;
            }
        }
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
            // Two different situations meet the same check here, and they must
            // not share a verdict. An edge *coarser* than this walk's
            // generation means the nest is stepping off its parent -- data
            // loss, and rightly fatal. An edge *finer* means a block refined
            // earlier in this same pass sits next door: the demand there is
            // already served, so the point is simply not seeded and the walk
            // goes on. Treating that as fatal refused whole continental
            // coastlines for touching a neighbour -- measured on a global run:
            // 10 of 43 groups refused, holding 98% of the circles.
            let mut borders_already_refined = false;
            let mut crosses_parent = false;
            for &iu in neighbors.iu.iter().take(neighbors.npoly) {
                require_method_c_id("Method-C refinement boundary U edge", iu, self.nud)?;
                let edge_generation = self.u_edges[iu].mrlu;
                if edge_generation < mrlo {
                    crosses_parent = true;
                } else if edge_generation > mrlo {
                    borders_already_refined = true;
                }
            }
            if crosses_parent {
                return Err(method_c_repairable_error(
                    MethodCRepairableKind::NonTripletPerimeter,
                    Some(im),
                    format!(
                        "Method-C perimeter length invalid: Current nested grid crosses the parent boundary / next coarser grid boundary at M point {im}"
                    ),
                ));
            }
            if borders_already_refined {
                continue;
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

/// Group regions by whether they touch, so each group is one connected march.
///
/// Circles overlap when their centres are closer than the sum of their radii.
/// Latitude bands keep the pairing affordable without ever losing a pair: two
/// circles that touch differ in latitude by at most that sum, so they share a
/// band or sit in adjacent ones. Longitude is not bucketed -- a bucket wide
/// enough at the equator spans most of the globe near a pole, and the seam and
/// the poles are exactly where a bucketed search drops pairs.
pub fn method_c_connected_region_groups(
    regions: &[MethodCRefinementRegion],
    use_cartesian_xy: bool,
) -> Vec<Vec<MethodCRefinementRegion>> {
    if use_cartesian_xy || regions.len() < 2 {
        return vec![regions.to_vec()];
    }
    let circles: Vec<Option<(f64, f64, f64)>> = regions
        .iter()
        .map(|region| match region {
            MethodCRefinementRegion::Circle {
                center,
                radius_meters,
                ..
            } => Some((center.lon_degrees, center.lat_degrees, *radius_meters)),
            _ => None,
        })
        .collect();
    // Anything that is not a circle keeps the old whole-set behaviour: a corridor
    // or closed curve is one region already, and pairing it by centre would be
    // wrong.
    if circles.iter().any(Option::is_none) {
        return vec![regions.to_vec()];
    }

    let mut parent: Vec<usize> = (0..regions.len()).collect();
    fn find(parent: &mut [usize], mut index: usize) -> usize {
        while parent[index] != index {
            parent[index] = parent[parent[index]];
            index = parent[index];
        }
        index
    }
    let max_radius = circles
        .iter()
        .flatten()
        .map(|(_, _, radius)| *radius)
        .fold(0.0_f64, f64::max);
    let meters_per_degree = std::f64::consts::PI * EARTH_RADIUS_METERS_METHOD_C / 180.0;
    let band_degrees = (2.0 * max_radius / meters_per_degree).max(1e-6);
    let mut bands: std::collections::HashMap<i64, Vec<usize>> = Default::default();
    for (index, circle) in circles.iter().enumerate() {
        let Some((_, lat, _)) = circle else { continue };
        bands
            .entry((lat / band_degrees).floor() as i64)
            .or_default()
            .push(index);
    }
    let band_keys: Vec<i64> = bands.keys().copied().collect();
    for band_key in band_keys {
        let here = bands.get(&band_key).cloned().unwrap_or_default();
        let below = bands.get(&(band_key + 1)).cloned().unwrap_or_default();
        for (position, &left) in here.iter().enumerate() {
            let Some((left_lon, left_lat, left_radius)) = circles[left] else {
                continue;
            };
            for &right in here[position + 1..].iter().chain(below.iter()) {
                let Some((right_lon, right_lat, right_radius)) = circles[right] else {
                    continue;
                };
                if method_c_great_circle_meters(left_lon, left_lat, right_lon, right_lat)
                    <= left_radius + right_radius
                {
                    let (a, b) = (find(&mut parent, left), find(&mut parent, right));
                    if a != b {
                        parent[a] = b;
                    }
                }
            }
        }
    }
    let mut groups: std::collections::BTreeMap<usize, Vec<MethodCRefinementRegion>> =
        Default::default();
    for (index, region) in regions.iter().enumerate() {
        groups
            .entry(find(&mut parent, index))
            .or_default()
            .push(region.clone());
    }
    groups.into_values().collect()
}

const EARTH_RADIUS_METERS_METHOD_C: f64 = 6_371_229.0;

fn method_c_great_circle_meters(lon_a: f64, lat_a: f64, lon_b: f64, lat_b: f64) -> f64 {
    let (a_lat, b_lat) = (lat_a.to_radians(), lat_b.to_radians());
    let delta = (lon_b - lon_a).to_radians();
    let cosine = a_lat.sin() * b_lat.sin() + a_lat.cos() * b_lat.cos() * delta.cos();
    cosine.clamp(-1.0, 1.0).acos() * EARTH_RADIUS_METERS_METHOD_C
}
