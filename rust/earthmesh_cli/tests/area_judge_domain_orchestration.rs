use earthmesh_cli::{
    area_judge_domain_builders::build_area_judge_domain_one_based,
    bbox_mask_io::write_bbox_mask_netcdf, bbox_mask_io::BBoxMask, bbox_mask_io::BBoxPoint,
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

#[test]
fn domain_orchestration_builds_non_global_mask_domain_from_sources() {
    let root = temp_root("area_judge_domain_orchestration");
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
    .expect("write first domain source");
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
    .expect("write second domain source");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();

    let report = build_area_judge_domain_one_based(
        &root,
        false,
        "bbox",
        2,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
        1,
        6,
        6,
    )
    .expect("build non-global domain");

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
    assert_eq!(report.nlons_select, 5);
    assert_eq!(report.nlats_select, 5);
    assert_eq!(report.is_in_domain[2][2], 1);
    assert_eq!(report.is_in_domain[5][5], 1);
    assert_eq!(report.is_in_domain[1][1], 0);
}

#[test]
fn domain_orchestration_preserves_global_domain_shortcut() {
    let root = temp_root("area_judge_domain_global");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();

    let report = build_area_judge_domain_one_based(
        &root,
        true,
        "bbox",
        0,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
        1,
        6,
        6,
    )
    .expect("build global domain");

    assert_eq!(report.numpatch, 36);
    assert_eq!(report.bounds.minlon_source, 1);
    assert_eq!(report.bounds.maxlon_source, 6);
    assert_eq!(report.nlons_select, 6);
    assert_eq!(report.nlats_select, 6);
    assert_eq!(report.is_in_domain[1][1], 1);
    assert_eq!(report.is_in_domain[6][6], 1);
}
