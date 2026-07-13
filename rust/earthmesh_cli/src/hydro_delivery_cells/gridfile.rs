use std::{fs, io, path::Path};

use crate::{
    mesh_canonical_id_for_row, mesh_row_for_canonical_id, read_gridfile_mesh_points,
    GridfileCellKind, GridfileMeshPoints,
};

use super::geometry::{
    cell_exceeds_supported_arc, order_around_spherical_center, ring_intersects_directed_bbox,
    unwrap_ring_lon,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GridfileCellExportReport {
    pub emitted_cells: usize,
    pub rejected_unsupported_cells: usize,
}

pub fn gridfile_cell_polygons_geojson(
    mesh: &GridfileMeshPoints,
    kind: GridfileCellKind,
    bbox: Option<[f64; 4]>,
    max_cells: Option<usize>,
) -> String {
    gridfile_cell_polygons_geojson_with_report(mesh, kind, bbox, max_cells).0
}

pub fn gridfile_cell_polygons_geojson_with_report(
    mesh: &GridfileMeshPoints,
    kind: GridfileCellKind,
    bbox: Option<[f64; 4]>,
    max_cells: Option<usize>,
) -> (String, GridfileCellExportReport) {
    let norm_lon = |lon: f64| ((lon + 180.0).rem_euclid(360.0)) - 180.0;
    let m_has_two_placeholders = gridfile_lonlat_has_two_placeholders(&mesh.m_lon, &mesh.m_lat);
    let w_has_two_placeholders = gridfile_lonlat_has_two_placeholders(&mesh.w_lon, &mesh.w_lat);
    let make_feature = |_idx: usize,
                        canonical_id: i32,
                        ring: &[(f64, f64)],
                        clon: f64,
                        clat: f64|
     -> String {
        let coords = ring
            .iter()
            .map(|(x, y)| format!("[{x}, {y}]"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "    {{\"type\": \"Feature\", \"geometry\": {{\"type\": \"Polygon\", \"coordinates\": [[{}]]}}, \
             \"properties\": {{\"cell_id\": \"{}\", \"cell_index\": {}, \"grid_kind\": \"earthmesh_cell\", \
             \"center_lon\": {}, \"center_lat\": {}}}}}",
            coords,
            canonical_id,
            canonical_id,
            clon,
            clat
        )
    };

    let mut features: Vec<String> = Vec::new();
    let mut rejected_unsupported_cells = 0usize;
    match kind {
        GridfileCellKind::Tri => {
            let wn = mesh.w_lon.len();
            for (ci, tri) in mesh.m_to_w.chunks_exact(3).enumerate() {
                if m_has_two_placeholders && ci < 2 {
                    continue;
                }
                let idx: Vec<usize> = tri
                    .iter()
                    .filter_map(|&v| mesh_row_for_canonical_id(v, wn, w_has_two_placeholders))
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
                let mut ring: Vec<(f64, f64)> = idx
                    .iter()
                    .map(|&i| (norm_lon(mesh.w_lon[i]), mesh.w_lat[i]))
                    .collect();
                if !ring_intersects_directed_bbox(&ring, bbox) {
                    continue;
                }
                if cell_exceeds_supported_arc(&ring, clon, clat, 120.0) {
                    rejected_unsupported_cells += 1;
                    continue;
                }
                unwrap_ring_lon(&mut ring, clon);
                ring.push(ring[0]);
                let Some(canonical_id) = mesh_canonical_id_for_row(ci, m_has_two_placeholders)
                else {
                    continue;
                };
                features.push(make_feature(ci, canonical_id, &ring, clon, clat));
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
                    if m_has_two_placeholders && mi < 2 {
                        continue;
                    }
                    for k in 0..3 {
                        let w1 = mesh.m_to_w[mi * 3 + k];
                        if let Some(w_row) =
                            mesh_row_for_canonical_id(w1, wn, w_has_two_placeholders)
                        {
                            incident[w_row].push(mi);
                        }
                    }
                }
            }
            let width = mesh.w_to_m_width;
            if use_inverse || width > 0 {
                for wi in 0..wn {
                    if w_has_two_placeholders && wi < 2 {
                        continue;
                    }
                    let clon = norm_lon(mesh.w_lon[wi]);
                    let clat = mesh.w_lat[wi];
                    let mut corners: Vec<(f64, f64)> = Vec::new();
                    if use_inverse {
                        for &mi in &incident[wi] {
                            if mi >= mn || (m_has_two_placeholders && mi < 2) {
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
                            if let Some(m_row) =
                                mesh_row_for_canonical_id(mid, mn, m_has_two_placeholders)
                            {
                                let (x, y) = (norm_lon(mesh.m_lon[m_row]), mesh.m_lat[m_row]);
                                corners.push((x, y));
                            }
                        }
                    }
                    if corners.len() < 3 {
                        continue;
                    }
                    if !ring_intersects_directed_bbox(&corners, bbox) {
                        continue;
                    }
                    if cell_exceeds_supported_arc(&corners, clon, clat, 120.0) {
                        rejected_unsupported_cells += 1;
                        continue;
                    }
                    let mut corners = order_around_spherical_center(corners, clon, clat);
                    unwrap_ring_lon(&mut corners, clon);
                    let mut ring = corners;
                    if ring.len() < 3 {
                        continue;
                    }
                    ring.push(ring[0]);
                    let Some(canonical_id) = mesh_canonical_id_for_row(wi, w_has_two_placeholders)
                    else {
                        continue;
                    };
                    features.push(make_feature(wi, canonical_id, &ring, clon, clat));
                    if max_cells.is_some_and(|mc| features.len() >= mc) {
                        break;
                    }
                }
            }
        }
    }

    let emitted_cells = features.len();
    (
        format!(
            "{{\n  \"type\": \"FeatureCollection\",\n  \"features\": [\n{}\n  ]\n}}\n",
            features.join(",\n")
        ),
        GridfileCellExportReport {
            emitted_cells,
            rejected_unsupported_cells,
        },
    )
}

pub(crate) fn gridfile_lonlat_has_two_placeholders(lon: &[f64], lat: &[f64]) -> bool {
    lon.len() > 2
        && lat.len() > 2
        && lon[0] == 0.0
        && lat[0] == 0.0
        && lon[1] == 0.0
        && lat[1] == 0.0
}

pub fn write_gridfile_cell_polygons_geojson(
    gridfile: impl AsRef<Path>,
    output: impl AsRef<Path>,
    kind: GridfileCellKind,
    bbox: Option<[f64; 4]>,
    max_cells: Option<usize>,
) -> io::Result<usize> {
    let mesh = read_gridfile_mesh_points(gridfile)?;
    let json = gridfile_cell_polygons_geojson(&mesh, kind, bbox, max_cells);
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
    let (json, report) = gridfile_cell_polygons_geojson_with_report(&mesh, kind, bbox, max_cells);
    crate::ensure_parent_dir(output.as_ref())?;
    fs::write(output.as_ref(), json.as_bytes())?;
    Ok(report)
}
