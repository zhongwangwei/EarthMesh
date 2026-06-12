use earthmesh_cli::{
    apply_area_judge_bbox_patch_source_fortran_indexed, read_bbox_mask_netcdf,
    write_bbox_mask_netcdf, BBoxMask, BBoxPoint,
};
use earthmesh_mesh::AreaJudgeSourceBounds;
use std::path::PathBuf;

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("create temp root");
    path
}

fn one_based_seaorland(nx: usize, ny: usize) -> Vec<Vec<i32>> {
    let mut values = vec![vec![0; ny + 1]; nx + 1];
    for i in 1..=nx {
        for j in 1..=ny {
            values[i][j] = 1;
        }
    }
    values
}

#[test]
fn bbox_patch_source_reads_netcdf_and_zeroes_selected_land_cells() {
    let root = temp_root("area_judge_bbox_patch_source");
    let source = root.join("mask_patch_bbox_0_01.nc4");
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
    .expect("write bbox source");

    let read_back = read_bbox_mask_netcdf(&source).expect("read bbox source");
    assert_eq!(read_back.refine_degree, 0);
    assert_eq!(read_back.points.len(), 1);

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
    let mut seaorland = one_based_seaorland(6, 6);

    let report = apply_area_judge_bbox_patch_source_fortran_indexed(
        &source,
        &mut seaorland,
        &lon_vertex,
        &lat_vertex,
        1,
        6,
        6,
    )
    .expect("apply bbox patch source");

    assert_eq!(
        report.bounds,
        AreaJudgeSourceBounds {
            minlon_source: 2,
            maxlon_source: 3,
            maxlat_source: 2,
            minlat_source: 3,
        }
    );
    assert_eq!(report.patched_cells, 4);
    assert_eq!(seaorland[1][1], 1);
    assert_eq!(seaorland[2][2], 0);
    assert_eq!(seaorland[2][3], 0);
    assert_eq!(seaorland[3][2], 0);
    assert_eq!(seaorland[3][3], 0);
    assert_eq!(seaorland[4][4], 1);
}
