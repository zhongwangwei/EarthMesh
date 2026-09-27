//! Concentrate a spherical grid's resolution without changing its topology.
//!
//! Some model formats cannot take the degree-5/degree-7 vertex pairs every
//! local refinement introduces: ICON allows at most six edges per vertex, and
//! on a closed sphere that leaves an icosahedral grid as the only choice
//! (guide 11.76). What such a format can still take is the same grid with its
//! vertices moved -- finer toward the demand, coarser away from it, the vertex
//! count and every connection unchanged.
//!
//! The move is the Schmidt transformation the stretched global models use:
//! project from the point opposite a focus onto the plane through the focus,
//! shrink that plane by the stretch factor `c`, and project back. It is a
//! Moebius map of the sphere, so it is conformal -- every small triangle keeps
//! its angles -- and the local mesh size becomes
//! `((c^2 + 1) - (c^2 - 1) cos(psi)) / (2c)` of what it was, at angle `psi`
//! from the focus: `1/c` at the focus, `c` opposite it. One focus, smooth:
//! a demand gathered in one area is served, one spread over the globe is not,
//! and the reconciliation says so.
//!
//! A spring relaxation toward per-vertex sizes was tried first: relaxed as
//! asked, a spring network reaches only about 1.6 of a requested 2x (it cannot
//! sit at every rest length at once), and re-aiming the rest lengths to close
//! the gap folded triangles to 0-180 degrees (guide 11.79).

/// A unit vector.
type P = [f64; 3];

fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: P, b: P) -> P {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(a: P) -> Option<P> {
    let n = dot(a, a).sqrt();
    (n.is_finite() && n > 0.0).then(|| [a[0] / n, a[1] / n, a[2] / n])
}

/// Local mesh size after a Schmidt stretch by `factor`, relative to before,
/// at `angle` (radians) from the focus.
pub fn schmidt_local_scale(angle: f64, factor: f64) -> f64 {
    let c2 = factor * factor;
    ((c2 + 1.0) - (c2 - 1.0) * angle.cos()) / (2.0 * factor)
}

/// Move unit vectors `points` by the Schmidt transformation toward `focus`
/// (a unit vector) with stretch `factor >= 1`. `factor == 1` changes nothing.
pub fn schmidt_stretch(points: &mut [P], focus: P, factor: f64) {
    if !(factor.is_finite() && factor > 1.0) {
        return;
    }
    let Some(f) = unit(focus) else {
        return;
    };
    // An orthonormal frame with `f` as its pole.
    let helper = if f[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let Some(e1) = unit(cross(f, helper)) else {
        return;
    };
    let e2 = cross(f, e1);
    for point in points.iter_mut() {
        let (x, y, z) = (dot(*point, e1), dot(*point, e2), dot(*point, f));
        // Stereographic from the antipode -f; the antipode itself stays.
        if 1.0 + z <= f64::EPSILON {
            continue;
        }
        let (u, v) = (x / (1.0 + z) / factor, y / (1.0 + z) / factor);
        let r2 = u * u + v * v;
        let (nx, ny, nz) = (
            2.0 * u / (1.0 + r2),
            2.0 * v / (1.0 + r2),
            (1.0 - r2) / (1.0 + r2),
        );
        *point = [
            nx * e1[0] + ny * e2[0] + nz * f[0],
            nx * e1[1] + ny * e2[1] + nz * f[1],
            nx * e1[2] + ny * e2[2] + nz * f[2],
        ];
    }
}

/// Where to put the focus and how hard to stretch, from each vertex's target
/// depth: the focus is the depth-weighted mean direction (weight `4^L - 1`,
/// the cell-count a depth-L demand adds), the factor `2^deepest` so the focus
/// reaches the deepest level asked, capped at `max_factor`. `None` when
/// nothing is demanded or the demand has no mean direction.
pub fn schmidt_focus_for_levels(
    points: &[P],
    levels: &[usize],
    max_factor: f64,
) -> Option<(P, f64)> {
    let mut sum = [0.0; 3];
    let mut deepest = 0usize;
    for (point, &level) in points.iter().zip(levels) {
        if level == 0 {
            continue;
        }
        deepest = deepest.max(level);
        let weight = 4f64.powi(level.min(16) as i32) - 1.0;
        for k in 0..3 {
            sum[k] += weight * point[k];
        }
    }
    if deepest == 0 {
        return None;
    }
    let focus = unit(sum)?;
    let factor = 2f64.powi(deepest.min(16) as i32).min(max_factor.max(1.0));
    Some((focus, factor))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{spherical_triangle_angles_deg, MeshState, TriangularMesh};

    fn icosahedral(nxp: usize) -> (Vec<P>, Vec<[usize; 3]>) {
        let mesh = TriangularMesh::from_icosahedron(nxp, 0, 1.0, 0.25).unwrap();
        let state = MeshState::from_triangular_mesh(&mesh).unwrap();
        let points = state
            .vertices()
            .iter()
            // Placeholder slots are zero; they are in no face.
            .map(|p| unit([p.x, p.y, p.z]).unwrap_or([0.0; 3]))
            .collect::<Vec<_>>();
        let faces = state
            .active_triangle_slots()
            .map(|face| state.triangles()[face])
            .collect::<Vec<_>>();
        (points, faces)
    }

    fn edge(a: P, b: P) -> f64 {
        dot(a, b).clamp(-1.0, 1.0).acos()
    }

    /// Mean edge (radians) of the faces whose centroid lies within `within`
    /// of `centre` (or beyond it).
    fn mean_edge(points: &[P], faces: &[[usize; 3]], centre: P, within: f64, inside: bool) -> f64 {
        let (mut total, mut count) = (0.0, 0usize);
        for &[a, b, c] in faces {
            let mid = unit([
                points[a][0] + points[b][0] + points[c][0],
                points[a][1] + points[b][1] + points[c][1],
                points[a][2] + points[b][2] + points[c][2],
            ])
            .unwrap();
            if (edge(mid, centre) < within) == inside {
                total += edge(points[a], points[b])
                    + edge(points[b], points[c])
                    + edge(points[c], points[a]);
                count += 3;
            }
        }
        total / count as f64
    }

    #[test]
    fn the_stretch_is_the_factor_at_the_focus_and_its_inverse_opposite() {
        let (mut points, faces) = icosahedral(16);
        let before = points.clone();
        let focus = unit([0.3, -0.5, 0.8]).unwrap();
        let opposite = [-focus[0], -focus[1], -focus[2]];
        let near = 12f64.to_radians();
        let (focus_before, far_before) = (
            mean_edge(&before, &faces, focus, near, true),
            mean_edge(&before, &faces, opposite, near, true),
        );
        schmidt_stretch(&mut points, focus, 3.0);
        let focus_after = mean_edge(&points, &faces, focus, near / 3.0, true);
        let far_after = mean_edge(&points, &faces, opposite, near * 3.0, true);
        let at_focus = focus_after / focus_before;
        let opposite_scale = far_after / far_before;
        assert!(
            (at_focus - 1.0 / 3.0).abs() < 0.03,
            "focus scale {at_focus}"
        );
        assert!(
            (opposite_scale - 3.0).abs() < 0.3,
            "opposite scale {opposite_scale}"
        );
        assert!((schmidt_local_scale(0.0, 3.0) - 1.0 / 3.0).abs() < 1e-12);
        assert!((schmidt_local_scale(std::f64::consts::PI, 3.0) - 3.0).abs() < 1e-12);
    }

    #[test]
    fn the_stretch_keeps_every_triangle_and_its_angles() {
        let (mut points, faces) = icosahedral(16);
        let angles = |points: &[P]| {
            faces
                .iter()
                .flat_map(|&face| spherical_triangle_angles_deg(face.map(|v| points[v])))
                .fold((180.0_f64, 0.0_f64), |(lo, hi), a| (lo.min(a), hi.max(a)))
        };
        let before = angles(&points);
        let orientation = |points: &[P], [a, b, c]: [usize; 3]| {
            let (pa, pb, pc) = (points[a], points[b], points[c]);
            dot(
                cross(
                    [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]],
                    [pc[0] - pa[0], pc[1] - pa[1], pc[2] - pa[2]],
                ),
                pa,
            )
        };
        let signs = faces
            .iter()
            .map(|&f| orientation(&points, f).signum())
            .collect::<Vec<_>>();
        schmidt_stretch(&mut points, unit([1.0, 1.0, 0.2]).unwrap(), 4.0);
        let after = angles(&points);
        for (&face, &sign) in faces.iter().zip(&signs) {
            assert!(
                orientation(&points, face) * sign > 0.0,
                "a triangle turned over"
            );
        }
        // Conformal: small triangles keep their angles up to the change of the
        // size field across one triangle -- about 4 degrees at a factor of 4 on
        // NXP 16 (48.3-72.0 -> 45.0-76.8), well inside the 35-85 contract.
        assert!(
            after.0 > before.0 - 6.0 && after.1 < before.1 + 6.0,
            "{before:?} -> {after:?}"
        );
        assert!(after.0 > 40.0 && after.1 < 80.0, "{after:?}");
    }

    #[test]
    fn the_focus_follows_the_deepest_demand_and_the_factor_its_depth() {
        let points = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let (focus, factor) = schmidt_focus_for_levels(&points, &[2, 0, 0], 16.0).unwrap();
        assert_eq!(focus, [1.0, 0.0, 0.0]);
        assert_eq!(factor, 4.0);
        let (focus, factor) = schmidt_focus_for_levels(&points, &[3, 3, 0], 4.0).unwrap();
        assert!((focus[0] - focus[1]).abs() < 1e-12 && focus[2] == 0.0);
        assert_eq!(factor, 4.0, "capped");
        assert!(schmidt_focus_for_levels(&points, &[0, 0, 0], 16.0).is_none());
    }

    #[test]
    fn a_unit_factor_changes_nothing() {
        let (mut points, _) = icosahedral(4);
        let before = points.clone();
        schmidt_stretch(&mut points, [0.0, 0.0, 1.0], 1.0);
        assert_eq!(points, before);
    }
}
