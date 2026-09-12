use std::{fs, path::Path};

use earthmesh_cli::{
    coordinate_types::LonLatPoint,
    mpas_gridfile_context::{read_mpas_gridfile_context, MpasGridfileContext},
    unstructured_mesh_support::{MethodCGridfileMetadataSlices, UnstructuredMesh},
};

fn root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_mpas_context_{name}_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn mesh() -> UnstructuredMesh {
    UnstructuredMesh {
        m_points: vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint {
                lon: 10.0,
                lat: 0.0,
            },
        ],
        w_points: vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 1.0 },
            LonLatPoint { lon: 1.0, lat: 0.0 },
            LonLatPoint { lon: 1.0, lat: 1.0 },
        ],
        m_to_w: vec![[1, 1, 1], [2, 3, 4]],
        w_to_m: vec![vec![1], vec![2], vec![2], vec![2]],
        n_w_to_m: vec![1, 1, 1, 1],
    }
}

fn context() -> MpasGridfileContext {
    MpasGridfileContext {
        cellwidth_km: vec![999.0, 100.0, 50.5, 25.25],
        base_nxp: 80,
        step: 2,
        density_reference_width_km: 25.25,
        source: "cmrc-test".to_string(),
    }
}

fn write_with_context(path: &Path, context: &MpasGridfileContext) {
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_method_c_metadata(
        path,
        &mesh(),
        MethodCGridfileMetadataSlices {
            mpas: Some(context),
            ..Default::default()
        },
    )
    .unwrap();
}

fn create_malformed(path: &Path, case: &str) {
    let mut file = netcdf::create(path).unwrap();
    file.add_dimension("lbx_points", 2).unwrap();
    file.add_dimension("wrong_points", 2).unwrap();
    match case {
        "partial" => {
            file.add_attribute("earthmesh_mpas_base_nxp", 80_i32)
                .unwrap();
            return;
        }
        "wrong_type" => {
            let mut var = file
                .add_variable::<f32>("earthmesh_w_cellwidth_km", &["lbx_points"])
                .unwrap();
            var.put_attribute("units", "km").unwrap();
            var.put_values(&[10.0_f32, 20.0], ..).unwrap();
        }
        "wrong_dim" => {
            let mut var = file
                .add_variable::<f64>("earthmesh_w_cellwidth_km", &["wrong_points"])
                .unwrap();
            var.put_attribute("units", "km").unwrap();
            var.put_values(&[10.0_f64, 20.0], ..).unwrap();
        }
        "wrong_units" => {
            let mut var = file
                .add_variable::<f64>("earthmesh_w_cellwidth_km", &["lbx_points"])
                .unwrap();
            var.put_attribute("units", "m").unwrap();
            var.put_values(&[10.0_f64, 20.0], ..).unwrap();
        }
        "missing_step" => {
            let mut var = file
                .add_variable::<f64>("earthmesh_w_cellwidth_km", &["lbx_points"])
                .unwrap();
            var.put_attribute("units", "km").unwrap();
            var.put_values(&[10.0_f64, 20.0], ..).unwrap();
            file.add_attribute("earthmesh_mpas_base_nxp", 80_i32)
                .unwrap();
            file.add_attribute("earthmesh_mpas_density_reference_width_km", 10.0_f64)
                .unwrap();
            file.add_attribute("earthmesh_mpas_cellwidth_source", "cmrc-test")
                .unwrap();
            return;
        }
        value_case => {
            let mut var = file
                .add_variable::<f64>("earthmesh_w_cellwidth_km", &["lbx_points"])
                .unwrap();
            var.put_attribute("units", "km").unwrap();
            let values = match value_case {
                "nan" => [f64::NAN, 20.0],
                "negative" => [-10.0, 20.0],
                _ => [10.0, 20.0],
            };
            var.put_values(&values, ..).unwrap();
        }
    }
    file.add_attribute(
        "earthmesh_mpas_base_nxp",
        if case == "zero_nxp" { 0_i32 } else { 80_i32 },
    )
    .unwrap();
    file.add_attribute(
        "earthmesh_mpas_step",
        if case == "zero_step" {
            0_i32
        } else if case == "oversized_step" {
            usize::BITS as i32 + 1
        } else {
            2_i32
        },
    )
    .unwrap();
    file.add_attribute(
        "earthmesh_mpas_density_reference_width_km",
        if case == "reference_nan" {
            f64::NAN
        } else if case == "reference_above_min_width" {
            15.0_f64
        } else {
            10.0_f64
        },
    )
    .unwrap();
    file.add_attribute(
        "earthmesh_mpas_cellwidth_source",
        if case == "empty_source" {
            " "
        } else {
            "cmrc-test"
        },
    )
    .unwrap();
}

#[test]
fn roundtrips_nonuniform_mpas_context_exactly() {
    let root = root("roundtrip");
    let output = root.join("grid.nc4");
    let expected = context();
    write_with_context(&output, &expected);

    assert_eq!(read_mpas_gridfile_context(&output).unwrap(), Some(expected));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn copied_gridfile_keeps_mpas_context_without_mutating_source() {
    let root = root("copy");
    let output = root.join("grid.nc4");
    let copy = root.join("copy.nc4");
    write_with_context(&output, &context());
    let source_before = fs::read(&output).unwrap();
    fs::copy(&output, &copy).unwrap();

    assert_eq!(fs::read(&copy).unwrap(), source_before);
    assert_eq!(
        read_mpas_gridfile_context(&copy).unwrap(),
        read_mpas_gridfile_context(&output).unwrap()
    );
    assert_eq!(fs::read(&output).unwrap(), source_before);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn returns_none_when_mpas_context_is_absent() {
    let root = root("missing");
    let output = root.join("grid.nc4");
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(&output, &mesh()).unwrap();

    assert_eq!(read_mpas_gridfile_context(&output).unwrap(), None);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn rejects_malformed_mpas_context_headers_and_values() {
    for case in [
        "partial",
        "wrong_type",
        "wrong_dim",
        "wrong_units",
        "missing_step",
        "nan",
        "negative",
        "zero_nxp",
        "zero_step",
        "oversized_step",
        "reference_nan",
        "reference_above_min_width",
        "empty_source",
    ] {
        let root = root(case);
        let output = root.join("bad.nc4");
        create_malformed(&output, case);

        assert!(
            read_mpas_gridfile_context(&output).is_err(),
            "case {case} should fail"
        );
        let _ = fs::remove_dir_all(&root);
    }
}

#[test]
fn invalid_mpas_metadata_does_not_overwrite_existing_gridfile() {
    let root = root("atomic");
    let output = root.join("grid.nc4");
    let old = context();
    write_with_context(&output, &old);
    let before = fs::read(&output).unwrap();
    let mut invalid = old.clone();
    invalid.cellwidth_km.pop();

    let err =
        earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_method_c_metadata(
            &output,
            &mesh(),
            MethodCGridfileMetadataSlices {
                mpas: Some(&invalid),
                ..Default::default()
            },
        )
        .unwrap_err();

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert_eq!(fs::read(&output).unwrap(), before);
    assert_eq!(read_mpas_gridfile_context(&output).unwrap(), Some(old));
    let _ = fs::remove_dir_all(&root);
}
