use std::fs;
use std::path::PathBuf;

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
}

fn write_landtype_file(path: &std::path::Path, nlons: usize, nlats: usize) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create landtype file");
    file.add_dimension("longitude", nlons)
        .expect("longitude dim");
    file.add_dimension("latitude", nlats).expect("latitude dim");
    let mut values = vec![0_i8; nlons * nlats];
    let idx = |lon: usize, lat: usize| lon * nlats + lat;
    values[idx(0, 0)] = 0;
    values[idx(1, 0)] = 2;
    values[idx(2, 1)] = 7;
    values[idx(nlons - 1, nlats - 1)] = 4;
    let mut var = file
        .add_variable::<i8>("landtype", &["longitude", "latitude"])
        .expect("landtype var");
    var.put_values(&values, (.., ..)).expect("write landtype");
}

fn write_landtype_file_with_points(path: &std::path::Path, land_points: &[(usize, usize)]) {
    let (nlons, nlats) = (360, 180);
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create landtype file");
    file.add_dimension("longitude", nlons)
        .expect("longitude dim");
    file.add_dimension("latitude", nlats).expect("latitude dim");
    let mut values = vec![0_i8; nlons * nlats];
    let idx = |lon: usize, lat: usize| lon * nlats + lat;
    for &(lon, lat) in land_points {
        values[idx(lon, lat)] = 1;
    }
    let mut var = file
        .add_variable::<i8>("landtype", &["longitude", "latitude"])
        .expect("landtype var");
    var.put_values(&values, (.., ..)).expect("write landtype");
}

fn write_landtype_lat_lon_file(path: &std::path::Path, nlons: usize, nlats: usize) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create lat-lon landtype file");
    file.add_dimension("latitude", nlats).expect("latitude dim");
    file.add_dimension("longitude", nlons)
        .expect("longitude dim");
    let mut values = vec![0_i8; nlons * nlats];
    let idx = |lat: usize, lon: usize| lat * nlons + lon;
    values[idx(0, 0)] = 0;
    values[idx(0, 1)] = 2;
    values[idx(1, 2)] = 7;
    values[idx(nlats - 1, nlons - 1)] = 4;
    let mut var = file
        .add_variable::<i8>("landtype", &["latitude", "longitude"])
        .expect("lat-lon landtype var");
    var.put_values(&values, (.., ..))
        .expect("write lat-lon landtype");
}

fn declare_sparse_landtype_file(path: &std::path::Path, nlons: usize, nlats: usize) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create sparse landtype file");
    file.add_dimension("longitude", nlons)
        .expect("longitude dim");
    file.add_dimension("latitude", nlats).expect("latitude dim");
    file.add_variable::<i8>("landtype", &["longitude", "latitude"])
        .expect("landtype var");
}

fn write_three_cell_tri_gridfile(path: &std::path::Path) {
    let mesh = earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: -178.5,
                lat: 89.5,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: -177.5,
                lat: 89.5,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: -176.5,
                lat: 89.5,
            },
        ],
        w_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 0.0,
                lat: -88.5,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 10.0,
                lat: -88.5,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 20.0,
                lat: -88.5,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 30.0,
                lat: -88.5,
            },
        ],
        m_to_w: vec![[1, 1, 1], [1, 2, 3], [1, 2, 3], [1, 2, 3]],
        w_to_m: vec![vec![1], vec![1, 2, 3], vec![1, 2, 3], vec![1, 2, 3]],
        n_w_to_m: vec![1, 3, 3, 3],
    };
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(path, &mesh)
        .expect("write gridfile");
}

