use std::fs;

fn read_i32(file: &netcdf::File, name: &str) -> Vec<i32> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<i32, _>(..)
        .unwrap_or_else(|err| panic!("read {name}: {err}"))
}

fn sample_ocean_source_mesh() -> earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
    let mut m_points = vec![earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 }; 8];
    for (idx, point) in m_points.iter_mut().enumerate() {
        point.lon = idx as f64;
        point.lat = idx as f64 * 0.5;
    }
    let mut w_points =
        vec![earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 }; 14];
    for (idx, point) in w_points.iter_mut().enumerate() {
        point.lon = 100.0 + idx as f64;
        point.lat = 40.0 + idx as f64 * 0.25;
    }

    let mut m_to_w = vec![[1, 1, 1]; 8];
    m_to_w[2] = [10, 11, 2];
    m_to_w[3] = [11, 12, 3];
    m_to_w[4] = [12, 13, 4];
    m_to_w[5] = [13, 10, 5];

    let mut w_to_m = vec![vec![1; 7]; 14];
    w_to_m[2] = vec![2, 1, 1, 1, 1, 1, 1];
    w_to_m[3] = vec![3, 1, 1, 1, 1, 1, 1];
    w_to_m[4] = vec![4, 1, 1, 1, 1, 1, 1];
    w_to_m[5] = vec![5, 1, 1, 1, 1, 1, 1];
    w_to_m[10] = vec![2, 5, 6, 7, 1, 1, 1];
    w_to_m[11] = vec![2, 3, 6, 7, 1, 1, 1];
    w_to_m[12] = vec![3, 4, 6, 7, 1, 1, 1];
    w_to_m[13] = vec![4, 5, 6, 7, 1, 1, 1];
    let mut n_w_to_m = vec![0; 14];
    n_w_to_m[2] = 1;
    n_w_to_m[3] = 1;
    n_w_to_m[4] = 1;
    n_w_to_m[5] = 1;
    n_w_to_m[10] = 5;
    n_w_to_m[11] = 5;
    n_w_to_m[12] = 5;
    n_w_to_m[13] = 5;

    earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
        m_points,
        w_points,
        m_to_w,
        w_to_m,
        n_w_to_m,
    }
}

