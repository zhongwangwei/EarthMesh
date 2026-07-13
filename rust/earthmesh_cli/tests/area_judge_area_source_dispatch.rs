use earthmesh_cli::{
    area_judge_sources::build_area_judge_area_sources_one_based,
    bbox_mask_io::write_bbox_mask_netcdf, bbox_mask_io::BBoxMask, bbox_mask_io::BBoxPoint,
    circle_close_mask_io::write_close_mask_netcdf, circle_close_mask_io::CloseMask,
    coordinate_types::LonLatPoint,
};
use earthmesh_mesh::AreaJudgeSourceBounds;
use std::path::PathBuf;

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

fn global_axes() -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
    let lon_vertex = std::iter::once(f64::NAN)
        .chain((0..=360).map(|idx| -180.0 + idx as f64))
        .collect::<Vec<_>>();
    let lat_vertex = std::iter::once(f64::NAN)
        .chain((0..=180).map(|idx| 90.0 - idx as f64))
        .collect::<Vec<_>>();
    let lon_i = std::iter::once(f64::NAN)
        .chain((0..360).map(|idx| -179.5 + idx as f64))
        .collect::<Vec<_>>();
    let lat_i = std::iter::once(f64::NAN)
        .chain((0..180).map(|idx| 89.5 - idx as f64))
        .collect::<Vec<_>>();
    (lon_vertex, lat_vertex, lon_i, lat_i)
}

#[test]
fn area_source_dispatch_reads_numbered_bbox_sources_and_merges_masks() {
    let root = temp_root("area_judge_area_source_dispatch_bbox");
    write_bbox_mask_netcdf(
        root.join("tmpfile/mask_domain_bbox_0_01.nc4"),
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
        root.join("tmpfile/mask_domain_bbox_0_02.nc4"),
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
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();

    let report = build_area_judge_area_sources_one_based(
        &root,
        "mask_domain",
        "bbox",
        0,
        2,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
        1,
        6,
        6,
    )
    .expect("dispatch bbox area sources");

    assert_eq!(
        report.bounds,
        AreaJudgeSourceBounds {
            minlon_source: 2,
            maxlon_source: 6,
            maxlat_source: 2,
            minlat_source: 6,
        }
    );
    assert_eq!(report.numpatch, 8);
    assert_eq!(report.is_in_area[2][2], 1);
    assert_eq!(report.is_in_area[3][3], 1);
    assert_eq!(report.is_in_area[5][5], 1);
    assert_eq!(report.is_in_area[1][1], 0);
    assert_eq!(report.is_in_area[6][6], 1);
}

#[test]
fn area_source_dispatch_uses_three_digit_close_numbering() {
    let root = temp_root("area_judge_area_source_dispatch_close");
    write_close_mask_netcdf(
        root.join("tmpfile/mask_refine_close_2_001.nc4"),
        &CloseMask {
            refine_degree: 0,
            points: vec![
                LonLatPoint { lon: 0.0, lat: 1.0 },
                LonLatPoint { lon: 2.0, lat: 1.0 },
                LonLatPoint {
                    lon: 2.0,
                    lat: -1.0,
                },
                LonLatPoint {
                    lon: 0.0,
                    lat: -1.0,
                },
            ],
        },
    )
    .expect("write close source");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = global_axes();

    let report = build_area_judge_area_sources_one_based(
        &root,
        "mask_refine",
        "close",
        2,
        1,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
        1,
        360,
        180,
    )
    .expect("dispatch close area source");

    assert_eq!(report.numpatch, 4);
    assert_eq!(report.is_in_area[181][90], 1);
    assert_eq!(report.is_in_area[182][90], 1);
    assert_eq!(report.is_in_area[181][91], 1);
    assert_eq!(report.is_in_area[182][91], 1);
}
