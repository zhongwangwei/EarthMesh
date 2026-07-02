use super::*;

pub(crate) fn active_mesh_radius(mesh: &OlamDelaunayMesh) -> io::Result<f64> {
    for point in mesh.m_points.iter().skip(2) {
        let radius = magnitude(*point);
        if radius.is_finite() && radius > 0.0 {
            return Ok(radius);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "OLAM mesh has no active point with a positive radius",
    ))
}

impl OlamDelaunayMesh {
    /// Apply OLAM `spring_dynamics_globe` to the active Delaunay M points.
    ///
    /// OLAM's global spring is a Delaunay-edge relaxation pass: U-edge lengths
    /// are pushed toward `beta * 2*pi*R / (5*nxp) / 1.2`, the target is adjusted
    /// by the two opposite triangle angles, all M points are projected back to
    /// the sphere, and the twelve original pentagon points (`impent`) are kept
    /// fixed.
    pub fn spring_global(&self, nxp: usize, niter: usize) -> io::Result<Self> {
        self.spring_global_with_controls(nxp, niter, 1.25, 0.035)
    }

    /// Same as [`Self::spring_global`], but exposes OLAM's two scalar controls
    /// so callers that still carry namelist values can opt into them explicitly.
    pub fn spring_global_with_controls(
        &self,
        nxp: usize,
        niter: usize,
        beta: f64,
        relax: f64,
    ) -> io::Result<Self> {
        self.spring_global_with_dist00_and_projection(nxp, niter, beta, relax, None, true)
    }

    /// Apply OLAM `spring_dynamics_globe` for Cartesian/regional native
    /// coordinates (`mdomain >= 2`): target spacing comes from `deltax`, and M
    /// points are not projected back to Earth radius.
    pub fn spring_global_cartesian_with_controls(
        &self,
        nxp: usize,
        niter: usize,
        deltax_meters: f64,
        relax: f64,
    ) -> io::Result<Self> {
        if !deltax_meters.is_finite() || deltax_meters < 0.001 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM Cartesian global spring deltax must be at least 0.001",
            ));
        }
        let cartesian_dist00 = deltax_meters * (2.0 / 3.0_f64.sqrt()).sqrt();
        self.spring_global_with_dist00_and_projection(
            nxp,
            niter,
            1.0,
            relax,
            Some(cartesian_dist00),
            false,
        )
    }

    fn spring_global_with_dist00_and_projection(
        &self,
        nxp: usize,
        niter: usize,
        beta: f64,
        relax: f64,
        dist00_override: Option<f64>,
        project_to_radius: bool,
    ) -> io::Result<Self> {
        if nxp == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM global spring requires positive NXP",
            ));
        }
        if !beta.is_finite() || beta <= 0.0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM global spring beta must be positive and finite",
            ));
        }
        if !relax.is_finite() || relax <= 0.0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM global spring relax must be positive and finite",
            ));
        }

        self.validate_topology()?;
        if niter == 0 {
            return Ok(self.clone());
        }

        let radius = active_mesh_radius(self)?;
        let topology =
            icosahedron_spring_topology_fortran(self.nmd, &self.u_edges, &self.m_neighbors, relax)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "failed to build OLAM global spring topology",
                    )
                })?;
        let dist00 = dist00_override.unwrap_or(olam_fortran_global_dist00(beta, radius, nxp));
        // Double buffering: pentagon/dummy slots are never written by the
        // iteration, so both buffers keep their initial positions there
        // forever, exactly like the historical clone-per-iteration version.
        let mut m_points = self.m_points.clone();
        let mut next_m_points = self.m_points.clone();
        let mut scratch = OlamGlobalSpringScratch::new(
            m_points.len(),
            topology.edge_m_points.len(),
            &self.impent,
        );

        for iteration in 1..=niter {
            if (iteration == 1 || iteration == niter || iteration % 20 == 0)
                && !earthmesh_core::progress::report("spring", iteration, niter)
            {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "OLAM global spring was cancelled",
                ));
            }
            olam_global_spring_iteration_into(
                &m_points,
                &topology,
                &mut scratch,
                dist00,
                if project_to_radius {
                    Some(radius)
                } else {
                    None
                },
                &mut next_m_points,
            )
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "failed to run OLAM global spring iteration",
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
