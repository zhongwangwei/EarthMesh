use earthmesh_mesh::{icosahedron_initial_grid_fortran, CartesianPoint};

const OLAM_FORTRAN_EARTH_RADIUS_METERS: f64 = 6_371_220.0;

fn approx_eq(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual {actual} expected {expected} tolerance {tolerance}"
    );
}

fn magnitude(point: CartesianPoint) -> f64 {
    (point.x * point.x + point.y * point.y + point.z * point.z).sqrt()
}

#[test]
fn icosahedron_initial_grid_counts_and_pentagon_indices_match_fortran_nxp1() {
    let grid = icosahedron_initial_grid_fortran(1).expect("valid nxp");

    assert_eq!(grid.nmd, 13);
    assert_eq!(grid.nud, 31);
    assert_eq!(grid.nwd, 21);
    assert_eq!(grid.impent, [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]);

    approx_eq(grid.m_points[2].x, 0.0, 1.0e-9);
    approx_eq(grid.m_points[2].y, 0.0, 1.0e-9);
    approx_eq(
        grid.m_points[2].z,
        -OLAM_FORTRAN_EARTH_RADIUS_METERS,
        1.0e-9,
    );
    approx_eq(grid.m_points[13].x, 0.0, 1.0e-9);
    approx_eq(grid.m_points[13].y, 0.0, 1.0e-9);
    approx_eq(
        grid.m_points[13].z,
        OLAM_FORTRAN_EARTH_RADIUS_METERS,
        1.0e-9,
    );
}

#[test]
fn icosahedron_initial_grid_projects_all_active_points_to_earth_radius() {
    let grid = icosahedron_initial_grid_fortran(2).expect("valid nxp");

    assert_eq!(grid.nmd, 43);
    assert_eq!(grid.nud, 121);
    assert_eq!(grid.nwd, 81);
    assert_eq!(grid.impent[0], 2);
    assert_eq!(grid.impent[11], 43);
    assert_eq!(grid.impent[1], 5);
    assert_eq!(grid.impent[6], 25);

    for point_id in 2..grid.m_points.len() {
        approx_eq(
            magnitude(grid.m_points[point_id]),
            OLAM_FORTRAN_EARTH_RADIUS_METERS,
            1.0,
        );
    }
}

#[test]
fn icosahedron_fill_diamonds_matches_fortran_first_southern_diamond_nxp1() {
    let connectivity = earthmesh_mesh::icosahedron_fill_diamonds_fortran(1)
        .expect("valid nxp fill_diamond connectivity");

    assert_eq!(connectivity.u_edges.len(), 32);
    assert_eq!(connectivity.w_faces.len(), 22);

    assert_eq!(connectivity.u_edges[3].im, [3, 4]);
    assert_eq!(connectivity.u_edges[3].iw[0..2], [2, 3]);
    assert_eq!(connectivity.u_edges[3].mrlu, 1);

    assert_eq!(connectivity.u_edges[2].im, [3, 2]);
    assert_eq!(connectivity.u_edges[2].iw[1], 2);
    assert_eq!(connectivity.u_edges[2].mrlu, 1);

    assert_eq!(connectivity.u_edges[4].im, [8, 3]);
    assert_eq!(connectivity.u_edges[4].iw[1], 3);
    assert_eq!(connectivity.u_edges[4].mrlu, 1);

    assert_eq!(connectivity.u_edges[5].iw[0], 2);
    assert_eq!(connectivity.u_edges[17].iw[0], 3);

    assert_eq!(connectivity.w_faces[2].iu, [3, 2, 5]);
    assert_eq!(connectivity.w_faces[2].mrlw, 1);
    assert_eq!(connectivity.w_faces[2].mrlw_orig, 1);
    assert_eq!(connectivity.w_faces[2].ngr, 1);

    assert_eq!(connectivity.w_faces[3].iu, [3, 17, 4]);
    assert_eq!(connectivity.w_faces[3].mrlw, 1);
    assert_eq!(connectivity.w_faces[3].mrlw_orig, 1);
    assert_eq!(connectivity.w_faces[3].ngr, 1);
}

