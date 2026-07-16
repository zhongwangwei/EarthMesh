//! Rust port of util/hydro_mesh/earthmesh_intersection.py::read_mpas_cell_polygons:
//! MPAS/EarthMesh cell arrays (radians, 1-based verticesOnCell) -> cell-polygon GeoJSON.

use earthmesh_cli::hydro_delivery_cells::{
    mpas_cell_polygons_geojson, write_mpas_cell_polygons_geojson,
};
use std::{fs, path::Path};

fn temp_root(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "earthmesh-mpas-polygons-{label}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    root
}

fn write_mpas_fixture(path: &Path, vertex_dims: [&str; 2], connectivity: &[i32]) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create MPAS fixture");
    for (name, len) in [
        ("nCells", 2),
        ("maxEdges", 3),
        ("nVertices", 6),
        ("rows", 2),
        ("columns", 3),
    ] {
        file.add_dimension(name, len).expect("add MPAS dimension");
    }
    file.add_variable::<f64>("lonCell", &["nCells"])
        .unwrap()
        .put_values(&[0.1, 1.1], ..)
        .unwrap();
    file.add_variable::<f64>("latCell", &["nCells"])
        .unwrap()
        .put_values(&[0.1, 0.1], ..)
        .unwrap();
    file.add_variable::<f64>("lonVertex", &["nVertices"])
        .unwrap()
        .put_values(&[0.0, 0.1, 0.2, 1.0, 1.1, 1.2], ..)
        .unwrap();
    file.add_variable::<f64>("latVertex", &["nVertices"])
        .unwrap()
        .put_values(&[0.0, 0.2, 0.0, 0.0, 0.2, 0.0], ..)
        .unwrap();
    file.add_variable::<i32>("nEdgesOnCell", &["nCells"])
        .unwrap()
        .put_values(&[3, 3], ..)
        .unwrap();
    file.add_variable::<i32>("verticesOnCell", &vertex_dims)
        .unwrap()
        .put_values(connectivity, (.., ..))
        .unwrap();
}

#[test]
fn one_square_cell_to_polygon_ring() {
    let d2r = |d: f64| d.to_radians();
    // one square cell with corners (100,20)(101,20)(101,21)(100,21), center (100.5,20.5)
    let lon_cell = [d2r(100.5)];
    let lat_cell = [d2r(20.5)];
    let lon_vertex = [d2r(100.0), d2r(101.0), d2r(101.0), d2r(100.0)];
    let lat_vertex = [d2r(20.0), d2r(20.0), d2r(21.0), d2r(21.0)];
    let n_edges_on_cell = [4i32];
    let vertices_on_cell = [1i32, 2, 3, 4]; // 1-based, flat (1 cell x max_edges 4)

    let json = mpas_cell_polygons_geojson(
        &lon_cell,
        &lat_cell,
        &lon_vertex,
        &lat_vertex,
        &n_edges_on_cell,
        &vertices_on_cell,
        None,
        None,
        None,
        None,
    )
    .expect("valid MPAS cell arrays");
    assert!(json.contains("\"cell_id\": \"1\""));
    assert!(json.contains("\"cell_index\": 1"));
    assert!(json.contains("\"center_lon\": 100.5"));
    assert!(json.contains("\"grid_kind\": \"earthmesh_cell\""));
    // ring corners in degrees
    for needle in ["[100, 20]", "[101, 20]", "[101, 21]", "[100, 21]"] {
        assert!(json.contains(needle), "missing {needle} in:\n{json}");
    }
}

#[test]
fn bbox_filters_cells_by_center() {
    let d2r = |d: f64| d.to_radians();
    // two cells far apart; bbox keeps only the first.
    let lon_cell = [d2r(0.5), d2r(50.5)];
    let lat_cell = [d2r(0.5), d2r(0.5)];
    let lon_vertex = [
        d2r(0.0),
        d2r(1.0),
        d2r(1.0),
        d2r(0.0),
        d2r(50.0),
        d2r(51.0),
        d2r(51.0),
        d2r(50.0),
    ];
    let lat_vertex = [
        d2r(0.0),
        d2r(0.0),
        d2r(1.0),
        d2r(1.0),
        d2r(0.0),
        d2r(0.0),
        d2r(1.0),
        d2r(1.0),
    ];
    let n_edges_on_cell = [4i32, 4];
    let vertices_on_cell = [1i32, 2, 3, 4, 5, 6, 7, 8]; // 2 cells x 4
    let json = mpas_cell_polygons_geojson(
        &lon_cell,
        &lat_cell,
        &lon_vertex,
        &lat_vertex,
        &n_edges_on_cell,
        &vertices_on_cell,
        None,
        None,
        Some([-1.0, -1.0, 2.0, 2.0]), // W S E N around the first cell only
        None,
    )
    .expect("valid MPAS cell arrays");
    assert_eq!(json.matches("\"type\": \"Feature\"").count(), 1, "{json}");
    assert!(json.contains("\"cell_index\": 1"));
    assert!(!json.contains("\"cell_index\": 2"));
}

