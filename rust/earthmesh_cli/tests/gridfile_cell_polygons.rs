//! EarthMesh gridfile (GLONM/GLONW + itab connectivity) -> cell-polygon GeoJSON.
//! Covers the triangle (m_to_w) and hexagon (w_to_m) views; coords are degrees.
//! Some gridfiles pad leading rows with (0,0) placeholders; placeholders are
//! skipped by row identity, not by treating every real (0,0) coordinate as dummy.

use earthmesh_cli::{
    hydro_delivery_cells::{
        gridfile_cell_polygons_geojson as try_gridfile_cell_polygons_geojson,
        gridfile_cell_polygons_geojson_page_with_report as try_gridfile_cell_polygons_geojson_page_with_report,
        gridfile_cell_polygons_geojson_strided_with_report as try_gridfile_cell_polygons_geojson_strided_with_report,
        gridfile_cell_polygons_geojson_with_report as try_gridfile_cell_polygons_geojson_with_report,
    },
    unstructured_mesh_support::GridfileCellKind,
    unstructured_mesh_support::GridfileMeshPoints,
};

fn gridfile_cell_polygons_geojson(
    mesh: &GridfileMeshPoints,
    kind: GridfileCellKind,
    bbox: Option<[f64; 4]>,
    max_cells: Option<usize>,
) -> String {
    try_gridfile_cell_polygons_geojson(mesh, kind, bbox, max_cells)
        .expect("valid gridfile cell arrays")
}

fn gridfile_cell_polygons_geojson_with_report(
    mesh: &GridfileMeshPoints,
    kind: GridfileCellKind,
    bbox: Option<[f64; 4]>,
    max_cells: Option<usize>,
) -> (
    String,
    earthmesh_cli::hydro_delivery_cells::GridfileCellExportReport,
) {
    try_gridfile_cell_polygons_geojson_with_report(mesh, kind, bbox, max_cells)
        .expect("valid gridfile cell arrays")
}

fn empty_mesh() -> GridfileMeshPoints {
    GridfileMeshPoints {
        m_lon: vec![],
        m_lat: vec![],
        w_lon: vec![],
        w_lat: vec![],
        m_to_w: vec![],
        m_refine_level: vec![],
        m_refine_level_orig: vec![],
        m_ngr: vec![],
        w_to_m: vec![],
        w_to_m_width: 0,
        n_w: vec![],
        w_refine_level: vec![],
        w_refine_level_orig: vec![],
        w_ngr: vec![],
    }
}

#[test]
fn tri_view_emits_one_polygon_per_triangle() {
    // 4 W vertices (non-origin) forming a unit square; 2 triangles share W1-W3.
    let mut mesh = empty_mesh();
    mesh.w_lon = vec![100.0, 101.0, 101.0, 100.0];
    mesh.w_lat = vec![20.0, 20.0, 21.0, 21.0];
    mesh.m_lon = vec![100.67, 100.33];
    mesh.m_lat = vec![20.33, 20.67];
    mesh.m_to_w = vec![1, 2, 3, 1, 3, 4]; // 1-based: A=(W1,W2,W3), B=(W1,W3,W4)

    let json = gridfile_cell_polygons_geojson(&mesh, GridfileCellKind::Tri, None, None);

    assert_eq!(json.matches("\"type\": \"Feature\"").count(), 2, "{json}");
    for needle in ["[100, 20]", "[101, 20]", "[101, 21]", "[100, 21]"] {
        assert!(json.contains(needle), "missing {needle} in:\n{json}");
    }
    assert!(json.contains("\"grid_kind\": \"earthmesh_cell\""), "{json}");
}

#[test]
fn tri_skips_degenerate_sentinel_triangles() {
    // Method-C dummy M cells carry a triplet that repeats an index.
    let mut mesh = empty_mesh();
    mesh.w_lon = vec![10.0, 11.0, 11.0];
    mesh.w_lat = vec![20.0, 20.0, 21.0];
    mesh.m_lon = vec![10.0, 10.66];
    mesh.m_lat = vec![20.0, 20.33];
    mesh.m_to_w = vec![1, 1, 1, 1, 2, 3]; // first triplet degenerate, second real

    let json = gridfile_cell_polygons_geojson(&mesh, GridfileCellKind::Tri, None, None);

    assert_eq!(json.matches("\"type\": \"Feature\"").count(), 1, "{json}");
}

