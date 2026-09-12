use std::fs;

use earthmesh_cli::{coordinate_types::LonLatPoint, unstructured_mesh_support::UnstructuredMesh};
use earthmesh_mesh::BoundaryOrders;

#[test]
fn fvcom_2dm_writer_preserves_canonical_ids_and_boundary_segments() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_fvcom_2dm_writer_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");

    let output = root.join("fvcom.2dm");
    let mesh = sample_mesh();
    let report =
        earthmesh_cli::fvcom_mesh_writer::write_fvcom_mesh_2dm(&output, &mesh, &[1, 2, 3, 1, 5])
            .expect("write fvcom 2dm");

    assert_eq!(report.output, output);
    assert_eq!(report.triangles, 2);
    assert_eq!(report.nodes, 4);
    assert_eq!(report.boundary_segments, 2);

    let content = fs::read_to_string(&report.output).expect("read 2dm");
    assert_eq!(
        content,
        concat!(
            "MESH2D\n",
            "MESHNAME \"FVCOM Mesh\"\n",
            "E3T 1 1 2 3 1\n",
            "E3T 2 1 3 4 1\n",
            "ND 1 113.000000 22.000000 0.000000\n",
            "ND 2 114.000000 22.000000 0.000000\n",
            "ND 3 114.000000 23.000000 0.000000\n",
            "ND 4 113.000000 23.000000 0.000000\n",
            "NS 1 -2 1\n",
            "NS -4 2\n",
        )
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn fvcom_mesh_save_wrapper_reads_patch_obc_and_writes_compatibility_result_path() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_fvcom_mesh_save_wrapper_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("result")).expect("create result dir");

    let orders = BoundaryOrders {
        bdy_order: vec![1, 2, 3, 1, 5],
        obc_order: vec![1, 2, 3, 1, 5],
        ibc_order: vec![1, 1, 1, 1, 1],
        rotation_start: None,
    };
    let obc = earthmesh_cli::obc_boundary_io::obc_boundary_output_path(&root, true);
    earthmesh_cli::obc_boundary_io::write_obc_boundary_netcdf(&obc, &orders)
        .expect("write patch obc");

    let report = earthmesh_cli::fvcom_mesh_writer::write_fvcom_mesh_save_outputs(
        &root,
        &sample_mesh(),
        true,
    )
    .expect("write fvcom outputs");

    assert_eq!(
        report.output,
        earthmesh_cli::fvcom_mesh_writer::fvcom_mesh_2dm_output_path(&root)
    );
    assert_eq!(report.boundary_segments, 2);
    let content = fs::read_to_string(&report.output).expect("read wrapper output");
    assert!(content.contains("MESHNAME \"FVCOM Mesh\"\n"));
    assert!(content.contains("NS 1 -2 1\n"));
    assert!(content.contains("NS -4 2\n"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn fvcom_ns_records_wrap_long_boundaries_without_repeated_tokens_on_one_line() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_fvcom_2dm_long_ns_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");

    let output = root.join("fvcom.2dm");
    let mesh = sample_mesh_with_nodes(13);
    let report = earthmesh_cli::fvcom_mesh_writer::write_fvcom_mesh_2dm(
        &output,
        &mesh,
        &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 1],
    )
    .expect("write fvcom 2dm");

    assert_eq!(report.boundary_segments, 1);
    let content = fs::read_to_string(&report.output).expect("read 2dm");
    let ns_lines = content
        .lines()
        .filter(|line| line.starts_with("NS "))
        .collect::<Vec<_>>();
    assert_eq!(ns_lines.len(), 2);
    assert!(ns_lines.iter().all(|line| line.matches("NS").count() == 1));
    assert_eq!(ns_lines[0], "NS 1 2 3 4 5 6 7 8 9 10 ");
    assert_eq!(ns_lines[1], "NS -11 1");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn fvcom_2dm_writer_rejects_connectivity_without_canonical_vertex_offset() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_fvcom_2dm_invalid_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");

    let mut mesh = sample_mesh();
    mesh.m_to_w[1] = [1, 2, 3];
    let err =
        earthmesh_cli::fvcom_mesh_writer::write_fvcom_mesh_2dm(root.join("bad.2dm"), &mesh, &[1])
            .expect_err("reject zero-offset connectivity");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(err.to_string().contains("Canonical index 2.."));

    let _ = fs::remove_dir_all(&root);
}

fn sample_mesh() -> UnstructuredMesh {
    UnstructuredMesh {
        m_points: vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint {
                lon: 113.5,
                lat: 22.3,
            },
            LonLatPoint {
                lon: 113.4,
                lat: 22.7,
            },
        ],
        w_points: vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint {
                lon: 113.0,
                lat: 22.0,
            },
            LonLatPoint {
                lon: 114.0,
                lat: 22.0,
            },
            LonLatPoint {
                lon: 114.0,
                lat: 23.0,
            },
            LonLatPoint {
                lon: 113.0,
                lat: 23.0,
            },
        ],
        m_to_w: vec![[1, 1, 1], [2, 3, 4], [2, 4, 5]],
        w_to_m: vec![vec![1], vec![1, 2], vec![1], vec![1, 2], vec![2]],
        n_w_to_m: vec![0, 2, 1, 2, 1],
    }
}

