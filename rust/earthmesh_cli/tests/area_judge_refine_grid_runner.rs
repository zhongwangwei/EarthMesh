use std::path::PathBuf;

use earthmesh_cli::{
    area_judge_grid_runs::run_area_judge_refine_grid_one_based,
    area_judge_types::AreaJudgeRefineGridRunConfig, bbox_mask_io::write_bbox_mask_netcdf,
    bbox_mask_io::BBoxMask, bbox_mask_io::BBoxPoint,
};
use earthmesh_mesh::AreaJudgeSourceBounds;

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

fn full_domain(nx: usize, ny: usize) -> Vec<Vec<bool>> {
    let mut values = vec![vec![false; ny + 1]; nx + 1];
    for lon in 1..=nx {
        for lat in 1..=ny {
            values[lon][lat] = true;
        }
    }
    values
}

fn read_i32_2d(file: &netcdf::File, name: &str) -> Vec<i32> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<i32, _>((.., ..))
        .expect("read i32 matrix")
}

#[test]
fn refine_grid_runner_writes_specified_iteration_refine_grid() {
    let root = temp_root("area_judge_refine_grid_runner_specified");
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
    .expect("write specified refine source");
    let output = root.join("result/IsInRfArea_grid_iter2.nc4");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();
    let domain = full_domain(6, 6);

    let report = run_area_judge_refine_grid_one_based(AreaJudgeRefineGridRunConfig {
        file_dir: &root,
        iter: 2,
        calculated_refine: None,
        mask_refine_spc_type: "bbox",
        mask_refine_ndm: 1,
        is_in_domain: &domain,
        lon_vertex: &lon_vertex,
        lat_vertex: &lat_vertex,
        lon_i: &lon_i,
        lat_i: &lat_i,
        gridnum_perdegree: 1,
        nlons_source: 6,
        nlats_source: 6,
        refine_output: &output,
    })
    .expect("run specified refine grid runner");

    assert_eq!(
        report.refine_step.bounds,
        AreaJudgeSourceBounds {
            minlon_source: 2,
            maxlon_source: 3,
            maxlat_source: 2,
            minlat_source: 3,
        }
    );
    assert_eq!(report.refine_step.selected_cells, 4);
    assert_eq!(report.refine_write.output, output);
    assert_eq!(report.refine_write.selected_cells, 4);
    assert!(!report.refine_write.has_seaorland);

    let file = netcdf::open(&output).expect("open written refine grid");
    assert_eq!(read_i32_2d(&file, "IsInArea_select"), vec![1, 1, 1, 1]);
}
