use super::*;

/// In-memory Rust orchestration for the global `mkgrd.F90:gridinit` mesh path.
///
/// This composes the current deterministic kernels without writing NetCDF:
/// `method_c_gridinit_factorization_canonical` -> `MethodCDelaunayMesh` expansion
/// -> `voronoi_grid_from_method_c_delaunay_mesh`
/// -> `pcvt_adjust_voronoi_grid_state` -> `grid_xyz2lonlat_one_based_state`.
/// The returned state intentionally remains one-based so callers can pass it to
/// `earthmesh_cli::write_gridfile_from_one_based_state` at the I/O boundary.
pub fn gridinit_voronoi_state_canonical(
    nxp0: usize,
    nspring: usize,
    beta: f64,
    spring_relax: f64,
    max_tris: usize,
) -> io::Result<VoronoiGridState> {
    let factors = crate::method_c_gridinit_factorization_canonical(nxp0).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid Method-C gridinit NXP {nxp0}"),
        )
    })?;
    let mut mesh = MethodCDelaunayMesh::from_icosahedron(
        factors.base_nxp,
        nspring,
        beta,
        spring_relax,
        max_tris,
    )
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "failed to build validated Method-C icosahedron grid",
        )
    })?;
    if factors.expansion_factor > 1 {
        mesh = mesh.expand_by_factor(factors.expansion_factor)?;
    }

    let mut state =
        voronoi_grid_from_method_c_delaunay_mesh(&mesh, METHOD_C_CANONICAL_EARTH_RADIUS_METERS)?;
    pcvt_adjust_voronoi_grid_state(&mut state)?;
    grid_xyz2lonlat_one_based_state(&mut state.grid)?;
    Ok(state)
}
