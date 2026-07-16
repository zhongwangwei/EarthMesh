use std::path::PathBuf;

use earthmesh_cli::{
    area_judge_branch_builders::build_area_judge_restart_one_based,
    area_judge_grid_io::write_area_judge_grid_netcdf, area_judge_grid_io::AreaJudgeGridPayload,
    area_judge_types::AreaJudgePatchConfig, bbox_mask_io::write_bbox_mask_netcdf,
    bbox_mask_io::BBoxMask, bbox_mask_io::BBoxPoint,
};
use earthmesh_mesh::AreaJudgeSourceBounds;

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(path.join("tmpfile")).expect("create temp root");
    path
}

fn small_axes() -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
    let lon_vertex = vec![
        f64::NAN,
        -180.0,
        -179.0,
        -178.0,
        -177.0,
        -176.0,
        -175.0,
        -174.0,
    ];
    let lat_vertex = vec![f64::NAN, 90.0, 89.0, 88.0, 87.0, 86.0, 85.0, 84.0];
    let lon_i = std::iter::once(f64::NAN)
        .chain((0..6).map(|idx| -179.5 + idx as f64))
        .collect::<Vec<_>>();
    let lat_i = std::iter::once(f64::NAN)
        .chain((0..6).map(|idx| 89.5 - idx as f64))
        .collect::<Vec<_>>();
    (lon_vertex, lat_vertex, lon_i, lat_i)
}

#[test]
fn restart_area_judge_reads_saved_domain_and_applies_patch_sources() {
    let root = temp_root("area_judge_restart_patch");
    let restart_input = root.join("result/IsInDmArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &restart_input,
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
            seaorland_select: Some(vec![vec![1, 0], vec![1, 1]]),
        },
    )
    .expect("write restart domain");
    write_bbox_mask_netcdf(
        root.join("tmpfile/mask_patch_bbox_0_01.nc4"),
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
    .expect("write patch source");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();

    let report = build_area_judge_restart_one_based(
        &root,
        &restart_input,
        Some(AreaJudgePatchConfig {
            mask_patch_type: "bbox",
            mask_patch_ndm: 1,
        }),
        false,
        None,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
        1,
        6,
        6,
    )
    .expect("restart Area_judge state");

    assert_eq!(report.domain.bounds.minlon_source, 2);
    assert_eq!(report.domain.numpatch, 4);
    assert_eq!(report.seaorland.sum_land_grid, 3);
    assert!(!report.seaorland.seaorland[2][2]);
    assert!(!report.seaorland.seaorland[2][3]);
    assert!(!report.seaorland.seaorland[3][2]);
    assert!(!report.seaorland.seaorland[3][3]);
    assert_eq!(
        report.patch.as_ref().expect("patch report").patched_cells,
        4
    );
    assert!(report.calculated_refine.is_none());
}

#[test]
fn restart_area_judge_can_continue_calculated_refine_from_restored_domain() {
    let root = temp_root("area_judge_restart_calculated_refine");
    let restart_input = root.join("result/IsInDmArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &restart_input,
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
    .expect("write restart domain");
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

    let report = build_area_judge_restart_one_based(
        &root,
        &restart_input,
        None,
        true,
        Some(
            earthmesh_cli::area_judge_types::AreaJudgeCalculatedRefineConfig {
                refine_setting: "threshold",
                mask_refine_cal_type: "bbox",
                mask_refine_ndm: 1,
            },
        ),
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
        1,
        6,
        6,
    )
    .expect("restart Area_judge calculated refine state");

    assert!(report.patch.is_none());
    let refine = report.calculated_refine.expect("calculated refine");
    assert_eq!(refine.bounds, report.domain.bounds);
    assert_eq!(refine.numpatch, 4);
    assert!(refine.is_in_area[2][2]);
    assert!(refine.is_in_area[3][3]);
}
