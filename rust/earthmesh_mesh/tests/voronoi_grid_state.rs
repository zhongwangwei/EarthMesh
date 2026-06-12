use earthmesh_core::EARTH_RADIUS_METERS;
use earthmesh_mesh::{
    gridinit_voronoi_state_fortran, icosahedron_relaxed_grid_fortran,
    pcvt_adjust_voronoi_grid_state, spherical_circumcenter_from_barycenter,
    voronoi_grid_from_icosahedron_relaxed, CartesianPoint,
};

fn approx_eq(actual: f32, expected: f64, tolerance: f64) {
    assert!(
        (f64::from(actual) - expected).abs() <= tolerance,
        "actual={actual} expected={expected} tolerance={tolerance}"
    );
}

#[test]
fn voronoi_grid_from_relaxed_icosahedron_swaps_delaunay_counts_and_keeps_one_based_slots() {
    let relaxed = icosahedron_relaxed_grid_fortran(1, 0, 1.0, 0.25, 100)
        .expect("relaxed icosahedron fixture");

    let state = voronoi_grid_from_icosahedron_relaxed(&relaxed, EARTH_RADIUS_METERS)
        .expect("voronoi grid state");

    assert_eq!(state.grid.nma, relaxed.nwd);
    assert_eq!(state.grid.nua, relaxed.nud);
    assert_eq!(state.grid.nva, relaxed.nud);
    assert_eq!(state.grid.nwa, relaxed.nmd);
    assert_eq!(state.grid.xem.len(), relaxed.nwd + 1);
    assert_eq!(state.grid.xew.len(), relaxed.nmd + 1);
    assert_eq!(state.tabs.m.len(), relaxed.nwd + 1);
    assert_eq!(state.tabs.w.len(), relaxed.nmd + 1);

    assert_eq!(state.grid.xew[0], 0.0);
    assert_eq!(state.tabs.m[0].iw, [1, 1, 1]);
    assert_eq!(
        state.tabs.m[2].iw,
        relaxed.connectivity.w_faces[2].im.map(|v| v as i32)
    );
    let npoly = relaxed.m_neighbors[3].npoly;
    let expected_w_im: Vec<i32> = relaxed.m_neighbors[3].iw[..npoly]
        .iter()
        .map(|&value| value as i32)
        .collect();
    assert_eq!(&state.tabs.w[3].im[..npoly], expected_w_im.as_slice());
    assert_eq!(state.grid.xew[2], relaxed.m_points[2].x as f32);
    assert_eq!(state.grid.yew[2], relaxed.m_points[2].y as f32);
    assert_eq!(state.grid.zew[2], relaxed.m_points[2].z as f32);
}

#[test]
fn voronoi_grid_from_relaxed_icosahedron_initializes_m_barycenters_on_sphere() {
    let relaxed = icosahedron_relaxed_grid_fortran(1, 0, 1.0, 0.25, 100)
        .expect("relaxed icosahedron fixture");
    let state = voronoi_grid_from_icosahedron_relaxed(&relaxed, EARTH_RADIUS_METERS)
        .expect("voronoi grid state");

    let face = &relaxed.connectivity.w_faces[2];
    let [iw1, iw2, iw3] = face.im;
    let x = (relaxed.m_points[iw1].x + relaxed.m_points[iw2].x + relaxed.m_points[iw3].x) / 3.0;
    let y = (relaxed.m_points[iw1].y + relaxed.m_points[iw2].y + relaxed.m_points[iw3].y) / 3.0;
    let z = (relaxed.m_points[iw1].z + relaxed.m_points[iw2].z + relaxed.m_points[iw3].z) / 3.0;
    let scale = EARTH_RADIUS_METERS / (x * x + y * y + z * z).sqrt();

    approx_eq(state.grid.xem[2], x * scale, 0.5);
    approx_eq(state.grid.yem[2], y * scale, 0.5);
    approx_eq(state.grid.zem[2], z * scale, 0.5);
}

