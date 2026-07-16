use earthmesh_cli::{
    area_judge_circle_sources::build_area_judge_circle_area_source_one_based,
    circle_close_mask_io::write_circle_mask_netcdf, circle_close_mask_io::CircleMask,
    coordinate_types::LonLatPoint,
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
fn circle_area_source_builds_is_in_area_grid_and_unique_numpatch() {
    let root = temp_root("area_judge_circle_area_source");
    let source = root.join("mask_domain_circle_0_01.nc4");
    write_circle_mask_netcdf(
        &source,
        &CircleMask {
            refine_degree: 0,
            points: vec![LonLatPoint { lon: 0.5, lat: 0.5 }],
            radius_km: vec![90.0],
        },
    )
    .expect("write circle source");

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

    let report = build_area_judge_circle_area_source_one_based(
        &source,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
        1,
        360,
        180,
    )
    .expect("build circle area source");

    assert_eq!(
        report.bounds,
        AreaJudgeSourceBounds {
            minlon_source: 181,
            maxlon_source: 181,
            maxlat_source: 90,
            minlat_source: 90,
        }
    );
    assert_eq!(report.numpatch, 1);
    assert!(report.is_in_area[181][90]);
    assert!(!report.is_in_area[180][90]);
    assert!(!report.is_in_area[181][89]);
    assert!(!report.is_in_area[182][90]);
}

#[test]
fn circle_area_source_rejects_empty_circle_file() {
    let root = temp_root("area_judge_circle_area_empty");
    let source = root.join("mask_domain_circle_0_01.nc4");
    write_circle_mask_netcdf(
        &source,
        &CircleMask {
            refine_degree: 0,
            points: vec![],
            radius_km: vec![],
        },
    )
    .expect("write empty circle source");

    let lon_vertex = vec![f64::NAN, -180.0, -179.0];
    let lat_vertex = vec![f64::NAN, 90.0, 89.0];
    let centers = vec![f64::NAN, 0.5];

    let err = build_area_judge_circle_area_source_one_based(
        &source,
        &lon_vertex,
        &lat_vertex,
        &centers,
        &centers,
        1,
        1,
        1,
    )
    .expect_err("empty circle source should fail");

    assert!(err
        .to_string()
        .contains("circle area source must contain at least one circle"));
}