#[test]
fn ocean_runner_reads_inputs_writes_final_gridfile_and_tri_boundaries() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mask_postproc_ocean_runner_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("result")).expect("create result dir");
    fs::create_dir_all(root.join("contain")).expect("create contain dir");

    let plan = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &root,
        9,
        "tri",
        "oceanmesh",
        false,
    )
    .expect("ocean plan");
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_refine_levels(
        &plan.source_gridfile,
        &sample_ocean_source_mesh(),
        Some(&[0, 1, 2, 3, 4, 5, 6, 7]),
        Some(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]),
    )
    .expect("write source mesh");
    let contain = earthmesh_cli::contain_io::ContainMesh {
        ustr_id: vec![
            vec![0, 0, 1],
            vec![0, 0, 1],
            vec![1, 0, 1],
            vec![1, 0, 1],
            vec![1, 0, 1],
            vec![1, 0, 1],
            vec![0, 0, 1],
            vec![0, 0, 1],
        ],
        ustr_ii: vec![vec![0, 0, 0]],
        is_in_area_ustr: vec![0, -1, 1, 1, 1, 1, -1, -1],
    };
    earthmesh_cli::contain_io::write_contain_netcdf(&plan.contain_domain, &contain)
        .expect("write contain domain");

    let report = earthmesh_cli::mask_postproc_domain::run_mask_postproc_ocean_domain(
        &plan,
        earthmesh_cli::mask_postproc_types::MaskPostprocOceanRunOptions {
            mask_sea_ratio: 0.5,
            num_vertex: 1,
        },
    )
    .expect("run ocean mask_postproc domain");

    assert_eq!(report.final_gridfile.output, plan.result_gridfile);
    assert_eq!(report.final_gridfile.sjx_points, 6);
    assert_eq!(
        report.obc.as_ref().expect("obc report").output,
        plan.obc_output.clone().unwrap()
    );
    assert_eq!(
        report.obcv2.as_ref().expect("obcv2 report").output,
        plan.obcv2_output.clone().unwrap()
    );
    assert_eq!(report.renewal.renewed.points_next, 5);
    assert_eq!(report.finalization.vertex_reindex.vertex_mapping[10], 6);

    let final_mesh = earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(
        &report.final_gridfile.output,
    )
    .expect("read final gridfile");
    assert_eq!(final_mesh.m_to_w[2], [6, 7, 2]);
    assert_eq!(final_mesh.m_to_w[5], [9, 6, 5]);
    let final_points = earthmesh_cli::grid_quality_pipeline::read_gridfile_mesh_points(
        &report.final_gridfile.output,
    )
    .expect("read final points");
    assert_eq!(final_points.m_refine_level.len(), final_mesh.m_points.len());
    assert_eq!(final_points.w_refine_level.len(), final_mesh.w_points.len());
    assert!(final_points.m_refine_level.contains(&5));
    assert!(final_points.w_refine_level.contains(&13));

    let obc_file = netcdf::open(report.obc.unwrap().output).expect("open obc");
    assert_eq!(read_i32(&obc_file, "bdy_order"), vec![1, 6, 7, 8, 9]);
    assert_eq!(read_i32(&obc_file, "obc_order"), vec![1, 1, 1, 1, 1]);
    assert_eq!(read_i32(&obc_file, "ibc_order"), vec![1, 6, 7, 8, 9]);

    let obcv2_file = netcdf::open(report.obcv2.unwrap().output).expect("open obcv2");
    assert_eq!(read_i32(&obcv2_file, "n_close_curve"), vec![4]);
    assert_eq!(
        read_i32(&obcv2_file, "close_curve"),
        vec![10, 11, 12, 13, 1]
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn ocean_runner_rejects_non_ocean_plan_before_writing_outputs() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mask_postproc_ocean_runner_reject_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let plan = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &root, 9, "tri", "landmesh", false,
    )
    .expect("land plan");

    let err = earthmesh_cli::mask_postproc_domain::run_mask_postproc_ocean_domain(
        &plan,
        earthmesh_cli::mask_postproc_types::MaskPostprocOceanRunOptions {
            mask_sea_ratio: 0.5,
            num_vertex: 1,
        },
    )
    .expect_err("non-ocean plan rejected");

    assert!(err.to_string().contains("oceanmesh"));
    assert!(!plan.result_gridfile.exists());

    let _ = fs::remove_dir_all(&root);
}

/// The carved domain's boundary topology is counted for hex too, not only tri.
///
/// The tri branch of the ocean renewal walks the boundary because isolated-ocean
/// removal needs it; the hex branch returns `boundary: None` and does no renewal
/// at all. That is a difference in what each carve *needs*, not in what each
/// carve *has* -- the arrays the walk takes exist either way -- so the topology
/// count computes its own rather than being available for one mode_grid only.
///
/// Written because the hex path was code-enabled and unmeasured: the fixture to
/// hand lacked a contain file and the carve stopped before reaching it, so
/// "the code path exists" was all that could honestly be said. This says more.
/// A mesh whose corners each touch three cells, which is what hex requires.
///
/// `sample_ocean_source_mesh` is tri-shaped: its M point 6 sits in four W
/// cells, and the hex layout refuses that by name. This is the dual of a
/// tetrahedron -- four cells, four corners, every corner in exactly three
/// cells -- which is the smallest arrangement that satisfies the rule.
fn sample_hex_source_mesh() -> earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
    let mut m_points = vec![earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 }; 6];
    for (index, point) in m_points.iter_mut().enumerate().skip(2) {
        let angle = (index as f64 - 2.0) * std::f64::consts::TAU / 4.0;
        point.lon = 10.0 * angle.cos();
        point.lat = 10.0 * angle.sin();
    }
    let mut w_points = vec![earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 }; 6];
    for (index, point) in w_points.iter_mut().enumerate().skip(2) {
        let angle = (index as f64 - 2.0) * std::f64::consts::TAU / 4.0 + 0.4;
        point.lon = 6.0 * angle.cos();
        point.lat = 6.0 * angle.sin();
    }

    // Corner -> its three cells, and cell -> its three corners.
    let mut m_to_w = vec![[1, 1, 1]; 6];
    m_to_w[2] = [2, 3, 4];
    m_to_w[3] = [2, 3, 5];
    m_to_w[4] = [2, 4, 5];
    m_to_w[5] = [3, 4, 5];
    let mut w_to_m = vec![vec![1; 7]; 6];
    w_to_m[2] = vec![2, 3, 4, 1, 1, 1, 1];
    w_to_m[3] = vec![2, 3, 5, 1, 1, 1, 1];
    w_to_m[4] = vec![2, 4, 5, 1, 1, 1, 1];
    w_to_m[5] = vec![3, 4, 5, 1, 1, 1, 1];
    let mut n_w_to_m = vec![0; 6];
    for cell in 2..6 {
        n_w_to_m[cell] = 3;
    }

    earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
        m_points,
        w_points,
        m_to_w,
        w_to_m,
        n_w_to_m,
    }
}

