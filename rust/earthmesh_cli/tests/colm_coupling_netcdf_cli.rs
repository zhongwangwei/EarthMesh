use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
}

#[test]
fn binary_writes_colm_coupling_netcdf_from_package_csv() {
    let root = temp_root("colm_coupling_netcdf_cli");
    let input_csv = root.join("colm_coupling_cells.csv");
    let output_nc = root.join("colm_coupling_cells.nc");
    let delivery_manifest = root.join("delivery_manifest.json");
    fs::write(
        &input_csv,
        "case_name,cell_id,cell_index,center_lon,center_lat,surface_class,has_river,river_class,river_fraction,estimated_river_area_m2,has_coast,coast_class,coastal_fraction,normalized_cell_area_m2,source_areaCell,source_areaCell_units\n\
case_delta,C001,1,113.5,22.4,LAND,true,R2,0.25,1200.5,false,,0.0,4802.0,0.0001,m2\n\
case_delta,C002,2,114.5,23.4,COAST,false,,0.0,,true,COAST_OCEAN,0.75,5000.0,0.0002,m2\n",
    )
    .expect("write CoLM coupling CSV");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--colm-coupling-csv-to-netcdf")
        .arg(&input_csv)
        .arg(&output_nc)
        .arg("--case-name")
        .arg("case_delta")
        .arg("--delivery-manifest")
        .arg(&delivery_manifest)
        .output()
        .expect("run earthmesh_cli CoLM coupling NetCDF export");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("colm_coupling_netcdf="),
        "stdout should report output path: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("colm_delivery_manifest="),
        "stdout should report delivery manifest path: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let file = netcdf::open(&output_nc).expect("open CoLM coupling NetCDF");
    assert_eq!(file.dimension("cell").expect("cell dimension").len(), 2);
    assert_eq!(
        file.variable("cell_index")
            .expect("cell_index")
            .get_values::<i32, _>(..)
            .expect("read cell_index"),
        vec![1, 2]
    );
    assert_eq!(
        file.variable("surface_class_code")
            .expect("surface_class_code")
            .get_values::<i8, _>(..)
            .expect("read surface class codes"),
        vec![1, 3]
    );
    assert_eq!(
        file.variable("river_class_code")
            .expect("river_class_code")
            .get_values::<i8, _>(..)
            .expect("read river class codes"),
        vec![2, 0]
    );
    assert_eq!(
        file.variable("coast_class_code")
            .expect("coast_class_code")
            .get_values::<i8, _>(..)
            .expect("read coast class codes"),
        vec![0, 3]
    );
    assert_eq!(
        file.variable("has_river")
            .expect("has_river")
            .get_values::<i8, _>(..)
            .expect("read has_river"),
        vec![1, 0]
    );
    assert_eq!(
        file.variable("has_coast")
            .expect("has_coast")
            .get_values::<i8, _>(..)
            .expect("read has_coast"),
        vec![0, 1]
    );
    assert_eq!(
        file.variable("river_fraction")
            .expect("river_fraction")
            .get_values::<f64, _>(..)
            .expect("read river fractions"),
        vec![0.25, 0.0]
    );
    let manifest = fs::read_to_string(&delivery_manifest).expect("read CoLM delivery manifest");
    assert!(
        manifest.contains("\"kind\":\"earthmesh_colm_package_manifest\""),
        "manifest should declare package kind: {manifest}"
    );
    assert!(
        manifest.contains("\"case_name\":\"case_delta\""),
        "manifest should record case name: {manifest}"
    );
    assert!(
        manifest.contains("\"coupling_netcdf\""),
        "manifest should list coupling NetCDF product: {manifest}"
    );
    assert!(
        manifest.contains(output_nc.to_string_lossy().as_ref()),
        "manifest should include coupling output path: {manifest}"
    );
    assert!(
        manifest.contains("\"rows\":2"),
        "manifest should record row count: {manifest}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_writes_colm_restart_template_from_package_csv() {
    let root = temp_root("colm_restart_template_cli");
    let input_csv = root.join("colm_coupling_cells.csv");
    let output_nc = root.join("colm_coupling_cells.nc");
    let restart_nc = root.join("colm_restart_template.nc");
    fs::write(
        &input_csv,
        "case_name,cell_id,cell_index,center_lon,center_lat,surface_class,has_river,river_class,river_fraction,estimated_river_area_m2,has_coast,coast_class,coastal_fraction,normalized_cell_area_m2,source_areaCell,source_areaCell_units\n\
case_delta,C001,1,113.5,22.4,LAND,true,R2,0.25,1200.5,false,,0.0,4802.0,0.0001,m2\n\
case_delta,C002,2,114.5,23.4,COAST,false,,0.0,,true,COAST_OCEAN,0.75,5000.0,0.0002,m2\n\
case_delta,C003,3,115.5,24.4,OCEAN,false,,0.0,,false,,0.0,6000.0,0.0003,m2\n",
    )
    .expect("write CoLM coupling CSV");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--colm-coupling-csv-to-netcdf")
        .arg(&input_csv)
        .arg(&output_nc)
        .arg("--case-name")
        .arg("case_delta")
        .arg("--restart-template-netcdf")
        .arg(&restart_nc)
        .output()
        .expect("run earthmesh_cli CoLM restart template export");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("colm_restart_template_netcdf="),
        "stdout should report restart template path: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let file = netcdf::open(&restart_nc).expect("open CoLM restart template NetCDF");
    assert_eq!(file.dimension("cell").expect("cell dimension").len(), 3);
    assert_eq!(
        file.variable("cell_index")
            .expect("cell_index")
            .get_values::<i32, _>(..)
            .expect("read cell_index"),
        vec![1, 2, 3]
    );
    assert_eq!(
        file.variable("land_fraction")
            .expect("land_fraction")
            .get_values::<f64, _>(..)
            .expect("read land fractions"),
        vec![1.0, 0.25, 0.0]
    );
    assert_eq!(
        file.variable("river_fraction")
            .expect("river_fraction")
            .get_values::<f64, _>(..)
            .expect("read river fractions"),
        vec![0.25, 0.0, 0.0]
    );
    assert_eq!(
        file.variable("coastal_fraction")
            .expect("coastal_fraction")
            .get_values::<f64, _>(..)
            .expect("read coastal fractions"),
        vec![0.0, 0.75, 0.0]
    );
    assert_eq!(
        file.variable("normalized_cell_area_m2")
            .expect("normalized_cell_area_m2")
            .get_values::<f64, _>(..)
            .expect("read normalized cell area"),
        vec![4802.0, 5000.0, 6000.0]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_writes_colm_forcing_template_from_package_csv() {
    let root = temp_root("colm_forcing_template_cli");
    let input_csv = root.join("colm_coupling_cells.csv");
    let output_nc = root.join("colm_coupling_cells.nc");
    let forcing_nc = root.join("colm_forcing_template.nc");
    fs::write(
        &input_csv,
        "case_name,cell_id,cell_index,center_lon,center_lat,surface_class,has_river,river_class,river_fraction,estimated_river_area_m2,has_coast,coast_class,coastal_fraction,normalized_cell_area_m2,source_areaCell,source_areaCell_units\n\
case_delta,C001,1,113.5,22.4,LAND,true,R2,0.25,1200.5,false,,0.0,4802.0,0.0001,m2\n\
case_delta,C002,2,114.5,23.4,COAST,false,,0.0,,true,COAST_OCEAN,0.75,5000.0,0.0002,m2\n\
case_delta,C003,3,115.5,24.4,OCEAN,false,,0.0,,false,,0.0,6000.0,0.0003,m2\n",
    )
    .expect("write CoLM coupling CSV");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--colm-coupling-csv-to-netcdf")
        .arg(&input_csv)
        .arg(&output_nc)
        .arg("--case-name")
        .arg("case_delta")
        .arg("--forcing-template-netcdf")
        .arg(&forcing_nc)
        .output()
        .expect("run earthmesh_cli CoLM forcing template export");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("colm_forcing_template_netcdf="),
        "stdout should report forcing template path: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let file = netcdf::open(&forcing_nc).expect("open CoLM forcing template NetCDF");
    assert_eq!(file.dimension("cell").expect("cell dimension").len(), 3);
    assert_eq!(
        file.variable("cell_index")
            .expect("cell_index")
            .get_values::<i32, _>(..)
            .expect("read cell_index"),
        vec![1, 2, 3]
    );
    assert_eq!(
        file.variable("land_forcing_area_m2")
            .expect("land_forcing_area_m2")
            .get_values::<f64, _>(..)
            .expect("read land forcing area"),
        vec![4802.0, 1250.0, 0.0]
    );
    assert_eq!(
        file.variable("river_forcing_area_m2")
            .expect("river_forcing_area_m2")
            .get_values::<f64, _>(..)
            .expect("read river forcing area"),
        vec![1200.5, 0.0, 0.0]
    );
    assert_eq!(
        file.variable("coastal_forcing_area_m2")
            .expect("coastal_forcing_area_m2")
            .get_values::<f64, _>(..)
            .expect("read coastal forcing area"),
        vec![0.0, 3750.0, 0.0]
    );

    let _ = fs::remove_dir_all(root);
}
