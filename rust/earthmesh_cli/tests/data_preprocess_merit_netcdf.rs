use std::fs;
use std::path::{Path, PathBuf};

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
}

fn write_merit_fixture(path: &Path) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create MERIT tile fixture");
    file.add_dimension("longitude", 3)
        .expect("add longitude dimension");
    file.add_dimension("latitude", 3)
        .expect("add latitude dimension");
    file.add_variable::<f64>("longitude", &["longitude"])
        .expect("add longitude")
        .put_values(&[100.0, 100.5, 101.0], ..)
        .expect("write longitude");
    file.add_variable::<f64>("latitude", &["latitude"])
        .expect("add latitude")
        .put_values(&[10.0, 10.5, 11.0], ..)
        .expect("write latitude");
    file.add_variable::<i32>("dir", &["longitude", "latitude"])
        .expect("add dir")
        .put_values(&[1, 2, 3, 4, 5, 6, 7, 8, 9], (.., ..))
        .expect("write dir");
    file.add_variable::<f64>("upa", &["longitude", "latitude"])
        .expect("add upa")
        .put_values(
            &[
                100.0, 5_000.0, 50_000.0, 100.0, -9999.0, 8_000.0, 100.0, 200.0, 100.0,
            ],
            (.., ..),
        )
        .expect("write upa");
    file.add_variable::<f64>("elv", &["longitude", "latitude"])
        .expect("add elv")
        .put_values(&[1.0, 2.0, 3.0, 4.0, -9999.0, 6.0, 7.0, 8.0, 9.0], (.., ..))
        .expect("write elv");
    file.add_variable::<f64>("wth", &["longitude", "latitude"])
        .expect("add wth")
        .put_values(
            &[10.0, 50.0, 300.0, 10.0, -9999.0, 75.0, 10.0, 20.0, 10.0],
            (.., ..),
        )
        .expect("write wth");
    file.add_variable::<i32>("landtype_igbp", &["longitude", "latitude"])
        .expect("add landtype")
        .put_values(&[1, 1, 1, 0, 0, 1, 17, 1, 1], (.., ..))
        .expect("write landtype");
}

fn write_lat_lon_merit_fixture(path: &Path) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create transposed fixture");
    file.add_dimension("latitude", 5)
        .expect("add latitude dimension");
    file.add_dimension("longitude", 6)
        .expect("add longitude dimension");
    file.add_variable::<f64>("longitude", &["longitude"])
        .expect("add longitude")
        .put_values(&[100.0, 101.0, 102.0, 103.0, 104.0, 105.0], ..)
        .expect("write longitude");
    file.add_variable::<f64>("latitude", &["latitude"])
        .expect("add latitude")
        .put_values(&[15.0, 14.0, 13.0, 12.0, 11.0], ..)
        .expect("write descending latitude");

    let i16_values = (0..5)
        .flat_map(|lat| (0..6).map(move |lon| (lat * 100 + lon) as i16))
        .collect::<Vec<_>>();
    let i8_values = (0..5)
        .flat_map(|lat| (0..6).map(move |lon| (lat * 10 + lon) as i8))
        .collect::<Vec<_>>();
    let f32_values = i16_values
        .iter()
        .map(|value| f32::from(*value) + 0.5)
        .collect::<Vec<_>>();
    file.add_variable::<i16>("dir", &["latitude", "longitude"])
        .expect("add dir")
        .put_values(&i16_values, (.., ..))
        .expect("write dir");
    for name in ["upa", "elv", "wth"] {
        file.add_variable::<f32>(name, &["latitude", "longitude"])
            .expect("add floating matrix")
            .put_values(&f32_values, (.., ..))
            .expect("write floating matrix");
    }
    file.add_variable::<i8>("landtype_igbp", &["latitude", "longitude"])
        .expect("add landtype")
        .put_values(&i8_values, (.., ..))
        .expect("write landtype");
}

fn synthetic_merit_window(
    tile_name: &str,
    lon: Vec<f64>,
    landtype_igbp: Vec<i32>,
    sampling_stride: usize,
) -> earthmesh_cli::merit_hydro_io::MeritHydroWindowReport {
    let width = lon.len();
    let height = 1;
    earthmesh_cli::merit_hydro_io::MeritHydroWindowReport {
        tile: PathBuf::from(tile_name),
        tile_name: tile_name.to_string(),
        lon,
        lat: vec![0.0],
        width,
        height,
        sampling_stride,
        dir: vec![0; width * height],
        upa_km2: vec![0.0; width * height],
        elv_m: vec![0.0; width * height],
        width_m: vec![0.0; width * height],
        landtype_igbp,
    }
}

