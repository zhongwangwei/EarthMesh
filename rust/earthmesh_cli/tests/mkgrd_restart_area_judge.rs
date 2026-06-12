use std::fs;
use std::path::PathBuf;

use earthmesh_cli::{
    read_area_judge_grid_netcdf, run_mkgrd_mask_restart_area_judge_namelist,
    write_area_judge_grid_netcdf, write_bbox_mask_netcdf, AreaJudgeGridPayload, BBoxMask,
    BBoxPoint, MkgrdRestartAreaJudgeOptions,
};
use earthmesh_mesh::AreaJudgeSourceBounds;

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
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
fn mask_restart_area_judge_runner_restores_patches_and_rewrites_domain_grid() {
    let root = temp_root("mkgrd_restart_area_judge");
    let case_dir = root.join("case_restart_patch");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
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

    let source = root.join("patch_source.nc4");
    write_bbox_mask_netcdf(
        &source,
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

    let namelist = root.join("mkgrd_restart_patch.nml");
    let base_dir = format!("{}/", root.display());
    let source_path = source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_patch'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.true.\n  NL%mask_patch_type='bbox'\n  NL%mask_patch_fprefix='{source_path}'\n/\n"
        ),
    )
    .expect("write namelist");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();

    let report = run_mkgrd_mask_restart_area_judge_namelist(
        &namelist,
        &root,
        7,
        MkgrdRestartAreaJudgeOptions {
            lon_vertex: &lon_vertex,
            lat_vertex: &lat_vertex,
            lon_i: &lon_i,
            lat_i: &lat_i,
            gridnum_perdegree: 1,
            nlons_source: 6,
            nlats_source: 6,
        },
    )
    .expect("run restart Area_judge continuation");

    assert_eq!(report.plan.remask.step, 8);
    assert_eq!(report.workspace_mask.mask_counts.mask_patch_ndm[0], 1);
    assert_eq!(report.area.domain.numpatch, 4);
    assert_eq!(report.area_write.output, restart_input);
    assert_eq!(report.area_write.selected_cells, 4);
    assert!(report.refine_write.is_none());

    let rewritten = read_area_judge_grid_netcdf(case_dir.join("result/IsInDmArea_grid.nc4"))
        .expect("read rewritten grid");
    assert_eq!(rewritten.is_in_area_select, vec![vec![1, 1], vec![1, 1]]);
    assert_eq!(
        rewritten.seaorland_select,
        Some(vec![vec![0, 0], vec![0, 0]])
    );

    let _ = fs::remove_dir_all(&root);
}
