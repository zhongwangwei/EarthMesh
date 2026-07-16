use std::io;

use earthmesh_core::{GridMemory, IjTabs, ItabM, ItabW};

use crate::coordinates::normalize_cartesian_to_radius;
use crate::{CartesianPoint, IcosahedronRelaxedGrid, MethodCDelaunayMesh};

/// One-based grid/connectivity state after `mkgrd.F90:voronoi`, before `pcvt`.
#[derive(Debug, Clone, PartialEq)]
pub struct VoronoiGridState {
    pub grid: GridMemory,
    pub tabs: IjTabs,
    pub impent: [usize; 12],
}

/// Port the global icosahedron branch of `mkgrd.F90:voronoi`.
///
/// The returned vectors intentionally keep Canonical-compatible one-based slots:
/// index `0` is unused, and valid records live in `1..=nma` and `1..=nwa`.
pub fn voronoi_grid_from_icosahedron_relaxed(
    relaxed: &IcosahedronRelaxedGrid,
    radius: f64,
) -> io::Result<VoronoiGridState> {
    let mesh = MethodCDelaunayMesh::from_relaxed_icosahedron(relaxed);
    voronoi_grid_from_method_c_delaunay_mesh(&mesh, radius)
}

/// Convert a generic Method-C Delaunay mesh to the Voronoi grid state used by the
/// existing EarthMesh gridfile writers.
///
/// This is the Method-C replacement boundary for `mkgrd.F90:voronoi`: callers should
/// produce or refine an [`MethodCDelaunayMesh`], validate it, then call this
/// adapter at the output boundary.
pub fn voronoi_grid_from_method_c_delaunay_mesh(
    mesh: &MethodCDelaunayMesh,
    radius: f64,
) -> io::Result<VoronoiGridState> {
    voronoi_grid_from_method_c_delaunay_mesh_with_projection(mesh, radius, true)
}

pub fn voronoi_grid_from_method_c_delaunay_mesh_cartesian(
    mesh: &MethodCDelaunayMesh,
    radius: f64,
) -> io::Result<VoronoiGridState> {
    voronoi_grid_from_method_c_delaunay_mesh_with_projection(mesh, radius, false)
}

fn voronoi_grid_from_method_c_delaunay_mesh_with_projection(
    mesh: &MethodCDelaunayMesh,
    radius: f64,
    project_cell_centers_to_radius: bool,
) -> io::Result<VoronoiGridState> {
    if project_cell_centers_to_radius && (!radius.is_finite() || radius <= 0.0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Voronoi projection radius {radius} must be positive and finite"),
        ));
    }
    mesh.validate_topology()?;

    let mut grid = GridMemory {
        nma: mesh.nwd,
        nua: mesh.nud,
        nva: mesh.nud,
        nwa: mesh.nmd,
        mma: mesh.nwd,
        mua: mesh.nud,
        mva: mesh.nud,
        mwa: mesh.nmd,
        ..GridMemory::default()
    };
    grid.allocate_xyzem(grid.nma + 1);
    grid.allocate_xyzew(grid.nwa + 1);

    for iw in 1..=grid.nwa {
        let point = mesh.m_points[iw];
        grid.xew[iw] = point.x;
        grid.yew[iw] = point.y;
        grid.zew[iw] = point.z;
    }

    for im in 2..=grid.nma {
        let face = &mesh.w_faces[im];
        if face
            .im
            .iter()
            .any(|&idx| idx < 2 || idx >= mesh.m_points.len())
        {
            continue;
        }
        let p1 = mesh.m_points[face.im[0]];
        let p2 = mesh.m_points[face.im[1]];
        let p3 = mesh.m_points[face.im[2]];
        let barycenter = CartesianPoint::new(
            (p1.x + p2.x + p3.x) / 3.0,
            (p1.y + p2.y + p3.y) / 3.0,
            (p1.z + p2.z + p3.z) / 3.0,
        );
        let point = if project_cell_centers_to_radius {
            normalize_cartesian_to_radius(barycenter, radius)?
        } else {
            barycenter
        };
        grid.xem[im] = point.x;
        grid.yem[im] = point.y;
        grid.zem[im] = point.z;
    }

    let mut tabs = IjTabs::allocate(grid.nma + 1, grid.nva + 1, grid.nwa + 1);
    for iw in 2..=grid.nwa {
        let neighbor = &mesh.m_neighbors[iw];
        let metadata = mesh.m_metadata[iw];
        tabs.w[iw] = ItabW {
            iwp: iw as i32,
            iwglobe: iw as i32,
            npoly: neighbor.npoly as i32,
            mrlw: metadata.mrlm as i32,
            mrlw_orig: metadata.mrlm_orig as i32,
            ngr: metadata.ngr as i32,
            im: neighbor.iw.map(|value| value as i32),
            iv: neighbor.iu.map(|value| value as i32),
            ..ItabW::default()
        };
    }
    for im in 2..=grid.nma {
        let face = &mesh.w_faces[im];
        tabs.m[im] = ItabM {
            imp: im as i32,
            imglobe: im as i32,
            npoly: face.npoly as i32,
            mrlm: face.mrlw as i32,
            mrlm_orig: face.mrlw_orig as i32,
            ngr: face.ngr as i32,
            iv: face.iu.map(|value| value as i32),
            iw: face.im.map(|value| value as i32),
            ..ItabM::default()
        };
    }

    Ok(VoronoiGridState {
        grid,
        tabs,
        impent: mesh.impent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spherical_voronoi_rejects_invalid_radius() {
        let mesh = MethodCDelaunayMesh::from_icosahedron(1, 0, 1.0, 0.25, 100)
            .expect("valid Method-C mesh");
        for radius in [-1.0, 0.0, f64::NAN, f64::INFINITY] {
            let error = voronoi_grid_from_method_c_delaunay_mesh(&mesh, radius)
                .expect_err("invalid spherical radius must be rejected");
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        }
    }
}
