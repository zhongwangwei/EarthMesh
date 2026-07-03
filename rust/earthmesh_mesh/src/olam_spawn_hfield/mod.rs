use std::io;

use super::*;

/// Method-C spawning driven by a quantized target-level field instead of
/// geometric regions ("M4" of the h-field integration).
///
/// The selection seam is the same `Vec<bool>` over Fortran-indexed W faces
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
impl OlamDelaunayMesh {
    /// True when the M point's sampled target level demands refinement at
    /// `pass` or deeper.
    fn m_point_demands_pass<F: Fn(f64, f64) -> u8>(
        &self,
        im: usize,
        target_level: &F,
        pass: usize,
    ) -> bool {
        let lonlat = xyz_to_lonlat_degrees(self.m_points[im]);
        usize::from(target_level(lonlat.lon_degrees, lonlat.lat_degrees)) >= pass
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
    ) -> io::Result<Vec<bool>> {
        require_olam_len("m_points", self.m_points.len(), self.nmd + 1)?;
        require_olam_len("w_faces", self.w_faces.len(), self.nwd + 1)?;
        let method_c_m_neighbors = self.method_c_m_neighbors()?;
        let mut selected = vec![false; self.nwd + 1];

        // Deterministic anchor: deepest demanded level, lowest M id on ties.
        // Pentagon points are skipped as anchors (the thirdm stride walk
        // assumes 6-edge interior points), matching the region start logic;
        // they can still join the selection as BFS neighbors like in the
        // region path.
        let mut start: Option<(usize, usize)> = None; // (level, im), max level then min im
        for im in 2..=self.nmd {
            if self.impent.contains(&im) {
                continue;
            }
            let lonlat = xyz_to_lonlat_degrees(self.m_points[im]);
            let level = usize::from(target_level(lonlat.lon_degrees, lonlat.lat_degrees));
            if level >= pass {
                let better = match start {
                    None => true,
                    Some((best_level, best_im)) => {
                        level > best_level || (level == best_level && im < best_im)
                    }
                };
                if better {
                    start = Some((level, im));
                }
            }
        }
        let Some((_, anchor_im)) = start else {
            return Ok(selected);
        };
        // Fortran start anchoring (mirrors
        // `olam_refinement_start_point_for_regions_with_neighbors`): the
        // thirdm walk is a stride-3 lattice march, and whenever a pentagon
        // lies inside the demanded set the walk MUST start from it — that
        // pins the seed sublattice to the icosahedral frame, which is what
        // keeps the selection boundary multiple-of-3 aligned for the
        // perimeter walker. Without a demanded pentagon, fall back to the
        // deepest-demand point. (The region path's extra "pentagon merely
        // nearby -> march from it" refinement is not replicated yet; the
        // annealing repair covers that rarer mis-anchoring, and the legality
        // error remains loud if it cannot.)
        let mut start = anchor_im;
        for &pentagon_id in &self.impent {
            if pentagon_id <= 1 || pentagon_id > self.nmd {
                continue;
            }
            if self.m_point_demands_pass(pentagon_id, target_level, pass) {
                start = pentagon_id;
                break;
            }
        }
        let mrlo = self.m_metadata[start].mrlm;

        // Seed growth: verbatim the region path's BFS over thirdm strides,
        // including the parent-boundary legality check on every popped point.
        let mut seeds = std::collections::BTreeSet::new();
        let mut jdone = vec![[false; 6]; self.nmd + 1];
        let mut lista = vec![start];
        while let Some(im) = lista.pop() {
            let neighbors = method_c_m_neighbors[im];
            for &iu in neighbors.iu.iter().take(neighbors.npoly) {
                require_olam_id("OLAM refinement boundary U edge", iu, self.nud)?;
                if self.u_edges[iu].mrlu != mrlo {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Method-C perimeter length invalid: h-field level set crosses the parent boundary / next coarser grid boundary at M point {im}"
                        ),
                    ));
                }
            }
            seeds.insert(im);

            for neighbor in self.olam_thirdm_neighbors_fortran_with_neighbors(
                im,
                &mut jdone,
                &method_c_m_neighbors,
            )? {
                let traversed_count = jdone[neighbor].iter().filter(|&&done| done).count();
                if traversed_count < 2 && self.m_point_demands_pass(neighbor, target_level, pass) {
                    lista.push(neighbor);
                }
            }
        }

        // rad3 footprints filtered to the seed generation, like the region path.
        for im in seeds {
            let mrl_seed = self.m_metadata[im].mrlm;
            let mut footprint = vec![false; self.nwd + 1];
            self.mark_fill_rad3_faces_with_neighbors(im, &mut footprint, &method_c_m_neighbors)?;
            for iw in 2..=self.nwd {
                if footprint[iw] && self.w_faces[iw].mrlw == mrl_seed {
                    selected[iw] = true;
                }
            }
        }
        Ok(selected)
    }

    pub(crate) fn spawn_nest_from_target_levels_internal<F: Fn(f64, f64) -> u8>(
        &self,
        target_level: &F,
        max_level: usize,
        max_mrows: usize,
        spring: Option<(usize, usize)>,
    ) -> io::Result<(Self, usize)> {
        self.validate_topology()?;
        if max_level == 0 {
            return Ok((self.clone(), 0));
        }
        if max_mrows == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM spawn_nest max_mrows must be greater than zero",
            ));
        }
        if max_level > 5 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("OLAM refinement max_level {max_level} must be in 1..=5"),
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

        for (grid_number, pass) in (first_grid_number..).zip(1..=max_level) {
            let selected_faces = mesh.selected_faces_from_target_levels(target_level, pass)?;
            if selected_faces.iter().skip(2).all(|selected| !*selected) {
                // The field demands nothing at this depth; deeper level sets
                // are subsets, so descending further cannot select anything.
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
                            format!("OLAM h-field spawn_nest pass {pass} failed: {error}"),
                        ));
                    }
                },
            }

            if let Some((nxp, niter)) = spring {
                if niter > 0 {
                    mesh = mesh.spring_nest_with_radius_projection(
                        nxp,
                        niter,
                        grid_number,
                        false,
                        true,
                        None,
                    )?;
                    spring_passes += 1;
                }
            }
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
        self.spawn_nest_from_target_levels_internal(&target_level, max_level, max_mrows, None)
            .map(|(mesh, _)| mesh)
    }

    /// Same as [`Self::spawn_nest_from_target_levels`], with the legacy
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
            Some((nxp, niter)),
        )
    }
}