#[test]
fn merit_tile_discovery_parses_tile_names_and_selects_bbox_intersections() {
    let root = temp_root("data_preprocess_merit_tile_discovery");
    for name in [
        "n10e100.nc",
        "n15e100.nc",
        "s05w010.nc",
        "notes.txt",
        "bad.nc",
    ] {
        fs::write(root.join(name), b"placeholder").expect("write placeholder tile");
    }

    let north_bounds =
        earthmesh_cli::merit_tile_selection::merit_tile_bounds_from_name("n10e100.nc")
            .expect("parse northern eastern tile");
    assert_eq!(north_bounds.west, 100.0);
    assert_eq!(north_bounds.south, 10.0);
    assert_eq!(north_bounds.east, 105.0);
    assert_eq!(north_bounds.north, 15.0);
    let south_bounds =
        earthmesh_cli::merit_tile_selection::merit_tile_bounds_from_name("s05w010.nc")
            .expect("parse southern western tile");
    assert_eq!(south_bounds.west, -10.0);
    assert_eq!(south_bounds.south, -5.0);
    assert_eq!(south_bounds.east, -5.0);
    assert_eq!(south_bounds.north, 0.0);

    let selected = earthmesh_cli::merit_tile_selection::select_merit_hydro_tiles(
        &root,
        earthmesh_cli::merit_tile_selection::MeritLonLatBbox {
            west: 104.0,
            south: 14.0,
            east: 106.0,
            north: 16.0,
        },
    )
    .expect("select intersecting MERIT tiles");
    let names: Vec<String> = selected
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert_eq!(names, vec!["n10e100.nc", "n15e100.nc"]);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn merit_hydro_reader_selects_bbox_window_cleans_fill_and_classifies_masks() {
    let root = temp_root("data_preprocess_merit_reader");
    let tile = root.join("n10e100.nc");
    write_merit_fixture(&tile);

    let report = earthmesh_cli::merit_hydro_io::read_merit_hydro_window(
        &tile,
        earthmesh_cli::merit_tile_selection::MeritLonLatBbox {
            west: 100.0,
            south: 10.0,
            east: 100.5,
            north: 11.0,
        },
        1,
    )
    .expect("read MERIT hydro window");

    assert_eq!(report.tile_name, "n10e100.nc");
    assert_eq!(report.lon, vec![100.0, 100.5]);
    assert_eq!(report.lat, vec![10.0, 10.5, 11.0]);
    assert_eq!(report.width, 2);
    assert_eq!(report.height, 3);
    assert_eq!(report.dir, vec![1, 2, 3, 4, 5, 6]);
    assert_eq!(report.upa_km2[0], 100.0);
    assert_eq!(report.upa_km2[1], 5_000.0);
    assert!(report.upa_km2[4].is_nan());
    assert!(report.elv_m[4].is_nan());
    assert!(report.width_m[4].is_nan());
    assert_eq!(report.landtype_igbp, vec![1, 1, 1, 0, 0, 1]);

    let mask = earthmesh_cli::merit_hydro_io::classify_merit_hydro_window(
        &report,
        earthmesh_cli::merit_hydro_io::MeritMaskThresholds::default(),
    )
    .expect("classify MERIT hydro window");
    assert_eq!(
        mask.classes,
        vec!["COAST_LAND", "R2", "R3", "COAST_OCEAN", "COAST_OCEAN", "R2"]
    );
    assert_eq!(mask.r3_cells, 1);
    assert_eq!(mask.r2_cells, 2);
    assert_eq!(mask.coast_land_cells, 1);
    assert_eq!(mask.coast_ocean_cells, 2);
    assert_eq!(mask.land_cells, 0);
    assert_eq!(mask.ocean_cells, 0);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn merit_hydro_reader_uses_strided_hyperslab_for_lat_lon_descending_axis() {
    let root = temp_root("data_preprocess_merit_hyperslab");
    let tile = root.join("n10e100.nc");
    write_lat_lon_merit_fixture(&tile);

    let report = earthmesh_cli::merit_hydro_io::read_merit_hydro_window(
        &tile,
        earthmesh_cli::merit_tile_selection::MeritLonLatBbox {
            west: 101.0,
            south: 11.0,
            east: 104.0,
            north: 14.0,
        },
        2,
    )
    .expect("read transposed strided MERIT window");

    assert_eq!(report.lon, vec![101.0, 103.0]);
    assert_eq!(report.lat, vec![14.0, 12.0]);
    assert_eq!((report.width, report.height), (2, 2));
    assert_eq!(report.sampling_stride, 2);
    assert_eq!(report.dir, vec![101, 301, 103, 303]);
    assert_eq!(report.upa_km2, vec![101.5, 301.5, 103.5, 303.5]);
    assert_eq!(report.elv_m, report.upa_km2);
    assert_eq!(report.width_m, report.upa_km2);
    assert_eq!(report.landtype_igbp, vec![11, 31, 13, 33]);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn merit_hydro_writer_exports_combined_and_split_geojson_layers_from_native_windows() {
    let root = temp_root("data_preprocess_merit_geojson_layers");
    let tile = root.join("n10e100.nc");
    write_merit_fixture(&tile);
    let output_dir = root.join("out");
    let window = earthmesh_cli::merit_hydro_io::read_merit_hydro_window(
        &tile,
        earthmesh_cli::merit_tile_selection::MeritLonLatBbox {
            west: 100.0,
            south: 10.0,
            east: 100.5,
            north: 11.0,
        },
        1,
    )
    .expect("read MERIT hydro window");

    let report = earthmesh_cli::merit_hydro_io::write_merit_hydro_mask_geojson_layers(
        &[window],
        earthmesh_cli::merit_hydro_io::MeritMaskThresholds::default(),
        &output_dir,
        true,
    )
    .expect("write MERIT GeoJSON layers");

    assert_eq!(report.window_count, 1);
    assert_eq!(report.combined_feature_count, 6);
    assert_eq!(report.river_feature_count, 3);
    assert_eq!(report.coast_feature_count, 3);
    assert_eq!(report.surface_feature_count, 0);
    assert_eq!(report.mask_counts.get("R3"), Some(&1));
    assert_eq!(report.mask_counts.get("R2"), Some(&2));
    assert_eq!(report.mask_counts.get("COAST_LAND"), Some(&1));
    assert_eq!(report.mask_counts.get("COAST_OCEAN"), Some(&2));
    assert!(report.combined_geojson.ends_with("merit_masks.geojson"));
    assert!(report.river_geojson.ends_with("merit_river_masks.geojson"));
    assert!(report.coast_geojson.ends_with("merit_coast_masks.geojson"));
    assert_eq!(
        report
            .surface_geojson
            .as_ref()
            .unwrap()
            .file_name()
            .unwrap(),
        "merit_surface_masks.geojson"
    );

    let combined = fs::read_to_string(&report.combined_geojson).expect("read combined geojson");
    assert!(combined.starts_with("{\"type\":\"FeatureCollection\",\"features\":["));
    assert!(combined.contains(r#""geometry":{"type":"Polygon","coordinates":[[[99.75,9.75],[100.25,9.75],[100.25,10.25],[99.75,10.25],[99.75,9.75]]]}"#));
    assert!(combined.contains(r#""feature_id":"n10e100:0:0:COAST_LAND""#));
    assert!(combined.contains(r#""mask_class":"R3""#));
    assert!(combined.contains(r#""source":"MERIT-Hydro""#));
    assert!(combined.contains(r#""upstream_area_km2":50000"#));
    assert!(combined.contains(r#""width_m":300"#));
    assert!(combined.contains(r#""elevation_m":null"#));

    let river = fs::read_to_string(&report.river_geojson).expect("read river geojson");
    assert_eq!(river.matches(r#""type":"Feature""#).count(), 3);
    assert!(river.contains(r#""mask_class":"R2""#));
    assert!(river.contains(r#""mask_class":"R3""#));
    assert!(!river.contains("COAST_OCEAN"));

    let coast = fs::read_to_string(&report.coast_geojson).expect("read coast geojson");
    assert_eq!(coast.matches(r#""type":"Feature""#).count(), 3);
    assert!(coast.contains("COAST_LAND"));
    assert!(coast.contains("COAST_OCEAN"));
    assert!(!coast.contains(r#""mask_class":"R2""#));

    let summary = fs::read_to_string(&report.summary_json).expect("read summary json");
    assert!(summary.contains(r#""tile_count":1"#));
    assert!(summary.contains(r#""feature_count":6"#));
    assert!(summary.contains(r#""R2":2"#));
    assert!(summary.contains(r#""COAST_OCEAN":2"#));
    assert!(summary.contains(r#""r3_width_m":300"#));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn merit_hydro_writer_classifies_coast_across_window_seams() {
    let root = temp_root("data_preprocess_merit_window_seam");
    let windows = vec![
        synthetic_merit_window("left.nc", vec![0.0, 1.0], vec![1, 1], 1),
        synthetic_merit_window("right.nc", vec![2.0, 3.0], vec![0, 0], 1),
    ];

    let report = earthmesh_cli::merit_hydro_io::write_merit_hydro_mask_geojson_layers(
        &windows,
        earthmesh_cli::merit_hydro_io::MeritMaskThresholds::default(),
        root.join("out"),
        true,
    )
    .expect("classify seam with all windows in one native adjacency grid");

    assert_eq!(report.coast_feature_count, 2);
    assert_eq!(report.mask_counts.get("COAST_LAND"), Some(&1));
    assert_eq!(report.mask_counts.get("COAST_OCEAN"), Some(&1));
    assert_eq!(report.mask_counts.get("LAND"), Some(&1));
    assert_eq!(report.mask_counts.get("OCEAN"), Some(&1));
    let coast = fs::read_to_string(report.coast_geojson).expect("read seam coast layer");
    assert!(coast.contains(r#""feature_id":"left:1:0:COAST_LAND""#));
    assert!(coast.contains(r#""feature_id":"right:0:0:COAST_OCEAN""#));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn merit_hydro_coast_classification_rejects_strided_samples() {
    let window = synthetic_merit_window("strided.nc", vec![0.0, 2.0], vec![1, 0], 2);
    let err = earthmesh_cli::merit_hydro_io::classify_merit_hydro_window(
        &window,
        earthmesh_cli::merit_hydro_io::MeritMaskThresholds::default(),
    )
    .expect_err("strided samples are not native neighbors");
    assert!(err.to_string().contains("requires native stride 1"));
}