#[test]
fn tri_skips_single_compact_sentinel_row() {
    let mut mesh = empty_mesh();
    mesh.m_lon = vec![0.0, 10.33];
    mesh.m_lat = vec![0.0, 20.33];
    mesh.w_lon = vec![0.0, 10.0, 11.0, 10.0];
    mesh.w_lat = vec![0.0, 20.0, 20.0, 21.0];
    mesh.m_to_w = vec![1, 1, 1, 2, 3, 4];

    let json = gridfile_cell_polygons_geojson(&mesh, GridfileCellKind::Tri, None, None);

    assert_eq!(json.matches("\"type\": \"Feature\"").count(), 1, "{json}");
    assert!(json.contains("\"cell_id\": \"2\""), "{json}");
    assert!(!json.contains("[0, 0]"), "sentinel corner leaked:\n{json}");
}

#[test]
fn tri_skips_two_placeholder_rows_by_row_identity() {
    // Rows 0/1 are dummy placeholders. Canonical id 1 maps to a placeholder and
    // must be ignored; the real triangle using ids 2..4 survives.
    let mut mesh = empty_mesh();
    mesh.w_lon = vec![0.0, 0.0, 100.0, 101.0, 101.0];
    mesh.w_lat = vec![0.0, 0.0, 20.0, 20.0, 21.0];
    mesh.m_lon = vec![67.0, 100.67];
    mesh.m_lat = vec![13.0, 20.33];
    mesh.m_to_w = vec![1, 2, 3, 2, 3, 4];

    let json = gridfile_cell_polygons_geojson(&mesh, GridfileCellKind::Tri, None, None);

    assert_eq!(json.matches("\"type\": \"Feature\"").count(), 1, "{json}");
    assert!(!json.contains("[0, 0]"), "sentinel corner leaked:\n{json}");
}

#[test]
fn tri_keeps_valid_zero_zero_vertex_by_row_identity() {
    // The real W row 2 is at (0,0); only rows 0/1 are placeholders.
    let mut mesh = empty_mesh();
    mesh.w_lon = vec![0.0, 0.0, 0.0, 1.0, 0.0];
    mesh.w_lat = vec![0.0, 0.0, 0.0, 0.0, 1.0];
    mesh.m_lon = vec![0.33];
    mesh.m_lat = vec![0.33];
    mesh.m_to_w = vec![2, 3, 4];

    let json = gridfile_cell_polygons_geojson(&mesh, GridfileCellKind::Tri, None, None);

    assert_eq!(json.matches("\"type\": \"Feature\"").count(), 1, "{json}");
    assert!(
        json.contains("[0, 0]"),
        "real origin vertex was dropped:\n{json}"
    );
}

#[test]
fn tri_view_maps_two_placeholder_gridfile_ids_to_matching_rows() {
    // mask-postproc gridfiles preserve rows 0 and 1 as (0,0) placeholders; real
    // Canonical ids then match row numbers, so id 2 must read row 2, not row 1.
    let mut mesh = empty_mesh();
    mesh.w_lon = vec![0.0, 0.0, 100.0, 101.0, 100.0];
    mesh.w_lat = vec![0.0, 0.0, 20.0, 20.0, 21.0];
    mesh.m_lon = vec![0.0, 0.0, 100.33];
    mesh.m_lat = vec![0.0, 0.0, 20.33];
    mesh.m_to_w = vec![1, 1, 1, 1, 1, 1, 2, 3, 4];

    let json = gridfile_cell_polygons_geojson(&mesh, GridfileCellKind::Tri, None, None);

    assert_eq!(json.matches("\"type\": \"Feature\"").count(), 1, "{json}");
    for needle in ["[100, 20]", "[101, 20]", "[100, 21]"] {
        assert!(json.contains(needle), "missing {needle} in:\n{json}");
    }
    assert!(!json.contains("[0, 0]"), "placeholder row leaked:\n{json}");
}

#[test]
fn hex_view_emits_polygon_from_w_to_m_corners() {
    // one W cell centered at (10.5,20.5) with 4 M corners forming a unit square.
    let mut mesh = empty_mesh();
    mesh.w_lon = vec![10.5];
    mesh.w_lat = vec![20.5];
    mesh.m_lon = vec![10.0, 11.0, 11.0, 10.0];
    mesh.m_lat = vec![20.0, 20.0, 21.0, 21.0];
    mesh.w_to_m = vec![1, 2, 3, 4];
    mesh.w_to_m_width = 4;
    mesh.n_w = vec![4];

    let json = gridfile_cell_polygons_geojson(&mesh, GridfileCellKind::Hex, None, None);

    assert_eq!(json.matches("\"type\": \"Feature\"").count(), 1, "{json}");
    for needle in ["[10, 20]", "[11, 20]", "[11, 21]", "[10, 21]"] {
        assert!(json.contains(needle), "missing {needle} in:\n{json}");
    }
    assert!(json.contains("\"center_lon\": 10.5"), "{json}");
}

