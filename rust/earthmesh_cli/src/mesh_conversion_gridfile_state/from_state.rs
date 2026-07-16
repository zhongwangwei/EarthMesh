use std::io;

use earthmesh_core::{GridMemory, IjTabs};

use crate::{require_len, LonLatPoint, UnstructuredMesh};

pub fn gridfile_mesh_from_state(grid: &GridMemory, tabs: &IjTabs) -> io::Result<UnstructuredMesh> {
    let nma = grid.nma;
    let nwa = grid.nwa;
    require_len("grid.glonm", grid.glonm.len(), nma)?;
    require_len("grid.glatm", grid.glatm.len(), nma)?;
    require_len("grid.glonw", grid.glonw.len(), nwa)?;
    require_len("grid.glatw", grid.glatw.len(), nwa)?;
    require_len("itab_m", tabs.m.len(), nma)?;
    require_len("itab_w", tabs.w.len(), nwa)?;

    let m_points = (0..nma)
        .map(|idx| LonLatPoint {
            lon: grid.glonm[idx],
            lat: grid.glatm[idx],
        })
        .collect();
    let w_points = (0..nwa)
        .map(|idx| LonLatPoint {
            lon: grid.glonw[idx],
            lat: grid.glatw[idx],
        })
        .collect();
    let m_to_w = tabs.m.iter().take(nma).map(|tab| tab.iw).collect();

    let mut n_w_to_m = vec![1; nwa];
    let mut w_to_m = Vec::with_capacity(nwa);
    for (idx, tab) in tabs.w.iter().take(nwa).enumerate() {
        if idx == 0 {
            n_w_to_m[idx] = 1;
        } else if tab.im[5] == 1 {
            n_w_to_m[idx] = 5;
        } else {
            n_w_to_m[idx] = 6;
        }
        w_to_m.push(tab.im.to_vec());
    }

    Ok(UnstructuredMesh {
        m_points,
        w_points,
        m_to_w,
        w_to_m,
        n_w_to_m,
    })
}

/// Build the `Unstructured_Mesh_Save` payload from Canonical-indexed grid state.
///
/// Some current kernels, especially the remaining `gridinit/voronoi/pcvt`
/// path, keep a direct Canonical-compatible layout with slot `0` unused and valid
/// records in `1..=nma` / `1..=nwa`. This adapter deliberately slices those
/// one-based slots into the compact NetCDF payload written by
/// `Unstructured_Mesh_Save`, without changing connectivity IDs.
pub fn gridfile_mesh_from_one_based_state(
    grid: &GridMemory,
    tabs: &IjTabs,
) -> io::Result<UnstructuredMesh> {
    let nma = grid.nma;
    let nwa = grid.nwa;
    require_len("grid.glonm", grid.glonm.len(), nma + 1)?;
    require_len("grid.glatm", grid.glatm.len(), nma + 1)?;
    require_len("grid.glonw", grid.glonw.len(), nwa + 1)?;
    require_len("grid.glatw", grid.glatw.len(), nwa + 1)?;
    require_len("itab_m", tabs.m.len(), nma + 1)?;
    require_len("itab_w", tabs.w.len(), nwa + 1)?;

    let m_points = (1..=nma)
        .map(|idx| LonLatPoint {
            lon: grid.glonm[idx],
            lat: grid.glatm[idx],
        })
        .collect();
    let w_points = (1..=nwa)
        .map(|idx| LonLatPoint {
            lon: grid.glonw[idx],
            lat: grid.glatw[idx],
        })
        .collect();
    let m_to_w = (1..=nma).map(|idx| tabs.m[idx].iw).collect();

    let mut n_w_to_m = Vec::with_capacity(nwa);
    let mut w_to_m = Vec::with_capacity(nwa);
    for iw in 1..=nwa {
        let explicit_npoly = tabs.w[iw].npoly;
        let count = if iw == 1 {
            1
        } else if explicit_npoly > 0 {
            let count = usize::try_from(explicit_npoly).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("itab_w[{iw}] has invalid npoly {explicit_npoly}"),
                )
            })?;
            if count > tabs.w[iw].im.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "itab_w[{iw}] npoly {count} exceeds im width {}",
                        tabs.w[iw].im.len()
                    ),
                ));
            }
            i32::try_from(count).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("itab_w[{iw}] npoly {count} does not fit i32"),
                )
            })?
        } else if tabs.w[iw].im[5] == 1 {
            5
        } else {
            6
        };
        n_w_to_m.push(count);
        w_to_m.push(tabs.w[iw].im.to_vec());
    }

    Ok(UnstructuredMesh {
        m_points,
        w_points,
        m_to_w,
        w_to_m,
        n_w_to_m,
    })
}