#[test]
fn icosahedron_loop_flags_match_mdloopf_sign_and_reset_rules() {
    let mut flags = [true; 7];
    earthmesh_mesh::apply_icosahedron_loop_flags_fortran(&mut flags, true, &[1, 2, 3, 4, 5, 0])
        .expect("valid initial loop flags");
    assert_eq!(flags, [true, true, true, true, true, false, false]);

    earthmesh_mesh::apply_icosahedron_loop_flags_fortran(&mut flags, false, &[-2, 7, 0])
        .expect("valid loop toggle flags");
    assert_eq!(flags, [true, false, true, true, true, false, true]);
}

#[test]
fn icosahedron_w_neighbor_derivation_matches_tri_neighbors_w_loops() {
    let mut connectivity = earthmesh_mesh::IcosahedronDiamondConnectivity {
        u_edges: vec![earthmesh_mesh::IcosahedronUEdge::default(); 7],
        w_faces: vec![earthmesh_mesh::IcosahedronWFace::default(); 7],
    };

    connectivity.w_faces[2].iu = [2, 3, 4];
    connectivity.w_faces[3].iw[0..3].copy_from_slice(&[2, 5, 6]);
    connectivity.w_faces[4].iw[0..3].copy_from_slice(&[6, 2, 5]);

    connectivity.u_edges[2].im = [10, 20];
    connectivity.u_edges[2].iw[0] = 2;
    connectivity.u_edges[2].iw[1] = 3;

    connectivity.u_edges[3].im = [20, 30];
    connectivity.u_edges[3].iw[0] = 4;
    connectivity.u_edges[3].iw[1] = 2;

    connectivity.u_edges[4].im = [30, 10];
    connectivity.u_edges[4].iw[0] = 2;
    connectivity.u_edges[4].iw[1] = 4;

    earthmesh_mesh::derive_icosahedron_w_neighbors_fortran(&mut connectivity)
        .expect("valid W-face neighbor derivation");

    assert_eq!(connectivity.w_faces[2].npoly, 3);
    assert_eq!(connectivity.w_faces[2].im, [10, 30, 20]);
    assert_eq!(connectivity.w_faces[2].iw, [3, 4, 4, 5, 6, 5, 6, 5, 6]);
}

#[test]
fn icosahedron_u_neighbor_derivation_matches_tri_neighbors_u_loop() {
    let mut connectivity = earthmesh_mesh::IcosahedronDiamondConnectivity {
        u_edges: vec![earthmesh_mesh::IcosahedronUEdge::default(); 21],
        w_faces: vec![earthmesh_mesh::IcosahedronWFace::default(); 13],
    };

    connectivity.u_edges[2].iw[0..2].copy_from_slice(&[3, 4]);
    connectivity.w_faces[3].iu = [2, 5, 6];
    connectivity.w_faces[3].mrlw = 9;
    connectivity.w_faces[4].iu = [7, 2, 8];
    connectivity.w_faces[4].mrlw = 4;

    connectivity.u_edges[5].iw[0..2].copy_from_slice(&[3, 9]);
    connectivity.u_edges[6].iw[0..2].copy_from_slice(&[10, 3]);
    connectivity.u_edges[7].iw[0..2].copy_from_slice(&[4, 11]);
    connectivity.u_edges[8].iw[0..2].copy_from_slice(&[12, 4]);

    connectivity.w_faces[9].iu = [5, 13, 14];
    connectivity.w_faces[10].iu = [15, 6, 16];
    connectivity.w_faces[11].iu = [17, 18, 7];
    connectivity.w_faces[12].iu = [8, 19, 20];

    earthmesh_mesh::derive_icosahedron_u_neighbors_fortran(&mut connectivity)
        .expect("valid U-edge neighbor derivation");

    assert_eq!(connectivity.u_edges[2].mrlu, 9);
    assert_eq!(connectivity.u_edges[2].iw, [3, 4, 9, 10, 11, 12]);
    assert_eq!(
        connectivity.u_edges[2].iu,
        [5, 6, 7, 8, 13, 14, 16, 15, 18, 17, 20, 19]
    );
}