#[test]
fn hex_skips_single_compact_sentinel_row() {
    let mut mesh = empty_mesh();
    mesh.m_lon = vec![0.0, 0.0, 1.0, 1.0, 0.0];
    mesh.m_lat = vec![0.0, 0.0, 0.0, 1.0, 1.0];
    mesh.w_lon = vec![0.0, 0.5];
    mesh.w_lat = vec![0.0, 0.5];
    mesh.w_to_m = vec![1, 1, 1, 1, 2, 3, 4, 5];
    mesh.w_to_m_width = 4;
    mesh.n_w = vec![1, 4];

    let json = gridfile_cell_polygons_geojson(&mesh, GridfileCellKind::Hex, None, None);

    assert_eq!(json.matches("\"type\": \"Feature\"").count(), 1, "{json}");
    assert!(json.contains("\"cell_id\": \"2\""), "{json}");
    assert!(
        json.contains("[0, 0]"),
        "physical origin corner missing:\n{json}"
    );
}

#[test]
fn hex_keeps_first_physical_w_cell_at_origin_after_single_sentinel() {
    let mut mesh = empty_mesh();
    mesh.m_lon = vec![0.0, -1.0, 1.0, 1.0, -1.0];
    mesh.m_lat = vec![0.0, -1.0, -1.0, 1.0, 1.0];
    mesh.w_lon = vec![0.0, 0.0];
    mesh.w_lat = vec![0.0, 0.0];
    mesh.w_to_m = vec![1, 1, 1, 1, 2, 3, 4, 5];
    mesh.w_to_m_width = 4;
    mesh.n_w = vec![1, 4];

    let json = gridfile_cell_polygons_geojson(&mesh, GridfileCellKind::Hex, None, None);

    assert_eq!(json.matches("\"type\": \"Feature\"").count(), 1, "{json}");
    assert!(json.contains("\"cell_id\": \"2\""), "{json}");
    assert!(
        json.contains("\"center_lon\": 0"),
        "physical origin cell missing:\n{json}"
    );
}

#[test]
fn hex_skips_two_placeholder_corners_by_row_identity() {
    // Rows 0/1 are dummy placeholders. Canonical id 1 is ignored; ids 2..5 are real.
    let mut mesh = empty_mesh();
    mesh.w_lon = vec![10.5];
    mesh.w_lat = vec![20.5];
    mesh.m_lon = vec![0.0, 0.0, 10.0, 11.0, 11.0, 10.0];
    mesh.m_lat = vec![0.0, 0.0, 20.0, 20.0, 21.0, 21.0];
    mesh.w_to_m = vec![1, 2, 3, 4, 5];
    mesh.w_to_m_width = 5;
    mesh.n_w = vec![5];

    let json = gridfile_cell_polygons_geojson(&mesh, GridfileCellKind::Hex, None, None);

    assert_eq!(json.matches("\"type\": \"Feature\"").count(), 1, "{json}");
    assert!(!json.contains("[0, 0]"), "sentinel corner leaked:\n{json}");
}

#[test]
fn hex_keeps_valid_zero_zero_center_and_corner_by_row_identity() {
    // W row 2 and M row 2 are real origin points; rows 0/1 are placeholders.
    let mut mesh = empty_mesh();
    mesh.w_lon = vec![0.0, 0.0, 0.0];
    mesh.w_lat = vec![0.0, 0.0, 0.0];
    mesh.m_lon = vec![0.0, 0.0, 0.0, 1.0, 1.0, 0.0];
    mesh.m_lat = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0];
    mesh.w_to_m = vec![0, 0, 0, 0, 0, 0, 0, 0, 2, 3, 4, 5];
    mesh.w_to_m_width = 4;
    mesh.n_w = vec![0, 0, 4];

    let json = gridfile_cell_polygons_geojson(&mesh, GridfileCellKind::Hex, None, None);

    assert_eq!(json.matches("\"type\": \"Feature\"").count(), 1, "{json}");
    assert!(json.contains("\"cell_index\": 2"), "{json}");
    assert!(json.contains("\"cell_id\": \"2\""), "{json}");
    assert!(json.contains("\"center_lon\": 0"), "{json}");
    assert!(
        json.contains("[0, 0]"),
        "real origin corner was dropped:\n{json}"
    );
}

