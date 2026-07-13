use earthmesh_cli::{
    area_judge_grid_io::read_area_judge_grid_netcdf,
    area_judge_grid_io::select_area_judge_grid_one_based,
    area_judge_grid_io::write_area_judge_grid_netcdf,
};
use earthmesh_mesh::AreaJudgeSourceBounds;
use std::path::PathBuf;

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("create temp root");
    path
}

fn one_based_grid(nx: usize, ny: usize) -> Vec<Vec<i32>> {
    let mut grid = vec![vec![0; ny + 1]; nx + 1];
    for i in 1..=nx {
        for j in 1..=ny {
            grid[i][j] = (i as i32) * 10 + j as i32;
        }
    }
    grid
}

#[test]
fn area_judge_grid_writer_saves_canonical_selected_bounds_and_restart_aliases() {
    let root = temp_root("area_judge_grid_writer");
    let output = root.join("IsInDmArea_grid.nc4");
    let area_grid = one_based_grid(4, 4);
    let mut seaorland = one_based_grid(4, 4);
    seaorland[2][2] = 0;
    let longitude = vec![f64::NAN, 10.0, 20.0, 30.0, 40.0];
    let latitude = vec![f64::NAN, 50.0, 40.0, 30.0, 20.0];

    let payload = select_area_judge_grid_one_based(
        &area_grid,
        Some(&seaorland),
        &longitude,
        &latitude,
        AreaJudgeSourceBounds {
            minlon_source: 2,
            maxlon_source: 3,
            maxlat_source: 1,
            minlat_source: 3,
        },
    )
    .expect("selected area payload");
    write_area_judge_grid_netcdf(&output, &payload).expect("write area grid");

    let file = netcdf::open(&output).expect("open area grid");
    assert_eq!(file.dimension("nlons_select").unwrap().len(), 2);
    assert_eq!(file.dimension("nlats_select").unwrap().len(), 3);
    assert_eq!(read_i32(&file, "minlon_DmArea"), vec![2]);
    assert_eq!(read_i32(&file, "maxlon_DmArea"), vec![3]);
    assert_eq!(read_i32(&file, "maxlat_DmArea"), vec![1]);
    assert_eq!(read_i32(&file, "minlat_DmArea"), vec![3]);
    assert_eq!(read_f64(&file, "longitude"), vec![20.0, 30.0]);
    assert_eq!(read_f64(&file, "latitude"), vec![50.0, 40.0, 30.0]);
    assert_eq!(
        read_i32_2d(&file, "IsInArea_select"),
        vec![21, 22, 23, 31, 32, 33]
    );
    assert_eq!(
        read_i32_2d(&file, "IsInDmArea_select"),
        vec![21, 22, 23, 31, 32, 33]
    );
    assert_eq!(
        read_i32_2d(&file, "seaorland_select"),
        vec![21, 0, 23, 31, 32, 33]
    );
}

#[test]
fn area_judge_grid_reader_accepts_restart_schema_and_preserves_bounds() {
    let root = temp_root("area_judge_grid_reader");
    let input = root.join("restart.nc4");
    {
        let mut file = earthmesh_cli::create_netcdf_quiet(&input).expect("create restart");
        file.add_dimension("nlons_select", 2).unwrap();
        file.add_dimension("nlats_select", 2).unwrap();
        put_i32_scalar(&mut file, "minlon_DmArea", 4);
        put_i32_scalar(&mut file, "maxlon_DmArea", 5);
        put_i32_scalar(&mut file, "maxlat_DmArea", 6);
        put_i32_scalar(&mut file, "minlat_DmArea", 7);
        file.add_variable::<f64>("longitude", &["nlons_select"])
            .unwrap()
            .put_values(&[100.0, 101.0], ..)
            .unwrap();
        file.add_variable::<f64>("latitude", &["nlats_select"])
            .unwrap()
            .put_values(&[20.0, 19.0], ..)
            .unwrap();
        file.add_variable::<i32>("IsInDmArea_select", &["nlons_select", "nlats_select"])
            .unwrap()
            .put_values(&[1, 0, 0, 1], (.., ..))
            .unwrap();
        file.add_variable::<i32>("seaorland_select", &["nlons_select", "nlats_select"])
            .unwrap()
            .put_values(&[1, 0, 1, 0], (.., ..))
            .unwrap();
    }

    let payload = read_area_judge_grid_netcdf(&input).expect("read restart");
    assert_eq!(payload.bounds.minlon_source, 4);
    assert_eq!(payload.bounds.maxlon_source, 5);
    assert_eq!(payload.bounds.maxlat_source, 6);
    assert_eq!(payload.bounds.minlat_source, 7);
    assert_eq!(payload.longitude, vec![100.0, 101.0]);
    assert_eq!(payload.latitude, vec![20.0, 19.0]);
    assert_eq!(payload.is_in_area_select, vec![vec![1, 0], vec![0, 1]]);
    assert_eq!(payload.seaorland_select, Some(vec![vec![1, 0], vec![1, 0]]));
}

fn read_i32(file: &netcdf::File, name: &str) -> Vec<i32> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<i32, _>(..)
        .expect("read i32")
}

fn read_i32_2d(file: &netcdf::File, name: &str) -> Vec<i32> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<i32, _>((.., ..))
        .expect("read i32 2d")
}

fn read_f64(file: &netcdf::File, name: &str) -> Vec<f64> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<f64, _>(..)
        .expect("read f64")
}

fn put_i32_scalar(file: &mut netcdf::FileMut, name: &str, value: i32) {
    file.add_variable::<i32>(name, &[])
        .unwrap()
        .put_values(&[value], ..)
        .unwrap();
}
