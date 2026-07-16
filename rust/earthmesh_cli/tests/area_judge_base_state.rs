use earthmesh_cli::{
    area_judge_domain_builders::build_area_judge_base_state_one_based,
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

fn one_based_landtypes(nx: usize, ny: usize, land_cells: &[(usize, usize)]) -> Vec<Vec<i32>> {
    let mut values = vec![vec![0; ny + 1]; nx + 1];
    for &(lon, lat) in land_cells {
        values[lon][lat] = 1;
    }
    values
}

#[test]
fn base_state_builds_domain_and_seaorland_before_patch_or_refine() {
    let root = temp_root("area_judge_base_state");
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
    .expect("write domain source");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();
    let landtypes_global = one_based_landtypes(6, 6, &[(2, 2), (3, 3), (6, 6)]);

    let report = build_area_judge_base_state_one_based(
        &root,
        false,
        "bbox",
        1,
        &landtypes_global,
        "landmesh",
        false,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
        1,
        6,
        6,
    )
    .expect("build Area_judge base state");

    assert_eq!(
        report.domain.bounds,
        AreaJudgeSourceBounds {
            minlon_source: 2,
            maxlon_source: 3,
            maxlat_source: 2,
            minlat_source: 3,
        }
    );
    assert_eq!(report.domain.numpatch, 4);
    assert_eq!(report.seaorland.sum_land_grid, 2);
    assert!(report.seaorland.seaorland[2][2]);
    assert!(report.seaorland.seaorland[3][3]);
    assert!(!report.seaorland.seaorland[6][6]);
}