fn write_two_cell_hex_gridfile(path: &std::path::Path) {
    let mut m_points = vec![earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 }];
    m_points.extend([
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: -179.0,
            lat: 89.0,
        },
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: -178.5,
            lat: 89.0,
        },
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: -178.0,
            lat: 89.0,
        },
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: -178.0,
            lat: 90.0,
        },
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: -178.5,
            lat: 90.0,
        },
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: -179.0,
            lat: 90.0,
        },
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: -178.0,
            lat: 89.0,
        },
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: -177.5,
            lat: 89.0,
        },
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: -177.0,
            lat: 89.0,
        },
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: -177.0,
            lat: 90.0,
        },
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: -177.5,
            lat: 90.0,
        },
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: -178.0,
            lat: 90.0,
        },
    ]);
    let mesh = earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
        m_points,
        w_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: -178.5,
                lat: 89.5,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: -177.5,
                lat: 89.5,
            },
        ],
        m_to_w: vec![[1, 1, 1]; 13],
        w_to_m: vec![
            vec![1, 1, 1, 1, 1, 1],
            vec![2, 3, 4, 5, 6, 7],
            vec![8, 9, 10, 11, 12, 13],
        ],
        n_w_to_m: vec![0, 6, 6],
    };
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(path, &mesh)
        .expect("write hex gridfile");
}

