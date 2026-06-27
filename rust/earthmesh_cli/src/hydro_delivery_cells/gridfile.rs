use std::{fs, io, path::Path};

use crate::{
    mesh_row_for_fortran_id, read_gridfile_mesh_points, GridfileCellKind, GridfileMeshPoints,
};

use super::geometry::{convex_hull_ccw, preview_cell_too_large, unwrap_ring_lon};

pub fn gridfile_cell_polygons_geojson(
    mesh: &GridfileMeshPoints,
    kind: GridfileCellKind,
    bbox: Option<[f64; 4]>,
    max_cells: Option<usize>,
) -> String {
    let norm_lon = |lon: f64| ((lon + 180.0).rem_euclid(360.0)) - 180.0;
    let m_has_two_placeholders = gridfile_lonlat_has_two_placeholders(&mesh.m_lon, &mesh.m_lat);
    let w_has_two_placeholders = gridfile_lonlat_has_two_placeholders(&mesh.w_lon, &mesh.w_lat);
    let in_bbox = |clon: f64, clat: f64| match bbox {
        Some(b) => b[0] <= clon && clon <= b[2] && b[1] <= clat && clat <= b[3],
        None => true,
    };
    let make_feature = |idx: usize, ring: &[(f64, f64)], clon: f64, clat: f64| -> String {
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
            idx + 1,
            idx,
            clon,
            clat
        )
    };

    let mut features: Vec<String> = Vec::new();
    match kind {
        GridfileCellKind::Tri => {
            let wn = mesh.w_lon.len();
            for (ci, tri) in mesh.m_to_w.chunks_exact(3).enumerate() {
                let idx: Vec<usize> = tri
                    .iter()
                    .filter_map(|&v| mesh_row_for_fortran_id(v, wn, w_has_two_placeholders))
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
                if !in_bbox(clon, clat) {
                    continue;
                }
                let mut ring: Vec<(f64, f64)> = idx
                    .iter()
                    .map(|&i| (norm_lon(mesh.w_lon[i]), mesh.w_lat[i]))
                    .collect();
                if (clon == 0.0 && clat == 0.0) || ring.iter().any(|&(x, y)| x == 0.0 && y == 0.0) {
                    continue;
                }
                if preview_cell_too_large(&ring, clon, clat, 30.0) {
                    continue;
                }
                if ring.iter().any(|&(_, lat)| lat.abs() > 80.0) {
                    continue;
                }
                unwrap_ring_lon(&mut ring, clon);
                let (lo, hi) = ring.iter().fold((f64::MAX, f64::MIN), |(lo, hi), &(x, _)| {
                    (lo.min(x), hi.max(x))
                });
                if hi - lo > 45.0 {
                    continue;
                }
                ring.push(ring[0]);
                features.push(make_feature(ci, &ring, clon, clat));
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
                    for k in 0..3 {
                        let w1 = mesh.m_to_w[mi * 3 + k];
                        if let Some(w_row) = mesh_row_for_fortran_id(w1, wn, w_has_two_placeholders)
                        {
                            incident[w_row].push(mi);
                        }
                    }
                }
            }
            let width = mesh.w_to_m_width;
            if use_inverse || width > 0 {
                for wi in 0..wn {
                    let clon = norm_lon(mesh.w_lon[wi]);
                    let clat = mesh.w_lat[wi];
                    if !in_bbox(clon, clat) || (clon == 0.0 && clat == 0.0) {
                        continue;
                    }
                    let mut corners: Vec<(f64, f64)> = Vec::new();
                    if use_inverse {
                        for &mi in &incident[wi] {
                            if mi >= mn {
                                continue;
                            }
                            let (x, y) = (norm_lon(mesh.m_lon[mi]), mesh.m_lat[mi]);
                            if x == 0.0 && y == 0.0 {
                                continue;
                            }
                            corners.push((x, y));
                        }
                    } else {
                        let nv =
                            (mesh.n_w.get(wi).copied().unwrap_or(0).max(0) as usize).min(width);
                        for k in 0..nv {
                            let mid = mesh.w_to_m[wi * width + k];
                            if let Some(m_row) =
                                mesh_row_for_fortran_id(mid, mn, m_has_two_placeholders)
                            {
                                let (x, y) = (norm_lon(mesh.m_lon[m_row]), mesh.m_lat[m_row]);
                                if x == 0.0 && y == 0.0 {
                                    continue;
                                }
                                corners.push((x, y));
                            }
                        }
                    }
                    if corners.len() < 3 {
                        continue;
                    }
                    if preview_cell_too_large(&corners, clon, clat, 30.0) {
                        continue;
                    }
                    if corners.iter().any(|&(_, lat)| lat.abs() > 80.0) {
                        continue;
                    }
                    unwrap_ring_lon(&mut corners, clon);
                    let (lo, hi) = corners
                        .iter()
                        .fold((f64::MAX, f64::MIN), |(lo, hi), &(x, _)| {
                            (lo.min(x), hi.max(x))
                        });
                    if hi - lo > 45.0 {
                        continue;
                    }
                    let mut ring = convex_hull_ccw(corners);
                    if ring.len() < 3 {
                        continue;
                    }
                    ring.push(ring[0]);
                    features.push(make_feature(wi, &ring, clon, clat));
                    if max_cells.is_some_and(|mc| features.len() >= mc) {
                        break;
                    }
                }
            }
        }
    }

    format!(
        "{{\n  \"type\": \"FeatureCollection\",\n  \"features\": [\n{}\n  ]\n}}\n",
        features.join(",\n")
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
    if let Some(parent) = output.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output.as_ref(), json.as_bytes())?;
    Ok(json.matches("\"type\": \"Feature\"").count())
}