fn sample_mesh_with_nodes(nodes: usize) -> UnstructuredMesh {
    let mut mesh = sample_mesh();
    mesh.w_points = (0..=nodes)
        .map(|idx| LonLatPoint {
            lon: idx as f64,
            lat: 0.0,
        })
        .collect();
    mesh.w_to_m = vec![vec![]; nodes + 1];
    mesh.n_w_to_m = vec![0; nodes + 1];
    mesh
}

#[test]
fn final_fvcom_delivery_keeps_embedded_obc_and_rejects_missing_or_invalid_context_atomically() {
    let root = std::env::temp_dir().join(format!("earthmesh_fvcom_final_{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let input = root.join("selected.nc4");
    let output = root.join("final.2dm");
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(&input, &sample_mesh())
        .unwrap();
    fs::write(&output, "old delivery").unwrap();
    let write = || {
        earthmesh_cli::regional_gridfile_writers::write_fvcom_from_final_gridfile(&input, &output)
    };
    let missing = write().unwrap_err();
    assert!(missing.to_string().contains("OBC"), "{missing}");
    assert_eq!(fs::read_to_string(&output).unwrap(), "old delivery");
    for order in [
        vec![1, 2, 999, 1],
        vec![1, 2, 4, 1],
        vec![1, -2, 1],
        vec![2, 3, 1],
    ] {
        let mut file = netcdf::append(&input).unwrap();
        file.add_attribute("earthmesh_fvcom_obc_order", order)
            .unwrap();
        file.close().unwrap();
        assert!(write().is_err());
        assert_eq!(fs::read_to_string(&output).unwrap(), "old delivery");
    }
    let mut wrong_type = netcdf::append(&input).unwrap();
    wrong_type
        .add_attribute("earthmesh_fvcom_obc_order", "not integer context")
        .unwrap();
    wrong_type.close().unwrap();
    assert!(write().is_err());
    assert_eq!(fs::read_to_string(&output).unwrap(), "old delivery");
    let mut file = netcdf::append(&input).unwrap();
    file.add_attribute("earthmesh_fvcom_obc_order", vec![1_i32, 2, 3, 1, 5])
        .unwrap();
    file.close().unwrap();
    // Empty-but-present records no classified open chains, not complete BCs;
    // it must survive a byte copy and stay distinct from missing metadata.
    let empty = root.join("closed_coast.nc4");
    fs::copy(&input, &empty).unwrap();
    let mut file = netcdf::append(&empty).unwrap();
    file.add_attribute("earthmesh_fvcom_obc_order", Vec::<i32>::new())
        .unwrap();
    file.close().unwrap();
    let copied = root.join("closed_coast_copy.nc4");
    fs::copy(&empty, &copied).unwrap();
    assert_eq!(
        earthmesh_cli::obc_boundary_io::read_gridfile_obc_order(&copied).unwrap(),
        Some(Vec::new())
    );
    let empty_report = earthmesh_cli::regional_gridfile_writers::write_fvcom_from_final_gridfile(
        &copied,
        &root.join("closed_coast.2dm"),
    )
    .unwrap();
    assert_eq!(empty_report.boundary_segments, 0);
    let before = fs::read(&input).unwrap();
    let report = write().unwrap();
    assert_eq!(report.triangles, 2);
    assert_eq!(report.nodes, 4);
    assert_eq!(report.boundary_segments, 2);
    let text = fs::read_to_string(&output).unwrap();
    assert!(text.contains("NS 1 -2 1\n") && text.contains("NS -4 2\n"));
    // An unused physical node reaches the post-write count guard. Even after
    // creating a temporary .2dm, failure must preserve the prior delivery.
    let extra_input = root.join("extra_node.nc4");
    let mut extra = sample_mesh();
    extra.w_points.push(LonLatPoint {
        lon: 115.,
        lat: 24.,
    });
    extra.w_to_m.push(Vec::new());
    extra.n_w_to_m.push(0);
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(&extra_input, &extra)
        .unwrap();
    let mut file = netcdf::append(&extra_input).unwrap();
    file.add_attribute("earthmesh_fvcom_obc_order", vec![1_i32, 2, 3, 1, 5])
        .unwrap();
    file.close().unwrap();
    let error = earthmesh_cli::regional_gridfile_writers::write_fvcom_from_final_gridfile(
        &extra_input,
        &output,
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("changed physical counts"),
        "{error}"
    );
    assert_eq!(fs::read_to_string(&output).unwrap(), text);
    assert_eq!(fs::read(&input).unwrap(), before);
    assert!(
        earthmesh_cli::regional_gridfile_writers::write_fvcom_from_final_gridfile(&input, &input)
            .is_err()
    );
    assert_eq!(fs::read(&input).unwrap(), before);
    assert!(!fs::read_dir(&root).unwrap().any(|p| p
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains(".tmp-")));
    fs::remove_dir_all(root).unwrap();
}
