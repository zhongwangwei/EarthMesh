#[test]
fn mpas_full_builder_composes_geometry_payload_and_writer() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mpas_full_builder_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let mesh = closed_fixture_mesh();
    let cellwidth = vec![100.0; mesh.w_points.len()];

    let mpas =
        earthmesh_cli::mpas_unstructured_mesh_builders::build_mpas_mesh_from_unstructured_one_based(&mesh, &cellwidth, 9, 3)
            .expect("build full MPAS mesh payload");

    assert_eq!(mpas.x_cell.len(), mesh.w_points.len());
    assert_eq!(mpas.x_vertex.len(), mesh.m_points.len());
    assert!(mpas.x_edge.len() > 2);
    assert_eq!(mpas.n_edges_on_cell[2], 3);
    assert_eq!(mpas.vertices_on_cell[2].len(), 10);
    assert!(mpas.vertices_on_cell[2][0] >= 1);
    assert!(mpas.cells_on_vertex[2].iter().all(|id| *id >= 0));
    assert!(mpas.area_cell[2] > 0.0);
    assert!(mpas.area_triangle[2] > 0.0);
    assert_eq!(mpas.kite_areas_on_vertex[2].len(), 3);
    assert_eq!(mpas.edges_on_edge[2].len(), 20);
    assert_eq!(mpas.weights_on_edge[2].len(), 20);
    assert!(mpas.nominal_min_dc > 0.0);

    let output = root.join("MPASOUT_NXP0009_global.nc4");
    let report =
        earthmesh_cli::write_mpas_mesh_netcdf(&output, &mpas).expect("write full MPAS mesh");
    assert_eq!(report.n_cells, mesh.w_points.len() - 1);
    assert_eq!(report.n_vertices, mesh.m_points.len() - 1);
    assert_eq!(report.output, output);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn icon_writer_matches_the_official_core_grid_contract() {
    let root =
        std::env::temp_dir().join(format!("earthmesh_cli_icon_writer_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create ICON root");
    let mesh = canonical_single_placeholder_fixture_mesh();
    let gridfile = root.join("gridfile.nc4");
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(&gridfile, &mesh)
        .expect("write ICON source gridfile");
    let output = root.join("icon_grid.nc");
    let report = earthmesh_cli::mpas_gridfile_writers::write_standard_icon_from_gridfile(
        &gridfile, &output, 9,
    )
    .expect("write ICON grid");
    assert_eq!((report.cells, report.vertices, report.edges), (4, 4, 6));
    assert!(report.global_grid);

    let file = netcdf::open(&output).expect("open ICON grid");
    for (name, len) in [
        ("cell", 4),
        ("vertex", 4),
        ("edge", 6),
        ("nc", 2),
        ("nv", 3),
        ("ne", 6),
    ] {
        assert_eq!(file.dimension(name).expect(name).len(), len);
    }
    for name in [
        "clon",
        "clat",
        "edge_of_cell",
        "vertex_of_cell",
        "adjacent_cell_of_edge",
        "cells_of_vertex",
        "orientation_of_normal",
        "refin_c_ctrl",
    ] {
        assert!(
            file.variable(name).is_some(),
            "missing ICON variable {name}"
        );
    }
    let radius: f64 = file
        .attribute("semi_major_axis")
        .expect("semi_major_axis")
        .value()
        .expect("read semi_major_axis")
        .try_into()
        .expect("f64 semi_major_axis");
    assert_eq!(radius, earthmesh_cli::ICON_SPHERE_RADIUS_METERS);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn mpas_full_builder_restores_canonical_single_placeholder_payload_shape() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mpas_full_canonical_placeholder_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let mesh = canonical_single_placeholder_fixture_mesh();
    let cellwidth = vec![100.0; mesh.w_points.len()];

    let mpas =
        earthmesh_cli::mpas_unstructured_mesh_builders::build_mpas_mesh_from_unstructured_one_based(&mesh, &cellwidth, 9, 3)
            .expect("build MPAS from Canonical single-placeholder mesh");
    assert_eq!(mpas.x_cell.len(), mesh.w_points.len());
    assert_eq!(mpas.x_vertex.len(), mesh.m_points.len());

    let output = root.join("MPASOUT_NXP0009_global.nc4");
    let report =
        earthmesh_cli::write_mpas_mesh_netcdf(&output, &mpas).expect("write full MPAS mesh");
    assert_eq!(report.n_cells, mesh.w_points.len() - 1);
    assert_eq!(report.n_vertices, mesh.m_points.len() - 1);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn mpas_full_builder_rejects_bad_cellwidth_length() {
    let mesh = closed_fixture_mesh();
    let err =
        earthmesh_cli::mpas_unstructured_mesh_builders::build_mpas_mesh_from_unstructured_one_based(&mesh, &[100.0], 9, 3)
            .expect_err("bad cellwidth rejected");
    assert!(err.to_string().contains("cellwidth length"));
}

fn canonical_single_placeholder_fixture_mesh(
) -> earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
    let compatibility = closed_fixture_mesh();
    earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
        m_points: std::iter::once(compatibility.m_points[0])
            .chain(compatibility.m_points[2..].iter().copied())
            .collect(),
        w_points: std::iter::once(compatibility.w_points[0])
            .chain(compatibility.w_points[2..].iter().copied())
            .collect(),
        m_to_w: std::iter::once([1, 1, 1])
            .chain(compatibility.m_to_w[2..].iter().copied())
            .collect(),
        w_to_m: std::iter::once(vec![1])
            .chain(compatibility.w_to_m[2..].iter().cloned())
            .collect(),
        n_w_to_m: std::iter::once(0)
            .chain(compatibility.n_w_to_m[2..].iter().copied())
            .collect(),
    }
}

