use std::fs;
use std::io;
use std::path::Path;

/// Ray-cast point-in-(arbitrary)-ring test, for nesting union holes under their outer.
fn point_in_polygon_ring(p: earthmesh_geometry::Point, ring: &[earthmesh_geometry::Point]) -> bool {
    let mut inside = false;
    let n = ring.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (ring[i], ring[j]);
        if ((a.y > p.y) != (b.y > p.y)) && (p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Dissolve a coastal-band mask into a merged MultiPolygon GeoJSON (the `dissolve=True`
/// path of `coastal_band.py::coastal_band_geojson`). The band cells are equal grid
/// boxes, so the union is exact via `earthmesh_geometry::dissolve_axis_aligned_boxes`
/// (edge cancellation), then CW hole rings are nested under their containing CCW outer.
/// Returns the polygon (outer-ring) count.
pub fn write_coastal_band_dissolve_geojson(
    band: &[Vec<bool>],
    x_start: i64,
    y_start: i64,
    west: f64,
    south: f64,
    grid_size_deg: f64,
    output_geojson: impl AsRef<Path>,
) -> io::Result<usize> {
    use earthmesh_geometry::{dissolve_axis_aligned_boxes, signed_ring_area, Point};
    let mut boxes = Vec::new();
    let mut cell_count = 0usize;
    for (row, line) in band.iter().enumerate() {
        for (col, &selected) in line.iter().enumerate() {
            if !selected {
                continue;
            }
            cell_count += 1;
            let x0 = west + (x_start + col as i64) as f64 * grid_size_deg;
            let y0 = south + (y_start + row as i64) as f64 * grid_size_deg;
            boxes.push((x0, y0, x0 + grid_size_deg, y0 + grid_size_deg));
        }
    }
    let rings = dissolve_axis_aligned_boxes(&boxes);
    let outers: Vec<&Vec<Point>> = rings.iter().filter(|r| signed_ring_area(r) > 0.0).collect();
    let holes: Vec<&Vec<Point>> = rings.iter().filter(|r| signed_ring_area(r) < 0.0).collect();

    let ring_coords = |ring: &[Point]| -> String {
        let mut pts: Vec<String> = ring.iter().map(|p| format!("[{}, {}]", p.x, p.y)).collect();
        if let Some(first) = ring.first() {
            pts.push(format!("[{}, {}]", first.x, first.y)); // close the ring
        }
        format!("[{}]", pts.join(", "))
    };

    let mut polygons: Vec<String> = Vec::new();
    for &outer in &outers {
        let mut rings_json = vec![ring_coords(outer)];
        for &hole in &holes {
            if hole
                .first()
                .is_some_and(|p| point_in_polygon_ring(*p, outer))
            {
                rings_json.push(ring_coords(hole));
            }
        }
        polygons.push(format!("[{}]", rings_json.join(", ")));
    }

    let geometry = format!(
        "{{\"type\": \"MultiPolygon\", \"coordinates\": [{}]}}",
        polygons.join(", ")
    );
    let out = format!(
        "{{\n  \"type\": \"FeatureCollection\",\n  \"features\": [\n    {{\"type\": \"Feature\", \
         \"geometry\": {}, \"properties\": {{\"mask_class\": \"COAST\", \
         \"coastal_band_cell_count\": {}, \"corridor_source_geometry\": \"cama_elevtn_coastal_band\"}}}}\n  ]\n}}\n",
        geometry, cell_count
    );
    crate::ensure_parent_dir(output_geojson.as_ref())?;
    fs::write(output_geojson, out)?;
    Ok(outers.len())
}

/// Non-dissolve coastal-band output: one Polygon Feature per band cell (port of the
/// `coastal_band.py::coastal_band_geojson` dissolve=False path).
pub(super) fn write_coastal_band_cells_geojson(
    band: &[Vec<bool>],
    land_mask: &[Vec<bool>],
    x_start: i64,
    y_start: i64,
    west: f64,
    south: f64,
    grid_size_deg: f64,
    output_geojson: impl AsRef<Path>,
) -> io::Result<usize> {
    let mut cells: Vec<(i64, i64, bool)> = Vec::new();
    for (row, line) in band.iter().enumerate() {
        for (col, &selected) in line.iter().enumerate() {
            if selected {
                let is_land = land_mask
                    .get(row)
                    .and_then(|r| r.get(col))
                    .copied()
                    .unwrap_or(false);
                cells.push((x_start + col as i64, y_start + row as i64, is_land));
            }
        }
    }
    let land_count = cells.iter().filter(|(_, _, l)| *l).count();
    let total = cells.len();
    let mut features = Vec::new();
    for (xi, yi, is_land) in &cells {
        let x0 = west + *xi as f64 * grid_size_deg;
        let y0 = south + *yi as f64 * grid_size_deg;
        let (x1, y1) = (x0 + grid_size_deg, y0 + grid_size_deg);
        features.push(format!(
            "    {{\"type\": \"Feature\", \"geometry\": {{\"type\": \"Polygon\", \"coordinates\": \
             [[[{x0}, {y0}], [{x1}, {y0}], [{x1}, {y1}], [{x0}, {y1}], [{x0}, {y0}]]]}}, \
             \"properties\": {{\"mask_class\": \"COAST\", \"coastal_band_cell_count\": {total}, \
             \"land_side_cell_count\": {land_count}, \"ocean_side_cell_count\": {}, \
             \"corridor_source_geometry\": \"cama_elevtn_coastal_band\", \"x_index\": {xi}, \
             \"y_index\": {yi}, \"coastal_side\": \"{}\"}}}}",
            total - land_count,
            if *is_land { "land" } else { "ocean" }
        ));
    }
    let out = format!(
        "{{\n  \"type\": \"FeatureCollection\",\n  \"features\": [\n{}\n  ]\n}}\n",
        features.join(",\n")
    );
    crate::ensure_parent_dir(output_geojson.as_ref())?;
    fs::write(output_geojson, out)?;
    Ok(total)
}