#[test]
fn hex_respects_n_w_valid_count() {
    // width 6 but only 4 valid corners; the padding entries (0) must be ignored.
    let mut mesh = empty_mesh();
    mesh.w_lon = vec![10.5];
    mesh.w_lat = vec![20.5];
    mesh.m_lon = vec![10.0, 11.0, 11.0, 10.0];
    mesh.m_lat = vec![20.0, 20.0, 21.0, 21.0];
    mesh.w_to_m = vec![1, 2, 3, 4, 0, 0]; // 6-wide, last two padding
    mesh.w_to_m_width = 6;
    mesh.n_w = vec![4];

    let json = gridfile_cell_polygons_geojson(&mesh, GridfileCellKind::Hex, None, None);

    assert_eq!(json.matches("\"type\": \"Feature\"").count(), 1, "{json}");
    let ring_pairs = json.matches("], [").count();
    assert!(
        ring_pairs >= 4,
        "expected >=4 segment joins, got {ring_pairs}:\n{json}"
    );
}

#[test]
fn max_cells_caps_feature_count() {
    let mut mesh = empty_mesh();
    mesh.w_lon = vec![100.0, 101.0, 101.0, 100.0];
    mesh.w_lat = vec![20.0, 20.0, 21.0, 21.0];
    mesh.m_lon = vec![100.6, 100.4];
    mesh.m_lat = vec![20.4, 20.6];
    mesh.m_to_w = vec![1, 2, 3, 1, 3, 4];

    let json = gridfile_cell_polygons_geojson(&mesh, GridfileCellKind::Tri, None, Some(1));

    assert_eq!(json.matches("\"type\": \"Feature\"").count(), 1, "{json}");
}

#[test]
fn pages_continue_after_the_previous_eligible_cell() {
    let mut mesh = empty_mesh();
    mesh.w_lon = vec![100.0, 101.0, 101.0, 100.0];
    mesh.w_lat = vec![20.0, 20.0, 21.0, 21.0];
    mesh.m_lon = vec![100.6, 100.4];
    mesh.m_lat = vec![20.4, 20.6];
    mesh.m_to_w = vec![1, 2, 3, 1, 3, 4];

    let (json, report) = try_gridfile_cell_polygons_geojson_page_with_report(
        &mesh,
        GridfileCellKind::Tri,
        None,
        1,
        Some(1),
    )
    .expect("second page");

    assert_eq!(report.emitted_cells, 1);
    assert!(json.contains("\"cell_index\": 2"), "{json}");
    assert!(!json.contains("\"cell_index\": 1"), "{json}");
}

#[test]
fn stride_samples_across_the_full_cell_order() {
    let mut mesh = empty_mesh();
    for cell in 0..4 {
        let lon = 100.0 + f64::from(cell);
        mesh.w_lon.extend([lon, lon + 0.4, lon]);
        mesh.w_lat.extend([20.0, 20.0, 20.4]);
        mesh.m_lon.push(lon + 0.13);
        mesh.m_lat.push(20.13);
        let first = cell * 3 + 1;
        mesh.m_to_w
            .extend([first as i32, first as i32 + 1, first as i32 + 2]);
    }

    let (json, report) = try_gridfile_cell_polygons_geojson_strided_with_report(
        &mesh,
        GridfileCellKind::Tri,
        None,
        0,
        Some(2),
        2,
    )
    .expect("strided overview");

    assert_eq!(report.emitted_cells, 2);
    assert!(json.contains("\"cell_index\": 1"), "{json}");
    assert!(json.contains("\"cell_index\": 3"), "{json}");
    assert!(!json.contains("\"cell_index\": 2"), "{json}");
    assert!(!json.contains("\"cell_index\": 4"), "{json}");
}

#[test]
fn bbox_broad_phase_keeps_cells_that_cross_the_boundary() {
    let mut mesh = empty_mesh();
    mesh.w_lon = vec![98.5, 100.0, 100.0, 150.0, 151.0, 151.0];
    mesh.w_lat = vec![20.0, 20.0, 21.0, 20.0, 20.0, 21.0];
    mesh.m_lon = vec![98.8, 150.66]; // first center is outside but its polygon crosses 99E
    mesh.m_lat = vec![20.33, 20.33];
    mesh.m_to_w = vec![1, 2, 3, 4, 5, 6];

    let json = gridfile_cell_polygons_geojson(
        &mesh,
        GridfileCellKind::Tri,
        Some([99.0, 19.0, 102.0, 22.0]), // W S E N around the first triangle only
        None,
    );

    assert_eq!(json.matches("\"type\": \"Feature\"").count(), 1, "{json}");
    assert!(json.contains("\"cell_index\": 1"), "{json}");
    assert!(!json.contains("\"cell_index\": 2"), "{json}");
}

