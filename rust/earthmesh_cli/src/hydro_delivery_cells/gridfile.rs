use std::{fs, io, path::Path};

use crate::{
    gridfile_m_row_layout, gridfile_w_row_layout, read_gridfile_mesh_points, GridfileCellKind,
    GridfileMeshPoints,
};

use super::geometry::{
    cell_has_unsupported_edge_arc, order_around_spherical_center, ring_intersects_directed_bbox,
    split_ring_at_antimeridian,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GridfileCellExportReport {
    pub emitted_cells: usize,
    pub rejected_unsupported_cells: usize,
}

fn polygon_geometry(ring: &[(f64, f64)]) -> String {
    let coords = ring
        .iter()
        .map(|(x, y)| format!("[{x}, {y}]"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{\"type\": \"Polygon\", \"coordinates\": [[{coords}]]}}")
}

fn seam_safe_polygon_geometry(corners: &[(f64, f64)], center_lon: f64) -> Option<String> {
    let rings = split_ring_at_antimeridian(corners, center_lon);
    if rings.len() == 1 {
        return Some(polygon_geometry(&rings[0]));
    }
    if rings.is_empty() {
        return None;
    }
    let polygons = rings
        .iter()
        .map(|ring| {
            let coordinates = ring
                .iter()
                .map(|(lon, lat)| format!("[{lon}, {lat}]"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[[{coordinates}]]")
        })
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!(
        "{{\"type\": \"MultiPolygon\", \"coordinates\": [{polygons}]}}"
    ))
}

fn polar_cap_geometry(corners: &[(f64, f64)], pole_lat: f64) -> String {
    let mut sorted = corners.to_vec();
    sorted.sort_by(|left, right| left.0.total_cmp(&right.0));
    let first = sorted[0];
    let last = sorted[sorted.len() - 1];
    let wrapped_first = (first.0 + 360.0, first.1);
    let span = wrapped_first.0 - last.0;
    let crossing_lat = last.1 + (wrapped_first.1 - last.1) * ((180.0 - last.0) / span);

    // GeoJSON is planar, so express the spherical polar cell as one simple
    // cap ring bounded by both sides of the antimeridian. Splitting it into
    // adjacent MultiPolygon wedges makes the members share complete meridian
    // edges, which strict OGC/GEOS consumers correctly reject as invalid.
    let mut ring = sorted;
    if last.0 < 180.0 {
        ring.push((180.0, crossing_lat));
    }
    ring.push((180.0, pole_lat));
    ring.push((-180.0, pole_lat));
    if first.0 > -180.0 {
        ring.push((-180.0, crossing_lat));
    }
    ring.push(first);

    polygon_geometry(&ring)
}

fn enclosing_pole_lat(corners: &[(f64, f64)], center_lat: f64) -> Option<f64> {
    let longitude_winding = corners
        .iter()
        .zip(corners.iter().cycle().skip(1))
        .take(corners.len())
        .map(|(left, right)| ((right.0 - left.0 + 180.0).rem_euclid(360.0)) - 180.0)
        .sum::<f64>();
    (longitude_winding.abs() > 180.0).then_some(if center_lat >= 0.0 { 90.0 } else { -90.0 })
}

fn polar_cap_intersects_directed_bbox(
    corners: &[(f64, f64)],
    pole_lat: f64,
    bbox: Option<[f64; 4]>,
) -> bool {
    let Some([_, south, _, north]) = bbox else {
        return true;
    };
    let (cap_south, cap_north) = if pole_lat > 0.0 {
        (
            corners
                .iter()
                .map(|corner| corner.1)
                .fold(f64::INFINITY, f64::min),
            pole_lat,
        )
    } else {
        (
            pole_lat,
            corners
                .iter()
                .map(|corner| corner.1)
                .fold(f64::NEG_INFINITY, f64::max),
        )
    };
    cap_south <= north && cap_north >= south
}

pub fn gridfile_cell_polygons_geojson(
    mesh: &GridfileMeshPoints,
    kind: GridfileCellKind,
    bbox: Option<[f64; 4]>,
    max_cells: Option<usize>,
) -> io::Result<String> {
    Ok(gridfile_cell_polygons_geojson_with_report(mesh, kind, bbox, max_cells)?.0)
}

pub fn gridfile_cell_polygons_geojson_with_report(
    mesh: &GridfileMeshPoints,
    kind: GridfileCellKind,
    bbox: Option<[f64; 4]>,
    max_cells: Option<usize>,
) -> io::Result<(String, GridfileCellExportReport)> {
    validate_gridfile_cell_arrays(mesh, kind, bbox)?;
    let norm_lon = |lon: f64| ((lon + 180.0).rem_euclid(360.0)) - 180.0;
    let m_layout = gridfile_m_row_layout(mesh);
    let w_layout = gridfile_w_row_layout(mesh);
    let make_feature = |canonical_id: i32,
                        geometry: String,
                        clon: f64,
                        clat: f64,
                        refine_level: i32|
     -> String {
        format!(
                "    {{\"type\": \"Feature\", \"geometry\": {}, \
                 \"properties\": {{\"cell_id\": \"{}\", \"cell_index\": {}, \"grid_kind\": \"earthmesh_cell\", \
                 \"center_lon\": {}, \"center_lat\": {}, \"refine_level\": {}}}}}",
                geometry, canonical_id, canonical_id, clon, clat, refine_level
            )
    };

    let mut features: Vec<String> = Vec::new();
    let mut rejected_unsupported_cells = 0usize;
    if max_cells == Some(0) {
        return Ok((
            empty_feature_collection(),
            GridfileCellExportReport::default(),
        ));
    }
    match kind {
        GridfileCellKind::Tri => {
            let wn = mesh.w_lon.len();
            for (ci, tri) in mesh.m_to_w.chunks_exact(3).enumerate() {
                if !m_layout.is_physical_row(ci) {
                    continue;
                }
                let idx: Vec<usize> = tri
                    .iter()
                    .filter_map(|&v| w_layout.physical_row_for_canonical_id(v, wn))
                    .collect();
                if idx.len() != 3 || idx[0] == idx[1] || idx[1] == idx[2] || idx[0] == idx[2] {
                    continue;
                }
                let (clon, clat) = if ci < mesh.m_lon.len() {
                    (norm_lon(mesh.m_lon[ci]), mesh.m_lat[ci])
                } else {
                    let cx = idx.iter().map(|&i| norm_lon(mesh.w_lon[i])).sum::<f64>() / 3.0;
                    let cy = idx.iter().map(|&i| mesh.w_lat[i]).sum::<f64>() / 3.0;
                    (cx, cy)
                };
                let ring: Vec<(f64, f64)> = idx
                    .iter()
                    .map(|&i| (norm_lon(mesh.w_lon[i]), mesh.w_lat[i]))
                    .collect();
                let pole_lat = enclosing_pole_lat(&ring, clat);
                let intersects_bbox = pole_lat.map_or_else(
                    || ring_intersects_directed_bbox(&ring, bbox),
                    |pole_lat| polar_cap_intersects_directed_bbox(&ring, pole_lat, bbox),
                );
                if !intersects_bbox {
                    continue;
                }
                if cell_has_unsupported_edge_arc(&ring, 120.0) {
                    rejected_unsupported_cells += 1;
                    continue;
                }
                let geometry = if let Some(pole_lat) = pole_lat {
                    polar_cap_geometry(&ring, pole_lat)
                } else {
                    let Some(geometry) = seam_safe_polygon_geometry(&ring, clon) else {
                        continue;
                    };
                    geometry
                };
                let Some(canonical_id) = m_layout.canonical_id_for_physical_row(ci) else {
                    continue;
                };
                features.push(make_feature(
                    canonical_id,
                    geometry,
                    clon,
                    clat,
                    mesh.m_refine_level.get(ci).copied().unwrap_or(0),
                ));
                if max_cells.is_some_and(|mc| features.len() >= mc) {
                    break;
                }
            }
        }
        GridfileCellKind::Hex => {
            let mn = mesh.m_lon.len().min(mesh.m_lat.len());
            let wn = mesh.w_lon.len();
            let tris = mesh.m_to_w.len() / 3;
            let use_inverse = tris > 0;
            let mut incident: Vec<Vec<usize>> = Vec::new();
            if use_inverse {
                incident = vec![Vec::new(); wn];
                for mi in 0..tris {
                    if !m_layout.is_physical_row(mi) {
                        continue;
                    }
                    for k in 0..3 {
                        let w1 = mesh.m_to_w[mi * 3 + k];
                        if let Some(w_row) = w_layout.physical_row_for_canonical_id(w1, wn) {
                            incident[w_row].push(mi);
                        }
                    }
                }
            }
            let width = mesh.w_to_m_width;
            if use_inverse || width > 0 {
                for wi in 0..wn {
                    if !w_layout.is_physical_row(wi) {
                        continue;
                    }
                    let clon = norm_lon(mesh.w_lon[wi]);
                    let clat = mesh.w_lat[wi];
                    let mut corners: Vec<(f64, f64)> = Vec::new();
                    if use_inverse {
                        for &mi in &incident[wi] {
                            if mi >= mn || !m_layout.is_physical_row(mi) {
                                continue;
                            }
                            let (x, y) = (norm_lon(mesh.m_lon[mi]), mesh.m_lat[mi]);
                            corners.push((x, y));
                        }
                    } else {
                        let nv =
                            (mesh.n_w.get(wi).copied().unwrap_or(0).max(0) as usize).min(width);
                        for k in 0..nv {
                            let mid = mesh.w_to_m[wi * width + k];
                            if let Some(m_row) = m_layout.physical_row_for_canonical_id(mid, mn) {
                                let (x, y) = (norm_lon(mesh.m_lon[m_row]), mesh.m_lat[m_row]);
                                corners.push((x, y));
                            }
                        }
                    }
                    if corners.len() < 3 {
                        continue;
                    }
                    let corners = order_around_spherical_center(corners, clon, clat);
                    if corners.len() < 3 {
                        continue;
                    }
                    let pole_lat = enclosing_pole_lat(&corners, clat);
                    let intersects_bbox = pole_lat.map_or_else(
                        || ring_intersects_directed_bbox(&corners, bbox),
                        |pole_lat| polar_cap_intersects_directed_bbox(&corners, pole_lat, bbox),
                    );
                    if !intersects_bbox {
                        continue;
                    }
                    if cell_has_unsupported_edge_arc(&corners, 120.0) {
                        rejected_unsupported_cells += 1;
                        continue;
                    }
                    let geometry = if let Some(pole_lat) = pole_lat {
                        polar_cap_geometry(&corners, pole_lat)
                    } else {
                        let Some(geometry) = seam_safe_polygon_geometry(&corners, clon) else {
                            continue;
                        };
                        geometry
                    };
                    let Some(canonical_id) = w_layout.canonical_id_for_physical_row(wi) else {
                        continue;
                    };
                    features.push(make_feature(
                        canonical_id,
                        geometry,
                        clon,
                        clat,
                        mesh.w_refine_level.get(wi).copied().unwrap_or(0),
                    ));
                    if max_cells.is_some_and(|mc| features.len() >= mc) {
                        break;
                    }
                }
            }
        }
    }

    let emitted_cells = features.len();
    Ok((
        format!(
            "{{\n  \"type\": \"FeatureCollection\",\n  \"features\": [\n{}\n  ]\n}}\n",
            features.join(",\n")
        ),
        GridfileCellExportReport {
            emitted_cells,
            rejected_unsupported_cells,
        },
    ))
}

fn empty_feature_collection() -> String {
    "{\n  \"type\": \"FeatureCollection\",\n  \"features\": [\n\n  ]\n}\n".to_string()
}

fn validate_gridfile_cell_arrays(
    mesh: &GridfileMeshPoints,
    kind: GridfileCellKind,
    bbox: Option<[f64; 4]>,
) -> io::Result<()> {
    let invalid = |message: String| io::Error::new(io::ErrorKind::InvalidData, message);
    if mesh.m_lon.len() != mesh.m_lat.len() || mesh.w_lon.len() != mesh.w_lat.len() {
        return Err(invalid(format!(
            "gridfile M/W longitude and latitude lengths must match; got M {}/{}, W {}/{}",
            mesh.m_lon.len(),
            mesh.m_lat.len(),
            mesh.w_lon.len(),
            mesh.w_lat.len()
        )));
    }
    if mesh
        .m_lon
        .iter()
        .chain(&mesh.m_lat)
        .chain(&mesh.w_lon)
        .chain(&mesh.w_lat)
        .any(|value| !value.is_finite())
        || mesh
            .m_lat
            .iter()
            .chain(&mesh.w_lat)
            .any(|lat| lat.abs() > 90.0)
    {
        return Err(invalid(
            "gridfile cell coordinates must be finite with latitude in [-90, 90]".to_string(),
        ));
    }
    if let Some([west, south, east, north]) = bbox {
        if [west, south, east, north]
            .iter()
            .any(|value| !value.is_finite())
            || south > north
            || !(-180.0..=180.0).contains(&west)
            || !(-180.0..=180.0).contains(&east)
            || south < -90.0
            || north > 90.0
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "gridfile bbox must contain finite geographic W S E N bounds with south <= north",
            ));
        }
    }
    if !mesh.m_to_w.len().is_multiple_of(3) {
        return Err(invalid(format!(
            "gridfile itab_m%iw length {} is not divisible by 3",
            mesh.m_to_w.len()
        )));
    }
    if (!mesh.m_to_w.is_empty() || kind == GridfileCellKind::Tri)
        && mesh.m_to_w.len() / 3 != mesh.m_lon.len()
    {
        return Err(invalid(format!(
            "gridfile itab_m%iw row count {} must equal M coordinate count {}",
            mesh.m_to_w.len() / 3,
            mesh.m_lon.len()
        )));
    }

    let m_layout = gridfile_m_row_layout(mesh);
    let w_layout = gridfile_w_row_layout(mesh);
    for (row, triangle) in mesh.m_to_w.chunks_exact(3).enumerate() {
        if !m_layout.is_physical_row(row) {
            continue;
        }
        for &vertex_id in triangle {
            if w_layout
                .physical_row_for_canonical_id(vertex_id, mesh.w_lon.len())
                .is_none()
                && !(w_layout.has_two_placeholder_rows && vertex_id == 1)
            {
                return Err(invalid(format!(
                    "gridfile M row {row} references invalid W id {vertex_id}"
                )));
            }
        }
    }

    if mesh.w_to_m_width == 0 {
        if !mesh.w_to_m.is_empty() || !mesh.n_w.is_empty() {
            return Err(invalid(
                "gridfile W connectivity values require a positive matrix width".to_string(),
            ));
        }
    } else {
        let expected = mesh
            .w_lon
            .len()
            .checked_mul(mesh.w_to_m_width)
            .ok_or_else(|| invalid("gridfile W connectivity shape overflows usize".to_string()))?;
        if mesh.w_to_m.len() != expected || mesh.n_w.len() != mesh.w_lon.len() {
            return Err(invalid(format!(
                "gridfile W connectivity shape must be {}x{} with one count per row",
                mesh.w_lon.len(),
                mesh.w_to_m_width
            )));
        }
        for wi in 0..mesh.w_lon.len() {
            if !w_layout.is_physical_row(wi) {
                continue;
            }
            let count = usize::try_from(mesh.n_w[wi])
                .map_err(|_| invalid(format!("gridfile W row {wi} has negative neighbor count")))?;
            if count > mesh.w_to_m_width {
                return Err(invalid(format!(
                    "gridfile W row {wi} neighbor count {count} exceeds width {}",
                    mesh.w_to_m_width
                )));
            }
            let start = wi * mesh.w_to_m_width;
            for &center_id in &mesh.w_to_m[start..start + count] {
                if m_layout
                    .physical_row_for_canonical_id(center_id, mesh.m_lon.len())
                    .is_none()
                    && !(m_layout.has_two_placeholder_rows && center_id == 1)
                {
                    return Err(invalid(format!(
                        "gridfile W row {wi} references invalid M id {center_id}"
                    )));
                }
            }
        }
    }
    Ok(())
}

pub fn write_gridfile_cell_polygons_geojson(
    gridfile: impl AsRef<Path>,
    output: impl AsRef<Path>,
    kind: GridfileCellKind,
    bbox: Option<[f64; 4]>,
    max_cells: Option<usize>,
) -> io::Result<usize> {
    let mesh = read_gridfile_mesh_points(gridfile)?;
    let json = gridfile_cell_polygons_geojson(&mesh, kind, bbox, max_cells)?;
    crate::ensure_parent_dir(output.as_ref())?;
    fs::write(output.as_ref(), json.as_bytes())?;
    Ok(json.matches("\"type\": \"Feature\"").count())
}

pub fn write_gridfile_cell_polygons_geojson_with_report(
    gridfile: impl AsRef<Path>,
    output: impl AsRef<Path>,
    kind: GridfileCellKind,
    bbox: Option<[f64; 4]>,
    max_cells: Option<usize>,
) -> io::Result<GridfileCellExportReport> {
    let mesh = read_gridfile_mesh_points(gridfile)?;
    let (json, report) = gridfile_cell_polygons_geojson_with_report(&mesh, kind, bbox, max_cells)?;
    crate::ensure_parent_dir(output.as_ref())?;
    fs::write(output.as_ref(), json.as_bytes())?;
    Ok(report)
}
