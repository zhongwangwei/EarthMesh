use earthmesh_cli::{
    apply_area_judge_lambert_patch_source_fortran_indexed, read_mode4_mesh_netcdf,
    write_mode4_mesh_netcdf, LonLatPoint, Mode4Mesh,
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
fn lambert_patch_source_reads_mode4_mesh_and_applies_convex_cells() {
    let root = temp_root("area_judge_lambert_patch_source");
    let source = root.join("mask_patch_lambert_0_01.nc4");
    write_mode4_mesh_netcdf(
        &source,
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

    let read_back = read_mode4_mesh_netcdf(&source).expect("read lambert mode4 source");
    assert_eq!(read_back.bound_points(), 5);
    assert_eq!(read_back.mode_points(), 2);
    assert_eq!(read_back.ngr_bound[1], [2, 3, 4, 5]);

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
    let mut seaorland = one_based_seaorland(360, 180);

    let report = apply_area_judge_lambert_patch_source_fortran_indexed(
        &source,
        &mut seaorland,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
        1,
        360,
        180,
    )
    .expect("apply lambert patch source");

    assert_eq!(
        report.bounds,
        AreaJudgeSourceBounds {
            minlon_source: 181,
            maxlon_source: 181,
            maxlat_source: 89,
            minlat_source: 89,
        }
    );
    assert_eq!(report.patched_cells, 1);
    assert_eq!(seaorland[181][89], 0);
    assert_eq!(seaorland[182][89], 1);
    assert_eq!(seaorland[181][90], 1);
    assert_eq!(seaorland[180][89], 1);
}
