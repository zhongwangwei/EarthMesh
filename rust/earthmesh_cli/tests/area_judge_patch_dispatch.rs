use earthmesh_cli::{
    apply_area_judge_patch_sources_fortran_indexed, write_bbox_mask_netcdf, BBoxMask, BBoxPoint,
};
use earthmesh_mesh::AreaJudgeSourceBounds;
use std::path::PathBuf;

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(path.join("tmpfile")).expect("create temp root");
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
fn patch_dispatch_reads_fortran_numbered_bbox_sources_and_merges_bounds() {
    let root = temp_root("area_judge_patch_dispatch");
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
    .expect("write first bbox source");
    write_bbox_mask_netcdf(
        root.join("tmpfile/mask_patch_bbox_0_02.nc4"),
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: -176.5,
                east: -174.0,
                north: 86.5,
                south: 84.0,
            }],
        },
    )
    .expect("write second bbox source");

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
    let mut seaorland = one_based_seaorland(6, 6);

    let report = apply_area_judge_patch_sources_fortran_indexed(
        &root,
        "bbox",
        0,
        2,
        &mut seaorland,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
        1,
        6,
        6,
    )
    .expect("dispatch bbox patch sources");

    assert_eq!(report.source_reports.len(), 2);
    assert_eq!(report.patched_cells, 8);
    assert_eq!(
        report.bounds,
        Some(AreaJudgeSourceBounds {
            minlon_source: 2,
            maxlon_source: 6,
            maxlat_source: 2,
            minlat_source: 6,
        })
    );
    assert_eq!(seaorland[2][2], 0);
    assert_eq!(seaorland[3][3], 0);
    assert_eq!(seaorland[5][5], 0);
    assert_eq!(seaorland[1][1], 1);
    assert_eq!(seaorland[6][6], 0);
}
