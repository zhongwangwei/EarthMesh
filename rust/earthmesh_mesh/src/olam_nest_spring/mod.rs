use super::*;

impl OlamDelaunayMesh {
    /// Apply the core OLAM `spring_dynamics_nest` relaxation to a refined nest.
    ///
    /// With `move_interior=false` this mirrors OLAM's atmospheric nest call:
    /// only M points adjacent to transition-row faces with nonzero `mrow` move. With
    /// `move_interior=true`, M points adjacent to faces on `ngr` are also moved.
    pub fn spring_nest(
        &self,
        nxp: usize,
        niter: usize,
        ngr: usize,
        move_interior: bool,
    ) -> io::Result<Self> {
        self.spring_nest_with_radius_projection(nxp, niter, ngr, move_interior, true, None)
    }

    pub(crate) fn spring_nest_with_radius_projection(
        &self,
        nxp: usize,
        niter: usize,
        ngr: usize,
        move_interior: bool,
        project_to_radius: bool,
        dist00_override: Option<f64>,
    ) -> io::Result<Self> {
        if nxp == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM nest spring requires positive NXP",
            ));
        }
        if ngr <= 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM nest spring NGR must be greater than one",
            ));
        }

        self.validate_topology()?;
        if niter == 0 {
            return Ok(self.clone());
        }

        let movable_m_points = olam_nest_movable_m_points(self, ngr, move_interior)?;
        if movable_m_points.iter().skip(2).all(|movable| !*movable) {
            return Ok(self.clone());
        }

        let radius = active_mesh_radius(self)?;
        let topology =
            icosahedron_spring_topology_fortran(self.nmd, &self.u_edges, &self.m_neighbors, 0.035)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "failed to build OLAM nest spring topology",
                    )
                })?;
        let dist00 = dist00_override.unwrap_or(olam_fortran_global_dist00(1.0, radius, nxp));
        // Loop-invariant masks/targets + reusable buffers, hoisted out of the
        // per-iteration hot path (bit-identical; see OlamNestSpringScratch).
        let mut scratch = OlamNestSpringScratch::new(
            self,
            &topology,
            &movable_m_points,
            dist00,
            project_to_radius,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "failed to prepare OLAM nest spring buffers",
            )
        })?;
        // Double buffering: unmovable/dummy slots are never written by the
        // iteration, so both buffers keep their initial positions there
        // forever, exactly like the historical clone-per-iteration version.
        let mut m_points = self.m_points.clone();
        let mut next_m_points = self.m_points.clone();

        for iteration in 1..=niter {
            if (iteration == 1 || iteration == niter || iteration % 100 == 0)
                && !earthmesh_core::progress::report("olam-nest-spring", iteration, niter)
            {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "OLAM nest spring was cancelled",
                ));
            }
            olam_nest_spring_iteration_into(
                &m_points,
                &topology,
                &movable_m_points,
                &mut scratch,
                &mut next_m_points,
            )
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "failed to run OLAM nest spring iteration",
                )
            })?;
            std::mem::swap(&mut m_points, &mut next_m_points);
        }

        for point in m_points.iter_mut().skip(2) {
            point.x = point.x as f32 as f64;
            point.y = point.y as f32 as f64;
            point.z = point.z as f32 as f64;
        }

        let adjusted = Self {
            nmd: self.nmd,
            nud: self.nud,
            nwd: self.nwd,
            impent: self.impent,
            m_points,
            m_metadata: self.m_metadata.clone(),
            u_edges: self.u_edges.clone(),
            w_faces: self.w_faces.clone(),
            m_neighbors: self.m_neighbors.clone(),
            m_prognostic: self.m_prognostic.clone(),
            u_prognostic: self.u_prognostic.clone(),
            w_prognostic: self.w_prognostic.clone(),
            boundary_rows: self.boundary_rows.clone(),
        };
        adjusted.validate_topology()?;
        Ok(adjusted)
    }

    /// H-field variant of [`Self::spring_nest`]: per-edge target lengths come
    /// from the caller (typically sampled from an `earthmesh_hfield` cell-width
    /// field via [`olam_edge_target_lengths_from_field`]) instead of the
    /// level/mrow-derived spacing. Movable-point selection, the Jacobi
    /// iteration structure, the trailing default-real rounding, and topology
    /// validation are identical to the legacy path, which stays untouched as
    /// the compat default.
    pub fn spring_nest_with_edge_targets(
        &self,
        niter: usize,
        ngr: usize,
        move_interior: bool,
        project_to_radius: bool,
        edge_targets_m: &[f64],
    ) -> io::Result<Self> {
        if ngr <= 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM nest spring NGR must be greater than one",
            ));
        }
        if edge_targets_m.len() < self.nud + 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "edge target lengths ({}) must cover Fortran U ids 0..={}",
                    edge_targets_m.len(),
                    self.nud
                ),
            ));
        }

        self.validate_topology()?;
        if niter == 0 {
            return Ok(self.clone());
        }

        let movable_m_points = olam_nest_movable_m_points(self, ngr, move_interior)?;
        if movable_m_points.iter().skip(2).all(|movable| !*movable) {
            return Ok(self.clone());
        }

        let topology =
            icosahedron_spring_topology_fortran(self.nmd, &self.u_edges, &self.m_neighbors, 0.035)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "failed to build OLAM nest spring topology",
                    )
                })?;
        let mut scratch = OlamNestSpringScratch::with_edge_target_lengths(
            self,
            &topology,
            &movable_m_points,
            edge_targets_m,
            project_to_radius,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "failed to prepare OLAM h-field nest spring buffers",
            )
        })?;
        let mut m_points = self.m_points.clone();
        let mut next_m_points = self.m_points.clone();

        for iteration in 1..=niter {
            if (iteration == 1 || iteration == niter || iteration % 100 == 0)
                && !earthmesh_core::progress::report("olam-nest-spring", iteration, niter)
            {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "OLAM nest spring was cancelled",
                ));
            }
            olam_nest_spring_iteration_into(
                &m_points,
                &topology,
                &movable_m_points,
                &mut scratch,
                &mut next_m_points,
            )
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "failed to run OLAM nest spring iteration",
                )
            })?;
            std::mem::swap(&mut m_points, &mut next_m_points);
        }

        for point in m_points.iter_mut().skip(2) {
            point.x = point.x as f32 as f64;
            point.y = point.y as f32 as f64;
            point.z = point.z as f32 as f64;
        }

        let adjusted = Self {
            nmd: self.nmd,
            nud: self.nud,
            nwd: self.nwd,
            impent: self.impent,
            m_points,
            m_metadata: self.m_metadata.clone(),
            u_edges: self.u_edges.clone(),
            w_faces: self.w_faces.clone(),
            m_neighbors: self.m_neighbors.clone(),
            m_prognostic: self.m_prognostic.clone(),
            u_prognostic: self.u_prognostic.clone(),
            w_prognostic: self.w_prognostic.clone(),
            boundary_rows: self.boundary_rows.clone(),
        };
        adjusted.validate_topology()?;
        Ok(adjusted)
    }
}

