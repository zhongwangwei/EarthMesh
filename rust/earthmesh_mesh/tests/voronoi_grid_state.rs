use earthmesh_mesh::METHOD_C_CANONICAL_EARTH_RADIUS_METERS;
use earthmesh_mesh::{
    grid_xyz2lonlat_one_based_state, gridinit_voronoi_state_canonical,
    icosahedron_relaxed_grid_canonical, lonlat_degrees_to_unit_xyz, pcvt_adjust_voronoi_grid_state,
    spherical_centroid_degrees, spherical_circumcenter_from_barycenter,
    voronoi_grid_from_icosahedron_relaxed, voronoi_grid_from_method_c_delaunay_mesh,
    CartesianPoint, LonLatDegrees, MethodCDelaunayMesh,
};

fn approx_eq(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual={actual} expected={expected} tolerance={tolerance}"
    );
}

#[test]
fn voronoi_grid_from_relaxed_icosahedron_swaps_delaunay_counts_and_keeps_one_based_slots() {
    let relaxed = icosahedron_relaxed_grid_canonical(1, 0, 1.0, 0.25, 100)
        .expect("relaxed icosahedron fixture");

    let state =
        voronoi_grid_from_icosahedron_relaxed(&relaxed, METHOD_C_CANONICAL_EARTH_RADIUS_METERS)
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
    assert_eq!(state.grid.xew[2], relaxed.m_points[2].x);
    assert_eq!(state.grid.yew[2], relaxed.m_points[2].y);
    assert_eq!(state.grid.zew[2], relaxed.m_points[2].z);
}

#[test]
fn voronoi_grid_from_relaxed_icosahedron_initializes_m_barycenters_on_sphere() {
    let relaxed = icosahedron_relaxed_grid_canonical(1, 0, 1.0, 0.25, 100)
        .expect("relaxed icosahedron fixture");
    let state =
        voronoi_grid_from_icosahedron_relaxed(&relaxed, METHOD_C_CANONICAL_EARTH_RADIUS_METERS)
            .expect("voronoi grid state");

    let face = &relaxed.connectivity.w_faces[2];
    let [iw1, iw2, iw3] = face.im;
    let x = (relaxed.m_points[iw1].x + relaxed.m_points[iw2].x + relaxed.m_points[iw3].x) / 3.0;
    let y = (relaxed.m_points[iw1].y + relaxed.m_points[iw2].y + relaxed.m_points[iw3].y) / 3.0;
    let z = (relaxed.m_points[iw1].z + relaxed.m_points[iw2].z + relaxed.m_points[iw3].z) / 3.0;
    let scale = METHOD_C_CANONICAL_EARTH_RADIUS_METERS / (x * x + y * y + z * z).sqrt();

    approx_eq(state.grid.xem[2], x * scale, 1.0e-6);
    approx_eq(state.grid.yem[2], y * scale, 1.0e-6);
    approx_eq(state.grid.zem[2], z * scale, 1.0e-6);
}

#[test]
fn one_based_voronoi_state_can_fill_one_based_lonlat_arrays() {
    let relaxed = icosahedron_relaxed_grid_canonical(1, 0, 1.0, 0.25, 100)
        .expect("relaxed icosahedron fixture");
    let mut state =
        voronoi_grid_from_icosahedron_relaxed(&relaxed, METHOD_C_CANONICAL_EARTH_RADIUS_METERS)
            .expect("voronoi grid state");

    earthmesh_mesh::grid_xyz2lonlat_one_based_state(&mut state.grid)
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
    let relaxed = icosahedron_relaxed_grid_canonical(1, 0, 1.0, 0.25, 100)
        .expect("relaxed icosahedron fixture");
    let mut state =
        voronoi_grid_from_icosahedron_relaxed(&relaxed, METHOD_C_CANONICAL_EARTH_RADIUS_METERS)
            .expect("voronoi grid state");

    let im = (2..=state.grid.nma)
        .find(|&candidate| {
            let iw = state.tabs.m[candidate].iw;
            iw.iter().all(|&value| value >= 2)
                && state.grid.xem[candidate].abs() > f64::EPSILON
                && state.grid.xem[candidate].hypot(state.grid.yem[candidate]) > 1.0
        })
        .expect("non-degenerate triangle away from projection pole");
    let initial = CartesianPoint::new(state.grid.xem[im], state.grid.yem[im], state.grid.zem[im]);
    let vertex_ids = state.tabs.m[im].iw.map(|value| value as usize);
    let vertices = vertex_ids
        .map(|iw| CartesianPoint::new(state.grid.xew[iw], state.grid.yew[iw], state.grid.zew[iw]));
    let expected = spherical_circumcenter_from_barycenter(initial, vertices)
        .expect("non-degenerate circumcenter");

    pcvt_adjust_voronoi_grid_state(&mut state).expect("pcvt adjustment");

    approx_eq(state.grid.xem[im], expected.x, 1.0e-6);
    approx_eq(state.grid.yem[im], expected.y, 1.0e-6);
    approx_eq(state.grid.zem[im], expected.z, 1.0e-6);
    let radius =
        (state.grid.xem[im].powi(2) + state.grid.yem[im].powi(2) + state.grid.zem[im].powi(2))
            .sqrt();
    assert!((radius - METHOD_C_CANONICAL_EARTH_RADIUS_METERS).abs() <= 1.0e-6);
}

