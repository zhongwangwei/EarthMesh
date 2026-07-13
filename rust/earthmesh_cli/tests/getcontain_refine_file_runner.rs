use std::fs;

use earthmesh_cli::{
    area_judge_getcontain_refine::run_getcontain_refine_file_one_based,
    area_judge_grid_io::select_area_judge_grid_one_based,
    area_judge_grid_io::write_area_judge_grid_netcdf, contain_io::read_contain_netcdf,
    coordinate_types::LonLatPoint, getcontain_types::GetContainMeshKind,
    getcontain_types::GetContainRefineFileRunConfig,
    unstructured_mesh_io::write_unstructured_mesh_netcdf,
    unstructured_mesh_support::UnstructuredMesh,
};
use earthmesh_mesh::AreaJudgeSourceBounds;

#[test]
fn getcontain_refine_file_runner_reads_grid_and_area_then_writes_compatibility_contain() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_getcontain_refine_file_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("gridfile")).expect("create gridfile dir");
    fs::create_dir_all(root.join("result")).expect("create result dir");

    let gridfile = root.join("gridfile/gridfile_NXP0004_01_tri.nc4");
    write_unstructured_mesh_netcdf(
        &gridfile,
        &UnstructuredMesh {
            m_points: vec![LonLatPoint { lon: 1.0, lat: 1.0 }],
            w_points: vec![
                LonLatPoint { lon: 0.0, lat: 0.0 },
                LonLatPoint { lon: 2.0, lat: 0.0 },
                LonLatPoint { lon: 0.0, lat: 2.0 },
            ],
            m_to_w: vec![[1, 2, 3]],
            w_to_m: vec![vec![1], vec![1], vec![1]],
            n_w_to_m: vec![1, 1, 1],
        },
    )
    .expect("write gridfile");

    let lon_i = vec![f64::NAN, 0.5, 1.5, 2.5];
    let lat_i = vec![f64::NAN, 1.5, 0.5];
    let lon_vertex = vec![f64::NAN, 0.0, 2.0, 3.0];
    let lat_vertex = vec![f64::NAN, 2.0, 0.0];
    let mut refine_grid = vec![vec![0; lat_i.len()]; lon_i.len()];
    refine_grid[1][1] = 1;
    refine_grid[1][2] = 1;
    refine_grid[2][1] = 1;
    refine_grid[2][2] = 1;
    let payload = select_area_judge_grid_one_based(
        &refine_grid,
        None,
        &lon_i,
        &lat_i,
        AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: 2,
            maxlat_source: 1,
            minlat_source: 2,
        },
    )
    .expect("select refine grid");
    let area_grid = root.join("result/IsInRfArea_grid_iter1.nc4");
    write_area_judge_grid_netcdf(&area_grid, &payload).expect("write area grid");

    let mut seaorland = vec![vec![0; lat_i.len()]; lon_i.len()];
    seaorland[1][1] = 1;
    seaorland[1][2] = 1;
    seaorland[2][1] = 0;
    seaorland[2][2] = 1;
    let output = root.join("contain/contain_landmesh_refine_spc_NXP0004_01_tri.nc4");

    let report = run_getcontain_refine_file_one_based(GetContainRefineFileRunConfig {
        gridfile: &gridfile,
        area_grid_file: &area_grid,
        output: &output,
        mesh_kind: GetContainMeshKind::Land,
        seaorland: &seaorland,
        lon_vertex: &lon_vertex,
        lat_vertex: &lat_vertex,
        lon_i: &lon_i,
        lat_i: &lat_i,
        num_vertex: 0,
    })
    .expect("run Get_Contain refine file adapter");

    assert_eq!(report.output, output);
    assert_eq!(report.active_unstructured_cells, 1);
    assert_eq!(report.contained_source_pixels, 3);
    assert_eq!(report.runtime_counts.current_num_mp_step, 1);
    assert_eq!(report.runtime_counts.current_num_wp_step, 3);
    assert_eq!(report.runtime_counts.previous_num_vertex, 0);

    let contain = read_contain_netcdf(&report.output).expect("read contain output");
    assert_eq!(contain.is_in_area_ustr, vec![0, 1]);
    assert_eq!(contain.ustr_id, vec![vec![0, 0], vec![3, 1]]);
    assert_eq!(contain.ustr_ii, vec![vec![1, 1], vec![1, 2], vec![2, 2]]);

    let _ = fs::remove_dir_all(&root);
}
