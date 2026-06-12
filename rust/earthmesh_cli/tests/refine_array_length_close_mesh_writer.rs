use std::fs;

use earthmesh_cli::{read_close_mesh_netcdf, write_refine_array_length_close_meshes, LonLatPoint};
use earthmesh_mesh::{
    BoundaryClosedCurves, BoundaryConnection, RefineArrayLengthCalculation, RefineArrayLengthHalo,
};

fn sample_calculation() -> RefineArrayLengthCalculation {
    RefineArrayLengthCalculation {
        halo: RefineArrayLengthHalo {
            expanded_mrl: vec![0, 1, 4],
            initial_boundary_mask: vec![0, 0, 1, 1],
            transition_boundary_mask: vec![0, 0, 0, 0],
            boundary_refine: vec![2, 3],
            boundary_refine_transition: vec![],
            num_transition_row_triangles: 1,
        },
        boundary: BoundaryConnection {
            bdy_num_in: 4,
            boundary_order: vec![1, 10, 11, 12],
            boundary_neighbors: vec![vec![]; 13],
            curves: BoundaryClosedCurves {
                num_closed_curve: 2,
                num_bdy_long: [4, 1, 1],
                close_curves: vec![vec![], vec![10, 11, 12], vec![12, 11]],
                n_close_curve: vec![0, 3, 2],
            },
        },
    }
}

#[test]
fn refine_array_length_close_mesh_writer_uses_fortran_tmpfile_paths_and_coordinates() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_refine_array_close_mesh_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");

    let mut wp = vec![LonLatPoint { lon: 0.0, lat: 0.0 }; 13];
    wp[10] = LonLatPoint {
        lon: 113.0,
        lat: 22.0,
    };
    wp[11] = LonLatPoint {
        lon: 114.0,
        lat: 23.0,
    };
    wp[12] = LonLatPoint {
        lon: 115.0,
        lat: 24.0,
    };

    let report = write_refine_array_length_close_meshes(&root, 4, &sample_calculation(), &wp)
        .expect("write Array_length_calculation close_Mesh_Save outputs");

    assert_eq!(report.mask_patch_ndm, 2);
    assert_eq!(report.outputs.len(), 2);
    assert_eq!(
        report.outputs[0].output,
        root.join("tmpfile/mask_patch_close_4_001.nc4")
    );
    assert_eq!(
        report.outputs[1].output,
        root.join("tmpfile/mask_patch_close_4_002.nc4")
    );
    assert_eq!(report.outputs[0].close_num, 3);
    assert_eq!(report.outputs[1].close_num, 2);

    assert_eq!(
        read_close_mesh_netcdf(root.join("tmpfile/mask_patch_close_4_001.nc4")).unwrap(),
        vec![wp[10], wp[11], wp[12]]
    );
    assert_eq!(
        read_close_mesh_netcdf(root.join("tmpfile/mask_patch_close_4_002.nc4")).unwrap(),
        vec![wp[12], wp[11]]
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn refine_array_length_close_mesh_writer_rejects_missing_curve_coordinates() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_refine_array_close_mesh_bad_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");

    let wp = vec![LonLatPoint { lon: 0.0, lat: 0.0 }; 12];
    let err = write_refine_array_length_close_meshes(&root, 4, &sample_calculation(), &wp)
        .expect_err("curve vertex 12 has no wp coordinate");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);

    let _ = fs::remove_dir_all(&root);
}
