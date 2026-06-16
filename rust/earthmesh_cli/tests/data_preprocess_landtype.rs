use std::fs;
use std::path::PathBuf;

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
}

fn write_landtype_file(path: &std::path::Path, nlons: usize, nlats: usize) {
    let mut file = netcdf::create(path).expect("create landtype file");
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

fn write_landtype_lat_lon_file(path: &std::path::Path, nlons: usize, nlats: usize) {
    let mut file = netcdf::create(path).expect("create lat-lon landtype file");
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

#[test]
fn data_preprocess_v3_source_descriptors_cover_landtype_threshold_hydro_and_coast_inputs() {
    let root = temp_root("data_preprocess_v3_sources");
    let landtype_file = root.join("landtype.nc");
    write_landtype_file(&landtype_file, 360, 180);

    let landtype = earthmesh_cli::build_v3_data_source_descriptor(
        earthmesh_cli::V3DataSourceKind::Landtype,
        &landtype_file,
    )
    .expect("landtype source descriptor");
    assert_eq!(landtype.kind, earthmesh_cli::V3DataSourceKind::Landtype);
    assert_eq!(landtype.semantic_layers, vec!["landtype"]);

    let hydro = earthmesh_cli::build_v3_data_source_descriptor(
        earthmesh_cli::V3DataSourceKind::Hydro,
        root.join("MERIT_Hydro"),
    )
    .expect("hydro source descriptor");
    assert_eq!(
        hydro.semantic_layers,
        vec!["river_r2", "river_r3", "estuary"]
    );

    let coast = earthmesh_cli::build_v3_data_source_descriptor(
        earthmesh_cli::V3DataSourceKind::Coast,
        root.join("coast"),
    )
    .expect("coast source descriptor");
    assert_eq!(
        coast.semantic_layers,
        vec!["coast_land", "coast_ocean", "shoreline"]
    );

    let threshold = earthmesh_cli::build_v3_data_source_descriptor(
        earthmesh_cli::V3DataSourceKind::Threshold,
        root.join("thresholds"),
    )
    .expect("threshold source descriptor");
    assert_eq!(threshold.semantic_layers, vec!["threshold_fields"]);

    let report = earthmesh_cli::read_landtype_data_preprocess_fortran_indexed(&landtype_file, 1)
        .expect("read landtype preprocess data");
    assert_eq!(
        report.source.kind,
        earthmesh_cli::V3DataSourceKind::Landtype
    );
    assert_eq!(report.source.path, landtype_file);

    let source_state = earthmesh_cli::build_mkgrd_data_preprocess_source_state_fortran_indexed(
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
        earthmesh_cli::V3DataSourceKind::Landtype
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
        earthmesh_cli::build_mkgrd_data_preprocess_source_state_from_config_fortran_indexed(
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

    let report = earthmesh_cli::read_landtype_data_preprocess_fortran_indexed(&landtype_file, 1)
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
fn data_preprocess_reads_landtype_file_axes_and_maxlc_like_fortran() {
    let root = temp_root("data_preprocess_landtype");
    let landtype_file = root.join("landtype.nc");
    write_landtype_file(&landtype_file, 360, 180);

    let report = earthmesh_cli::read_landtype_data_preprocess_fortran_indexed(&landtype_file, 1)
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

    let source_grid = report.refine_prepare_source_grid(11);
    assert_eq!(source_grid.gridnum_perdegree, 1);
    assert_eq!(source_grid.nlons_source, 360);
    assert_eq!(source_grid.nlats_source, 180);
    assert_eq!(source_grid.first_triangle_id, 11);
    assert_eq!(source_grid.lon_i[1], report.lon_i[1]);
    assert_eq!(source_grid.lat_vertex[181], -90.0);

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

    let report = earthmesh_cli::read_data_preprocess_area_judge_source_fortran_indexed(
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

    let report = earthmesh_cli::read_data_preprocess_area_judge_base_state_fortran_indexed(
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
