use std::fs;

fn read_i32(file: &netcdf::File, name: &str) -> Vec<i32> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<i32, _>(..)
        .unwrap_or_else(|err| panic!("read {name}: {err}"))
}

fn sample_source_mesh() -> earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
    earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 1.0, lat: 1.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 2.0, lat: 2.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 3.0, lat: 3.0 },
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
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 13.0,
                lat: 13.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 14.0,
                lat: 14.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 15.0,
                lat: 15.0,
            },
        ],
        m_to_w: vec![[1, 1, 1], [1, 1, 1], [2, 4, 5], [5, 6, 4]],
        w_to_m: vec![vec![1], vec![1], vec![2], vec![], vec![2, 3], vec![2, 3]],
        n_w_to_m: vec![0, 0, 1, 0, 2, 2],
    }
}

#[test]
fn land_runner_reads_inputs_and_writes_patchtype_and_final_gridfile() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mask_postproc_land_runner_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("result")).expect("create result dir");
    fs::create_dir_all(root.join("contain")).expect("create contain dir");
    fs::create_dir_all(root.join("patchtype")).expect("create patchtype dir");

    let plan = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &root, 9, "tri", "landmesh", true,
    )
    .expect("land plan");
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(
        &plan.source_gridfile,
        &sample_source_mesh(),
    )
    .expect("write source mesh");
    let contain = earthmesh_cli::contain_io::ContainMesh {
        ustr_id: vec![vec![0, 0], vec![0, 0], vec![2, 1], vec![1, 3]],
        ustr_ii: vec![vec![10, 20], vec![11, 20], vec![10, 21]],
        is_in_area_ustr: vec![0, 0, 1, -1],
    };
    earthmesh_cli::contain_io::write_contain_netcdf(&plan.contain_domain, &contain)
        .expect("write contain domain");

    let lon_vertex = (0..13).map(|idx| 90.0 + idx as f64).collect::<Vec<_>>();
    let lat_vertex = (0..23).map(|idx| 70.0 - idx as f64).collect::<Vec<_>>();
    let lon_i = (0..12).map(|idx| 90.5 + idx as f64).collect::<Vec<_>>();
    let lat_i = (0..22).map(|idx| 69.5 - idx as f64).collect::<Vec<_>>();
    let report = earthmesh_cli::mask_postproc_domain::run_mask_postproc_land_domain(
        &plan,
        earthmesh_cli::mask_postproc_types::MaskPostprocLandRunOptions {
            seaorland: &[vec![false, false], vec![false, false]],
            minlon_dm_area: 10,
            maxlat_dm_area: 20,
            nlons_dm_select: 2,
            nlats_dm_select: 2,
            lon_vertex: &lon_vertex,
            lat_vertex: &lat_vertex,
            lon_i: &lon_i,
            lat_i: &lat_i,
        },
    )
    .expect("run land mask_postproc domain");

    assert_eq!(report.final_gridfile.output, plan.result_gridfile);
    assert_eq!(
        report.patchtype.output,
        plan.patchtype_output.clone().unwrap()
    );
    assert_eq!(report.patchtypes.filled_ignored_land_pixels, 0);
    assert_eq!(report.final_gridfile.sjx_points, 3);

    let patch_file = netcdf::open(&report.patchtype.output).expect("open patchtype");
    assert_eq!(read_i32(&patch_file, "elmindex"), vec![3, 4, 3, 0]);

    let final_mesh = earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(
        &report.final_gridfile.output,
    )
    .expect("read final gridfile");
    assert_eq!(final_mesh.m_to_w[2], [2, 3, 4]);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn land_runner_rejects_non_land_plan_before_writing_outputs() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mask_postproc_land_runner_reject_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let plan = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &root,
        9,
        "tri",
        "earthmesh",
        false,
    )
    .expect("earth plan");

    let err = earthmesh_cli::mask_postproc_domain::run_mask_postproc_land_domain(
        &plan,
        earthmesh_cli::mask_postproc_types::MaskPostprocLandRunOptions {
            seaorland: &[vec![false, false], vec![false, false]],
            minlon_dm_area: 10,
            maxlat_dm_area: 20,
            nlons_dm_select: 2,
            nlats_dm_select: 2,
            lon_vertex: &[0.0],
            lat_vertex: &[0.0],
            lon_i: &[0.0],
            lat_i: &[0.0],
        },
    )
    .expect_err("non-land plan rejected");

    assert!(err.to_string().contains("landmesh"));
    assert!(!plan.result_gridfile.exists());

    let _ = fs::remove_dir_all(&root);
}