#[test]
fn the_carved_boundary_is_counted_for_hex_as_well_as_tri() {
    let mut counts = Vec::new();
    for mode_grid in ["tri", "hex"] {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_cli_mask_postproc_ocean_topology_{mode_grid}_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("result")).expect("create result dir");
        fs::create_dir_all(root.join("contain")).expect("create contain dir");

        let plan = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
            &root,
            9,
            mode_grid,
            "oceanmesh",
            false,
        )
        .expect("ocean plan");
        if mode_grid == "tri" {
            earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_refine_levels(
                &plan.source_gridfile,
                &sample_ocean_source_mesh(),
                Some(&[0, 1, 2, 3, 4, 5, 6, 7]),
                Some(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]),
            )
            .expect("write source mesh");
        } else {
            earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_refine_levels(
                &plan.source_gridfile,
                &sample_hex_source_mesh(),
                Some(&[0, 1, 2, 3, 4, 5]),
                Some(&[0, 1, 2, 3, 4, 5]),
            )
            .expect("write source mesh");
        }
        // The unstructured points a carve walks are the mesh's M points under
        // tri and its W points under hex, so the containment array is a
        // different length for each -- 8 against 14 here. Sizing it for tri and
        // reusing it was the first attempt, and the runner refused it by name
        // rather than reading past the end.
        let (ustr_points, in_area): (usize, Vec<i32>) = if mode_grid == "tri" {
            (8, vec![0, -1, 1, 1, 1, 1, -1, -1])
        } else {
            // Three of the four cells are the domain, so the boundary runs
            // around the fourth and has three corners -- the fewest a ring can
            // have. Two in-domain cells were the first attempt and left a
            // two-point curve, which the walker refuses by name.
            (6, vec![0, -1, 1, 1, 1, -1])
        };
        let contain = earthmesh_cli::contain_io::ContainMesh {
            ustr_id: (0..ustr_points)
                .map(|point| vec![i32::from(in_area[point] > 0), 0, 1])
                .collect(),
            ustr_ii: vec![vec![0, 0, 0]],
            is_in_area_ustr: in_area,
        };
        earthmesh_cli::contain_io::write_contain_netcdf(&plan.contain_domain, &contain)
            .expect("write contain domain");

        let report = earthmesh_cli::mask_postproc_domain::run_mask_postproc_ocean_domain(
            &plan,
            earthmesh_cli::mask_postproc_types::MaskPostprocOceanRunOptions {
                mask_sea_ratio: 0.5,
                num_vertex: 1,
            },
        )
        .expect("run ocean mask_postproc domain");

        // Only tri computes the boundary for its own purposes, so this is the
        // half that would go uncounted if the topology check took the renewal's
        // word for whether a boundary exists.
        assert_eq!(
            report.renewal.boundary.is_some(),
            mode_grid == "tri",
            "{mode_grid}: the renewal's own boundary"
        );
        counts.push((
            mode_grid,
            report
                .boundary_topology
                .unwrap_or_else(|| panic!("{mode_grid}: no topology counted")),
        ));
        let _ = fs::remove_dir_all(&root);
    }

    // The same domain either way: one carved region, no lakes.
    assert_eq!(counts[0].1, (1, 0), "tri: {counts:?}");
    assert_eq!(counts[1].1, (1, 0), "hex: {counts:?}");
}