fn closed_fixture_mesh() -> earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
    earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.2, lat: 0.2 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.8, lat: 0.2 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.2, lat: 0.8 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.8, lat: 0.8 },
        ],
        w_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 1.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 1.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 1.0, lat: 1.0 },
        ],
        m_to_w: vec![
            [1, 1, 1],
            [1, 1, 1],
            [2, 3, 4],
            [2, 3, 5],
            [2, 4, 5],
            [3, 4, 5],
        ],
        w_to_m: vec![
            vec![1],
            vec![1],
            vec![2, 3, 4],
            vec![2, 3, 5],
            vec![2, 4, 5],
            vec![3, 4, 5],
        ],
        n_w_to_m: vec![1, 1, 3, 3, 3, 3],
    }
}

#[test]
fn mpas_full_file_pipeline_reads_inputs_and_writes_mesh_plus_graph() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mpas_full_pipeline_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("result")).expect("create result root");
    let gridfile = root.join("result/gridfile_NXP0009_hex.nc4");
    let cellwidth_file = root.join("result/cellwidth_NXP0009_global.nc4");
    let mesh_output = root.join("result/MPASOUT_NXP0009_global.nc4");
    let graph_output = root.join("result/MPASOUT_NXP0009_global.graph.info");
    let mesh = closed_fixture_mesh();
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(&gridfile, &mesh)
        .expect("write gridfile");
    earthmesh_cli::mesh_metric_writers::write_cellwidth_netcdf(
        &cellwidth_file,
        &earthmesh_cli::mesh_metric_writers::CellwidthMesh {
            cell_points: mesh.w_points.clone(),
            cellwidth: vec![100.0; mesh.w_points.len()],
        },
    )
    .expect("write cellwidth");

    let report = earthmesh_cli::gridfile_output_writers::write_mpas_mesh_from_netcdf_inputs(
        &gridfile,
        &cellwidth_file,
        &mesh_output,
        &graph_output,
        9,
        3,
    )
    .expect("write full MPAS from file inputs");

    assert_eq!(report.mesh.output, mesh_output);
    assert_eq!(report.graph_info.output, graph_output);
    assert!(report.graph_info.n_cells_written > 0);
    assert!(std::fs::read_to_string(&report.graph_info.output)
        .expect("read graph")
        .starts_with("         "));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn mpas_full_builder_nominal_min_dc_uses_canonical_integer_nxp_division() {
    let mesh = closed_fixture_mesh();
    let cellwidth = vec![100.0; mesh.w_points.len()];

    let mpas =
        earthmesh_cli::mpas_unstructured_mesh_builders::build_mpas_mesh_from_unstructured_one_based(&mesh, &cellwidth, 112, 4)
            .expect("build MPAS with non-divisible NXP");

    let expected =
        (7680 / 112 / 2_usize.pow(3)) as f64 / earthmesh_core::EARTH_RADIUS_METERS * 1000.0;
    assert_eq!(mpas.nominal_min_dc, expected);
}

