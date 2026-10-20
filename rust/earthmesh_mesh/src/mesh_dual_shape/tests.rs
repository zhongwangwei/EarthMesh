use super::*;

fn lonlat(lon: f64, lat: f64) -> P {
    let (lon, lat) = (lon.to_radians(), lat.to_radians());
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}

/// A hexagon of vertices on open edges, one degree across, around a free
/// centre at `centre` (lon, lat degrees).
fn fan(centre: (f64, f64)) -> (Vec<P>, Vec<[usize; 3]>) {
    let mut points = vec![lonlat(centre.0, centre.1)];
    for k in 0..6 {
        let t = (60.0 * k as f64).to_radians();
        points.push(lonlat(t.cos(), t.sin()));
    }
    let faces = (0..6).map(|k| [0, 1 + k, 1 + (k + 1) % 6]).collect();
    (points, faces)
}

fn icosahedron() -> (Vec<P>, Vec<[usize; 3]>) {
    let t = (1.0 + 5.0_f64.sqrt()) / 2.0;
    let points = [
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
    let faces = vec![
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
    (points, faces)
}

/// Largest sum of the two angles opposite an interior edge.
fn worst_opposite_sum(points: &[P], faces: &[[usize; 3]]) -> f64 {
    let mut around: HashMap<(usize, usize), Vec<f64>> = HashMap::new();
    for f in faces {
        let angles = spherical_triangle_angles_deg(f.map(|i| points[i]));
        for k in 0..3 {
            let (a, b) = (f[(k + 1) % 3], f[(k + 2) % 3]);
            around
                .entry((a.min(b), a.max(b)))
                .or_default()
                .push(angles[k]);
        }
    }
    around
        .values()
        .filter(|angles| angles.len() == 2)
        .map(|angles| angles[0] + angles[1])
        .fold(0.0, f64::max)
}

#[test]
fn a_lopsided_cell_is_evened_out_inside_the_narrowed_window() {
    let (mut points, faces) = fan((0.35, 0.0));
    let ring = points[1..].to_vec();
    let options = DualShapeOptions {
        aspect_limit: 1.5,
        edge_cv_limit: 0.1,
        ..DualShapeOptions::new((35.25, 84.75))
    };
    let report = even_out_dual_cells(&mut points, &faces, options);
    assert!(report.max_edge_cv_before > 0.25, "{report:?}");
    assert!(report.max_edge_cv_after < 0.05, "{report:?}");
    assert!(report.moves > 0);
    assert_eq!(report.over_limit_before, 1);
    assert_eq!(report.over_limit_after, 0);
    // The hexagon is on open edges and stays; the triangles end inside the
    // window narrowed by three degrees, and every spoke stays Delaunay.
    assert_eq!(&points[1..], &ring[..]);
    for f in &faces {
        let angles = spherical_triangle_angles_deg(f.map(|i| points[i]));
        assert!(
            angles.iter().all(|&a| (38.25..=81.75).contains(&a)),
            "{angles:?}"
        );
    }
    assert!(worst_opposite_sum(&points, &faces) < 179.0);
}

#[test]
fn a_regular_cell_is_left_where_it_is() {
    let (mut points, faces) = fan((0.0, 0.0));
    let before = points.clone();
    let report = even_out_dual_cells(&mut points, &faces, DualShapeOptions::new((35.25, 84.75)));
    assert_eq!(report.moves, 0);
    assert_eq!(report.sweeps, 0);
    assert_eq!(points, before);
    assert!(report.max_edge_cv_before < 1.0e-3, "{report:?}");
}

#[test]
fn a_closed_mesh_moves_toward_regular_cells_without_folding_any() {
    let (mut points, faces) = icosahedron();
    // Pull one vertex a tenth of an edge toward a neighbour: its cell and
    // the cells around it are no longer regular pentagons.
    let pulled = unit([
        points[0][0] * 0.9 + points[1][0] * 0.1,
        points[0][1] * 0.9 + points[1][1] * 0.1,
        points[0][2] * 0.9 + points[1][2] * 0.1,
    ]);
    points[0] = pulled;
    let options = DualShapeOptions {
        aspect_limit: 1.05,
        edge_cv_limit: 0.02,
        ..DualShapeOptions::new((35.25, 84.75))
    };
    let report = even_out_dual_cells(&mut points, &faces, options);
    assert!(report.moves > 0, "{report:?}");
    assert!(
        report.max_edge_cv_after < report.max_edge_cv_before * 0.5,
        "{report:?}"
    );
    assert!(
        report.max_aspect_after < report.max_aspect_before,
        "{report:?}"
    );
    for f in &faces {
        let [a, b, c] = f.map(|i| points[i]);
        assert!(dotp(crossp(sub(b, a), sub(c, a)), a) > 0.0);
        let angles = spherical_triangle_angles_deg([a, b, c]);
        assert!(
            angles.iter().all(|&x| (35.25..=84.75).contains(&x)),
            "{angles:?}"
        );
    }
    assert!(worst_opposite_sum(&points, &faces) < 179.0);
}

#[test]
fn ninety_metre_right_triangle_area_is_accurate_after_rotation() {
    let angle = 90.0_f64 / 6_371_000.0;
    let (sin, cos) = angle.sin_cos();
    let expected = 2.0 * (sin * sin).atan2((1.0 + cos).powi(2));
    for (lon, lat) in [
        (0.0_f64, 0.0_f64),
        (100.0, 38.0),
        (-60.0, 0.0),
        (90.0, 59.0),
    ] {
        let a = lonlat(lon, lat);
        let (lon, lat) = (lon.to_radians(), lat.to_radians());
        let east = [-lon.sin(), lon.cos(), 0.0];
        let north = [-lat.sin() * lon.cos(), -lat.sin() * lon.sin(), lat.cos()];
        let b = std::array::from_fn(|i| cos * a[i] + sin * east[i]);
        let c = std::array::from_fn(|i| cos * a[i] + sin * north[i]);
        let actual = triangle_area(a, b, c);
        let relative_error = (actual - expected).abs() / expected;
        assert!(
            relative_error < 1e-10,
            "area {actual:.16e}, expected {expected:.16e}, relative error {relative_error:.3e} at ({lon}, {lat}) radians"
        );
    }
}
