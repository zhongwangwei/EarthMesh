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

/// A Method-C gridfile can carry two leading placeholder rows, which the
/// single-placeholder `write_fvcom_mesh_2dm` rejects with
/// `triangle 1 canonicals vertex 1`. The standard delivery path must accept it.
#[test]
fn standard_fvcom_delivery_accepts_two_placeholder_gridfiles() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_fvcom_two_placeholder_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");

    let gridfile = root.join("gridfile.nc4");
    let mesh = two_placeholder_mesh();
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(&gridfile, &mesh)
        .expect("write two-placeholder gridfile");

    let output = root.join("FVCOM_two_placeholder.2dm");
    let report =
        earthmesh_cli::regional_gridfile_writers::write_standard_fvcom_from_gridfile(
            &gridfile, &output,
        )
        .expect("two-placeholder gridfile must deliver FVCOM");

    assert_eq!(report.triangles, 2);
    assert_eq!(report.nodes, 4);
    let content = fs::read_to_string(&output).expect("read 2dm");
    // Elements are renumbered 1-based and reference renumbered 1-based nodes;
    // neither placeholder row may leak into the element table.
    assert!(content.contains("E3T 1 1 2 3 1\n"), "{content}");
    assert!(content.contains("E3T 2 1 3 4 1\n"), "{content}");
    assert_eq!(content.matches("E3T ").count(), 2, "{content}");
    assert_eq!(content.matches("ND ").count(), 4, "{content}");

    let _ = fs::remove_dir_all(&root);
}

/// Same geometry as [`sample_mesh`], shifted so both row 0 and row 1 are
/// explicit `(0, 0)` placeholders and canonical ids equal row numbers.
fn two_placeholder_mesh() -> UnstructuredMesh {
    let origin = LonLatPoint { lon: 0.0, lat: 0.0 };
    UnstructuredMesh {
        m_points: vec![
            origin,
            origin,
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
            origin,
            origin,
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
        m_to_w: vec![[1, 1, 1], [1, 1, 1], [2, 3, 4], [2, 4, 5]],
        w_to_m: vec![vec![1], vec![1], vec![2], vec![2, 3], vec![2, 3], vec![3]],
        n_w_to_m: vec![0, 0, 1, 2, 2, 1],
    }
}
