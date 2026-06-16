use std::fs;

use earthmesh_cli::{
    run_mkgrd_initial_grid_quality_check, write_unstructured_mesh_netcdf, LonLatPoint,
    UnstructuredMesh,
};

#[test]
fn initial_grid_quality_check_reads_gridfile_and_writes_orial_quality() {
    let root = temp_root("earthmesh_cli_initial_quality_check");
    let input_gridfile = root.join("gridfile/gridfile_NXP0009_01_hex.nc4");
    let quality_output = root.join("result/quality_NXP0009_01_global_orial.nc4");
    write_unstructured_mesh_netcdf(&input_gridfile, &fixture_mesh())
        .expect("write fixture gridfile");

    run_mkgrd_initial_grid_quality_check(&input_gridfile, &quality_output)
        .expect("run initial quality check");

    let file = netcdf::open(&quality_output).expect("open initial quality output");
    assert_eq!(file.dimension("num_sjx").expect("num_sjx").len(), 6);
    assert!(file.variable("length_sjx").is_some());

    let _ = fs::remove_dir_all(&root);
}

fn fixture_mesh() -> UnstructuredMesh {
    UnstructuredMesh {
        m_points: vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 0.2, lat: 0.2 },
            LonLatPoint { lon: 0.8, lat: 0.2 },
            LonLatPoint { lon: 0.2, lat: 0.8 },
            LonLatPoint { lon: 0.8, lat: 0.8 },
        ],
        w_points: vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 1.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 1.0 },
            LonLatPoint { lon: 1.0, lat: 1.0 },
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

fn temp_root(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("{label}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("gridfile")).expect("create gridfile dir");
    root
}
