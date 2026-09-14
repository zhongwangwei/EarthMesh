use std::{
    fs,
    path::{Path, PathBuf},
};

use earthmesh_cli::{
    coordinate_types::{GridRegion, LonLatPoint},
    hfield_gridfile_context::{read_hfield_gridfile_context, HfieldGridfileContext},
    unstructured_mesh_support::{MethodCGridfileMetadataSlices, UnstructuredMesh},
};
use earthmesh_hfield::HField;

fn root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_hfield_context_{name}_{}",
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
                lon: -10.0,
                lat: 0.0,
            },
            LonLatPoint {
                lon: 10.0,
                lat: 0.0,
            },
        ],
        w_points: vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint {
                lon: -20.0,
                lat: -10.0,
            },
            LonLatPoint {
                lon: 0.0,
                lat: -10.0,
            },
            LonLatPoint {
                lon: 20.0,
                lat: -10.0,
            },
            LonLatPoint {
                lon: -20.0,
                lat: 10.0,
            },
            LonLatPoint {
                lon: 0.0,
                lat: 10.0,
            },
            LonLatPoint {
                lon: 20.0,
                lat: 10.0,
            },
        ],
        m_to_w: vec![[1, 1, 1], [2, 3, 5], [3, 4, 6]],
        w_to_m: vec![
            vec![1],
            vec![2],
            vec![2, 3],
            vec![3],
            vec![2],
            vec![2, 3],
            vec![3],
        ],
        n_w_to_m: vec![1, 1, 2, 1, 1, 2, 1],
    }
}

fn context() -> HfieldGridfileContext {
    HfieldGridfileContext {
        field: HField::from_values(
            4,
            3,
            vec![
                400.0, 320.0, 240.0, 160.0, 200.0, 150.0, 125.0, 100.0, 500.0, 250.0, 125.0, 62.5,
            ],
        )
        .unwrap(),
        base_m: 500.0,
        max_level: 5,
    }
}

fn write_gridfile(path: &Path, hfield: Option<&HfieldGridfileContext>) {
    write_mesh_gridfile(path, &mesh(), hfield);
}

fn write_mesh_gridfile(
    path: &Path,
    mesh: &UnstructuredMesh,
    hfield: Option<&HfieldGridfileContext>,
) {
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_method_c_metadata(
        path,
        mesh,
        MethodCGridfileMetadataSlices {
            hfield,
            ..Default::default()
        },
    )
    .unwrap();
}

fn canonical_hex_mesh() -> UnstructuredMesh {
    let state = earthmesh_mesh::gridinit_voronoi_state_canonical(1, 0, 1.0, 0.25, 100)
        .expect("gridinit voronoi state");
    earthmesh_cli::mesh_conversion_gridfile_state::gridfile_mesh_from_one_based_state(
        &state.grid,
        &state.tabs,
    )
    .expect("one-based gridinit fixture to unstructured mesh")
}

fn physical_m_rows(mesh: &UnstructuredMesh) -> usize {
    mesh.m_to_w
        .iter()
        .filter(|row| !row.windows(2).all(|pair| pair[0] == pair[1]))
        .count()
}

fn physical_w_rows(mesh: &UnstructuredMesh) -> usize {
    mesh.n_w_to_m
        .iter()
        .zip(&mesh.w_to_m)
        .filter(|(&count, ring)| !(count <= 1 && ring.windows(2).all(|pair| pair[0] == pair[1])))
        .count()
}

fn assert_same_context(actual: &HfieldGridfileContext, expected: &HfieldGridfileContext) {
    assert_eq!(actual.base_m, expected.base_m);
    assert_eq!(actual.max_level, expected.max_level);
    assert_eq!(actual.field.nlon(), expected.field.nlon());
    assert_eq!(actual.field.nlat(), expected.field.nlat());
    assert_eq!(actual.field.values(), expected.field.values());
}

