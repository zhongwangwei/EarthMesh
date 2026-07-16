use std::io;

use crate::coordinates::{magnitude, require_grid_coordinate_len};
use crate::spherical_circumcenter_mesh::circumcenter_is_local_enough;
use crate::{spherical_circumcenter_from_barycenter_with_radius, CartesianPoint, VoronoiGridState};

/// Port of `mkgrd.F90:pcvt` for the one-based Voronoi grid state.
///
/// The input state is the direct output of `voronoi_grid_from_icosahedron_relaxed`:
/// M points are initialized as triangle barycenters and `tabs.m[im].iw[0..3]`
/// points to the three surrounding W vertices.  This routine mirrors the
/// Canonical loop over `im = 2, nma`: invalid placeholder triangles are skipped;
/// valid triangles are replaced by spherical circumcenters and normalized back
/// to the Earth radius by `spherical_circumcenter_from_barycenter`.
pub fn pcvt_adjust_voronoi_grid_state(state: &mut VoronoiGridState) -> io::Result<()> {
    require_grid_coordinate_len("xem", state.grid.xem.len(), state.grid.nma + 1)?;
    require_grid_coordinate_len("yem", state.grid.yem.len(), state.grid.nma + 1)?;
    require_grid_coordinate_len("zem", state.grid.zem.len(), state.grid.nma + 1)?;
    require_grid_coordinate_len("xew", state.grid.xew.len(), state.grid.nwa + 1)?;
    require_grid_coordinate_len("yew", state.grid.yew.len(), state.grid.nwa + 1)?;
    require_grid_coordinate_len("zew", state.grid.zew.len(), state.grid.nwa + 1)?;
    require_grid_coordinate_len("tabs.m", state.tabs.m.len(), state.grid.nma + 1)?;
    let earth_radius = active_voronoi_grid_radius(state)?;

    for im in 2..=state.grid.nma {
        let vertex_ids = state.tabs.m[im].iw;
        if vertex_ids.iter().any(|&iw| iw < 2) {
            continue;
        }
        let vertex_ids = [
            usize::try_from(vertex_ids[0])
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "negative W vertex id"))?,
            usize::try_from(vertex_ids[1])
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "negative W vertex id"))?,
            usize::try_from(vertex_ids[2])
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "negative W vertex id"))?,
        ];
        if vertex_ids.iter().any(|&iw| iw > state.grid.nwa) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("M point {im} references a W vertex beyond nwa"),
            ));
        }

        let barycenter =
            CartesianPoint::new(state.grid.xem[im], state.grid.yem[im], state.grid.zem[im]);
        let vertices = vertex_ids.map(|iw| {
            CartesianPoint::new(state.grid.xew[iw], state.grid.yew[iw], state.grid.zew[iw])
        });
        let circumcenter =
            spherical_circumcenter_from_barycenter_with_radius(barycenter, vertices, earth_radius)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("M point {im} has degenerate spherical circumcenter"),
                    )
                })?;
        if !circumcenter_is_local_enough(barycenter, circumcenter, vertices) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("M point {im} has a non-local spherical circumcenter"),
            ));
        }
        state.grid.xem[im] = circumcenter.x;
        state.grid.yem[im] = circumcenter.y;
        state.grid.zem[im] = circumcenter.z;
    }

    Ok(())
}

fn active_voronoi_grid_radius(state: &VoronoiGridState) -> io::Result<f64> {
    for iw in 2..=state.grid.nwa {
        let point = CartesianPoint::new(state.grid.xew[iw], state.grid.yew[iw], state.grid.zew[iw]);
        let radius = magnitude(point);
        if radius.is_finite() && radius > 0.0 {
            return Ok(radius);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "Voronoi grid state has no active W point with a positive radius",
    ))
}
