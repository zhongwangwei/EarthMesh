use earthmesh_cli::{
    build_area_judge_specified_refine_fortran_indexed, write_bbox_mask_netcdf, BBoxMask, BBoxPoint,
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

fn domain_mask(nx: usize, ny: usize, bounds: AreaJudgeSourceBounds) -> Vec<Vec<i32>> {
    let mut values = vec![vec![0; ny + 1]; nx + 1];
    for lon in bounds.minlon_source..=bounds.maxlon_source {
        for lat in bounds.maxlat_source..=bounds.minlat_source {
            values[lon][lat] = 1;
        }
    }
    values
}

#[test]
fn specified_refine_builds_iter_specific_mask_refine_sources_inside_domain() {
    let root = temp_root("area_judge_specified_refine");
    write_bbox_mask_netcdf(
        root.join("tmpfile/mask_refine_bbox_2_01.nc4"),
        &BBoxMask {
            refine_degree: 2,
            points: vec![BBoxPoint {
                west: -178.5,
                east: -176.0,
                north: 88.5,
                south: 86.0,
            }],
        },
    )
    .expect("write specified refine source");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();
    let domain = domain_mask(
        6,
        6,
        AreaJudgeSourceBounds {
            minlon_source: 2,
            maxlon_source: 5,
            maxlat_source: 2,
            minlat_source: 5,
        },
    );

    let report = build_area_judge_specified_refine_fortran_indexed(
        &root,
        2,
        "bbox",
        1,
        &domain,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
        1,
        6,
        6,
    )
    .expect("build specified refine");

    assert_eq!(
        report.bounds,
        AreaJudgeSourceBounds {
            minlon_source: 3,
            maxlon_source: 3,
            maxlat_source: 3,
            minlat_source: 3,
        }
    );
    assert_eq!(report.numpatch, 1);
    assert_eq!(report.is_in_area[3][3], 1);
    assert_eq!(report.is_in_area[4][4], 0);
}

#[test]
fn specified_refine_rejects_iter_zero_activation_branch() {
    let root = temp_root("area_judge_specified_refine_iter0");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();
    let domain = domain_mask(
        6,
        6,
        AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: 6,
            maxlat_source: 1,
            minlat_source: 6,
        },
    );

    let err = build_area_judge_specified_refine_fortran_indexed(
        &root,
        0,
        "bbox",
        1,
        &domain,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
        1,
        6,
        6,
    )
    .expect_err("iter zero should use calculated activation");

    assert!(
        err.to_string()
            .contains("specified Area_judge_refine requires iter > 0"),
        "unexpected error: {err}"
    );
}

#[test]
fn specified_refine_rejects_sources_outside_domain() {
    let root = temp_root("area_judge_specified_refine_outside");
    write_bbox_mask_netcdf(
        root.join("tmpfile/mask_refine_bbox_1_01.nc4"),
        &BBoxMask {
            refine_degree: 1,
            points: vec![BBoxPoint {
                west: -175.5,
                east: -174.0,
                north: 85.5,
                south: 84.0,
            }],
        },
    )
    .expect("write outside specified refine source");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();
    let domain = domain_mask(
        6,
        6,
        AreaJudgeSourceBounds {
            minlon_source: 2,
            maxlon_source: 4,
            maxlat_source: 2,
            minlat_source: 4,
        },
    );

    let err = build_area_judge_specified_refine_fortran_indexed(
        &root,
        1,
        "bbox",
        1,
        &domain,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
        1,
        6,
        6,
    )
    .expect_err("outside specified refine should fail");

    assert!(
        err.to_string().contains("refine area exceeds domain area"),
        "unexpected error: {err}"
    );
}
