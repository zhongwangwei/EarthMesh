use std::{fs, io, path::Path};

use crate::{netcdf_to_io_error, required_values_f64, required_values_i32, required_values_i32_2d};

fn round12(value: f64) -> f64 {
    (value * 1e12).round() / 1e12
}

fn mpas_deg_lon(rad: f64) -> f64 {
    let d = rad.to_degrees();
    let mut n = (d + 180.0).rem_euclid(360.0) - 180.0;
    if n == -180.0 && d > 0.0 {
        n = 180.0;
    }
    round12(n)
}

fn mpas_deg_lat(rad: f64) -> f64 {
    round12(rad.to_degrees())
}

#[allow(clippy::too_many_arguments)]
pub fn mpas_cell_polygons_geojson(
    lon_cell: &[f64],
    lat_cell: &[f64],
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    n_edges_on_cell: &[i32],
    vertices_on_cell: &[i32],
    cell_ids: Option<&[i32]>,
    area_cell: Option<&[f64]>,
    bbox: Option<[f64; 4]>,
    max_cells: Option<usize>,
) -> String {
    let n_cells = lon_cell.len();
    let max_edges = vertices_on_cell.len().checked_div(n_cells).unwrap_or(0);
    let lon_v: Vec<f64> = lon_vertex.iter().map(|&r| mpas_deg_lon(r)).collect();
    let lat_v: Vec<f64> = lat_vertex.iter().map(|&r| mpas_deg_lat(r)).collect();
    let mut features: Vec<String> = Vec::new();
    for ci in 0..n_cells {
        let clon = mpas_deg_lon(lon_cell[ci]);
        let clat = mpas_deg_lat(lat_cell[ci]);
        if let Some(b) = bbox {
            if !(b[0] <= clon && clon <= b[2] && b[1] <= clat && clat <= b[3]) {
                continue;
            }
        }
        let ne = (n_edges_on_cell.get(ci).copied().unwrap_or(0).max(0) as usize).min(max_edges);
        let mut ring: Vec<(f64, f64)> = Vec::with_capacity(ne + 1);
        for k in 0..ne {
            let vid = vertices_on_cell[ci * max_edges + k];
            if vid > 0 && (vid as usize) <= lon_v.len() {
                ring.push((lon_v[vid as usize - 1], lat_v[vid as usize - 1]));
            }
        }
        if ring.len() < 3 {
            continue;
        }
        let first = ring[0];
        ring.push(first);
        let coords = ring
            .iter()
            .map(|(x, y)| format!("[{x}, {y}]"))
            .collect::<Vec<_>>()
            .join(", ");
        let cell_id = cell_ids
            .and_then(|ids| ids.get(ci))
            .map(|id| id.to_string())
            .unwrap_or_else(|| (ci + 1).to_string());
        let mut props = format!(
            "\"cell_id\": \"{}\", \"cell_index\": {}, \"grid_kind\": \"earthmesh_cell\", \
             \"center_lon\": {}, \"center_lat\": {}",
            cell_id, ci, clon, clat
        );
        if let Some(area) = area_cell {
            if let Some(a) = area.get(ci) {
                props.push_str(&format!(
                    ", \"source_areaCell\": {}, \"source_areaCell_units\": \"file_units\"",
                    a
                ));
            }
        }
        features.push(format!(
            "    {{\"type\": \"Feature\", \"geometry\": {{\"type\": \"Polygon\", \"coordinates\": [[{}]]}}, \"properties\": {{{}}}}}",
            coords, props
        ));
        if let Some(mc) = max_cells {
            if features.len() >= mc {
                break;
            }
        }
    }
    format!(
        "{{\n  \"type\": \"FeatureCollection\",\n  \"features\": [\n{}\n  ]\n}}\n",
        features.join(",\n")
    )
}

pub fn write_mpas_cell_polygons_geojson(
    mesh_netcdf: impl AsRef<Path>,
    output_geojson: impl AsRef<Path>,
    bbox: Option<[f64; 4]>,
    max_cells: Option<usize>,
) -> io::Result<usize> {
    let file = crate::open_netcdf(mesh_netcdf.as_ref()).map_err(netcdf_to_io_error)?;
    let lon_cell = required_values_f64(&file, "lonCell")?;
    let lat_cell = required_values_f64(&file, "latCell")?;
    let lon_vertex = required_values_f64(&file, "lonVertex")?;
    let lat_vertex = required_values_f64(&file, "latVertex")?;
    let n_edges_on_cell = required_values_i32(&file, "nEdgesOnCell")?;
    let vertices_on_cell = required_values_i32_2d(&file, "verticesOnCell")?;
    let cell_ids = required_values_i32(&file, "indexToCellID").ok();
    let area_cell = required_values_f64(&file, "areaCell").ok();
    let json = mpas_cell_polygons_geojson(
        &lon_cell,
        &lat_cell,
        &lon_vertex,
        &lat_vertex,
        &n_edges_on_cell,
        &vertices_on_cell,
        cell_ids.as_deref(),
        area_cell.as_deref(),
        bbox,
        max_cells,
    );
    let feature_count = json.matches("\"type\": \"Feature\"").count();
    if let Some(parent) = output_geojson.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output_geojson, json)?;
    Ok(feature_count)
}
