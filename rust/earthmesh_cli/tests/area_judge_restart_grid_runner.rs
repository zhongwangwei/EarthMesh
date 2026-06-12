use std::path::PathBuf;

use earthmesh_cli::{
    run_area_judge_restart_grid_fortran_indexed, write_area_judge_grid_netcdf,
    AreaJudgeGridPayload, AreaJudgeRestartGridRunConfig,
};
use earthmesh_mesh::AreaJudgeSourceBounds;

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("create temp root");
    path
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

    let report = run_area_judge_restart_grid_fortran_indexed(AreaJudgeRestartGridRunConfig {
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
