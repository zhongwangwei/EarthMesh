use std::fs;

use earthmesh_cli::write_getref_specified_threshold_netcdf;

#[test]
fn specified_threshold_writer_preserves_fortran_schema_and_skips_placeholder() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_getref_specified_threshold_writer_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let output = root.join("threshold_specified_NXP0009_03.nc4");
    let is_in_refine_sjx = vec![0, 0, 1, -1, 1];

    let written = write_getref_specified_threshold_netcdf(&output, &is_in_refine_sjx)
        .expect("write specified threshold netcdf");

    assert_eq!(written.output, output);
    assert_eq!(written.sjx_points, 4);

    let file = netcdf::open(&written.output).expect("open specified threshold file");
    assert_eq!(file.dimension("sjx_points").unwrap().len(), 4);
    assert_eq!(
        read_i32(&file, "IsInRfArea_sjx_specified"),
        vec![0, 1, -1, 1]
    );
}

fn read_i32(file: &netcdf::File, name: &str) -> Vec<i32> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<i32, _>(..)
        .expect("read i32 values")
}
