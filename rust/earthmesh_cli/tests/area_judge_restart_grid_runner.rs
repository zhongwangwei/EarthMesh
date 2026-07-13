use std::path::PathBuf;

use earthmesh_cli::{
    area_judge_grid_io::read_area_judge_grid_netcdf,
    area_judge_grid_io::run_area_judge_restart_grid_one_based,
    area_judge_grid_io::write_area_judge_grid_netcdf, area_judge_grid_io::AreaJudgeGridPayload,
    area_judge_grid_io::AreaJudgeRestartGridRunConfig,
    area_judge_grid_runs::run_area_judge_restart_grids_one_based,
    area_judge_types::AreaJudgeCalculatedRefineConfig,
    area_judge_types::AreaJudgeRestartGridsRunConfig, bbox_mask_io::write_bbox_mask_netcdf,
    bbox_mask_io::BBoxMask, bbox_mask_io::BBoxPoint,
};
use earthmesh_mesh::AreaJudgeSourceBounds;

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("create temp root");
    path
}

fn small_axes() -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
    let lon_vertex = vec![f64::NAN, -180.0, -179.0, -178.0, -177.0, -176.0, -175.0];
    let lat_vertex = vec![f64::NAN, 90.0, 89.0, 88.0, 87.0, 86.0, 85.0];
    let lon_i = std::iter::once(f64::NAN)
        .chain((0..6).map(|idx| -179.5 + idx as f64))
        .collect::<Vec<_>>();
    let lat_i = std::iter::once(f64::NAN)
        .chain((0..6).map(|idx| 89.5 - idx as f64))
        .collect::<Vec<_>>();
    (lon_vertex, lat_vertex, lon_i, lat_i)
}

#[test]
fn restart_grid_runner_reads_file_and_expands_selected_domain_state() {
    let root = temp_root("area_judge_restart_grid_runner");
    let input = root.join("result/IsInDmArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &input,
        &AreaJudgeGridPayload {
            bounds: AreaJudgeSourceBounds {
                minlon_source: 2,
                maxlon_source: 3,
                maxlat_source: 1,
                minlat_source: 2,
            },
            longitude: vec![20.0, 30.0],
            latitude: vec![50.0, 40.0],
            is_in_area_select: vec![vec![21, 22], vec![31, 32]],
            seaorland_select: Some(vec![vec![1, 0], vec![0, 1]]),
        },
    )
    .expect("write restart input");

    let report = run_area_judge_restart_grid_one_based(AreaJudgeRestartGridRunConfig {
        input: &input,
        nlons_source: 4,
        nlats_source: 4,
    })
    .expect("read and expand Area_judge restart grid");

    assert_eq!(report.input, input);
    assert_eq!(report.payload.bounds.minlon_source, 2);
    assert_eq!(report.expanded.nlons_select, 2);
    assert_eq!(report.expanded.nlats_select, 2);
    assert_eq!(report.expanded.is_in_domain[2][1], 21);
    assert_eq!(report.expanded.is_in_domain[2][2], 22);
    assert_eq!(report.expanded.is_in_domain[3][1], 31);
    assert_eq!(report.expanded.is_in_domain[3][2], 32);
    assert_eq!(report.expanded.is_in_domain[1][1], 0);
    assert_eq!(report.expanded.seaorland[2][1], 1);
    assert_eq!(report.expanded.seaorland[2][2], 0);
    assert_eq!(report.expanded.seaorland[3][1], 0);
    assert_eq!(report.expanded.seaorland[3][2], 1);
}

#[test]
fn restart_grids_runner_continues_iter_zero_refine_and_writes_selected_grid() {
    let root = temp_root("area_judge_restart_grids_refine");
    std::fs::create_dir_all(root.join("tmpfile")).expect("create tmpfile dir");
    let input = root.join("result/IsInDmArea_grid.nc4");
    let refine_output = root.join("result/IsInRfArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &input,
        &AreaJudgeGridPayload {
            bounds: AreaJudgeSourceBounds {
                minlon_source: 2,
                maxlon_source: 3,
                maxlat_source: 2,
                minlat_source: 3,
            },
            longitude: vec![-178.5, -177.5],
            latitude: vec![88.5, 87.5],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 1], vec![1, 1]]),
        },
    )
    .expect("write restart input");
    write_bbox_mask_netcdf(
        root.join("tmpfile/mask_refine_bbox_0_01.nc4"),
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -176.0,
                north: 89.5,
                south: 86.0,
            }],
        },
    )
    .expect("write calculated refine source");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();

    let report = run_area_judge_restart_grids_one_based(AreaJudgeRestartGridsRunConfig {
        file_dir: &root,
        restart_input: &input,
        mask_patch: None,
        refine: true,
        calculated_refine: Some(AreaJudgeCalculatedRefineConfig {
            refine_setting: "threshold",
            mask_refine_cal_type: "bbox",
            mask_refine_ndm: 1,
        }),
        lon_vertex: &lon_vertex,
        lat_vertex: &lat_vertex,
        lon_i: &lon_i,
        lat_i: &lat_i,
        gridnum_perdegree: 1,
        nlons_source: 6,
        nlats_source: 6,
        domain_output: None,
        refine_output: Some(&refine_output),
    })
    .expect("restart Area_judge grids runner should continue calculated refine");

    assert_eq!(report.area.domain.numpatch, 4);
    let refine = report.refine_step.as_ref().expect("iter-zero refine step");
    assert_eq!(refine.bounds, report.area.domain.bounds);
    assert_eq!(refine.selected_cells, 4);
    let write = report.refine_write.as_ref().expect("refine grid write");
    assert_eq!(write.output, refine_output);
    assert_eq!(write.selected_cells, 4);
    assert!(!write.has_seaorland);

    let payload = read_area_judge_grid_netcdf(&refine_output).expect("read written refine grid");
    assert_eq!(payload.bounds, report.area.domain.bounds);
    assert!(payload.seaorland_select.is_none());
    assert_eq!(payload.is_in_area_select, vec![vec![1, 1], vec![1, 1]]);
}
