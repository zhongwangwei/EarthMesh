use earthmesh_cli::{
    apply_area_judge_circle_patch_source_fortran_indexed, read_circle_mask_netcdf,
    write_circle_mask_netcdf, CircleMask, LonLatPoint,
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
fn circle_patch_source_reads_netcdf_and_zeroes_only_cells_inside_radius() {
    let root = temp_root("area_judge_circle_patch_source");
    let source = root.join("mask_patch_circle_0_01.nc4");
    write_circle_mask_netcdf(
        &source,
        &CircleMask {
            refine_degree: 0,
            points: vec![LonLatPoint { lon: 0.5, lat: 0.5 }],
            radius_km: vec![90.0],
        },
    )
    .expect("write circle source");

    let read_back = read_circle_mask_netcdf(&source).expect("read circle source");
    assert_eq!(read_back.refine_degree, 0);
    assert_eq!(read_back.points, vec![LonLatPoint { lon: 0.5, lat: 0.5 }]);
    assert_eq!(read_back.radius_km, vec![90.0]);

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

    let report = apply_area_judge_circle_patch_source_fortran_indexed(
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
    .expect("apply circle patch source");

    assert_eq!(
        report.bounds,
        AreaJudgeSourceBounds {
            minlon_source: 181,
            maxlon_source: 181,
            maxlat_source: 90,
            minlat_source: 90,
        }
    );
    assert_eq!(report.patched_cells, 1);
    assert_eq!(seaorland[181][90], 0);
    assert_eq!(seaorland[180][90], 1);
    assert_eq!(seaorland[181][89], 1);
    assert_eq!(seaorland[182][90], 1);
}