#[test]
fn icosahedron_m_neighbor_derivation_matches_tri_neighbors_m_loop() {
    let mut u_edges = vec![earthmesh_mesh::IcosahedronUEdge::default(); 5];
    let mut w_faces = vec![earthmesh_mesh::IcosahedronWFace::default(); 11];

    u_edges[2].im = [10, 20];
    u_edges[2].iw[0..2].copy_from_slice(&[5, 6]);
    u_edges[2].iu[2] = 3;

    u_edges[3].im = [10, 30];
    u_edges[3].iw[0..2].copy_from_slice(&[7, 8]);
    u_edges[3].iu[2] = 4;

    u_edges[4].im = [10, 40];
    u_edges[4].iw[0..2].copy_from_slice(&[9, 10]);
    u_edges[4].iu[2] = 2;

    w_faces[5].npoly = 3;
    w_faces[7].npoly = 3;
    w_faces[9].npoly = 3;

    let m_neighbors =
        earthmesh_mesh::derive_icosahedron_m_neighbors_fortran(40, &u_edges, &w_faces)
            .expect("valid M-point polygon derivation");

    assert_eq!(m_neighbors[10].npoly, 3);
    assert_eq!(m_neighbors[10].iu, [2, 3, 4, 1, 1, 1, 1]);
    assert_eq!(m_neighbors[10].iw, [6, 8, 10, 1, 1, 1, 1]);
}

#[test]
fn icosahedron_tri_neighbors_wrapper_matches_manual_w_u_m_sequence() {
    let grid = earthmesh_mesh::icosahedron_initial_grid_fortran(1).expect("valid nxp grid");
    let mut manual = earthmesh_mesh::icosahedron_fill_diamonds_fortran(1)
        .expect("valid fill_diamond connectivity");
    earthmesh_mesh::derive_icosahedron_w_neighbors_fortran(&mut manual)
        .expect("valid manual W derivation");
    earthmesh_mesh::derive_icosahedron_u_neighbors_fortran(&mut manual)
        .expect("valid manual U derivation");
    let expected_m = earthmesh_mesh::derive_icosahedron_m_neighbors_fortran(
        grid.nmd,
        &manual.u_edges,
        &manual.w_faces,
    )
    .expect("valid manual M derivation");

    let mut wrapped = earthmesh_mesh::icosahedron_fill_diamonds_fortran(1)
        .expect("valid fill_diamond connectivity");
    let actual_m = earthmesh_mesh::derive_icosahedron_tri_neighbors_fortran(grid.nmd, &mut wrapped)
        .expect("valid integrated tri_neighbors derivation");

    assert_eq!(wrapped, manual);
    assert_eq!(actual_m, expected_m);
    assert_eq!(wrapped.w_faces[2].npoly, 3);
    assert_eq!(wrapped.u_edges[3].mrlu, 1);
}