#[test]
fn mpas_full_builder_supports_limited_area_boundary_metrics_and_weights() {
    let mesh = regional_two_cell_patch();
    let cellwidth = vec![100.0; mesh.w_points.len()];
    let mpas = earthmesh_cli::mpas_unstructured_mesh_builders::build_mpas_mesh_from_unstructured_one_based(
        &mesh,
        &cellwidth,
        9,
        1,
    )
    .expect("build regional MPAS payload");

    let topology = earthmesh_cli::mpas_topology::check_mpas_mesh_topology(&mpas);
    assert!(topology.is_consistent(), "{:?}", topology.violations);
    assert_eq!(topology.euler_characteristic, 1);
    assert_eq!(topology.boundary_edges, 4);

    for edge in 1..mpas.cells_on_edge.len() {
        assert!(mpas.dv_edge[edge].is_finite() && mpas.dv_edge[edge] > 0.0);
        assert!(mpas.dc_edge[edge].is_finite() && mpas.dc_edge[edge] > 0.0);
        assert!(mpas.angle_edge[edge].is_finite());
        if mpas.cells_on_edge[edge][1] == 0 {
            assert!((mpas.dc_edge[edge] - 3.0_f64.sqrt() * mpas.dv_edge[edge]).abs() < 1.0e-12);
            assert_eq!(mpas.n_edges_on_edge[edge], 2);
            assert!(mpas.weights_on_edge[edge][..2]
                .iter()
                .all(|weight| weight.is_finite()));
        }
    }

    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_regional_mpas_full_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create regional MPAS root");
    let gridfile = root.join("gridfile.nc4");
    let output = root.join("regional_mpas.nc4");
    let graph = root.join("regional.graph.info");
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(&gridfile, &mesh)
        .expect("write regional source gridfile");
    let report = earthmesh_cli::mpas_gridfile_writers::write_standard_mpas_from_gridfile(
        &gridfile, &output, &graph, 9,
    )
    .expect("write full regional MPAS product");
    assert_eq!(report.mesh.n_cells, 2);
    assert_eq!(report.mesh.n_edges, 5);
    assert!(report.mesh.output.is_file());
    assert!(report.graph_info.output.is_file());
    let file = netcdf::open(&report.mesh.output).expect("open regional MPAS product");
    let cells_on_edge = file
        .variable("cellsOnEdge")
        .expect("cellsOnEdge")
        .get_values::<i32, _>(..)
        .expect("read cellsOnEdge");
    assert_eq!(cells_on_edge.iter().filter(|cell| **cell == 0).count(), 4);
    let boundary_vertex = file
        .variable("boundaryVertex")
        .expect("boundaryVertex")
        .get_values::<i32, _>(..)
        .expect("read boundaryVertex");
    assert_eq!(boundary_vertex, vec![0, 0, 1, 1]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn icon_writer_serializes_limited_area_open_vertex_fans() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_regional_icon_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create regional ICON root");
    let output = root.join("regional_icon.nc");
    let mesh = canonical_single_placeholder_fixture_mesh();
    let cellwidth = vec![100.0; mesh.w_points.len()];
    let global = earthmesh_cli::mpas_unstructured_mesh_builders::build_mpas_mesh_from_unstructured_one_based(
        &mesh,
        &cellwidth,
        9,
        1,
    )
    .expect("build global MPAS payload");
    let mut keep = vec![false; global.lat_cell.len()];
    keep[1..=3].fill(true);
    let regional = earthmesh_cli::mpas_topology::subset_mpas_mesh(&global, &keep)
        .expect("subset limited-area MPAS payload");
    let report = earthmesh_cli::write_icon_grid_netcdf(&output, &regional)
        .expect("write regional ICON product");
    assert!(!report.global_grid);

    let file = netcdf::open(&output).expect("open regional ICON product");
    let cells = file.dimension("cell").expect("cell").len();
    let vertices = file.dimension("vertex").expect("vertex").len();
    let cells_of_vertex = file
        .variable("cells_of_vertex")
        .expect("cells_of_vertex")
        .get_values::<i32, _>(..)
        .expect("read cells_of_vertex");
    let edges_of_vertex = file
        .variable("edges_of_vertex")
        .expect("edges_of_vertex")
        .get_values::<i32, _>(..)
        .expect("read edges_of_vertex");
    assert_eq!(cells, report.cells);
    assert_eq!(vertices, report.vertices);
    assert_eq!(cells_of_vertex.len(), 6 * vertices);
    assert_eq!(edges_of_vertex.len(), 6 * vertices);
    assert!(cells_of_vertex.contains(&-1));
    assert!(edges_of_vertex.contains(&-1));

    let _ = std::fs::remove_dir_all(&root);
}

fn regional_two_cell_patch() -> earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
    let point = |lon, lat| earthmesh_cli::coordinate_types::LonLatPoint { lon, lat };
    // One physical placeholder row. Connectivity retains Canonical ids (2+);
    // the MPAS builder inserts its compatibility row before calculation.
    earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
        m_points: vec![
            point(0.0, 0.0),
            point(0.0, 0.0),
            point(1.0, 0.0),
            point(0.5, 1.0),
            point(0.5, -1.0),
        ],
        w_points: vec![point(0.0, 0.0), point(0.4, 0.3), point(0.6, -0.3)],
        m_to_w: vec![[1, 1, 1], [2, 3, 1], [2, 3, 1], [2, 1, 1], [3, 1, 1]],
        w_to_m: vec![vec![1], vec![2, 3, 4], vec![3, 2, 5]],
        n_w_to_m: vec![0, 3, 3],
    }
}