#[test]
fn tri_skips_cells_spanning_the_antimeridian() {
    // A triangle whose vertices span >180° of longitude (antimeridian / pole) would
    // be drawn as a band across a flat map; it must be dropped.
    let mut mesh = empty_mesh();
    mesh.w_lon = vec![170.0, -170.0, 10.0]; // spans 340°
    mesh.w_lat = vec![10.0, 12.0, 80.0];
    mesh.m_lon = vec![5.0];
    mesh.m_lat = vec![34.0];
    mesh.m_to_w = vec![1, 2, 3];

    let (json, report) =
        gridfile_cell_polygons_geojson_with_report(&mesh, GridfileCellKind::Tri, None, None);

    assert_eq!(json.matches("\"type\": \"Feature\"").count(), 0, "{json}");
    assert_eq!(report.rejected_unsupported_cells, 1);
}

#[test]
fn hex_exports_compact_polar_cells_spanning_all_longitudes() {
    // Longitude span is not a physical size criterion near the pole.
    let mut mesh = empty_mesh();
    mesh.w_lon = vec![5.0];
    mesh.w_lat = vec![88.0];
    mesh.m_lon = vec![144.0, 0.0, -144.0]; // spans 288°
    mesh.m_lat = vec![85.0, 89.0, 85.0];
    mesh.w_to_m = vec![1, 2, 3];
    mesh.w_to_m_width = 3;
    mesh.n_w = vec![3];

    let json = gridfile_cell_polygons_geojson(&mesh, GridfileCellKind::Hex, None, None);

    assert_eq!(json.matches("\"type\": \"Feature\"").count(), 1, "{json}");
}

#[test]
fn tri_exports_compact_cells_above_eighty_degrees() {
    let mut mesh = empty_mesh();
    mesh.w_lon = vec![10.0, 11.0, 10.5];
    mesh.w_lat = vec![84.5, 84.5, 85.0];
    mesh.m_lon = vec![10.5];
    mesh.m_lat = vec![84.67];
    mesh.m_to_w = vec![1, 2, 3];

    let json = gridfile_cell_polygons_geojson(&mesh, GridfileCellKind::Tri, None, None);

    assert_eq!(json.matches("\"type\": \"Feature\"").count(), 1, "{json}");
    assert!(json.contains("84.5"), "{json}");
}

#[test]
fn full_longitude_bbox_does_not_collapse_to_a_meridian() {
    let mut mesh = empty_mesh();
    mesh.w_lon = vec![-1.0, 1.0, 0.0];
    mesh.w_lat = vec![-1.0, -1.0, 1.0];
    mesh.m_lon = vec![0.0];
    mesh.m_lat = vec![-0.33];
    mesh.m_to_w = vec![1, 2, 3];

    let json = gridfile_cell_polygons_geojson(
        &mesh,
        GridfileCellKind::Tri,
        Some([-180.0, -10.0, 180.0, 10.0]),
        None,
    );

    assert_eq!(json.matches("\"type\": \"Feature\"").count(), 1, "{json}");
}

#[test]
fn gridfile_export_rejects_mismatched_non_finite_and_invalid_arrays() {
    let mut mesh = empty_mesh();
    mesh.m_lon = vec![0.0];
    mesh.m_lat = vec![];
    mesh.w_lon = vec![0.0, 1.0, 0.0];
    mesh.w_lat = vec![0.0, 0.0, 1.0];
    mesh.m_to_w = vec![1, 2, 3];
    assert!(try_gridfile_cell_polygons_geojson(&mesh, GridfileCellKind::Tri, None, None).is_err());

    mesh.m_lat = vec![0.0];
    mesh.w_lon[2] = f64::NAN;
    assert!(try_gridfile_cell_polygons_geojson(&mesh, GridfileCellKind::Tri, None, None).is_err());

    mesh.w_lon[2] = 0.0;
    mesh.m_to_w = vec![1, 2];
    assert!(try_gridfile_cell_polygons_geojson(&mesh, GridfileCellKind::Tri, None, None).is_err());

    mesh.m_to_w = vec![1, 2, 4];
    assert!(try_gridfile_cell_polygons_geojson(&mesh, GridfileCellKind::Tri, None, None).is_err());
}
