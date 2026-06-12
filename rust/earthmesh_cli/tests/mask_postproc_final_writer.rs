use std::fs;

#[test]
fn write_mask_postproc_final_gridfile_uses_plan_result_path_and_legacy_schema() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mask_postproc_final_writer_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("result")).expect("create result dir");

    let plan = earthmesh_cli::plan_mask_postproc_domain_io(&root, 9, "tri", "landmesh", true)
        .expect("mask postproc plan");
    let layout = earthmesh_cli::MaskPostprocLayout {
        ustr_points: 4,
        ustr_bounds: 6,
        center_points: vec![
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 1.0, lat: 1.0 },
            earthmesh_cli::LonLatPoint { lon: 2.0, lat: 2.0 },
            earthmesh_cli::LonLatPoint { lon: 3.0, lat: 3.0 },
        ],
        vertex_points: vec![
            earthmesh_cli::LonLatPoint {
                lon: 10.0,
                lat: 10.0,
            },
            earthmesh_cli::LonLatPoint {
                lon: 11.0,
                lat: 11.0,
            },
            earthmesh_cli::LonLatPoint {
                lon: 12.0,
                lat: 12.0,
            },
            earthmesh_cli::LonLatPoint {
                lon: 13.0,
                lat: 13.0,
            },
            earthmesh_cli::LonLatPoint {
                lon: 14.0,
                lat: 14.0,
            },
            earthmesh_cli::LonLatPoint {
                lon: 15.0,
                lat: 15.0,
            },
        ],
        center_neighbors: vec![vec![1, 1, 1], vec![1, 1, 1], vec![2, 4, 5], vec![5, 6, 4]],
        vertex_neighbors: vec![vec![1], vec![1], vec![2], vec![], vec![2, 3], vec![2, 3]],
        center_neighbor_counts: vec![0, 0, 3, 3],
        vertex_neighbor_counts: vec![0, 0, 1, 0, 2, 2],
    };
    let is_in_domain = vec![0, 0, 1, -1];

    let report = earthmesh_cli::write_mask_postproc_final_gridfile(&plan, &layout, &is_in_domain)
        .expect("write final mask_postproc gridfile");

    assert_eq!(report.output, plan.result_gridfile);
    assert!(report.output.exists());
    assert_eq!(report.sjx_points, 3);
    assert_eq!(report.lbx_points, 5);

    let mesh = earthmesh_cli::read_unstructured_mesh_netcdf(&report.output)
        .expect("read final mask_postproc gridfile");
    assert_eq!(mesh.m_to_w[2], [2, 3, 4]);
    assert_eq!(mesh.n_w_to_m, vec![0, 0, 1, 1, 1]);

    let _ = fs::remove_dir_all(&root);
}
