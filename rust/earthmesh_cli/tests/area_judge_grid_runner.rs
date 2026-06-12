use std::path::PathBuf;

use earthmesh_cli::{
    run_area_judge_non_restart_grids_fortran_indexed, write_bbox_mask_netcdf,
    AreaJudgeCalculatedRefineConfig, AreaJudgeGridRunConfig, BBoxMask, BBoxPoint,
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

fn one_based_landtypes(nx: usize, ny: usize, land_cells: &[(usize, usize)]) -> Vec<Vec<i32>> {
    let mut values = vec![vec![0; ny + 1]; nx + 1];
    for &(lon, lat) in land_cells {
        values[lon][lat] = 1;
    }
    values
}

fn write_single_bbox(path: PathBuf) {
    write_bbox_mask_netcdf(
        path,
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
    .expect("write bbox source");
}

fn read_i32_2d(file: &netcdf::File, name: &str) -> Vec<i32> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<i32, _>((.., ..))
        .expect("read i32 matrix")
}

#[test]
fn area_judge_grid_runner_writes_domain_and_iter_zero_refine_grids() {
    let root = temp_root("area_judge_grid_runner");
    write_single_bbox(root.join("tmpfile/mask_domain_bbox_0_01.nc4"));
    write_single_bbox(root.join("tmpfile/mask_refine_bbox_0_01.nc4"));
    let domain_output = root.join("result/IsInDmArea_grid.nc4");
    let refine_output = root.join("result/IsInRfArea_grid.nc4");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();
    let landtypes_global = one_based_landtypes(6, 6, &[(2, 2), (3, 3), (6, 6)]);

    let report = run_area_judge_non_restart_grids_fortran_indexed(AreaJudgeGridRunConfig {
        file_dir: &root,
        mask_domain_global: false,
        mask_domain_type: "bbox",
        mask_domain_ndm: 1,
        mask_patch: None,
        refine: true,
        calculated_refine: Some(AreaJudgeCalculatedRefineConfig {
            refine_setting: "threshold",
            mask_refine_cal_type: "bbox",
            mask_refine_ndm: 1,
        }),
        landtypes_global: &landtypes_global,
        mesh_type: "landmesh",
        lon_vertex: &lon_vertex,
        lat_vertex: &lat_vertex,
        lon_i: &lon_i,
        lat_i: &lat_i,
        gridnum_perdegree: 1,
        nlons_source: 6,
        nlats_source: 6,
        domain_output: Some(&domain_output),
        refine_output: Some(&refine_output),
    })
    .expect("run Area_judge grid orchestration");

    assert_eq!(
        report.area.domain.bounds,
        AreaJudgeSourceBounds {
            minlon_source: 2,
            maxlon_source: 3,
            maxlat_source: 2,
            minlat_source: 3,
        }
    );
    assert_eq!(report.domain_write.as_ref().unwrap().output, domain_output);
    assert_eq!(report.refine_write.as_ref().unwrap().output, refine_output);
    assert_eq!(report.refine_step.as_ref().unwrap().selected_cells, 4);

    let domain_file = netcdf::open(&domain_output).expect("open domain grid");
    assert_eq!(
        read_i32_2d(&domain_file, "IsInDmArea_select"),
        vec![1, 1, 1, 1]
    );
    assert_eq!(
        read_i32_2d(&domain_file, "seaorland_select"),
        vec![1, 0, 0, 1]
    );

    let refine_file = netcdf::open(&refine_output).expect("open refine grid");
    assert_eq!(
        read_i32_2d(&refine_file, "IsInArea_select"),
        vec![1, 1, 1, 1]
    );
}
