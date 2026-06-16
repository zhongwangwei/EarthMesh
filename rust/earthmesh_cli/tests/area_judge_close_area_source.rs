use earthmesh_cli::{
    build_area_judge_close_area_source_fortran_indexed, write_close_mask_netcdf,
    write_close_mesh_netcdf, CloseMask, LonLatPoint,
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
fn close_area_source_accepts_close_mesh_schema_without_refine_degree() {
    let root = temp_root("area_judge_close_area_source_close_mesh_schema");
    let source = root.join("mask_patch_close_1_001.nc4");
    write_close_mesh_netcdf(
        &source,
        &[
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
    )
    .expect("write close_Mesh_Save source");

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
    .expect("build close area source from close_Mesh_Save schema");

    assert_eq!(report.numpatch, 4);
    assert_eq!(report.is_in_area[181][90], 1);
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

#[test]
fn close_area_source_accepts_subcell_polygon_like_fortran_empty_loop() {
    let root = temp_root("area_judge_close_area_subcell_empty");
    let source = root.join("mask_refine_close_1_001.nc4");
    write_close_mask_netcdf(
        &source,
        &CloseMask {
            refine_degree: 1,
            points: vec![
                LonLatPoint {
                    lon: 113.10,
                    lat: 22.20,
                },
                LonLatPoint {
                    lon: 113.20,
                    lat: 22.10,
                },
                LonLatPoint {
                    lon: 113.20,
                    lat: 22.20,
                },
            ],
        },
    )
    .expect("write subcell close source");

    let gridnum_perdegree = 1;
    let nlons_source = 360 * gridnum_perdegree;
    let nlats_source = 180 * gridnum_perdegree;
    let lon_vertex = std::iter::once(f64::NAN)
        .chain((0..=nlons_source).map(|idx| -180.0 + idx as f64 / gridnum_perdegree as f64))
        .collect::<Vec<_>>();
    let lat_vertex = std::iter::once(f64::NAN)
        .chain((0..=nlats_source).map(|idx| 90.0 - idx as f64 / gridnum_perdegree as f64))
        .collect::<Vec<_>>();

    let report = build_area_judge_close_area_source_fortran_indexed(
        &source,
        &lon_vertex,
        &lat_vertex,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
    )
    .expect("subcell close source should mirror Fortran zero-iteration behavior");

    assert_eq!(report.numpatch, 0);
}

#[test]
fn close_area_source_can_return_sparse_cells_without_dense_global_mask() {
    let root = temp_root("area_judge_close_area_sparse_cells");
    let source = root.join("mask_refine_close_2_001.nc4");
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

    let report = earthmesh_cli::build_area_judge_close_area_source_cells_fortran_indexed(
        &source,
        &lon_vertex,
        &lat_vertex,
        1,
        360,
        180,
    )
    .expect("build sparse close source");

    assert_eq!(report.numpatch, 4);
    assert_eq!(
        report.cells,
        vec![(181, 90), (182, 90), (181, 91), (182, 91)]
    );
    assert_eq!(
        report.bounds,
        AreaJudgeSourceBounds {
            minlon_source: 181,
            maxlon_source: 181,
            maxlat_source: 90,
            minlat_source: 90,
        }
    );
}
