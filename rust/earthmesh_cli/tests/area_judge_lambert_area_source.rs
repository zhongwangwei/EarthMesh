use earthmesh_cli::{
    area_judge_lambert_sources::build_area_judge_lambert_area_source_one_based,
    coordinate_types::LonLatPoint, lambert_mode4_io::write_mode4_mesh_netcdf,
    lambert_mode4_io::Mode4Mesh,
};
use earthmesh_mesh::AreaJudgeSourceBounds;
use std::path::PathBuf;

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("create temp root");
    path
}

fn source_axes() -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
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

fn write_rectangle_mode4(path: &std::path::Path) {
    write_mode4_mesh_netcdf(
        path,
        &Mode4Mesh {
            lonlat_bound: vec![
                LonLatPoint {
                    lon: -999.0,
                    lat: -999.0,
                },
                LonLatPoint { lon: 0.0, lat: 2.0 },
                LonLatPoint { lon: 2.0, lat: 2.0 },
                LonLatPoint { lon: 2.0, lat: 0.0 },
                LonLatPoint { lon: 0.0, lat: 0.0 },
            ],
            ngr_bound: vec![[1, 1, 1, 1], [2, 3, 4, 5]],
            n_ngr: vec![4, 4],
        },
    )
    .expect("write lambert mode4 source");
}

#[test]
fn lambert_area_source_builds_is_in_area_grid_and_canonical_numpatch() {
    let root = temp_root("area_judge_lambert_area_source");
    let source = root.join("mask_domain_lambert_0_01.nc4");
    write_rectangle_mode4(&source);
    let (lon_vertex, lat_vertex, lon_i, lat_i) = source_axes();

    let report = build_area_judge_lambert_area_source_one_based(
        &source,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
        1,
        360,
        180,
    )
    .expect("build lambert area source");

    assert_eq!(
        report.bounds,
        AreaJudgeSourceBounds {
            minlon_source: 181,
            maxlon_source: 181,
            maxlat_source: 89,
            minlat_source: 89,
        }
    );
    assert_eq!(report.numpatch, 4);
    assert_eq!(report.is_in_area[181][89], 1);
    assert_eq!(report.is_in_area[182][89], 1);
    assert_eq!(report.is_in_area[181][90], 1);
    assert_eq!(report.is_in_area[182][90], 1);
    assert_eq!(report.is_in_area[180][89], 0);
    assert_eq!(report.is_in_area[183][89], 0);
}

#[test]
fn lambert_area_source_rejects_cells_with_too_few_vertices() {
    let root = temp_root("area_judge_lambert_area_bad_cell");
    let source = root.join("mask_domain_lambert_0_01.nc4");
    write_mode4_mesh_netcdf(
        &source,
        &Mode4Mesh {
            lonlat_bound: vec![
                LonLatPoint {
                    lon: -999.0,
                    lat: -999.0,
                },
                LonLatPoint { lon: 0.0, lat: 1.0 },
                LonLatPoint { lon: 1.0, lat: 1.0 },
                LonLatPoint { lon: 0.0, lat: 0.0 },
            ],
            ngr_bound: vec![[1, 1, 1, 1], [2, 3, 4, 4]],
            n_ngr: vec![4, 3],
        },
    )
    .expect("write bad lambert source");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = source_axes();

    let err = build_area_judge_lambert_area_source_one_based(
        &source,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
        1,
        360,
        180,
    )
    .expect_err("bad mode4 cell should be rejected");

    assert!(
        err.to_string()
            .contains("lambert mode4 cell 1 must have at least four vertices"),
        "unexpected error: {err}"
    );
}
