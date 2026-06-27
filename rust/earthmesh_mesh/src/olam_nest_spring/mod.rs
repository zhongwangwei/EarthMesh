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
        let mut m_points = self.m_points.clone();

        for iteration in 1..=niter {
            if (iteration == 1 || iteration == niter || iteration % 100 == 0)
                && !earthmesh_core::progress::report("olam-nest-spring", iteration, niter)
            {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "OLAM nest spring was cancelled",
                ));
            }
            m_points = olam_nest_spring_iteration(
                &m_points,
                self,
                &topology,
                &movable_m_points,
                dist00,
                project_to_radius,
            )
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "failed to run OLAM nest spring iteration",
                )
            })?;
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