#[test]
fn icosahedron_spring_topology_matches_spring_dynamics1_setup_tables() {
    let mut u_edges = vec![earthmesh_mesh::IcosahedronUEdge::default(); 7];
    let mut m_neighbors = vec![earthmesh_mesh::IcosahedronMPointNeighbors::default(); 21];

    u_edges[2].im = [10, 20];
    u_edges[2].iu[0..4].copy_from_slice(&[3, 4, 5, 6]);
    u_edges[3].im = [30, 10];

    m_neighbors[10].npoly = 2;
    m_neighbors[10].iu[0..2].copy_from_slice(&[2, 3]);

    let topology =
        earthmesh_mesh::icosahedron_spring_topology_fortran(20, &u_edges, &m_neighbors, 0.25)
            .expect("valid spring topology setup");

    assert_eq!(topology.edge_m_points[2], [10, 20]);
    assert_eq!(topology.edge_neighbor_u[2], [3, 4, 5, 6]);
    assert_eq!(topology.m_npoly[10], 2);
    assert_eq!(topology.m_u_edges[10], [2, 3, 1, 1, 1, 1, 1]);
    assert_eq!(
        topology.directions[10],
        [-0.25, 0.25, 0.0, 0.0, 0.0, 0.0, 0.0]
    );
}

#[test]
fn icosahedron_spring_iteration_matches_spring_dynamics1_edge_update() {
    let mut topology = earthmesh_mesh::IcosahedronSpringTopology {
        edge_m_points: vec![[1, 1]; 7],
        edge_neighbor_u: vec![[2, 2, 2, 2]; 7],
        m_npoly: vec![0; 6],
        m_u_edges: vec![[1; 7]; 6],
        directions: vec![[0.0; 7]; 6],
    };
    topology.edge_m_points[2] = [2, 3];
    topology.edge_m_points[3] = [2, 4];
    topology.edge_m_points[4] = [3, 4];
    topology.edge_m_points[5] = [2, 5];
    topology.edge_m_points[6] = [3, 5];
    topology.edge_neighbor_u[2] = [3, 4, 5, 6];
    topology.m_npoly[2] = 1;
    topology.m_npoly[3] = 1;
    topology.m_u_edges[2][0] = 2;
    topology.m_u_edges[3][0] = 2;
    topology.directions[2][0] = -0.5;
    topology.directions[3][0] = 0.5;

    let points = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(1.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 1.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 1.0),
        CartesianPoint::new(-1.0, 0.0, 0.0),
    ];

    let output = earthmesh_mesh::icosahedron_spring_iteration_fortran(&points, &topology, 2.0, 1.0)
        .expect("valid spring_dynamics1 iteration");

    let current_distance = 2.0_f64.sqrt();
    let frac_change = (2.0 - current_distance) / current_distance;
    approx_eq(output.edge_distances[2], current_distance, 1.0e-12);
    approx_eq(output.edge_displacements[2].x, -frac_change, 1.0e-12);
    approx_eq(output.edge_displacements[2].y, frac_change, 1.0e-12);

    let unnormalized_x = 1.0 + 0.5 * frac_change;
    let unnormalized_y = -0.5 * frac_change;
    let norm = (unnormalized_x * unnormalized_x + unnormalized_y * unnormalized_y).sqrt();
    approx_eq(output.updated_m_points[2].x, unnormalized_x / norm, 1.0e-12);
    approx_eq(output.updated_m_points[2].y, unnormalized_y / norm, 1.0e-12);
    approx_eq(magnitude(output.updated_m_points[2]), 1.0, 1.0e-12);
}

