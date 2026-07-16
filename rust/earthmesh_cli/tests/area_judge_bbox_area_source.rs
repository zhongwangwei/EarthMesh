use earthmesh_cli::{
    area_judge_bbox_sources::build_area_judge_bbox_area_source_one_based,
    bbox_mask_io::write_bbox_mask_netcdf, bbox_mask_io::BBoxMask, bbox_mask_io::BBoxPoint,
};
use earthmesh_mesh::AreaJudgeSourceBounds;
use std::path::PathBuf;

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("create temp root");
    path
}

#[test]
fn bbox_area_source_builds_is_in_area_grid_and_canonical_numpatch() {
    let root = temp_root("area_judge_bbox_area_source");
    let source = root.join("mask_domain_bbox_0_01.nc4");
    write_bbox_mask_netcdf(
        &source,
        &BBoxMask {
            refine_degree: 0,
            points: vec![
                BBoxPoint {
                    west: -179.5,
                    east: -176.0,
                    north: 89.5,
                    south: 86.0,
                },
                BBoxPoint {
                    west: -176.5,
                    east: -174.0,
                    north: 86.5,
                    south: 84.0,
                },
            ],
        },
    )
    .expect("write bbox source");

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

    let report =
        build_area_judge_bbox_area_source_one_based(&source, &lon_vertex, &lat_vertex, 1, 6, 6)
            .expect("build bbox area source");

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
    assert!(report.is_in_area[2][2]);
    assert!(report.is_in_area[3][3]);
    assert!(report.is_in_area[5][5]);
    assert!(!report.is_in_area[1][1]);
}

#[test]
fn bbox_area_source_rejects_empty_bbox_file() {
    let root = temp_root("area_judge_bbox_area_empty");
    let source = root.join("mask_domain_bbox_0_01.nc4");
    write_bbox_mask_netcdf(
        &source,
        &BBoxMask {
            refine_degree: 0,
            points: vec![],
        },
    )
    .expect("write empty bbox source");

    let lon_vertex = vec![f64::NAN, -180.0, -179.0];
    let lat_vertex = vec![f64::NAN, 90.0, 89.0];

    let err =
        build_area_judge_bbox_area_source_one_based(&source, &lon_vertex, &lat_vertex, 1, 2, 2)
            .expect_err("empty bbox source should fail");

    assert!(err
        .to_string()
        .contains("bbox area source must contain at least one bbox point"));
}

#[test]
fn bbox_area_source_splits_directed_dateline_bbox() {
    let root = temp_root("area_judge_bbox_area_dateline");
    let source = root.join("mask_domain_bbox_0_01.nc4");
    write_bbox_mask_netcdf(
        &source,
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: 178.0,
                east: -178.0,
                north: 89.5,
                south: 86.0,
            }],
        },
    )
    .expect("write dateline bbox source");

    let mut lon_vertex = vec![f64::NAN];
    lon_vertex.extend((-180..=180).map(f64::from));
    let lat_vertex = vec![f64::NAN, 90.0, 89.0, 88.0, 87.0, 86.0, 85.0, 84.0];

    let report =
        build_area_judge_bbox_area_source_one_based(&source, &lon_vertex, &lat_vertex, 1, 360, 6)
            .expect("build directed dateline bbox area source");

    assert_eq!(
        report.bounds,
        AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: 360,
            maxlat_source: 2,
            minlat_source: 3,
        }
    );
    assert_eq!(report.numpatch, 6);
    assert!(report.is_in_area[1][2]);
    assert!(report.is_in_area[359][2]);
    assert!(report.is_in_area[360][3]);
    assert!(!report.is_in_area[180][2]);
}
