use std::fs;
use std::path::{Path, PathBuf};

use earthmesh_cli::coordinate_types::LonLatPoint;
use earthmesh_cli::unstructured_mesh_support::UnstructuredMesh;

fn temp_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_clean_ocean_window_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create clean-ocean root");
    root
}

fn write_ocean_landtype(path: &Path) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create landtype file");
    file.add_dimension("longitude", 360).expect("longitude dim");
    file.add_dimension("latitude", 180).expect("latitude dim");
    let mut variable = file
        .add_variable::<i8>("landtype", &["latitude", "longitude"])
        .expect("landtype variable");
    variable
        .put_values(&vec![0_i8; 360 * 180], (.., ..))
        .expect("write ocean landtype");
}

fn write_two_triangle_gridfile(path: &Path) {
    let mesh = UnstructuredMesh {
        m_points: vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint {
                lon: 113.6,
                lat: 22.4,
            },
            LonLatPoint {
                lon: 113.4,
                lat: 22.6,
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
    };
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(path, &mesh)
        .expect("write two-triangle gridfile");
}

#[test]
fn clean_ocean_window_preserves_triangle_count_obc_and_global_source_indices() {
    let root = temp_root();
    let source = root.join("source.nc4");
    let landtype = root.join("landtype.nc");
    write_two_triangle_gridfile(&source);
    write_ocean_landtype(&landtype);
    let close = [
        LonLatPoint {
            lon: 112.0,
            lat: 20.0,
        },
        LonLatPoint {
            lon: 116.0,
            lat: 20.0,
        },
        LonLatPoint {
            lon: 116.0,
            lat: 25.0,
        },
        LonLatPoint {
            lon: 112.0,
            lat: 25.0,
        },
    ];

    let plan = earthmesh_cli::regional_gridfile_writers::write_clean_regional_ocean_gridfile(
        &source, &close, &landtype, 7, 1, 0.5, &root,
    )
    .expect("write clean regional ocean");
    let final_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&plan.result_gridfile)
            .expect("read clean-ocean result");
    assert_eq!(final_mesh.m_to_w.len().saturating_sub(2), 2);
    assert!(plan.obc_output.as_ref().expect("obc path").exists());
    assert!(plan.obcv2_output.as_ref().expect("obcv2 path").exists());
    let obc = earthmesh_cli::obc_boundary_io::read_obc_order_netcdf(
        plan.obc_output.as_ref().expect("obc path"),
    )
    .expect("read OBC order");
    assert!(!obc.is_empty());

    let contain =
        earthmesh_cli::contain_io::read_contain_netcdf(&plan.contain_domain).expect("read contain");
    assert!(!contain.ustr_ii.is_empty());
    assert!(contain.ustr_ii.iter().all(|row| {
        row.len() == 2 && (292..=297).contains(&row[0]) && (65..=71).contains(&row[1])
    }));
    assert!(contain.ustr_ii.contains(&vec![294, 68]));

    let fvcom_root = root.join("fvcom");
    let fvcom = fvcom_root.join("clean.2dm");
    let triangles = earthmesh_cli::regional_gridfile_writers::write_clean_regional_ocean_fvcom(
        &source,
        &close,
        &landtype,
        7,
        1,
        0.5,
        &fvcom_root,
        &fvcom,
    )
    .expect("write clean-ocean FVCOM output");
    assert_eq!(triangles, 2);
    let fvcom_text = fs::read_to_string(&fvcom).expect("read clean-ocean FVCOM output");
    assert_eq!(
        fvcom_text
            .lines()
            .filter(|line| line.starts_with("E3T "))
            .count(),
        2
    );

    let _ = fs::remove_dir_all(root);
}