#[test]
fn pcvt_rejects_nonlocal_near_collinear_circumcenter() {
    let relaxed = icosahedron_relaxed_grid_canonical(1, 0, 1.0, 0.25, 100)
        .expect("relaxed icosahedron fixture");
    let mut state =
        voronoi_grid_from_icosahedron_relaxed(&relaxed, METHOD_C_CANONICAL_EARTH_RADIUS_METERS)
            .expect("voronoi grid state");
    let triangle = [
        LonLatDegrees::new(-21.591571, -51.613641),
        LonLatDegrees::new(-28.757277, -51.768891),
        LonLatDegrees::new(-24.323722, -51.622878),
    ];
    let barycenter = spherical_centroid_degrees(&triangle).expect("spherical barycenter");
    let scale = |point: CartesianPoint| {
        CartesianPoint::new(
            point.x * METHOD_C_CANONICAL_EARTH_RADIUS_METERS,
            point.y * METHOD_C_CANONICAL_EARTH_RADIUS_METERS,
            point.z * METHOD_C_CANONICAL_EARTH_RADIUS_METERS,
        )
    };
    state.tabs.m[2].iw = [2, 3, 4];
    for (iw, point) in (2..=4).zip(triangle) {
        let point = scale(lonlat_degrees_to_unit_xyz(point));
        state.grid.xew[iw] = point.x;
        state.grid.yew[iw] = point.y;
        state.grid.zew[iw] = point.z;
    }
    let barycenter = scale(lonlat_degrees_to_unit_xyz(barycenter));
    state.grid.xem[2] = barycenter.x;
    state.grid.yem[2] = barycenter.y;
    state.grid.zem[2] = barycenter.z;

    let err = pcvt_adjust_voronoi_grid_state(&mut state).expect_err("non-local circumcenter");
    assert!(err.to_string().contains("non-local spherical circumcenter"));
}

#[test]
fn gridinit_voronoi_state_runs_relax_voronoi_pcvt_and_lonlat_fill() {
    let state =
        gridinit_voronoi_state_canonical(1, 0, 1.0, 0.25, 100).expect("gridinit in-memory state");

    assert_eq!(state.grid.glonm.len(), state.grid.nma + 1);
    assert_eq!(state.grid.glatm.len(), state.grid.nma + 1);
    assert_eq!(state.grid.glonw.len(), state.grid.nwa + 1);
    assert_eq!(state.grid.glatw.len(), state.grid.nwa + 1);
    assert_eq!(state.grid.glonm[0], 0.0);
    assert_eq!(state.grid.glatw[0], 0.0);

    for im in 2..=state.grid.nma {
        let radius =
            (state.grid.xem[im].powi(2) + state.grid.yem[im].powi(2) + state.grid.zem[im].powi(2))
                .sqrt();
        assert!(
            (radius - METHOD_C_CANONICAL_EARTH_RADIUS_METERS).abs() <= 1.0e-6,
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

#[test]
fn gridinit_voronoi_state_enforces_max_tris() {
    let err = gridinit_voronoi_state_canonical(1, 0, 1.0, 0.25, 1)
        .expect_err("NXP=1 needs 20 active triangles");
    assert!(err.to_string().contains("exceeds max_tris 1"));

    let state = gridinit_voronoi_state_canonical(1, 0, 1.0, 0.25, 20)
        .expect("the exact triangle limit must pass");
    assert_eq!(state.grid.nma - 1, 20);
}

#[test]
fn gridinit_voronoi_state_uses_method_c_factor2_expansion_when_selected() {
    let base = MethodCDelaunayMesh::from_icosahedron(24, 0, 1.0, 0.25, 100)
        .expect("Method-C base NXP 24 mesh");
    let expanded = base
        .expand_by_factor(2)
        .expect("Method-C factor-2 expansion");
    let mut expected =
        voronoi_grid_from_method_c_delaunay_mesh(&expanded, METHOD_C_CANONICAL_EARTH_RADIUS_METERS)
            .expect("expanded Method-C Voronoi state");
    pcvt_adjust_voronoi_grid_state(&mut expected).expect("expected pcvt");
    grid_xyz2lonlat_one_based_state(&mut expected.grid).expect("expected lonlat fill");

    let actual =
        gridinit_voronoi_state_canonical(48, 0, 1.0, 0.25, 100_000).expect("factorized gridinit");

    assert_eq!(actual.grid.nma, expected.grid.nma);
    assert_eq!(actual.grid.nua, expected.grid.nua);
    assert_eq!(actual.grid.nwa, expected.grid.nwa);

    let first_inserted_midpoint = base.nmd + 1;
    approx_eq(
        actual.grid.xew[first_inserted_midpoint],
        expected.grid.xew[first_inserted_midpoint],
        1.0e-6,
    );
    approx_eq(
        actual.grid.yew[first_inserted_midpoint],
        expected.grid.yew[first_inserted_midpoint],
        1.0e-6,
    );
    approx_eq(
        actual.grid.zew[first_inserted_midpoint],
        expected.grid.zew[first_inserted_midpoint],
        1.0e-6,
    );
}