fn create_malformed(path: &Path, case: &str) {
    let mut file = netcdf::create(path).unwrap();
    file.add_dimension("earthmesh_hfield_nlon", 4).unwrap();
    file.add_dimension("earthmesh_hfield_nlat", 3).unwrap();
    file.add_dimension("wrong_dim", 12).unwrap();
    if case == "partial" {
        file.add_attribute("earthmesh_hfield_base_m", 500.0_f64)
            .unwrap();
        return;
    }
    match case {
        "wrong_type" => {
            let mut var = file
                .add_variable::<f32>(
                    "earthmesh_hfield_target_m",
                    &["earthmesh_hfield_nlat", "earthmesh_hfield_nlon"],
                )
                .unwrap();
            var.put_attribute("units", "m").unwrap();
            var.put_values(&[100.0_f32; 12], (.., ..)).unwrap();
        }
        "wrong_dim" => {
            let mut var = file
                .add_variable::<f64>("earthmesh_hfield_target_m", &["wrong_dim"])
                .unwrap();
            var.put_attribute("units", "m").unwrap();
            var.put_values(&[100.0_f64; 12], ..).unwrap();
        }
        "wrong_units" => {
            let mut var = file
                .add_variable::<f64>(
                    "earthmesh_hfield_target_m",
                    &["earthmesh_hfield_nlat", "earthmesh_hfield_nlon"],
                )
                .unwrap();
            var.put_attribute("units", "km").unwrap();
            var.put_values(&[100.0_f64; 12], (.., ..)).unwrap();
        }
        value_case => {
            let mut values = vec![100.0_f64; 12];
            if value_case == "nan_value" {
                values[4] = f64::NAN;
            }
            if value_case == "zero_value" {
                values[4] = 0.0;
            }
            let mut var = file
                .add_variable::<f64>(
                    "earthmesh_hfield_target_m",
                    &["earthmesh_hfield_nlat", "earthmesh_hfield_nlon"],
                )
                .unwrap();
            var.put_attribute("units", "m").unwrap();
            var.put_values(&values, (.., ..)).unwrap();
        }
    }
    file.add_attribute(
        "earthmesh_hfield_base_m",
        match case {
            "zero_base" => 0.0,
            "nan_base" => f64::NAN,
            _ => 500.0,
        },
    )
    .unwrap();
    file.add_attribute(
        "earthmesh_hfield_max_level",
        match case {
            "level_zero" => 0_i32,
            "level_six" => 6_i32,
            _ => 5_i32,
        },
    )
    .unwrap();
    file.add_attribute(
        "earthmesh_hfield_semantics",
        if case == "bad_semantics" {
            "other"
        } else {
            "spherical_effective_demand_v1"
        },
    )
    .unwrap();
}