#[test]
fn landtype_masked_gridfile_keeps_land_or_ocean_cells_and_reindexes() {
    let root = temp_root("landtype_masked_gridfile");
    let landtype_file = root.join("landtype.nc");
    let input = root.join("gridfile.nc4");
    let land_output = root.join("land_gridfile.nc4");
    let ocean_output = root.join("ocean_gridfile.nc4");
    write_landtype_file_with_points(&landtype_file, &[(1, 0), (3, 0)]);
    write_three_cell_tri_gridfile(&input);

    let land_count = earthmesh_cli::regional_gridfile_writers::write_landtype_masked_gridfile(
        &input,
        &land_output,
        &landtype_file,
        1,
        "tri",
        "landmesh",
    )
    .expect("write land-only gridfile");
    let ocean_count = earthmesh_cli::regional_gridfile_writers::write_landtype_masked_gridfile(
        &input,
        &ocean_output,
        &landtype_file,
        1,
        "tri",
        "oceanmesh",
    )
    .expect("write ocean-only gridfile");

    assert_eq!(land_count, 2);
    assert_eq!(ocean_count, 1);
    let land_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&land_output)
            .expect("read land masked gridfile");
    let ocean_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&ocean_output)
            .expect("read ocean masked gridfile");
    let land_topology =
        earthmesh_cli::unstructured_mesh_support::check_unstructured_mesh_topology(&land_mesh);
    let ocean_topology =
        earthmesh_cli::unstructured_mesh_support::check_unstructured_mesh_topology(&ocean_mesh);
    assert!(land_mesh.m_points.len() >= land_count);
    assert!(ocean_mesh.m_points.len() >= ocean_count);
    assert!(
        land_topology.is_consistent(),
        "land topology violations: {:?}",
        land_topology.violations
    );
    assert!(
        ocean_topology.is_consistent(),
        "ocean topology violations: {:?}",
        ocean_topology.violations
    );
    assert!(land_mesh
        .m_to_w
        .iter()
        .flatten()
        .all(|&vertex| vertex >= 0 && vertex as usize <= land_mesh.w_points.len()));
    assert!(ocean_mesh
        .m_to_w
        .iter()
        .flatten()
        .all(|&vertex| vertex >= 0 && vertex as usize <= ocean_mesh.w_points.len()));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn landtype_masked_hex_gridfile_preserves_cell_corner_geometry_after_reindex() {
    let root = temp_root("landtype_masked_hex_geometry");
    let landtype_file = root.join("landtype.nc");
    let input = root.join("gridfile.nc4");
    let output = root.join("land_gridfile.nc4");
    write_landtype_file_with_points(&landtype_file, &[(1, 0), (2, 0)]);
    write_two_cell_hex_gridfile(&input);

    let land_count = earthmesh_cli::regional_gridfile_writers::write_landtype_masked_gridfile(
        &input,
        &output,
        &landtype_file,
        1,
        "hex",
        "landmesh",
    )
    .expect("write hex land-only gridfile");

    let land_mesh = earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&output)
        .expect("read hex land gridfile");
    let first_cell = land_mesh
        .w_points
        .iter()
        .position(|point| (point.lon + 178.5).abs() < 1.0e-12 && (point.lat - 89.5).abs() < 1.0e-12)
        .expect("first land hex center preserved");
    let corner_count = land_mesh.n_w_to_m[first_cell] as usize;
    let corner_ids = &land_mesh.w_to_m[first_cell][..corner_count];

    assert_eq!(land_count, 2);
    assert_eq!(corner_count, 6);
    for &corner_id in corner_ids {
        let corner = &land_mesh.m_points[corner_id as usize];
        assert!(
            (corner.lon + 178.5).abs() <= 0.6 && (corner.lat - 89.5).abs() <= 0.6,
            "corner id {corner_id} points to remote coordinate {:?}",
            corner
        );
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn regional_carve_after_ocean_hex_mask_does_not_reapply_placeholder_shift() {
    let root = temp_root("regional_after_ocean_hex_mask");
    let landtype_file = root.join("landtype.nc");
    let input = root.join("gridfile.nc4");
    let ocean_output = root.join("ocean_gridfile.nc4");
    let regional_output = root.join("regional_ocean_gridfile.nc4");
    write_landtype_file_with_points(&landtype_file, &[(1, 0)]);
    write_two_cell_hex_gridfile(&input);

    earthmesh_cli::regional_gridfile_writers::write_landtype_masked_gridfile(
        &input,
        &ocean_output,
        &landtype_file,
        1,
        "hex",
        "oceanmesh",
    )
    .expect("write hex ocean-only gridfile");
    let region = earthmesh_cli::coordinate_types::GridRegion::Bbox {
        west: -178.0,
        east: -177.0,
        north: 90.0,
        south: 89.0,
    };
    let kept = earthmesh_cli::regional_gridfile_writers::write_regional_gridfile(
        &ocean_output,
        &regional_output,
        &region,
        "hex",
    )
    .expect("write regional ocean gridfile");

    let regional_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&regional_output)
            .expect("read regional ocean gridfile");
    let ocean_cell = regional_mesh
        .w_points
        .iter()
        .position(|point| (point.lon + 177.5).abs() < 1.0e-12 && (point.lat - 89.5).abs() < 1.0e-12)
        .expect("regional ocean hex center preserved");
    let corner_count = regional_mesh.n_w_to_m[ocean_cell] as usize;
    let corner_ids = &regional_mesh.w_to_m[ocean_cell][..corner_count];

    assert_eq!(kept, 1);
    assert_eq!(corner_count, 6);
    for &corner_id in corner_ids {
        let corner = &regional_mesh.m_points[corner_id as usize];
        assert!(
            (corner.lon + 177.5).abs() <= 0.6 && (corner.lat - 89.5).abs() <= 0.6,
            "corner id {corner_id} points to remote coordinate {:?}",
            corner
        );
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn regional_carve_after_land_hex_mask_does_not_reapply_placeholder_shift() {
    let root = temp_root("regional_after_land_hex_mask");
    let landtype_file = root.join("landtype.nc");
    let input = root.join("gridfile.nc4");
    let land_output = root.join("land_gridfile.nc4");
    let regional_output = root.join("regional_land_gridfile.nc4");
    write_landtype_file_with_points(&landtype_file, &[(1, 0)]);
    write_two_cell_hex_gridfile(&input);

    earthmesh_cli::regional_gridfile_writers::write_landtype_masked_gridfile(
        &input,
        &land_output,
        &landtype_file,
        1,
        "hex",
        "landmesh",
    )
    .expect("write hex land-only gridfile");
    let region = earthmesh_cli::coordinate_types::GridRegion::Bbox {
        west: -179.0,
        east: -178.0,
        north: 90.0,
        south: 89.0,
    };
    let kept = earthmesh_cli::regional_gridfile_writers::write_regional_gridfile(
        &land_output,
        &regional_output,
        &region,
        "hex",
    )
    .expect("write regional land gridfile");

    let regional_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&regional_output)
            .expect("read regional land gridfile");
    let land_cell = regional_mesh
        .w_points
        .iter()
        .position(|point| (point.lon + 178.5).abs() < 1.0e-12 && (point.lat - 89.5).abs() < 1.0e-12)
        .expect("regional land hex center preserved");
    let corner_count = regional_mesh.n_w_to_m[land_cell] as usize;
    let corner_ids = &regional_mesh.w_to_m[land_cell][..corner_count];

    assert_eq!(kept, 1);
    assert_eq!(corner_count, 6);
    for &corner_id in corner_ids {
        let corner = &regional_mesh.m_points[corner_id as usize];
        assert!(
            (corner.lon + 178.5).abs() <= 0.6 && (corner.lat - 89.5).abs() <= 0.6,
            "corner id {corner_id} points to remote coordinate {:?}",
            corner
        );
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn landtype_point_sampler_reads_requested_points_in_both_dimension_orders() {
    let root = temp_root("landtype_point_sampler");
    let lon_lat_file = root.join("landtype_lon_lat.nc");
    let lat_lon_file = root.join("landtype_lat_lon.nc");
    write_landtype_file(&lon_lat_file, 360, 180);
    write_landtype_lat_lon_file(&lat_lon_file, 360, 180);
    let points = [
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: -178.5,
            lat: 89.5,
        },
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: -177.5,
            lat: 88.5,
        },
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: 179.5,
            lat: -89.5,
        },
    ];

    let lon_lat_values =
        earthmesh_cli::mkgrd_data_preprocess_source::sample_landtype_values_for_points_one_based(
            &lon_lat_file,
            1,
            &points,
        )
        .expect("sample longitude-latitude landtype");
    let lat_lon_values =
        earthmesh_cli::mkgrd_data_preprocess_source::sample_landtype_values_for_points_one_based(
            &lat_lon_file,
            1,
            &points,
        )
        .expect("sample latitude-longitude landtype");

    assert_eq!(lon_lat_values, vec![2, 7, 4]);
    assert_eq!(lat_lon_values, lon_lat_values);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn landtype_surface_class_sampler_returns_preview_codes_without_coupling_file() {
    let root = temp_root("landtype_surface_class_sampler");
    let landtype_file = root.join("landtype.nc");
    write_landtype_file_with_points(&landtype_file, &[(1, 0), (3, 0)]);
    let points = [
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: -178.5,
            lat: 89.5,
        },
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: -177.5,
            lat: 89.5,
        },
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: -176.5,
            lat: 89.5,
        },
    ];

    let codes = earthmesh_cli::mkgrd_data_preprocess_source::sample_landtype_surface_class_codes_for_points_one_based(
        &landtype_file,
        1,
        &points,
    )
    .expect("sample preview surface class codes");

    assert_eq!(codes, vec![1, 2, 1]);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn data_preprocess_v3_source_descriptors_cover_landtype_threshold_hydro_and_coast_inputs() {
    let root = temp_root("data_preprocess_v3_sources");
    let landtype_file = root.join("landtype.nc");
    write_landtype_file(&landtype_file, 360, 180);

    let landtype = earthmesh_cli::v3_data_source_io::build_v3_data_source_descriptor(
        earthmesh_cli::v3_data_source_io::V3DataSourceKind::Landtype,
        &landtype_file,
    )
    .expect("landtype source descriptor");
    assert_eq!(
        landtype.kind,
        earthmesh_cli::v3_data_source_io::V3DataSourceKind::Landtype
    );
    assert_eq!(landtype.semantic_layers, vec!["landtype"]);

    let hydro = earthmesh_cli::v3_data_source_io::build_v3_data_source_descriptor(
        earthmesh_cli::v3_data_source_io::V3DataSourceKind::Hydro,
        root.join("MERIT_Hydro"),
    )
    .expect("hydro source descriptor");
    assert_eq!(
        hydro.semantic_layers,
        vec!["river_r2", "river_r3", "estuary"]
    );

    let coast = earthmesh_cli::v3_data_source_io::build_v3_data_source_descriptor(
        earthmesh_cli::v3_data_source_io::V3DataSourceKind::Coast,
        root.join("coast"),
    )
    .expect("coast source descriptor");
    assert_eq!(
        coast.semantic_layers,
        vec!["coast_land", "coast_ocean", "shoreline"]
    );

    let threshold = earthmesh_cli::v3_data_source_io::build_v3_data_source_descriptor(
        earthmesh_cli::v3_data_source_io::V3DataSourceKind::Threshold,
        root.join("thresholds"),
    )
    .expect("threshold source descriptor");
    assert_eq!(threshold.semantic_layers, vec!["threshold_fields"]);

    let report =
        earthmesh_cli::mkgrd_data_preprocess_source::read_landtype_data_preprocess_one_based(
            &landtype_file,
            1,
        )
        .expect("read landtype preprocess data");
    assert_eq!(
        report.source.kind,
        earthmesh_cli::v3_data_source_io::V3DataSourceKind::Landtype
    );
    assert_eq!(report.source.path, landtype_file);

    let source_state = earthmesh_cli::mkgrd_data_preprocess_source::build_mkgrd_data_preprocess_source_state_one_based(
        &root,
        &landtype_file,
        1,
        true,
        "bbox",
        1,
        "landmesh",
        true,
        3,
        1,
    )
    .expect("build mkgrd source state");
    assert_eq!(source_state.sources.len(), 1);
    assert_eq!(
        source_state.sources[0].kind,
        earthmesh_cli::v3_data_source_io::V3DataSourceKind::Landtype
    );
    assert_eq!(source_state.sources[0].path, landtype_file);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn data_preprocess_source_state_can_be_built_from_mkgrd_config_with_grid_override() {
    let root = temp_root("data_preprocess_source_state_from_config");
    let landtype_file = root.join("landtype.nc");
    write_landtype_file(&landtype_file, 360, 180);
    let config = earthmesh_core::EarthmeshConfig::from_mkgrd_namelist(&format!(
        "&mkgrd\n  NL%EXPNME='case_source_config'\n  NL%base_dir='{}/'\n  NL%NXP=1\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%landtype_file='{}'\n  NL%gridnum_perdegree=120\n  NL%mask_domain_global=.true.\n  NL%mask_domain_type='bbox'\n  NL%refine=.true.\n  NL%output_format='CoLM'\n/\n",
        root.display(),
        landtype_file.display()
    ))
    .expect("parse mkgrd config");

    let source_state =
        earthmesh_cli::mkgrd_data_preprocess_source::build_mkgrd_data_preprocess_source_state_from_config_one_based(
            &root,
            &config,
            Some(1),
            17,
        )
        .expect("build source state from config");

    assert_eq!(source_state.gridnum_perdegree, 1);
    assert_eq!(source_state.nlons_source, 360);
    assert_eq!(source_state.nlats_source, 180);
    assert_eq!(source_state.first_triangle_id, 17);
    assert_eq!(source_state.num_vertex, 1);
    assert_eq!(source_state.sources[0].path, landtype_file);
    assert_eq!(source_state.seaorland[2][1], 1);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn data_preprocess_reads_real_landtype_latitude_longitude_variable_order() {
    let root = temp_root("data_preprocess_landtype_lat_lon_order");
    let landtype_file = root.join("landtype_lat_lon.nc");
    write_landtype_lat_lon_file(&landtype_file, 360, 180);

    let report =
        earthmesh_cli::mkgrd_data_preprocess_source::read_landtype_data_preprocess_one_based(
            &landtype_file,
            1,
        )
        .expect("read latitude-longitude landtype preprocess data");

    assert_eq!(report.nlons_source, 360);
    assert_eq!(report.nlats_source, 180);
    assert_eq!(report.maxlc, 7);
    assert_eq!(report.landtypes_global[1][1], 0);
    assert_eq!(report.landtypes_global[2][1], 2);
    assert_eq!(report.landtypes_global[3][2], 7);
    assert_eq!(report.landtypes_global[360][180], 4);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn data_preprocess_reads_landtype_file_axes_and_maxlc_like_canonical() {
    let root = temp_root("data_preprocess_landtype");
    let landtype_file = root.join("landtype.nc");
    write_landtype_file(&landtype_file, 360, 180);

    let report =
        earthmesh_cli::mkgrd_data_preprocess_source::read_landtype_data_preprocess_one_based(
            &landtype_file,
            1,
        )
        .expect("read landtype preprocess data");

    assert_eq!(report.nlons_source, 360);
    assert_eq!(report.nlats_source, 180);
    assert_eq!(report.maxlc, 7);
    assert_eq!(report.landtypes_global[1][1], 0);
    assert_eq!(report.landtypes_global[2][1], 2);
    assert_eq!(report.landtypes_global[3][2], 7);
    assert_eq!(report.landtypes_global[360][180], 4);
    assert!((report.lon_i[1] + 179.5).abs() < 1.0e-12);
    assert!((report.lat_i[1] - 89.5).abs() < 1.0e-12);
    assert_eq!(report.lon_vertex[1], -180.0);
    assert_eq!(report.lon_vertex[361], 180.0);
    assert_eq!(report.lat_vertex[1], 90.0);
    assert_eq!(report.lat_vertex[181], -90.0);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn landtype_bbox_window_matches_dense_reader_for_both_dimension_orders() {
    let root = temp_root("landtype_bbox_window_orders");
    let lon_lat_file = root.join("landtype_lon_lat.nc");
    let lat_lon_file = root.join("landtype_lat_lon.nc");
    write_landtype_file(&lon_lat_file, 360, 180);
    write_landtype_lat_lon_file(&lat_lon_file, 360, 180);
    let bounds = earthmesh_mesh::AreaJudgeSourceBounds {
        minlon_source: 1,
        maxlon_source: 4,
        maxlat_source: 1,
        minlat_source: 3,
    };

    for path in [&lon_lat_file, &lat_lon_file] {
        let dense =
            earthmesh_cli::mkgrd_data_preprocess_source::read_landtype_data_preprocess_one_based(
                path, 1,
            )
            .expect("read dense fixture");
        let window =
            earthmesh_cli::mkgrd_data_preprocess_source::read_landtype_bbox_window_one_based(
                path, 1, bounds,
            )
            .expect("read landtype window");

        assert_eq!(window.nlons, 4);
        assert_eq!(window.nlats, 3);
        assert_eq!(window.values.len(), 12);
        for lon_index in bounds.minlon_source..=bounds.maxlon_source {
            for lat_index in bounds.maxlat_source..=bounds.minlat_source {
                assert_eq!(
                    window.value_at_global(lon_index, lat_index),
                    Some(dense.landtypes_global[lon_index][lat_index] as i8),
                    "mismatch at ({lon_index},{lat_index}) for {}",
                    path.display()
                );
            }
        }
        assert_eq!(window.value_at_global(5, 1), None);
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn production_resolution_landtype_uses_sparse_window_and_rejects_dense_compatibility_read() {
    let root = temp_root("landtype_sparse_production_window");
    let landtype_file = root.join("landtype_30gpd.nc");
    declare_sparse_landtype_file(&landtype_file, 10_800, 5_400);
    let bounds = earthmesh_mesh::AreaJudgeSourceBounds {
        minlon_source: 4_000,
        maxlon_source: 4_015,
        maxlat_source: 2_000,
        minlat_source: 2_015,
    };

    let window = earthmesh_cli::mkgrd_data_preprocess_source::read_landtype_bbox_window_one_based(
        &landtype_file,
        30,
        bounds,
    )
    .expect("read sparse production-resolution window");
    assert_eq!(window.nlons, 16);
    assert_eq!(window.nlats, 16);
    assert_eq!(window.values.len(), 256);

    let err = earthmesh_cli::mkgrd_data_preprocess_source::read_landtype_data_preprocess_one_based(
        &landtype_file,
        30,
    )
    .expect_err("dense production-resolution read must be rejected before allocation");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(err
        .to_string()
        .contains("dense landtype compatibility reader"));
    assert!(err
        .to_string()
        .contains("read_landtype_bbox_window_one_based"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn data_preprocess_area_judge_source_builds_seaorland_from_landtype_file() {
    let root = temp_root("data_preprocess_area_judge_source");
    let landtype_file = root.join("landtype.nc");
    write_landtype_file(&landtype_file, 360, 180);

    let mut is_in_domain = vec![vec![0_i32; 181]; 361];
    is_in_domain[1][1] = 1;
    is_in_domain[2][1] = 1;
    is_in_domain[3][2] = 1;
    is_in_domain[4][2] = 0;
    let bounds = earthmesh_mesh::AreaJudgeSourceBounds {
        minlon_source: 1,
        maxlon_source: 4,
        maxlat_source: 1,
        minlat_source: 2,
    };

    let report = earthmesh_cli::mkgrd_data_preprocess_source::read_data_preprocess_area_judge_source_one_based(
        &landtype_file,
        1,
        &is_in_domain,
        bounds,
        "landmesh",
        true,
    )
    .expect("read data_preprocess and build Area_judge source state");

    assert_eq!(report.preprocess.maxlc, 7);
    assert_eq!(report.preprocess.landtypes_global[1][1], 0);
    assert_eq!(report.preprocess.landtypes_global[2][1], 2);
    assert_eq!(report.preprocess.landtypes_global[3][2], 7);
    assert_eq!(report.seaorland.sum_land_grid, 2);
    assert_eq!(report.seaorland.seaorland[1][1], 0);
    assert_eq!(report.seaorland.seaorland[2][1], 1);
    assert_eq!(report.seaorland.seaorland[3][2], 1);
    assert_eq!(report.seaorland.seaorland[360][180], 0);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn data_preprocess_area_judge_base_state_reads_landtype_file_before_domain_seaorland() {
    let root = temp_root("data_preprocess_area_judge_base_state");
    let landtype_file = root.join("landtype.nc");
    write_landtype_file(&landtype_file, 360, 180);

    let report = earthmesh_cli::mkgrd_data_preprocess_source::read_data_preprocess_area_judge_base_state_one_based(
        &root,
        &landtype_file,
        1,
        true,
        "bbox",
        1,
        "landmesh",
        true,
    )
    .expect("read data_preprocess and build Area_judge base state");

    assert_eq!(report.domain.bounds.minlon_source, 1);
    assert_eq!(report.domain.bounds.maxlon_source, 360);
    assert_eq!(report.domain.bounds.maxlat_source, 1);
    assert_eq!(report.domain.bounds.minlat_source, 180);
    assert_eq!(report.domain.numpatch, 360 * 180);
    assert_eq!(report.seaorland.sum_land_grid, 3);
    assert_eq!(report.seaorland.seaorland[1][1], 0);
    assert_eq!(report.seaorland.seaorland[2][1], 1);
    assert_eq!(report.seaorland.seaorland[3][2], 1);
    assert_eq!(report.seaorland.seaorland[360][180], 1);

    let _ = fs::remove_dir_all(root);
}
