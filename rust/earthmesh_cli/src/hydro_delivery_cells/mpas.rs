use std::{collections::HashSet, fs, io, path::Path};

use crate::netcdf_io::required_values_i32_matrix_named;
use crate::{netcdf_to_io_error, required_dimension_len, required_values_f64, required_values_i32};

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
) -> io::Result<String> {
    validate_mpas_cell_arrays(
        lon_cell,
        lat_cell,
        lon_vertex,
        lat_vertex,
        n_edges_on_cell,
        vertices_on_cell,
        cell_ids,
        area_cell,
        bbox,
    )?;
    let n_cells = lon_cell.len();
    let max_edges = vertices_on_cell.len().checked_div(n_cells).unwrap_or(0);
    if max_cells == Some(0) {
        return Ok(
            "{\n  \"type\": \"FeatureCollection\",\n  \"features\": [\n\n  ]\n}\n".to_string(),
        );
    }
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
        let ne = n_edges_on_cell[ci] as usize;
        let mut ring: Vec<(f64, f64)> = Vec::with_capacity(ne + 1);
        for k in 0..ne {
            let vid = vertices_on_cell[ci * max_edges + k];
            let vertex = vid as usize - 1;
            ring.push((lon_v[vertex], lat_v[vertex]));
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
            cell_id,
            ci + 1,
            clon,
            clat
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
    Ok(format!(
        "{{\n  \"type\": \"FeatureCollection\",\n  \"features\": [\n{}\n  ]\n}}\n",
        features.join(",\n")
    ))
}

#[allow(clippy::too_many_arguments)]
fn validate_mpas_cell_arrays(
    lon_cell: &[f64],
    lat_cell: &[f64],
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    n_edges_on_cell: &[i32],
    vertices_on_cell: &[i32],
    cell_ids: Option<&[i32]>,
    area_cell: Option<&[f64]>,
    bbox: Option<[f64; 4]>,
) -> io::Result<()> {
    let invalid = |message: String| io::Error::new(io::ErrorKind::InvalidData, message);
    let n_cells = lon_cell.len();
    if lat_cell.len() != n_cells || n_edges_on_cell.len() != n_cells {
        return Err(invalid(format!(
            "MPAS lonCell/latCell/nEdgesOnCell lengths must match; got {n_cells}/{}/{}",
            lat_cell.len(),
            n_edges_on_cell.len()
        )));
    }
    if lon_vertex.len() != lat_vertex.len() {
        return Err(invalid(format!(
            "MPAS lonVertex/latVertex lengths must match; got {}/{}",
            lon_vertex.len(),
            lat_vertex.len()
        )));
    }
    if lon_cell
        .iter()
        .chain(lat_cell)
        .chain(lon_vertex)
        .chain(lat_vertex)
        .any(|value| !value.is_finite())
    {
        return Err(invalid(
            "MPAS cell and vertex coordinates must be finite".to_string(),
        ));
    }
    if lon_cell
        .iter()
        .chain(lon_vertex)
        .any(|&lon| !mpas_deg_lon(lon).is_finite())
        || lat_cell.iter().chain(lat_vertex).any(|&lat| {
            let degrees = mpas_deg_lat(lat);
            !degrees.is_finite() || degrees.abs() > 90.0
        })
    {
        return Err(invalid(
            "MPAS radian coordinates must convert to finite geographic longitude/latitude"
                .to_string(),
        ));
    }
    if let Some(ids) = cell_ids {
        if ids.len() != n_cells {
            return Err(invalid(format!(
                "MPAS indexToCellID length {} must equal cell count {n_cells}",
                ids.len()
            )));
        }
        let unique = ids.iter().copied().collect::<HashSet<_>>();
        if unique.len() != ids.len() {
            return Err(invalid(
                "MPAS indexToCellID values must be unique".to_string(),
            ));
        }
    }
    if let Some(areas) = area_cell {
        if areas.len() != n_cells || areas.iter().any(|area| !area.is_finite()) {
            return Err(invalid(format!(
                "MPAS areaCell must contain {n_cells} finite values"
            )));
        }
    }
    if let Some([west, south, east, north]) = bbox {
        if [west, south, east, north]
            .iter()
            .any(|value| !value.is_finite())
            || west > east
            || south > north
            || west < -180.0
            || east > 180.0
            || south < -90.0
            || north > 90.0
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "MPAS bbox must contain finite ordered geographic W S E N bounds",
            ));
        }
    }
    if n_cells == 0 {
        if !vertices_on_cell.is_empty() {
            return Err(invalid(
                "MPAS verticesOnCell must be empty when there are no cells".to_string(),
            ));
        }
        return Ok(());
    }
    if !vertices_on_cell.len().is_multiple_of(n_cells) {
        return Err(invalid(format!(
            "MPAS verticesOnCell length {} is not divisible by cell count {n_cells}",
            vertices_on_cell.len()
        )));
    }
    let max_edges = vertices_on_cell.len() / n_cells;
    if max_edges < 3 {
        return Err(invalid(
            "MPAS verticesOnCell matrix width must be at least 3".to_string(),
        ));
    }
    for (cell, &edge_count) in n_edges_on_cell.iter().enumerate() {
        let edge_count = usize::try_from(edge_count)
            .map_err(|_| invalid(format!("MPAS cell {} has negative nEdgesOnCell", cell + 1)))?;
        if !(3..=max_edges).contains(&edge_count) {
            return Err(invalid(format!(
                "MPAS cell {} nEdgesOnCell {edge_count} is outside 3..={max_edges}",
                cell + 1
            )));
        }
        let start = cell * max_edges;
        let row = &vertices_on_cell[start..start + edge_count];
        let mut seen = Vec::with_capacity(edge_count);
        for &vertex_id in row {
            let vertex = usize::try_from(vertex_id)
                .ok()
                .and_then(|value| value.checked_sub(1))
                .filter(|&value| value < lon_vertex.len())
                .ok_or_else(|| {
                    invalid(format!(
                        "MPAS cell {} references invalid one-based vertex id {vertex_id}",
                        cell + 1
                    ))
                })?;
            if seen.contains(&vertex) {
                return Err(invalid(format!(
                    "MPAS cell {} repeats vertex id {vertex_id}",
                    cell + 1
                )));
            }
            seen.push(vertex);
        }
    }
    Ok(())
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
    let n_cells = required_dimension_len(&file, "nCells")?;
    let max_edges = required_dimension_len(&file, "maxEdges")?;
    let vertices_on_cell = required_values_i32_matrix_named(
        &file,
        "verticesOnCell",
        "nCells",
        "maxEdges",
        n_cells,
        max_edges,
    )?;
    let cell_ids = file
        .variable("indexToCellID")
        .map(|_| required_values_i32(&file, "indexToCellID"))
        .transpose()?;
    let area_cell = file
        .variable("areaCell")
        .map(|_| required_values_f64(&file, "areaCell"))
        .transpose()?;
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
    )?;
    let feature_count = json.matches("\"type\": \"Feature\"").count();
    crate::ensure_parent_dir(output_geojson.as_ref())?;
    fs::write(output_geojson, json)?;
    Ok(feature_count)
}