#[test]
fn icosahedron_spring_dynamics_repeats_iterations_and_records_max_ds() {
    let mut topology = earthmesh_mesh::IcosahedronSpringTopology {
        edge_m_points: vec![[1, 1]; 7],
        edge_neighbor_u: vec![[2, 2, 2, 2]; 7],
        m_npoly: vec![0; 6],
        m_u_edges: vec![[1; 7]; 6],
        directions: vec![[0.0; 7]; 6],
    };
    topology.edge_m_points[2] = [2, 3];
    topology.edge_m_points[3] = [2, 4];
    topology.edge_m_points[4] = [3, 4];
    topology.edge_m_points[5] = [2, 5];
    topology.edge_m_points[6] = [3, 5];
    topology.edge_neighbor_u[2] = [3, 4, 5, 6];
    topology.m_npoly[2] = 1;
    topology.m_npoly[3] = 1;
    topology.m_u_edges[2][0] = 2;
    topology.m_u_edges[3][0] = 2;
    topology.directions[2][0] = -0.5;
    topology.directions[3][0] = 0.5;

    let points = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(1.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 1.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 1.0),
        CartesianPoint::new(-1.0, 0.0, 0.0),
    ];

    let first = earthmesh_mesh::icosahedron_spring_iteration_fortran(&points, &topology, 2.0, 1.0)
        .expect("first iteration");
    let second = earthmesh_mesh::icosahedron_spring_iteration_fortran(
        &first.updated_m_points,
        &topology,
        2.0,
        1.0,
    )
    .expect("second iteration");

    let output =
        earthmesh_mesh::icosahedron_spring_dynamics1_fortran(&points, &topology, 2, 2.0, 1.0, 1)
            .expect("valid spring_dynamics1 wrapper");

    assert_eq!(output.updated_m_points, second.updated_m_points);
    assert_eq!(output.last_edge_displacements, second.edge_displacements);
    assert_eq!(output.diagnostic_max_displacements.len(), 2);
    assert_eq!(output.diagnostic_max_displacements[0].iteration, 1);
    assert_eq!(output.diagnostic_max_displacements[1].iteration, 2);
    assert!(output.diagnostic_max_displacements[0].max_displacement > 0.0);
    assert!(output.diagnostic_max_displacements[1].max_displacement > 0.0);
}

#[test]
fn icosahedron_relaxed_grid_wrapper_wires_initial_connectivity_and_spring() {
    let initial = earthmesh_mesh::icosahedron_initial_grid_fortran(1).expect("valid initial grid");

    let relaxed = earthmesh_mesh::icosahedron_relaxed_grid_fortran(1, 0, 1.0, 0.25, 100)
        .expect("valid integrated icosahedron grid");

    assert_eq!(relaxed.nmd, initial.nmd);
    assert_eq!(relaxed.nud, initial.nud);
    assert_eq!(relaxed.nwd, initial.nwd);
    assert_eq!(relaxed.impent, initial.impent);
    assert_eq!(relaxed.m_points, initial.m_points);
    assert_eq!(relaxed.connectivity.w_faces[2].npoly, 3);
    assert_eq!(relaxed.connectivity.u_edges[3].mrlu, 1);
    assert_eq!(relaxed.m_neighbors[3].npoly, 5);
    assert!(relaxed.spring.diagnostic_max_displacements.is_empty());
}

#[test]
#[ignore = "NXP64 post-spring parity is a long fixture check; run explicitly after icosahedron changes"]
fn icosahedron_relaxed_grid_matches_fortran_nxp64_glonw_glatw_fixture() {
    let relaxed = earthmesh_mesh::icosahedron_relaxed_grid_fortran(64, 5000, 1.0, 0.035, 100)
        .expect("valid NXP64 relaxed icosahedron grid");

    assert_eq!(relaxed.nmd, 40963);
    assert_eq!(relaxed.nud, 122881);
    assert_eq!(relaxed.nwd, 81921);

    let expected = [
        (2usize, 110.43421173095703, -90.0),
        (3usize, -35.99998092651367, -89.12533569335938),
        (4usize, 1.8392731362837367e-05, -88.49825286865234),
        (5usize, 13.142210960388184, -87.66699981689453),
        (64usize, 34.966949462890625, -28.71109962463379),
        (65usize, 35.00239944458008, -27.776504516601562),
        (40962usize, -72.00001525878906, 89.12533569335938),
    ];

    for (point_id, expected_lon, expected_lat) in expected {
        let lonlat = earthmesh_mesh::xyz_to_lonlat_degrees(relaxed.m_points[point_id]);
        approx_eq(lonlat.lat_degrees, expected_lat, 2.0e-4);
        if expected_lat.abs() < 89.999 {
            approx_eq(lonlat.lon_degrees, expected_lon, 2.0e-4);
        }
    }
}
