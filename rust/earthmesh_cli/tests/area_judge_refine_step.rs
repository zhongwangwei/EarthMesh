use earthmesh_cli::{
    area_judge_refine_steps::run_area_judge_refine_one_based, bbox_mask_io::write_bbox_mask_netcdf,
    bbox_mask_io::BBoxMask, bbox_mask_io::BBoxPoint,
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

fn full_domain(nx: usize, ny: usize) -> Vec<Vec<i32>> {
    let mut values = vec![vec![0; ny + 1]; nx + 1];
    for lon in 1..=nx {
        for lat in 1..=ny {
            values[lon][lat] = 1;
        }
    }
    values
}

#[test]
fn refine_step_iter_zero_activates_calculated_grid_without_reading_sources() {
    let root = temp_root("area_judge_refine_step_iter0");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();
    let domain = full_domain(6, 6);
    let mut calculated = vec![vec![false; 7]; 7];
    calculated[2][2] = true;
    calculated[3][3] = true;
    let calculated_bounds = AreaJudgeSourceBounds {
        minlon_source: 2,
        maxlon_source: 3,
        maxlat_source: 2,
        minlat_source: 3,
    };

    let report = run_area_judge_refine_one_based(
        &root,
        0,
        Some((&calculated, calculated_bounds)),
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
    .expect("activate iter zero refine");

    assert_eq!(report.bounds, calculated_bounds);
    assert_eq!(report.nlons_select, 2);
    assert_eq!(report.nlats_select, 2);
    assert_eq!(report.selected_cells, 2);
    assert_eq!(report.source_numpatch, None);
    assert!(report.is_in_refine[2][2]);
    assert!(report.is_in_refine[3][3]);
}

#[test]
fn refine_step_iter_positive_reads_specified_sources() {
    let root = temp_root("area_judge_refine_step_specified");
    write_bbox_mask_netcdf(
        root.join("tmpfile/mask_refine_bbox_2_01.nc4"),
        &BBoxMask {
            refine_degree: 2,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -176.0,
                north: 89.5,
                south: 86.0,
            }],
        },
    )
    .expect("write specified source");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();
    let domain = full_domain(6, 6);

    let report = run_area_judge_refine_one_based(
        &root,
        2,
        None,
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
    .expect("read specified refine");

    assert_eq!(
        report.bounds,
        AreaJudgeSourceBounds {
            minlon_source: 2,
            maxlon_source: 3,
            maxlat_source: 2,
            minlat_source: 3,
        }
    );
    assert_eq!(report.selected_cells, 4);
    assert_eq!(report.source_numpatch, Some(4));
    assert!(report.is_in_refine[2][2]);
    assert!(report.is_in_refine[3][3]);
}