#[test]
fn fortran_indexed_voronoi_state_can_fill_one_based_lonlat_arrays() {
    let relaxed = icosahedron_relaxed_grid_fortran(1, 0, 1.0, 0.25, 100)
        .expect("relaxed icosahedron fixture");
    let mut state = voronoi_grid_from_icosahedron_relaxed(&relaxed, EARTH_RADIUS_METERS)
        .expect("voronoi grid state");

    earthmesh_mesh::grid_xyz2lonlat_fortran_indexed_state(&mut state.grid)
        .expect("fill one-based lonlat arrays");

    assert_eq!(state.grid.glonm.len(), state.grid.nma + 1);
    assert_eq!(state.grid.glatm.len(), state.grid.nma + 1);
    assert_eq!(state.grid.glonw.len(), state.grid.nwa + 1);
    assert_eq!(state.grid.glatw.len(), state.grid.nwa + 1);
    assert_eq!(state.grid.glonm[0], 0.0);
    assert_eq!(state.grid.glatm[0], 0.0);
    assert!((state.grid.glatw[2] + 90.0).abs() < 1.0e-4);
}

#[test]
fn pcvt_adjusts_voronoi_m_points_to_spherical_circumcenters() {
    let relaxed = icosahedron_relaxed_grid_fortran(1, 0, 1.0, 0.25, 100)
        .expect("relaxed icosahedron fixture");
    let mut state = voronoi_grid_from_icosahedron_relaxed(&relaxed, EARTH_RADIUS_METERS)
        .expect("voronoi grid state");

    let im = (2..=state.grid.nma)
        .find(|&candidate| {
            let iw = state.tabs.m[candidate].iw;
            iw.iter().all(|&value| value >= 2)
                && state.grid.xem[candidate].abs() > f32::EPSILON
                && state.grid.xem[candidate].hypot(state.grid.yem[candidate]) > 1.0
        })
        .expect("non-degenerate triangle away from projection pole");
    let initial = CartesianPoint::new(
        f64::from(state.grid.xem[im]),
        f64::from(state.grid.yem[im]),
        f64::from(state.grid.zem[im]),
    );
    let vertex_ids = state.tabs.m[im].iw.map(|value| value as usize);
    let vertices = vertex_ids.map(|iw| {
        CartesianPoint::new(
            f64::from(state.grid.xew[iw]),
            f64::from(state.grid.yew[iw]),
            f64::from(state.grid.zew[iw]),
        )
    });
    let expected = spherical_circumcenter_from_barycenter(initial, vertices)
        .expect("non-degenerate circumcenter");

    pcvt_adjust_voronoi_grid_state(&mut state).expect("pcvt adjustment");

    approx_eq(state.grid.xem[im], expected.x, 0.5);
    approx_eq(state.grid.yem[im], expected.y, 0.5);
    approx_eq(state.grid.zem[im], expected.z, 0.5);
    let radius = (f64::from(state.grid.xem[im]).powi(2)
        + f64::from(state.grid.yem[im]).powi(2)
        + f64::from(state.grid.zem[im]).powi(2))
    .sqrt();
    assert!((radius - EARTH_RADIUS_METERS).abs() <= 0.5);
}

#[test]
fn gridinit_voronoi_state_runs_relax_voronoi_pcvt_and_lonlat_fill() {
    let state =
        gridinit_voronoi_state_fortran(1, 0, 1.0, 0.25, 100).expect("gridinit in-memory state");

    assert_eq!(state.grid.glonm.len(), state.grid.nma + 1);
    assert_eq!(state.grid.glatm.len(), state.grid.nma + 1);
    assert_eq!(state.grid.glonw.len(), state.grid.nwa + 1);
    assert_eq!(state.grid.glatw.len(), state.grid.nwa + 1);
    assert_eq!(state.grid.glonm[0], 0.0);
    assert_eq!(state.grid.glatw[0], 0.0);

    for im in 2..=state.grid.nma {
        let radius = (f64::from(state.grid.xem[im]).powi(2)
            + f64::from(state.grid.yem[im]).powi(2)
            + f64::from(state.grid.zem[im]).powi(2))
        .sqrt();
        assert!(
            (radius - EARTH_RADIUS_METERS).abs() <= 0.5,
            "im={im} radius={radius}"
        );
        assert!(state.grid.glatm[im] >= -90.0 && state.grid.glatm[im] <= 90.0);
        assert!(state.grid.glonm[im] >= -180.0 && state.grid.glonm[im] <= 180.0);
    }

    for iw in 2..=state.grid.nwa {
        assert!(state.grid.glatw[iw] >= -90.0 && state.grid.glatw[iw] <= 90.0);
        assert!(state.grid.glonw[iw] >= -180.0 && state.grid.glonw[iw] <= 180.0);
    }
}