#[test]
fn preserves_mpas_index_to_cell_id_when_present() {
    let d2r = |d: f64| d.to_radians();
    let lon_cell = [d2r(0.5), d2r(50.5)];
    let lat_cell = [d2r(0.5), d2r(0.5)];
    let lon_vertex = [
        d2r(0.0),
        d2r(1.0),
        d2r(1.0),
        d2r(0.0),
        d2r(50.0),
        d2r(51.0),
        d2r(51.0),
        d2r(50.0),
    ];
    let lat_vertex = [
        d2r(0.0),
        d2r(0.0),
        d2r(1.0),
        d2r(1.0),
        d2r(0.0),
        d2r(0.0),
        d2r(1.0),
        d2r(1.0),
    ];
    let n_edges_on_cell = [4i32, 4];
    let vertices_on_cell = [1i32, 2, 3, 4, 5, 6, 7, 8];
    let cell_ids = [101i32, 202];
    let json = mpas_cell_polygons_geojson(
        &lon_cell,
        &lat_cell,
        &lon_vertex,
        &lat_vertex,
        &n_edges_on_cell,
        &vertices_on_cell,
        Some(&cell_ids),
        None,
        Some([-1.0, -1.0, 2.0, 2.0]),
        None,
    )
    .expect("valid MPAS cell arrays");
    assert_eq!(json.matches("\"type\": \"Feature\"").count(), 1, "{json}");
    assert!(json.contains("\"cell_id\": \"101\""), "{json}");
    assert!(!json.contains("\"cell_id\": \"1\""), "{json}");
    assert!(!json.contains("\"cell_id\": \"202\""), "{json}");
}

#[test]
fn rejects_mismatched_non_finite_and_invalid_mpas_arrays() {
    let valid = || {
        mpas_cell_polygons_geojson(
            &[0.0],
            &[0.0],
            &[0.0, 0.1, 0.1],
            &[0.0, 0.0, 0.1],
            &[3],
            &[1, 2, 3],
            None,
            None,
            None,
            None,
        )
    };
    assert!(valid().is_ok());
    assert!(mpas_cell_polygons_geojson(
        &[0.0],
        &[],
        &[0.0, 0.1, 0.1],
        &[0.0, 0.0, 0.1],
        &[3],
        &[1, 2, 3],
        None,
        None,
        None,
        None,
    )
    .is_err());

    assert!(mpas_cell_polygons_geojson(
        &[f64::MAX],
        &[0.0],
        &[0.0, 0.1, 0.1, 0.0],
        &[0.0, 0.0, 0.1, 0.1],
        &[4],
        &[1, 2, 3, 4],
        None,
        None,
        None,
        None,
    )
    .is_err());

    assert!(mpas_cell_polygons_geojson(
        &[0.0],
        &[std::f64::consts::PI],
        &[0.0, 0.1, 0.1, 0.0],
        &[0.0, 0.0, 0.1, 0.1],
        &[4],
        &[1, 2, 3, 4],
        None,
        None,
        None,
        None,
    )
    .is_err());
    assert!(mpas_cell_polygons_geojson(
        &[0.0],
        &[0.0],
        &[0.0, 0.1, f64::INFINITY],
        &[0.0, 0.0, 0.1],
        &[3],
        &[1, 2, 3],
        None,
        None,
        None,
        None,
    )
    .is_err());
    assert!(mpas_cell_polygons_geojson(
        &[0.0, 0.1],
        &[0.0, 0.1],
        &[0.0, 0.1, 0.1],
        &[0.0, 0.0, 0.1],
        &[3, 3],
        &[1, 2, 3, 1, 2],
        None,
        None,
        None,
        None,
    )
    .is_err());
    assert!(mpas_cell_polygons_geojson(
        &[0.0],
        &[0.0],
        &[0.0, 0.1, 0.1],
        &[0.0, 0.0, 0.1],
        &[3],
        &[1, 2, 4],
        None,
        None,
        None,
        None,
    )
    .is_err());
}

#[test]
fn mpas_writer_transposes_reversed_and_rejects_unrelated_connectivity_dimensions() {
    let root = temp_root("reversed-connectivity");
    let input = root.join("mesh.nc");
    let output = root.join("cells.geojson");
    write_mpas_fixture(&input, ["maxEdges", "nCells"], &[1, 4, 2, 5, 3, 6]);

    let count = write_mpas_cell_polygons_geojson(&input, &output, None, None)
        .expect("explicitly transpose maxEdges x nCells connectivity");
    assert_eq!(count, 2);
    let json = fs::read_to_string(&output).unwrap();
    assert!(json.contains("[0, 0]"), "{json}");
    assert!(json.contains("[57.295779513082, 0]"), "{json}");
    let _ = fs::remove_dir_all(root);

    let root = temp_root("wrong-connectivity-dimensions");
    let input = root.join("mesh.nc");
    write_mpas_fixture(&input, ["rows", "columns"], &[1, 2, 3, 4, 5, 6]);

    let error = write_mpas_cell_polygons_geojson(&input, root.join("cells.geojson"), None, None)
        .expect_err("unrelated connectivity dimensions must not be guessed from shape");
    assert!(
        error.to_string().contains("do not match required"),
        "{error}"
    );
    let _ = fs::remove_dir_all(root);
}
