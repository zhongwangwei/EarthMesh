//! Geometry shared by the certified mothers whose vertices are moved
//! (the Schmidt stretch and the equidistribution).

use earthmesh_mesh::{CartesianPoint, MeshState};

/// A point in 3D, a unit vector where it lies on the sphere.
pub(crate) type P = [f64; 3];

pub(crate) fn unit(p: P) -> Option<P> {
    let r = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
    (r.is_finite() && r > 0.0).then(|| [p[0] / r, p[1] / r, p[2] / r])
}

pub(crate) fn xyz(p: CartesianPoint) -> P {
    [p.x, p.y, p.z]
}

/// Voronoi (dual) area of every vertex slot, from the kites of each triangle
/// around its circumcentre; zero for inactive slots.
pub(crate) fn dual_areas(mesh: &MeshState) -> Vec<f64> {
    let cp = |p: P| CartesianPoint::new(p[0], p[1], p[2]);
    let mut area = vec![0.0; mesh.vertices().len()];
    for t in mesh.active_triangle_slots() {
        let tri = mesh.triangles()[t];
        let Some(p) = tri
            .iter()
            .map(|&v| unit(xyz(mesh.vertices()[v])))
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let u = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
        let w = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
        let Some(mut c) = unit([
            u[1] * w[2] - u[2] * w[1],
            u[2] * w[0] - u[0] * w[2],
            u[0] * w[1] - u[1] * w[0],
        ]) else {
            continue;
        };
        if c[0] * p[0][0] + c[1] * p[0][1] + c[2] * p[0][2] < 0.0 {
            c = [-c[0], -c[1], -c[2]];
        }
        for k in 0..3 {
            let (v, a, b) = (p[k], p[(k + 1) % 3], p[(k + 2) % 3]);
            let (Some(ma), Some(mb)) = (
                unit([v[0] + a[0], v[1] + a[1], v[2] + a[2]]),
                unit([v[0] + b[0], v[1] + b[1], v[2] + b[2]]),
            ) else {
                continue;
            };
            area[tri[k]] += earthmesh_mesh::spherical_kite_area_unit(cp(v), cp(ma), cp(mb), cp(c));
        }
    }
    area
}
