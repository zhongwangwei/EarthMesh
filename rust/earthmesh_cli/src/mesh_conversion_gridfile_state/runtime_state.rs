use std::io;

use earthmesh_core::{EarthmeshConfig, EarthmeshRuntimeState, GridMemory, IjTabs};

use crate::{usize_to_i32, validate_unstructured_mesh, UnstructuredMesh};

pub(crate) fn earthmesh_runtime_state_from_compact_mesh(
    config: &EarthmeshConfig,
    mesh: &UnstructuredMesh,
) -> io::Result<EarthmeshRuntimeState> {
    validate_unstructured_mesh(mesh)?;
    let nma = mesh.m_points.len();
    let nwa = mesh.w_points.len();
    let mut state = EarthmeshRuntimeState::new(config.clone());
    state.grid = GridMemory {
        nma,
        nua: 0,
        nva: 0,
        nwa,
        mma: nma,
        mua: 0,
        mva: 0,
        mwa: nwa,
        ..GridMemory::default()
    };
    state.grid.allocate_xyzem(nma);
    state.grid.allocate_xyzew(nwa);
    state.grid.allocate_grid_lonlatmw(nma, 0, nwa);
    for (idx, point) in mesh.m_points.iter().enumerate() {
        state.grid.glonm[idx] = point.lon;
        state.grid.glatm[idx] = point.lat;
    }
    for (idx, point) in mesh.w_points.iter().enumerate() {
        state.grid.glonw[idx] = point.lon;
        state.grid.glatw[idx] = point.lat;
    }

    state
        .record_mesh_counts_for_step(1, nma, nwa)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

    state.ijtabs = IjTabs::allocate(nma, 0, nwa);
    for (idx, row) in mesh.m_to_w.iter().enumerate() {
        state.ijtabs.m[idx].imp = usize_to_i32("compact mesh M index", idx + 1)?;
        state.ijtabs.m[idx].imglobe = state.ijtabs.m[idx].imp;
        state.ijtabs.m[idx].npoly = 3;
        state.ijtabs.m[idx].iw = *row;
    }
    for (idx, row) in mesh.w_to_m.iter().enumerate() {
        state.ijtabs.w[idx].iwp = usize_to_i32("compact mesh W index", idx + 1)?;
        state.ijtabs.w[idx].iwglobe = state.ijtabs.w[idx].iwp;
        state.ijtabs.w[idx].npoly = *mesh.n_w_to_m.get(idx).unwrap_or(&0);
        for (slot, value) in row.iter().copied().take(7).enumerate() {
            state.ijtabs.w[idx].im[slot] = value;
        }
    }

    Ok(state)
}
