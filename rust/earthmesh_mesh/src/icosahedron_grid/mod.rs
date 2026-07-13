use super::*;

/// Integrated Rust port of the deterministic in-memory portions of
/// `icosahedron.F90:icosahedron`.
///
/// This creates initial M-point coordinates, fills diamond U/W connectivity,
/// derives `tri_neighbors`, builds `spring_dynamics1` topology, computes the
/// Canonical coarse target distance `beta * pi2_r8 * erad8 / (5 * nxp0)`, and
/// applies the current spring loop for `niter` iterations.
pub fn icosahedron_relaxed_grid_canonical(
    nxp0: usize,
    niter: usize,
    beta: f64,
    relax: f64,
    diagnostic_every: usize,
) -> Option<IcosahedronRelaxedGrid> {
    let initial = icosahedron_initial_grid_canonical(nxp0)?;
    let mut connectivity = icosahedron_fill_diamonds_canonical(nxp0)?;
    let m_neighbors = derive_icosahedron_tri_neighbors_canonical(initial.nmd, &mut connectivity)?;
    let topology = icosahedron_spring_topology_canonical(
        initial.nmd,
        &connectivity.u_edges,
        &m_neighbors,
        relax,
    )?;
    let radius = METHOD_C_CANONICAL_EARTH_RADIUS_METERS;
    let dist00 = method_c_canonical_global_dist00(beta, radius, nxp0);
    let spring = icosahedron_spring_dynamics1_canonical(
        &initial.m_points,
        &topology,
        niter,
        dist00,
        radius,
        diagnostic_every,
    )?;

    Some(IcosahedronRelaxedGrid {
        nmd: initial.nmd,
        nud: initial.nud,
        nwd: initial.nwd,
        impent: initial.impent,
        m_points: spring.updated_m_points.clone(),
        connectivity,
        m_neighbors,
        spring,
    })
}

/// Shared Rust port of `icosahedron.F90:mdloopf`, `udloopf`, and `wdloopf`.
///
/// The three Canonical routines have identical flag semantics: `init == 'f'`
/// clears all loop flags, negative ids clear the selected loop, positive ids
/// set it, and zero ids are ignored. Input ids are Canonical 1-based.
pub fn apply_icosahedron_loop_flags_canonical(
    loop_flags: &mut [bool; ICOSAHEDRON_MLOOPS],
    initialize_false: bool,
    loop_ids: &[isize],
) -> Option<()> {
    if initialize_false {
        loop_flags.fill(false);
    }

    for &loop_id in loop_ids {
        if loop_id == 0 {
            continue;
        }
        let index = loop_id.unsigned_abs().checked_sub(1)?;
        let slot = loop_flags.get_mut(index)?;
        *slot = loop_id > 0;
    }

    Some(())
}
