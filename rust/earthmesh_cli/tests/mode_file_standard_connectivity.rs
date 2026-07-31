use std::fs;
use std::path::{Path, PathBuf};

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "earthmesh_cli_{name}_{}_{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create test root");
    path
}

fn add_f64_1d(file: &mut netcdf::FileMut, name: &str, dim: &str, values: &[f64]) {
    file.add_variable::<f64>(name, &[dim])
        .unwrap()
        .put_values(values, ..)
        .unwrap();
}

fn add_i32_1d(file: &mut netcdf::FileMut, name: &str, dim: &str, values: &[i32]) {
    file.add_variable::<i32>(name, &[dim])
        .unwrap()
        .put_values(values, ..)
        .unwrap();
}

fn write_standard_mpas(path: &Path) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).unwrap();
    file.add_dimension("nVertices", 3).unwrap();
    file.add_dimension("nCells", 1).unwrap();
    file.add_dimension("maxEdges", 3).unwrap();
    file.add_dimension("vertexDegree", 3).unwrap();
    add_f64_1d(&mut file, "lonVertex", "nVertices", &[0.0, 0.1, 0.2]);
    add_f64_1d(&mut file, "latVertex", "nVertices", &[0.0, 0.1, 0.0]);
    add_f64_1d(&mut file, "lonCell", "nCells", &[0.1]);
    add_f64_1d(&mut file, "latCell", "nCells", &[0.05]);
    file.add_variable::<i32>("cellsOnVertex", &["nVertices", "vertexDegree"])
        .unwrap()
        .put_values(&[1, 0, 0, 1, 0, 0, 1, 0, 0], (.., ..))
        .unwrap();
    file.add_variable::<i32>("verticesOnCell", &["nCells", "maxEdges"])
        .unwrap()
        .put_values(&[1, 2, 3], (.., ..))
        .unwrap();
    add_i32_1d(&mut file, "nEdgesOnCell", "nCells", &[3]);
}

fn write_standard_fvcom_fixture(
    path: &Path,
    nodes: usize,
    elements: usize,
    maxelem: usize,
    nv: &[i32],
    nbve: &[i32],
    ntve: &[i32],
) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).unwrap();
    file.add_dimension("node", nodes).unwrap();
    file.add_dimension("nele", elements).unwrap();
    file.add_dimension("three", 3).unwrap();
    file.add_dimension("maxelem", maxelem).unwrap();
    let lon = (0..nodes).map(|idx| idx as f64).collect::<Vec<_>>();
    let lat = vec![0.0; nodes];
    let lonc = (0..elements)
        .map(|idx| idx as f64 + 0.5)
        .collect::<Vec<_>>();
    let latc = vec![0.5; elements];
    add_f64_1d(&mut file, "lon", "node", &lon);
    add_f64_1d(&mut file, "lat", "node", &lat);
    add_f64_1d(&mut file, "lonc", "nele", &lonc);
    add_f64_1d(&mut file, "latc", "nele", &latc);
    file.add_variable::<i32>("nv", &["nele", "three"])
        .unwrap()
        .put_values(nv, (.., ..))
        .unwrap();
    file.add_variable::<i32>("nbve", &["node", "maxelem"])
        .unwrap()
        .put_values(nbve, (.., ..))
        .unwrap();
    add_i32_1d(&mut file, "ntve", "node", ntve);
}

fn write_standard_fvcom(path: &Path) {
    write_standard_fvcom_fixture(
        path,
        3,
        1,
        7,
        &[1, 2, 3],
        &[
            1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0,
        ],
        &[1, 1, 1],
    );
    let mut file = netcdf::append(path).unwrap();
    file.variable_mut("lon")
        .unwrap()
        .put_values(&[0.0, 0.0, 1.0], ..)
        .unwrap();
    file.variable_mut("lat")
        .unwrap()
        .put_values(&[0.0, 1.0, 0.0], ..)
        .unwrap();
}

#[test]
fn standard_mpas_one_based_connectivity_is_converted_automatically() {
    let root = temp_root("standard_mpas_one_based");
    let source = root.join("standard_mpas.nc");
    write_standard_mpas(&source);

    let report =
        earthmesh_cli::mode_file_io::convert_mpas_mode_file_to_earthmesh(&source, &root, 6, "hex")
            .expect("convert standard MPAS");
    let mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&report.output).unwrap();

    assert_eq!(mesh.m_to_w[1], [2, 1, 1]);
    assert_eq!(&mesh.w_to_m[1][..3], &[2, 3, 4]);
    assert_eq!(mesh.n_w_to_m[1], 3);
}

#[test]
fn standard_fvcom_one_based_connectivity_is_converted_automatically() {
    let root = temp_root("standard_fvcom_one_based");
    let source = root.join("standard_fvcom.nc");
    write_standard_fvcom(&source);

    let report =
        earthmesh_cli::mode_file_io::convert_fvcom_mode_file_to_earthmesh(&source, &root, 6, "tri")
            .expect("convert standard FVCOM");
    let mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&report.output).unwrap();

    assert_eq!(
        mesh.m_to_w[1],
        [2, 4, 3],
        "clockwise FVCOM connectivity must be canonicalized to CCW"
    );
    for node in 1..=3 {
        assert_eq!(mesh.w_to_m[node][0], 2);
        assert_eq!(mesh.n_w_to_m[node], 1);
    }
}

#[test]
fn standard_fvcom_maxelem_below_earthmesh_minimum_is_padded_automatically() {
    let root = temp_root("standard_fvcom_small_maxelem");
    let source = root.join("standard_fvcom_small_maxelem.nc");
    write_standard_fvcom_fixture(
        &source,
        3,
        1,
        3,
        &[1, 2, 3],
        &[1, 0, 0, 1, 0, 0, 1, 0, 0],
        &[1, 1, 1],
    );

    let report =
        earthmesh_cli::mode_file_io::convert_fvcom_mode_file_to_earthmesh(&source, &root, 6, "tri")
            .expect("convert standard FVCOM with small maxelem");
    assert_eq!(report.dimc, 7);
    let mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&report.output).unwrap();

    assert_eq!(mesh.n_w_to_m[1], 1);
    assert_eq!(mesh.w_to_m[1][0], 2);
}

#[test]
fn standard_fvcom_preserves_more_than_seven_incident_elements() {
    let root = temp_root("standard_fvcom_high_adjacency");
    let source = root.join("standard_fvcom_high_adjacency.nc");
    let mut nv = Vec::new();
    for element in 0..8 {
        nv.extend([1, element + 2, (element + 1) % 8 + 2]);
    }
    let mut nbve = vec![0_i32; 9 * 9];
    for element in 0..8 {
        nbve[element] = (element + 1) as i32;
        nbve[(element + 1) * 9] = (element + 1) as i32;
    }
    let mut ntve = vec![1_i32; 9];
    ntve[0] = 8;
    write_standard_fvcom_fixture(&source, 9, 8, 9, &nv, &nbve, &ntve);

    let report =
        earthmesh_cli::mode_file_io::convert_fvcom_mode_file_to_earthmesh(&source, &root, 6, "tri")
            .expect("convert standard FVCOM with high node adjacency");
    let mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&report.output).unwrap();

    assert_eq!(mesh.n_w_to_m[1], 8);
    assert_eq!(&mesh.w_to_m[1][..8], &[2, 3, 4, 5, 6, 7, 8, 9]);
    assert_eq!(mesh.w_to_m[1].len(), 9);
}