#[test]
fn roundtrips_exact_hfield_context_and_replays_level_at() {
    let root = root("roundtrip");
    let output = root.join("grid.nc4");
    let expected = context();
    write_gridfile(&output, Some(&expected));

    let actual = read_hfield_gridfile_context(&output).unwrap().unwrap();
    assert_same_context(&actual, &expected);
    let file = netcdf::open(&output).unwrap();
    let var = file.variable("earthmesh_hfield_target_m").unwrap();
    assert_eq!(
        var.dimensions()
            .iter()
            .map(|dim| dim.name())
            .collect::<Vec<_>>(),
        vec!["earthmesh_hfield_nlat", "earthmesh_hfield_nlon"]
    );
    let raw = var.get_values::<f64, _>((.., ..)).unwrap();
    for j in 0..expected.field.nlat() {
        for i in 0..expected.field.nlon() {
            assert_eq!(raw[j * expected.field.nlon() + i], expected.field.get(i, j));
        }
    }
    assert_eq!(
        actual
            .field
            .level_at(10.0, 10.0, actual.base_m, actual.max_level),
        expected
            .field
            .level_at(10.0, 10.0, expected.base_m, expected.max_level)
    );
    assert_eq!(
        earthmesh_cli::mpas_gridfile_context::read_mpas_gridfile_context(&output).unwrap(),
        None
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn absent_hfield_context_is_none_and_geometry_is_unchanged() {
    let root = root("absent");
    let output = root.join("grid.nc4");
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(&output, &mesh()).unwrap();
    let before =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&output).unwrap();

    assert!(read_hfield_gridfile_context(&output).unwrap().is_none());
    assert_eq!(
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&output).unwrap(),
        before
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_malformed_hfield_headers_and_values() {
    for case in [
        "partial",
        "wrong_type",
        "wrong_dim",
        "wrong_units",
        "nan_value",
        "zero_value",
        "zero_base",
        "nan_base",
        "level_zero",
        "level_six",
        "bad_semantics",
    ] {
        let root = root(case);
        let output = root.join("bad.nc4");
        create_malformed(&output, case);
        assert!(
            read_hfield_gridfile_context(&output).is_err(),
            "case {case} should fail"
        );
        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn invalid_hfield_metadata_does_not_overwrite_existing_gridfile() {
    let root = root("atomic");
    let output = root.join("grid.nc4");
    let old = context();
    write_gridfile(&output, Some(&old));
    let before = fs::read(&output).unwrap();
    let mut invalid = old;
    invalid.max_level = 6;

    let err =
        earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_method_c_metadata(
            &output,
            &mesh(),
            MethodCGridfileMetadataSlices {
                hfield: Some(&invalid),
                ..Default::default()
            },
        )
        .unwrap_err();

    assert!(matches!(
        err.kind(),
        std::io::ErrorKind::InvalidInput | std::io::ErrorKind::InvalidData
    ));
    assert_eq!(fs::read(&output).unwrap(), before);
    assert_same_context(
        &read_hfield_gridfile_context(&output).unwrap().unwrap(),
        &context(),
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn copied_and_regional_gridfiles_keep_full_hfield_without_mpas_context() {
    let root = root("regional");
    let input = root.join("global.nc4");
    let copy = root.join("copy.nc4");
    let tri = root.join("regional_tri.nc4");
    let hex_input = root.join("global_hex.nc4");
    let hex = root.join("regional_hex.nc4");
    let expected = context();
    write_gridfile(&input, Some(&expected));
    fs::copy(&input, &copy).unwrap();
    earthmesh_cli::regional_gridfile_writers::write_regional_gridfile(
        &input,
        &tri,
        &GridRegion::Bbox {
            west: -180.0,
            east: 0.0,
            south: -90.0,
            north: 90.0,
        },
        "tri",
    )
    .unwrap();
    let hex_mesh = canonical_hex_mesh();
    write_mesh_gridfile(&hex_input, &hex_mesh, Some(&expected));
    earthmesh_cli::regional_gridfile_writers::write_regional_gridfile(
        &hex_input,
        &hex,
        &GridRegion::Bbox {
            west: -181.0,
            east: 0.0,
            south: -91.0,
            north: 91.0,
        },
        "hex",
    )
    .unwrap();

    for regional in [&copy, &tri, &hex] {
        assert_same_context(
            &read_hfield_gridfile_context(regional).unwrap().unwrap(),
            &expected,
        );
        assert_eq!(
            earthmesh_cli::mpas_gridfile_context::read_mpas_gridfile_context(regional).unwrap(),
            None
        );
    }
    let tri_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&tri).unwrap();
    assert!(physical_m_rows(&tri_mesh) < physical_m_rows(&mesh()));
    let hex_regional =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&hex).unwrap();
    assert!(physical_w_rows(&hex_regional) < physical_w_rows(&hex_mesh));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn failed_regional_crop_does_not_overwrite_existing_hfield_gridfile() {
    let root = root("regional_atomic");
    let input = root.join("global.nc4");
    let output = root.join("regional.nc4");
    let expected = context();
    write_gridfile(&input, Some(&expected));
    write_gridfile(&output, Some(&expected));
    let before = fs::read(&output).unwrap();

    let err = earthmesh_cli::regional_gridfile_writers::write_regional_gridfile(
        &input,
        &output,
        &GridRegion::Bbox {
            west: -180.0,
            east: 0.0,
            south: -90.0,
            north: 90.0,
        },
        "bad-mode",
    )
    .unwrap_err();

    assert!(matches!(
        err.kind(),
        std::io::ErrorKind::InvalidInput | std::io::ErrorKind::InvalidData
    ));
    assert_eq!(fs::read(&output).unwrap(), before);
    assert_same_context(
        &read_hfield_gridfile_context(&output).unwrap().unwrap(),
        &expected,
    );
    let _ = fs::remove_dir_all(root);
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

#[test]
fn clean_ocean_preserves_hfield_only_context_without_mpas_or_levels() {
    let root = root("clean_ocean");
    let source = root.join("source.nc4");
    let landtype = root.join("landtype.nc");
    let expected = context();
    write_gridfile(&source, Some(&expected));
    assert!(
        earthmesh_cli::mpas_gridfile_context::read_mpas_gridfile_context(&source)
            .unwrap()
            .is_none()
    );
    write_ocean_landtype(&landtype);
    let close = [
        LonLatPoint {
            lon: -30.0,
            lat: -20.0,
        },
        LonLatPoint {
            lon: 30.0,
            lat: -20.0,
        },
        LonLatPoint {
            lon: 30.0,
            lat: 20.0,
        },
        LonLatPoint {
            lon: -30.0,
            lat: 20.0,
        },
    ];

    let plan = earthmesh_cli::regional_gridfile_writers::write_clean_regional_ocean_gridfile(
        &source, &close, &landtype, 7, 1, 0.5, &root,
    )
    .expect("write clean regional ocean");

    assert_same_context(
        &read_hfield_gridfile_context(&plan.result_gridfile)
            .unwrap()
            .unwrap(),
        &expected,
    );
    assert!(
        earthmesh_cli::mpas_gridfile_context::read_mpas_gridfile_context(&plan.result_gridfile)
            .unwrap()
            .is_none()
    );
    let clean_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&plan.result_gridfile)
            .unwrap();
    assert!(physical_m_rows(&clean_mesh) > 0);
    assert!(physical_m_rows(&clean_mesh) <= physical_m_rows(&mesh()));
    let _ = fs::remove_dir_all(root);
}
fn zero_placeholder_mesh() -> UnstructuredMesh {
    let mut out = canonical_hex_mesh();
    out.m_points.remove(0);
    out.w_points.remove(0);
    out.m_to_w.remove(0);
    out.w_to_m.remove(0);
    out.n_w_to_m.remove(0);
    for row in &mut out.m_to_w {
        for id in row {
            *id -= 1;
        }
    }
    for ring in &mut out.w_to_m {
        for id in ring {
            *id -= 1;
        }
    }
    out
}

fn two_placeholder_mesh() -> UnstructuredMesh {
    let point = LonLatPoint { lon: 0.0, lat: 0.0 };
    let mut out = canonical_hex_mesh();
    out.m_points.insert(0, point);
    out.w_points.insert(0, point);
    out.m_to_w.insert(0, [0, 0, 0]);
    out.w_to_m.insert(0, vec![0]);
    out.n_w_to_m.insert(0, 0);
    out
}

fn expected_hfield_reference_km(hfield: &HfieldGridfileContext) -> f64 {
    let finest = hfield
        .field
        .level_map(hfield.base_m, hfield.max_level)
        .unwrap()
        .into_iter()
        .max()
        .unwrap();
    hfield.base_m / 2.0_f64.powi(i32::from(finest)) / 1000.0
}

fn expected_hfield_width_km(hfield: &HfieldGridfileContext, point: LonLatPoint) -> f64 {
    let level = hfield
        .field
        .try_level_at(point.lon, point.lat, hfield.base_m, hfield.max_level)
        .unwrap();
    hfield.base_m / 2.0_f64.powi(i32::from(level)) / 1000.0
}

#[test]
fn builds_mpas_context_from_hfield_quantized_demand_and_roundtrips_schema() {
    for (name, mut mesh, first) in [
        ("zero_placeholder", zero_placeholder_mesh(), 0),
        ("one_placeholder", canonical_hex_mesh(), 1),
        ("two_placeholder", two_placeholder_mesh(), 2),
    ] {
        let mut hfield = context();
        hfield.base_m = 640.0;
        hfield.max_level = 2;
        mesh.w_points[first] = LonLatPoint { lon: 0.0, lat: 0.0 };

        let mpas = earthmesh_cli::mpas_gridfile_context::MpasGridfileContext::from_hfield_quantized_demand(
            &mesh,
            &hfield,
            12,
        )
        .unwrap();

        assert_eq!(mpas.source, "method_c_hfield_quantized_w_demand_v1");
        assert_eq!(mpas.base_nxp, 12);
        assert_eq!(mpas.step, 3);
        assert_eq!(mpas.cellwidth_km.len(), mesh.w_points.len());
        let reference = expected_hfield_reference_km(&hfield);
        assert_eq!(mpas.density_reference_width_km, reference);
        for width in &mpas.cellwidth_km[..first] {
            assert_eq!(*width, reference, "{name} placeholder width");
        }
        for (idx, point) in mesh.w_points[first..].iter().enumerate() {
            assert_eq!(
                mpas.cellwidth_km[first + idx],
                expected_hfield_width_km(&hfield, *point),
                "{name} physical width row {}",
                first + idx
            );
        }

        let root = root(name);
        let output = root.join("grid.nc4");
        earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_method_c_metadata(
            &output,
            &mesh,
            MethodCGridfileMetadataSlices {
                mpas: Some(&mpas),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            earthmesh_cli::mpas_gridfile_context::read_mpas_gridfile_context(&output).unwrap(),
            Some(mpas)
        );
        let _ = fs::remove_dir_all(root);
    }

    // The full field requests 160 m, even if every sampled W site asks for
    // 320 m. Do not normalize to sampled W widths or require density == 1.
    let mut hfield = context();
    hfield.base_m = 640.0;
    hfield.max_level = 2;
    let mut mesh = canonical_hex_mesh();
    for point in &mut mesh.w_points[1..] {
        *point = LonLatPoint {
            lon: -135.0,
            lat: 60.0,
        };
    }
    let mpas =
        earthmesh_cli::mpas_gridfile_context::MpasGridfileContext::from_hfield_quantized_demand(
            &mesh, &hfield, 12,
        )
        .unwrap();
    assert_eq!(mpas.density_reference_width_km, 0.16);
    assert!(mpas.cellwidth_km[1..].iter().all(|&width| width == 0.32));
}

#[test]
fn rejects_invalid_hfield_quantized_demand_inputs() {
    for case in ["bad_lon", "bad_lat", "bad_base", "bad_level", "underflow"] {
        let mut mesh = mesh();
        let mut hfield = context();
        match case {
            "bad_lon" => mesh.w_points[1].lon = f64::NAN,
            "bad_lat" => mesh.w_points[1].lat = 91.0,
            "bad_base" => hfield.base_m = 0.0,
            "bad_level" => hfield.max_level = 0,
            "underflow" => hfield.base_m = f64::from_bits(1),
            _ => unreachable!(),
        }

        assert!(
            earthmesh_cli::mpas_gridfile_context::MpasGridfileContext::from_hfield_quantized_demand(
                &mesh,
                &hfield,
                12,
            )
            .is_err(),
            "case {case} should fail"
        );
    }
}