/// Sample per-edge target lengths for
/// [`OlamDelaunayMesh::spring_nest_with_edge_targets`] from a
/// `(lon_degrees, lat_degrees) -> meters` closure, evaluated at each active U
/// edge's chordal midpoint (dateline-safe by construction: the midpoint is
/// averaged in Cartesian space before converting to lon/lat). Inactive
/// placeholder edges keep a `0.0` target, which is fine because only movable
/// edges are ever read and validated by the scratch builder.
pub fn olam_edge_target_lengths_from_field<F: Fn(f64, f64) -> f64>(
    mesh: &OlamDelaunayMesh,
    target_m: F,
) -> io::Result<Vec<f64>> {
    let mut targets = vec![0.0_f64; mesh.nud + 1];
    for iu in 2..=mesh.nud {
        let edge = mesh.u_edges[iu];
        let [im1, im2] = edge.im;
        if im1 <= 1 || im2 <= 1 || im1 > mesh.nmd || im2 > mesh.nmd {
            continue;
        }
        let p1 = mesh.m_points[im1];
        let p2 = mesh.m_points[im2];
        let midpoint = CartesianPoint::new(
            0.5 * (p1.x + p2.x),
            0.5 * (p1.y + p2.y),
            0.5 * (p1.z + p2.z),
        );
        let lonlat = xyz_to_lonlat_degrees(midpoint);
        let target = target_m(lonlat.lon_degrees, lonlat.lat_degrees);
        if !target.is_finite() || target <= 0.0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("edge {iu} target length {target} must be positive and finite"),
            ));
        }
        targets[iu] = target;
    }
    Ok(targets)
}

pub(crate) fn olam_nest_movable_m_points(
    mesh: &OlamDelaunayMesh,
    ngr: usize,
    move_interior: bool,
) -> io::Result<Vec<bool>> {
    let mut movable = vec![false; mesh.nmd + 1];

    for im in 2..=mesh.nmd {
        if mesh.m_metadata[im].ngr != ngr {
            continue;
        }

        if move_interior {
            movable[im] = true;
            continue;
        }

        let neighbors = mesh.m_neighbors[im];
        for &iw in neighbors.iw.iter().take(neighbors.npoly) {
            require_olam_id("OLAM nest spring movable W face", iw, mesh.nwd)?;
            if mesh.w_faces[iw].mrow != 0 {
                movable[im] = true;
                break;
            }
        }
    }

    Ok(movable)
}
