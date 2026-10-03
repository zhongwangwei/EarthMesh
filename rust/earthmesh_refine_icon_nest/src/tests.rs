use super::*;
use std::collections::BTreeSet;

/// An icosahedron bisected `levels` times: 20 * 4^levels triangles.
fn icosphere(levels: u32) -> (Vec<P>, Vec<[usize; 3]>) {
    let t = (1.0 + 5f64.sqrt()) / 2.0;
    let mut points = [
        [-1.0, t, 0.0],
        [1.0, t, 0.0],
        [-1.0, -t, 0.0],
        [1.0, -t, 0.0],
        [0.0, -1.0, t],
        [0.0, 1.0, t],
        [0.0, -1.0, -t],
        [0.0, 1.0, -t],
        [t, 0.0, -1.0],
        [t, 0.0, 1.0],
        [-t, 0.0, -1.0],
        [-t, 0.0, 1.0],
    ]
    .map(unit)
    .to_vec();
    let mut triangles = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    for _ in 0..levels {
        let mut mids = BTreeMap::new();
        let mut mid = |a: usize, b: usize, points: &mut Vec<P>| {
            *mids.entry((a.min(b), a.max(b))).or_insert_with(|| {
                let (p, q) = (points[a], points[b]);
                points.push(unit([p[0] + q[0], p[1] + q[1], p[2] + q[2]]));
                points.len() - 1
            })
        };
        let mut next = Vec::new();
        for [a, b, c] in triangles {
            let ab = mid(a, b, &mut points);
            let bc = mid(b, c, &mut points);
            let ca = mid(c, a, &mut points);
            next.extend([[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]);
        }
        triangles = next;
    }
    (points, triangles)
}

fn lonlat(lon: f64, lat: f64) -> P {
    let (lon, lat) = (lon.to_radians(), lat.to_radians());
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}

fn angle(a: P, b: P) -> f64 {
    (a[0] * b[0] + a[1] * b[1] + a[2] * b[2])
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees()
}

/// Level `level` within `radius` degrees of each centre.
fn circles(spots: Vec<(P, f64, u32)>) -> impl Fn(P) -> u32 {
    move |p| {
        spots
            .iter()
            .filter(|(c, r, _)| angle(p, *c) <= *r)
            .map(|(_, _, level)| *level)
            .max()
            .unwrap_or(0)
    }
}

fn outward(points: &[P], [a, b, c]: [usize; 3]) -> bool {
    let (a, b, c) = (points[a], points[b], points[c]);
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    n[0] * a[0] + n[1] * a[1] + n[2] * a[2] > 0.0
}

/// The ICON contract every nest must meet, checked against its parent.
fn check_nest(nest: &IconNestDomain, parent: &IconNestDomain, options: &IconNestOptions) {
    assert_eq!(nest.depth, parent.depth + 1);
    assert_eq!(nest.parent, parent.id);
    // Whole parent triangles, each split into exactly four.
    let mut children = BTreeMap::<usize, usize>::new();
    for &t in &nest.parent_triangle {
        *children.entry(t).or_default() += 1;
    }
    assert!(
        children.values().all(|&n| n == 4),
        "a parent triangle is split other than 1->4"
    );
    // Never within the parent's boundary indexing depth.
    assert!(children
        .keys()
        .all(|&t| parent.cell_row[t] > options.boundary_depth));
    // Children lie inside their parent triangle and keep its orientation.
    for (tri, &p) in nest.triangles.iter().zip(&nest.parent_triangle) {
        assert!(outward(&nest.points, *tri));
        let c = centroid(&nest.points, *tri);
        let pc = centroid(&parent.points, parent.triangles[p]);
        let size = angle(parent.points[parent.triangles[p][0]], pc);
        assert!(angle(c, pc) < size);
    }
    // Vertices come from the parent as kept corners or edge midpoints.
    for (point, origin) in nest.points.iter().zip(&nest.vertex_origin) {
        match *origin {
            NestVertexOrigin::Parent(v) => assert_eq!(*point, parent.points[v]),
            NestVertexOrigin::Midpoint(a, b) => {
                assert!(a < b);
                let m = unit([
                    parent.points[a][0] + parent.points[b][0],
                    parent.points[a][1] + parent.points[b][1],
                    parent.points[a][2] + parent.points[b][2],
                ]);
                assert!((0..3).all(|k| (point[k] - m[k]).abs() < 1e-15));
            }
            NestVertexOrigin::Base => panic!("a nest vertex has no parent origin"),
        }
    }
    // Deep enough for ICON's boundary zone; one fan per vertex, degree <= 6.
    assert!(nest.cell_row.iter().copied().max().unwrap() >= options.boundary_zone);
    let incident = vertex_triangles(nest.points.len(), &nest.triangles);
    let all = vec![true; nest.triangles.len()];
    for (v, around) in incident.iter().enumerate() {
        assert!(
            !around.is_empty() && around.len() <= 6,
            "vertex {v} has {} triangles",
            around.len()
        );
        assert_eq!(
            fan_runs(v, around, &nest.triangles, &all),
            1,
            "vertex {v} is pinched"
        );
    }
}

#[test]
fn no_demand_plans_only_the_global_grid() {
    let (points, triangles) = icosphere(3);
    let domains = plan_icon_nests(points, triangles, |_| 0, &IconNestOptions::default()).unwrap();
    assert_eq!(domains.len(), 1);
    assert_eq!(
        (domains[0].id, domains[0].parent, domains[0].depth),
        (1, 0, 0)
    );
}

#[test]
fn one_circle_is_one_nest_with_its_demand_clear_of_the_boundary_zone() {
    let options = IconNestOptions::default();
    let (points, triangles) = icosphere(5);
    let target = circles(vec![(lonlat(115.0, 23.0), 3.0, 1)]);
    let domains = plan_icon_nests(points, triangles, &target, &options).unwrap();
    assert_eq!(domains.len(), 2);
    check_nest(&domains[1], &domains[0], &options);
    // Every nest triangle that the demand covers is past the boundary zone.
    let nest = &domains[1];
    let served = nest
        .triangles
        .iter()
        .zip(&nest.cell_row)
        .filter(|(tri, _)| triangle_target(&nest.points, **tri, &target) >= 1)
        .collect::<Vec<_>>();
    assert!(!served.is_empty());
    assert!(served.iter().all(|(_, &row)| row > options.boundary_zone));
}

#[test]
fn a_deeper_demand_nests_inside_the_first_nest() {
    let options = IconNestOptions::default();
    let (points, triangles) = icosphere(5);
    let centre = lonlat(115.0, 23.0);
    let target = circles(vec![(centre, 6.0, 1), (centre, 2.0, 2)]);
    let domains = plan_icon_nests(points, triangles, &target, &options).unwrap();
    assert_eq!(domains.len(), 3);
    check_nest(&domains[1], &domains[0], &options);
    check_nest(&domains[2], &domains[1], &options);
    assert_eq!(domains[2].depth, 2);
    let inner = &domains[2];
    assert!(inner
        .triangles
        .iter()
        .zip(&inner.cell_row)
        .filter(|(tri, _)| triangle_target(&inner.points, **tri, &target) >= 2)
        .all(|(_, &row)| row > options.boundary_zone));
}

#[test]
fn distant_demands_are_sibling_nests_sharing_no_parent_triangle() {
    let options = IconNestOptions::default();
    let (points, triangles) = icosphere(5);
    let target = circles(vec![
        (lonlat(115.0, 23.0), 2.0, 1),
        (lonlat(-60.0, -20.0), 2.0, 1),
    ]);
    let domains = plan_icon_nests(points, triangles, &target, &options).unwrap();
    assert_eq!(domains.len(), 3);
    for nest in &domains[1..] {
        check_nest(nest, &domains[0], &options);
    }
    let a = domains[1]
        .parent_triangle
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let b = domains[2]
        .parent_triangle
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    assert!(a.is_disjoint(&b));
    // Nor a shared parent vertex: siblings are separated by whole triangles.
    let corners = |set: &BTreeSet<usize>| {
        set.iter()
            .flat_map(|&t| domains[0].triangles[t])
            .collect::<BTreeSet<_>>()
    };
    assert!(corners(&a).is_disjoint(&corners(&b)));
}

#[test]
fn more_domains_than_icon_accepts_is_refused() {
    let options = IconNestOptions {
        max_domains: 2,
        ..IconNestOptions::default()
    };
    let (points, triangles) = icosphere(5);
    let target = circles(vec![
        (lonlat(115.0, 23.0), 2.0, 1),
        (lonlat(-60.0, -20.0), 2.0, 1),
    ]);
    let error = plan_icon_nests(points, triangles, &target, &options).unwrap_err();
    assert!(error.contains("max_dom"), "{error}");
}

#[test]
fn margins_leave_room_for_every_deeper_level() {
    let options = IconNestOptions::default();
    assert_eq!(margin_rows(1, 1, &options), 7);
    assert_eq!(margin_rows(2, 2, &options), 7);
    assert_eq!(margin_rows(1, 2, &options), 11);
    assert_eq!(margin_rows(1, 3, &options), 13);
}
