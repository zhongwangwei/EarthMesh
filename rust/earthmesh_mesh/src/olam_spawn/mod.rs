use std::io;

use super::*;

impl OlamDelaunayMesh {
    /// Spawn specified OLAM refinement regions with independent per-region
    /// levels using OLAM Method-C. Each pass follows the legacy perimeter
    /// grouping and transition-patch table updates instead of a generic local
    /// triangulation.
    ///
    /// This defaults to surface-style Method-C transition width (`max_mrows = 7`)
    /// and is therefore intended for non-atmosphere meshes unless callers pass
    /// an explicit width.
    pub fn spawn_nest(
        &self,
        regions: &[OlamRefinementRegion],
        max_level: usize,
    ) -> io::Result<Self> {
        self.spawn_nest_with_max_mrows(regions, max_level, Self::METHOD_C_MAX_MROWS_SURFACE)
    }

    /// Spawn OLAM Method-C refinement with atmosphere-style transition width
    /// (`max_mrows = 13`).
    pub fn spawn_nest_as_atmosmesh(
        &self,
        regions: &[OlamRefinementRegion],
        max_level: usize,
    ) -> io::Result<Self> {
        self.spawn_nest_with_max_mrows(regions, max_level, Self::METHOD_C_MAX_MROWS_ATMOS)
    }

    /// Spawn OLAM Method-C refinement with surface-style transition width
    /// (`max_mrows = 7`).
    pub fn spawn_nest_as_surface(
        &self,
        regions: &[OlamRefinementRegion],
        max_level: usize,
    ) -> io::Result<Self> {
        self.spawn_nest(regions, max_level)
    }

    /// OLAM refinement using an explicit perimeter transition width.
    ///
    /// `max_mrows` controls the `perim_mrow` propagation width and allows callers
    /// to select atmosphere-like (13) or surface-like (7) boundary behavior.
    pub fn spawn_nest_with_max_mrows(
        &self,
        regions: &[OlamRefinementRegion],
        max_level: usize,
        max_mrows: usize,
    ) -> io::Result<Self> {
        self.spawn_nest_internal(regions, max_level, max_mrows, None, false)
            .map(|(mesh, _)| mesh)
    }

    /// OLAM Method-C refinement for Cartesian/native XY coordinates used by
    /// Fortran `ngr_area` when a Method-C spawn is actually active.
    pub fn spawn_nest_cartesian_xy_with_max_mrows(
        &self,
        regions: &[OlamRefinementRegion],
        max_level: usize,
        max_mrows: usize,
    ) -> io::Result<Self> {
        self.spawn_nest_internal(regions, max_level, max_mrows, None, true)
            .map(|(mesh, _)| mesh)
    }

    /// Spawn specified OLAM refinement regions and run OLAM nest spring after
    /// each pass that actually refines faces. The returned counter is the
    /// number of spring passes executed.
    pub fn spawn_nest_with_spring(
        &self,
        regions: &[OlamRefinementRegion],
        max_level: usize,
        nxp: usize,
        niter: usize,
    ) -> io::Result<(Self, usize)> {
        self.spawn_nest_with_spring_and_max_mrows(
            regions,
            max_level,
            Self::METHOD_C_MAX_MROWS_SURFACE,
            nxp,
            niter,
        )
    }

    /// Spawn OLAM Method-C refinement with atmosphere-style transition width
    /// (`max_mrows = 13`) and run OLAM nest spring after each pass.
    pub fn spawn_nest_with_spring_as_atmosmesh(
        &self,
        regions: &[OlamRefinementRegion],
        max_level: usize,
        nxp: usize,
        niter: usize,
    ) -> io::Result<(Self, usize)> {
        self.spawn_nest_with_spring_and_max_mrows(
            regions,
            max_level,
            Self::METHOD_C_MAX_MROWS_ATMOS,
            nxp,
            niter,
        )
    }

    /// Spawn specified OLAM refinement regions with explicit perimeter row width and
    /// optional springing.
    pub fn spawn_nest_with_spring_and_max_mrows(
        &self,
        regions: &[OlamRefinementRegion],
        max_level: usize,
        max_mrows: usize,
        nxp: usize,
        niter: usize,
    ) -> io::Result<(Self, usize)> {
        self.spawn_nest_internal(
            regions,
            max_level,
            max_mrows,
            Some((nxp, niter, None)),
            false,
        )
    }

    /// OLAM Method-C refinement with springing for Cartesian/native XY
    /// coordinates used by Fortran `ngr_area` when a Method-C spawn is active.
    pub fn spawn_nest_cartesian_xy_with_spring_and_max_mrows(
        &self,
        regions: &[OlamRefinementRegion],
        max_level: usize,
        max_mrows: usize,
        nxp: usize,
        niter: usize,
    ) -> io::Result<(Self, usize)> {
        self.spawn_nest_internal(
            regions,
            max_level,
            max_mrows,
            Some((nxp, niter, None)),
            true,
        )
    }

    /// OLAM Method-C refinement with springing for Cartesian/native XY
    /// coordinates, using Fortran `spring_dynamics_nest` target spacing:
    /// `deltax * sqrt(2 / sqrt(3))`.
    pub fn spawn_nest_cartesian_xy_with_spring_deltax_and_max_mrows(
        &self,
        regions: &[OlamRefinementRegion],
        max_level: usize,
        max_mrows: usize,
        nxp: usize,
        niter: usize,
        deltax_meters: f64,
    ) -> io::Result<(Self, usize)> {
        if !deltax_meters.is_finite() || deltax_meters < 0.001 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM Cartesian nest spring deltax must be at least 0.001",
            ));
        }
        let cartesian_dist00 = deltax_meters * (2.0 / 3.0_f64.sqrt()).sqrt();
        self.spawn_nest_internal(
            regions,
            max_level,
            max_mrows,
            Some((nxp, niter, Some(cartesian_dist00))),
            true,
        )
    }
}
