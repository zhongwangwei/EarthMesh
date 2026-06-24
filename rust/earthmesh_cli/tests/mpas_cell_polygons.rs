//! Rust port of util/hydro_mesh/earthmesh_intersection.py::read_mpas_cell_polygons:
//! MPAS/EarthMesh cell arrays (radians, 1-based verticesOnCell) -> cell-polygon GeoJSON.

use earthmesh_cli::mpas_cell_polygons_geojson;

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
    );
    assert!(json.contains("\"cell_id\": \"1\""));
    assert!(json.contains("\"cell_index\": 0"));
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
    );
    assert_eq!(json.matches("\"type\": \"Feature\"").count(), 1, "{json}");
    assert!(json.contains("\"cell_index\": 0"));
    assert!(!json.contains("\"cell_index\": 1"));
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
    );
    assert_eq!(json.matches("\"type\": \"Feature\"").count(), 1, "{json}");
    assert!(json.contains("\"cell_id\": \"101\""), "{json}");
    assert!(!json.contains("\"cell_id\": \"1\""), "{json}");
    assert!(!json.contains("\"cell_id\": \"202\""), "{json}");
}
