use std::fs;

#[test]
fn read_mask_postproc_domain_inputs_loads_gridfile_contain_and_layout_from_plan_paths() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mask_postproc_inputs_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("result")).expect("create result dir");
    fs::create_dir_all(root.join("contain")).expect("create contain dir");

    let plan = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &root, 5, "tri", "landmesh", false,
    )
    .expect("mask postproc plan");
    let mesh = earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 1.0, lat: 1.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 2.0, lat: 2.0 },
        ],
        w_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 10.0,
                lat: 10.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 11.0,
                lat: 11.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 12.0,
                lat: 12.0,
            },
        ],
        m_to_w: vec![[1, 1, 1], [1, 2, 3], [2, 3, 1]],
        w_to_m: vec![vec![1], vec![2, 3], vec![2, 3]],
        n_w_to_m: vec![0, 2, 2],
    };
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(
        &plan.source_gridfile,
        &mesh,
    )
    .expect("write source gridfile");

    let contain = earthmesh_cli::contain_io::ContainMesh {
        ustr_id: vec![vec![0, 0], vec![1, 1], vec![1, 2]],
        ustr_ii: vec![vec![10, 20], vec![11, 20]],
        is_in_area_ustr: vec![0, 1, -1],
    };
    earthmesh_cli::contain_io::write_contain_netcdf(&plan.contain_domain, &contain)
        .expect("write contain domain");

    let inputs = earthmesh_cli::mask_postproc_layout::read_mask_postproc_domain_inputs(&plan)
        .expect("read mask postproc domain inputs");

    assert_eq!(inputs.contain, contain);
    assert_eq!(inputs.is_in_domain_ustr, vec![0, 1, -1]);
    assert_eq!(inputs.layout.ustr_points, 3);
    assert_eq!(inputs.layout.ustr_bounds, 3);
    assert_eq!(inputs.layout.center_points, mesh.m_points);
    assert_eq!(inputs.layout.vertex_points, mesh.w_points);
    assert_eq!(
        inputs.layout.center_neighbors,
        vec![vec![1, 1, 1], vec![1, 2, 3], vec![2, 3, 1]]
    );
    assert_eq!(inputs.layout.vertex_neighbor_counts, vec![0, 2, 2]);

    let _ = fs::remove_dir_all(&root);
}
