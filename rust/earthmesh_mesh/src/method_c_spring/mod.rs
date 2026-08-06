use super::*;

pub(crate) fn active_mesh_radius(mesh: &MethodCDelaunayMesh) -> io::Result<f64> {
    for point in mesh.m_points.iter().skip(2) {
        let radius = magnitude(*point);
        if radius.is_finite() && radius > 0.0 {
            return Ok(radius);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "Method-C mesh has no active point with a positive radius",
    ))
}

impl MethodCDelaunayMesh {
    /// Apply Method-C `spring_dynamics_globe` to the active Delaunay M points.
    ///
    /// Method-C's global spring is a Delaunay-edge relaxation pass: U-edge lengths
    /// are pushed toward `beta * 2*pi*R / (5*nxp) / 1.2`, the target is adjusted
    /// by the two opposite triangle angles, and all M points are projected back
    /// to the sphere.
    pub fn spring_global(&self, nxp: usize, niter: usize) -> io::Result<Self> {
        self.spring_global_with_controls(nxp, niter, 1.25, 0.035)
    }

    /// Same as [`Self::spring_global`], but exposes Method-C's two scalar controls
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

    /// Apply Method-C `spring_dynamics_globe` for Cartesian/regional native
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
                "Method-C Cartesian global spring deltax must be at least 0.001",
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
                "Method-C global spring requires positive NXP",
            ));
        }
        if !beta.is_finite() || beta <= 0.0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C global spring beta must be positive and finite",
            ));
        }
        if !relax.is_finite() || relax <= 0.0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C global spring relax must be positive and finite",
            ));
        }

        self.validate_topology()?;
        if niter == 0 {
            return Ok(self.clone());
        }

        let radius = active_mesh_radius(self)?;
        let topology = icosahedron_spring_topology_canonical(
            self.nmd,
            &self.u_edges,
            &self.m_neighbors,
            relax,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "failed to build Method-C global spring topology",
            )
        })?;
        let dist00 = dist00_override.unwrap_or(canonical_global_dist00(beta, radius, nxp));
        // Double buffering keeps each relaxation step Jacobi-style. Dummy
        // slots are never written and retain their initial values.
        let mut m_points = self.m_points.clone();
        let mut next_m_points = self.m_points.clone();
        let mut scratch = MethodCGlobalSpringScratch::new(topology.edge_m_points.len());

        for iteration in 1..=niter {
            if (iteration == 1 || iteration == niter || iteration % 20 == 0)
                && !earthmesh_core::progress::report("spring", iteration, niter)
            {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Method-C global spring was cancelled",
                ));
            }
            method_c_global_spring_iteration_into(
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
                    "failed to run Method-C global spring iteration",
                )
            })?;
            std::mem::swap(&mut m_points, &mut next_m_points);
        }

        // Canonical storage is default Fortran `real`: the spring workspace
        // accumulates in r8, then writes the completed grid back through f32.
        for point in m_points.iter_mut().skip(2) {
            point.x = point.x as f32 as f64;
            point.y = point.y as f32 as f64;
            point.z = point.z as f32 as f64;
        }

        let adjusted = Self {
            // Spring moves points; it does not create or merge rows, so the
            // ancestry carries over unchanged.
            w_lineage: self.w_lineage.clone(),
            m_lineage: self.m_lineage.clone(),
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
