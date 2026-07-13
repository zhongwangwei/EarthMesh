use std::fs;
use std::path::PathBuf;

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
}

fn write_f32_grid(path: &std::path::Path, rows: &[Vec<f32>]) {
    let mut bytes = Vec::new();
    for row in rows {
        for value in row {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    fs::write(path, bytes).expect("write f32 binary grid");
}

#[test]
fn cama_elevtn_reader_reads_y_reversed_float32_window_and_classifies_surface() {
    let root = temp_root("data_preprocess_cama_binary");
    let elevtn_path = root.join("elevtn.bin");

    let logical_south = vec![10.0_f32, 20.0, -9999.0, 40.0];
    let logical_middle = vec![-9999.0_f32, -5.0, f32::INFINITY, 80.0];
    let logical_north = vec![1.0_f32, 2.0, 3.0, 4.0];
    write_f32_grid(
        &elevtn_path,
        &[
            logical_north.clone(),
            logical_middle.clone(),
            logical_south.clone(),
        ],
    );

    let grid = earthmesh_cli::cama_binary_io::CamaBinaryGridSpec {
        nx: 4,
        ny: 3,
        west: 100.0,
        south: 10.0,
        grid_size_deg: 0.5,
        little_endian: true,
        y_reversed_storage: true,
    };
    let window = grid
        .window_for_bbox(100.5, 101.5, 10.0, 11.5)
        .expect("bbox overlaps grid");
    assert_eq!(
        window,
        earthmesh_cli::cama_binary_io::CamaBinaryWindow {
            x_start: 1,
            y_start: 0,
            width: 2,
            height: 3,
        }
    );

    let report = earthmesh_cli::cama_binary_window_readers::read_cama_elevtn_surface_window(
        &elevtn_path,
        grid,
        window,
        -9999.0,
    )
    .expect("read CaMa elevtn window");

    assert_eq!(report.window, window);
    assert_eq!(report.elevation[0], vec![20.0, -9999.0]);
    assert_eq!(report.elevation[1][0], -5.0);
    assert!(report.elevation[1][1].is_infinite());
    assert_eq!(report.elevation[2], vec![2.0, 3.0]);
    assert_eq!(
        report.surface_mask,
        vec![
            vec![
                earthmesh_cli::cama_binary_io::CamaSurfaceClass::Land,
                earthmesh_cli::cama_binary_io::CamaSurfaceClass::Ocean
            ],
            vec![
                earthmesh_cli::cama_binary_io::CamaSurfaceClass::Land,
                earthmesh_cli::cama_binary_io::CamaSurfaceClass::Ocean
            ],
            vec![
                earthmesh_cli::cama_binary_io::CamaSurfaceClass::Land,
                earthmesh_cli::cama_binary_io::CamaSurfaceClass::Land
            ],
        ]
    );
    assert_eq!(report.land_cells, 4);
    assert_eq!(report.ocean_cells, 2);

    let _ = fs::remove_dir_all(root);
}

fn write_i32_planes(path: &std::path::Path, planes: &[Vec<Vec<i32>>]) {
    let mut bytes = Vec::new();
    for plane in planes {
        for row in plane {
            for value in row {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
    fs::write(path, bytes).expect("write i32 binary planes");
}

#[test]
fn cama_nextxy_reader_reads_planar_int32_topology_and_converts_to_logical_zero_based_indices() {
    let root = temp_root("data_preprocess_cama_nextxy_binary");
    let nextxy_path = root.join("nextxy.bin");

    let grid = earthmesh_cli::cama_binary_io::CamaBinaryGridSpec {
        nx: 4,
        ny: 3,
        west: 100.0,
        south: 10.0,
        grid_size_deg: 0.5,
        little_endian: true,
        y_reversed_storage: true,
    };
    let window = earthmesh_cli::cama_binary_io::CamaBinaryWindow {
        x_start: 1,
        y_start: 0,
        width: 2,
        height: 3,
    };

    let logical_south_x = vec![1, 2, 0, 4];
    let logical_middle_x = vec![4, 3, 2, 1];
    let logical_north_x = vec![0, 1, 2, 3];
    let logical_south_y = vec![3, 2, 0, 1];
    let logical_middle_y = vec![2, 1, 3, 0];
    let logical_north_y = vec![1, 3, 2, 0];

    write_i32_planes(
        &nextxy_path,
        &[
            vec![
                logical_north_x.clone(),
                logical_middle_x.clone(),
                logical_south_x.clone(),
            ],
            vec![
                logical_north_y.clone(),
                logical_middle_y.clone(),
                logical_south_y.clone(),
            ],
        ],
    );

    let report = earthmesh_cli::cama_binary_window_readers::read_cama_nextxy_window(
        &nextxy_path,
        grid,
        window,
    )
    .expect("read CaMa nextxy topology window");

    assert_eq!(report.grid, grid);
    assert_eq!(report.window, window);
    assert_eq!(report.next_x, vec![vec![1, 0], vec![2, 1], vec![0, 1]]);
    assert_eq!(report.next_y, vec![vec![1, 0], vec![2, 0], vec![0, 1]]);
    assert_eq!(
        report.terminal_or_ocean,
        vec![vec![false, true], vec![false, false], vec![false, false]]
    );
    assert_eq!(report.valid_downstream_links, 5);
    assert_eq!(report.terminal_or_ocean_links, 1);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn cama_metric_reader_reads_float32_windows_for_width_uparea_and_rivlen() {
    let root = temp_root("data_preprocess_cama_metric_binary");
    let width_path = root.join("width.bin");
    let uparea_path = root.join("uparea.bin");
    let rivlen_path = root.join("rivlen.bin");

    let grid = earthmesh_cli::cama_binary_io::CamaBinaryGridSpec {
        nx: 4,
        ny: 3,
        west: 100.0,
        south: 10.0,
        grid_size_deg: 0.5,
        little_endian: true,
        y_reversed_storage: true,
    };
    let window = earthmesh_cli::cama_binary_io::CamaBinaryWindow {
        x_start: 1,
        y_start: 0,
        width: 2,
        height: 3,
    };

    write_f32_grid(
        &width_path,
        &[
            vec![0.0, 7.0, 8.0, 9.0],
            vec![0.0, 5.0, -1.0, 6.0],
            vec![0.0, 1.5, 0.0, 2.5],
        ],
    );
    write_f32_grid(
        &uparea_path,
        &[
            vec![0.0, 70.0, 80.0, 90.0],
            vec![0.0, 50.0, 0.0, 60.0],
            vec![0.0, 15.0, 25.0, 35.0],
        ],
    );
    write_f32_grid(
        &rivlen_path,
        &[
            vec![0.0, 700.0, 800.0, 900.0],
            vec![0.0, 500.0, 600.0, 0.0],
            vec![0.0, 150.0, 250.0, 350.0],
        ],
    );

    let width = earthmesh_cli::cama_binary_window_readers::read_cama_float32_metric_window(
        &width_path,
        grid,
        window,
        earthmesh_cli::cama_binary_io::CamaMetricKind::RiverWidth,
    )
    .expect("read width metric");
    assert_eq!(
        width.kind,
        earthmesh_cli::cama_binary_io::CamaMetricKind::RiverWidth
    );
    assert_eq!(
        width.values,
        vec![vec![1.5, 0.0], vec![5.0, -1.0], vec![7.0, 8.0]]
    );
    assert_eq!(width.positive_cells, 4);
    assert_eq!(width.non_positive_or_invalid_cells, 2);

    let uparea = earthmesh_cli::cama_binary_window_readers::read_cama_float32_metric_window(
        &uparea_path,
        grid,
        window,
        earthmesh_cli::cama_binary_io::CamaMetricKind::UpstreamArea,
    )
    .expect("read uparea metric");
    assert_eq!(
        uparea.values,
        vec![vec![15.0, 25.0], vec![50.0, 0.0], vec![70.0, 80.0]]
    );
    assert_eq!(uparea.positive_cells, 5);

    let rivlen = earthmesh_cli::cama_binary_window_readers::read_cama_float32_metric_window(
        &rivlen_path,
        grid,
        window,
        earthmesh_cli::cama_binary_io::CamaMetricKind::RiverLength,
    )
    .expect("read rivlen metric");
    assert_eq!(
        rivlen.values,
        vec![vec![150.0, 250.0], vec![500.0, 600.0], vec![700.0, 800.0]]
    );
    assert_eq!(rivlen.positive_cells, 6);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn cama_reach_inventory_combines_metrics_and_nextxy_into_river_source_records() {
    let grid = earthmesh_cli::cama_binary_io::CamaBinaryGridSpec {
        nx: 4,
        ny: 3,
        west: 100.0,
        south: 10.0,
        grid_size_deg: 0.5,
        little_endian: true,
        y_reversed_storage: true,
    };
    let window = earthmesh_cli::cama_binary_io::CamaBinaryWindow {
        x_start: 1,
        y_start: 0,
        width: 2,
        height: 2,
    };
    let uparea = earthmesh_cli::cama_binary_io::CamaMetricWindowReport {
        grid,
        window,
        kind: earthmesh_cli::cama_binary_io::CamaMetricKind::UpstreamArea,
        values: vec![vec![10.0, 20.0], vec![30.0, 40.0]],
        positive_cells: 4,
        non_positive_or_invalid_cells: 0,
    };
    let width = earthmesh_cli::cama_binary_io::CamaMetricWindowReport {
        grid,
        window,
        kind: earthmesh_cli::cama_binary_io::CamaMetricKind::RiverWidth,
        values: vec![vec![5.0, 0.0], vec![7.5, 8.5]],
        positive_cells: 3,
        non_positive_or_invalid_cells: 1,
    };
    let rivlen = earthmesh_cli::cama_binary_io::CamaMetricWindowReport {
        grid,
        window,
        kind: earthmesh_cli::cama_binary_io::CamaMetricKind::RiverLength,
        values: vec![vec![100.0, 200.0], vec![0.0, 400.0]],
        positive_cells: 3,
        non_positive_or_invalid_cells: 1,
    };
    let nextxy = earthmesh_cli::cama_binary_io::CamaNextxyWindowReport {
        grid,
        window,
        next_x: vec![vec![0, 1], vec![2, -9]],
        next_y: vec![vec![0, 1], vec![0, -9]],
        terminal_or_ocean: vec![vec![false, false], vec![false, true]],
        valid_downstream_links: 3,
        terminal_or_ocean_links: 1,
    };

    let inventory = earthmesh_cli::cama_reach_inventory::build_cama_reach_inventory(
        grid, window, 2.5, 1000.0, &uparea, &width, &rivlen, &nextxy,
    )
    .expect("build CaMa reach inventory");

    assert_eq!(inventory.records.len(), 2);
    assert_eq!(inventory.valid_channel_cells, 2);
    assert_eq!(inventory.skipped_cells, 2);
    assert_eq!(inventory.records[0].reach_id, "cama-0-1");
    assert_eq!(inventory.records[0].x_index, 1);
    assert_eq!(inventory.records[0].y_index, 0);
    assert!((inventory.records[0].lon - 100.75).abs() < 1.0e-12);
    assert!((inventory.records[0].lat - 10.25).abs() < 1.0e-12);
    assert_eq!(inventory.records[0].upstream_area_km2, 10000.0);
    assert_eq!(inventory.records[0].width_m, 5.0);
    assert_eq!(inventory.records[0].river_length_m, 100.0);
    assert_eq!(inventory.records[0].target_dx_km, 2.5);
    assert!(!inventory.records[0].is_estuary);

    assert_eq!(inventory.records[1].reach_id, "cama-1-2");
    assert_eq!(inventory.records[1].x_index, 2);
    assert_eq!(inventory.records[1].y_index, 1);
    assert_eq!(inventory.records[1].downstream_x, -9);
    assert_eq!(inventory.records[1].downstream_y, -9);
    assert!(inventory.records[1].is_estuary);
    assert_eq!(inventory.records[1].upstream_area_km2, 40000.0);
    assert_eq!(inventory.records[1].width_m, 8.5);
    assert_eq!(inventory.records[1].river_length_m, 400.0);
}

#[test]
fn cama_map_dir_loader_reads_params_bbox_and_prefers_rivwth_for_reach_inventory() {
    let root = temp_root("data_preprocess_cama_map_dir_loader");
    fs::write(
        root.join("params.txt"),
        "4 !! endian=little\n3\n1\n0.5\n100.0\n102.0\n10.0\n11.5\n",
    )
    .expect("write params.txt");

    write_f32_grid(
        &root.join("uparea.bin"),
        &[
            vec![0.0, 70.0, 80.0, 90.0],
            vec![0.0, 30.0, 40.0, 60.0],
            vec![0.0, 10.0, 20.0, 35.0],
        ],
    );
    write_f32_grid(
        &root.join("width.bin"),
        &[
            vec![0.0, -70.0, -80.0, -90.0],
            vec![0.0, -30.0, -40.0, -60.0],
            vec![0.0, -10.0, -20.0, -35.0],
        ],
    );
    write_f32_grid(
        &root.join("rivwth.bin"),
        &[
            vec![0.0, 7.0, 8.0, 9.0],
            vec![0.0, 3.0, 4.0, 6.0],
            vec![0.0, 1.0, 2.0, 3.5],
        ],
    );
    write_f32_grid(
        &root.join("rivlen.bin"),
        &[
            vec![0.0, 700.0, 800.0, 900.0],
            vec![0.0, 300.0, 400.0, 600.0],
            vec![0.0, 100.0, 200.0, 350.0],
        ],
    );
    write_i32_planes(
        &root.join("nextxy.bin"),
        &[
            vec![vec![0, 1, 2, 3], vec![0, 1, 2, 3], vec![0, 1, -9, 3]],
            vec![vec![0, 3, 2, 1], vec![0, 2, 1, 3], vec![0, 1, -9, 3]],
        ],
    );

    let inventory = earthmesh_cli::cama_reach_inventory::read_cama_reach_inventory_from_map_dir(
        &root,
        earthmesh_cli::cama_binary_io::CamaLonLatBbox {
            west: 100.5,
            east: 101.5,
            south: 10.0,
            north: 11.0,
        },
        2.5,
        1000.0,
        true,
    )
    .expect("load reach inventory from CaMa map dir");

    assert_eq!(
        inventory.window,
        earthmesh_cli::cama_binary_io::CamaBinaryWindow {
            x_start: 1,
            y_start: 0,
            width: 2,
            height: 2,
        }
    );
    assert_eq!(inventory.grid.nx, 4);
    assert_eq!(inventory.grid.ny, 3);
    assert_eq!(inventory.records.len(), 4);
    assert_eq!(inventory.records[0].reach_id, "cama-0-1");
    assert_eq!(inventory.records[0].upstream_area_km2, 10000.0);
    assert_eq!(inventory.records[0].width_m, 1.0);
    assert_eq!(inventory.records[0].river_length_m, 100.0);
    assert_eq!(inventory.records[1].reach_id, "cama-0-2");
    assert_eq!(inventory.records[1].width_m, 2.0);
    assert!(inventory.records[1].is_estuary);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn cama_reach_inventory_jsonl_export_writes_hydro_source_records() {
    let root = temp_root("data_preprocess_cama_jsonl_export");
    let output = root.join("cama_reaches.jsonl");
    let grid = earthmesh_cli::cama_binary_io::CamaBinaryGridSpec {
        nx: 2,
        ny: 1,
        west: 100.0,
        south: 10.0,
        grid_size_deg: 0.5,
        little_endian: true,
        y_reversed_storage: false,
    };
    let window = earthmesh_cli::cama_binary_io::CamaBinaryWindow {
        x_start: 0,
        y_start: 0,
        width: 2,
        height: 1,
    };
    let inventory = earthmesh_cli::cama_binary_io::CamaReachInventoryReport {
        grid,
        window,
        records: vec![
            earthmesh_cli::cama_binary_io::CamaReachRecord {
                reach_id: "cama-0-0".to_string(),
                x_index: 0,
                y_index: 0,
                lon: 100.25,
                lat: 10.25,
                upstream_area_km2: 12_000.0,
                width_m: 45.0,
                floodplain_width_m: 0.0,
                target_dx_km: 2.5,
                is_estuary: false,
                river_length_m: 900.0,
                downstream_x: 1,
                downstream_y: 0,
            },
            earthmesh_cli::cama_binary_io::CamaReachRecord {
                reach_id: "cama-0-1".to_string(),
                x_index: 1,
                y_index: 0,
                lon: 100.75,
                lat: 10.25,
                upstream_area_km2: 55_000.0,
                width_m: 120.0,
                floodplain_width_m: 0.0,
                target_dx_km: 2.5,
                is_estuary: true,
                river_length_m: 800.0,
                downstream_x: 0,
                downstream_y: 0,
            },
        ],
        valid_channel_cells: 2,
        skipped_cells: 0,
    };

    let report =
        earthmesh_cli::cama_reach_inventory::write_cama_reach_inventory_jsonl(&inventory, &output)
            .expect("write reach inventory JSONL");

    assert_eq!(report.output, output);
    assert_eq!(report.record_count, 2);
    let text = fs::read_to_string(&report.output).expect("read jsonl export");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains(r#""reach_id":"cama-0-0""#));
    assert!(lines[0].contains(r#""upstream_area_km2":12000"#));
    assert!(lines[0].contains(r#""width_m":45"#));
    assert!(lines[0].contains(r#""river_length_m":900"#));
    assert!(lines[0].contains(r#""downstream_x":1"#));
    assert!(lines[0].contains(r#""river_class":"R2""#));
    assert!(lines[0].contains(r#""effective_width_m":45"#));
    assert!(lines[0].contains(r#""class_reasons":["upstream_area_r2"]"#));
    assert!(lines[1].contains(r#""reach_id":"cama-0-1""#));
    assert!(lines[1].contains(r#""is_estuary":true"#));
    assert!(lines[1].contains(r#""river_class":"R3""#));
    assert!(lines[1].contains(r#""class_reasons":["estuary"]"#));
    assert!(lines[1].contains(r#""source":"cama_reach_inventory""#));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn cama_reach_inventory_geojson_export_writes_point_feature_collection() {
    let root = temp_root("data_preprocess_cama_geojson_export");
    let output = root.join("cama_reaches.geojson");
    let grid = earthmesh_cli::cama_binary_io::CamaBinaryGridSpec {
        nx: 2,
        ny: 1,
        west: 100.0,
        south: 10.0,
        grid_size_deg: 0.5,
        little_endian: true,
        y_reversed_storage: false,
    };
    let window = earthmesh_cli::cama_binary_io::CamaBinaryWindow {
        x_start: 0,
        y_start: 0,
        width: 2,
        height: 1,
    };
    let inventory = earthmesh_cli::cama_binary_io::CamaReachInventoryReport {
        grid,
        window,
        records: vec![
            earthmesh_cli::cama_binary_io::CamaReachRecord {
                reach_id: "cama-0-0".to_string(),
                x_index: 0,
                y_index: 0,
                lon: 100.25,
                lat: 10.25,
                upstream_area_km2: 12_000.0,
                width_m: 45.0,
                floodplain_width_m: 0.0,
                target_dx_km: 2.5,
                is_estuary: false,
                river_length_m: 900.0,
                downstream_x: 1,
                downstream_y: 0,
            },
            earthmesh_cli::cama_binary_io::CamaReachRecord {
                reach_id: "cama-0-1".to_string(),
                x_index: 1,
                y_index: 0,
                lon: 100.75,
                lat: 10.25,
                upstream_area_km2: 55_000.0,
                width_m: 120.0,
                floodplain_width_m: 0.0,
                target_dx_km: 2.5,
                is_estuary: true,
                river_length_m: 800.0,
                downstream_x: 0,
                downstream_y: 0,
            },
        ],
        valid_channel_cells: 2,
        skipped_cells: 0,
    };

    let report = earthmesh_cli::cama_reach_inventory::write_cama_reach_inventory_point_geojson(
        &inventory, &output,
    )
    .expect("write reach inventory point GeoJSON");

    assert_eq!(report.output, output);
    assert_eq!(report.feature_count, 2);
    let text = fs::read_to_string(&report.output).expect("read geojson export");
    assert!(text.starts_with("{\"type\":\"FeatureCollection\",\"features\":["));
    assert!(text.ends_with("]}\n"));
    assert!(text.contains(r#""geometry":{"type":"Point","coordinates":[100.25,10.25]}"#));
    assert!(text.contains(r#""downstream_x":1,"downstream_y":0"#));
    assert!(text.contains(r#""source":"cama_reach_inventory""#));
    assert!(text.contains(r#""reach_id":"cama-0-0""#));
    assert!(text.contains(r#""river_class":"R2""#));
    assert!(text.contains(r#""class_reasons":["upstream_area_r2"]"#));
    assert!(text.contains(r#""geometry":{"type":"Point","coordinates":[100.75,10.25]}"#));
    assert!(text.contains(r#""is_estuary":true"#));
    assert!(text.contains(r#""river_class":"R3""#));
    assert!(text.contains(r#""class_reasons":["estuary"]"#));

    let _ = fs::remove_dir_all(root);
}
