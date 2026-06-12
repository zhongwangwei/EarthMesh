use earthmesh_cli::{
    build_area_judge_close_area_source_fortran_indexed, write_close_mask_netcdf, CloseMask,
    LonLatPoint,
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
fn close_area_source_builds_is_in_area_grid_and_fortran_numpatch() {
    let root = temp_root("area_judge_close_area_source");
    let source = root.join("mask_domain_close_0_001.nc4");
    write_close_mask_netcdf(
        &source,
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

    let lon_vertex = std::iter::once(f64::NAN)
        .chain((0..=360).map(|idx| -180.0 + idx as f64))
        .collect::<Vec<_>>();
    let lat_vertex = std::iter::once(f64::NAN)
        .chain((0..=180).map(|idx| 90.0 - idx as f64))
        .collect::<Vec<_>>();

    let report = build_area_judge_close_area_source_fortran_indexed(
        &source,
        &lon_vertex,
        &lat_vertex,
        1,
        360,
        180,
    )
    .expect("build close area source");

    assert_eq!(
        report.bounds,
        AreaJudgeSourceBounds {
            minlon_source: 181,
            maxlon_source: 181,
            maxlat_source: 90,
            minlat_source: 90,
        }
    );
    assert_eq!(report.numpatch, 4);
    assert_eq!(report.is_in_area[181][90], 1);
    assert_eq!(report.is_in_area[182][90], 1);
    assert_eq!(report.is_in_area[181][91], 1);
    assert_eq!(report.is_in_area[182][91], 1);
    assert_eq!(report.is_in_area[180][90], 0);
    assert_eq!(report.is_in_area[183][90], 0);
}

#[test]
fn close_area_source_rejects_self_intersection() {
    let root = temp_root("area_judge_close_area_self_intersection");
    let source = root.join("mask_domain_close_0_001.nc4");
    write_close_mask_netcdf(
        &source,
        &CloseMask {
            refine_degree: 0,
            points: vec![
                LonLatPoint { lon: 0.0, lat: 0.0 },
                LonLatPoint { lon: 2.0, lat: 2.0 },
                LonLatPoint { lon: 0.0, lat: 2.0 },
                LonLatPoint { lon: 2.0, lat: 0.0 },
            ],
        },
    )
    .expect("write self-intersecting close source");

    let lon_vertex = std::iter::once(f64::NAN)
        .chain((0..=360).map(|idx| -180.0 + idx as f64))
        .collect::<Vec<_>>();
    let lat_vertex = std::iter::once(f64::NAN)
        .chain((0..=180).map(|idx| 90.0 - idx as f64))
        .collect::<Vec<_>>();

    let err = build_area_judge_close_area_source_fortran_indexed(
        &source,
        &lon_vertex,
        &lat_vertex,
        1,
        360,
        180,
    )
    .expect_err("self-intersecting close source should fail");

    assert!(err.to_string().contains("close polygon self-intersects"));
}
