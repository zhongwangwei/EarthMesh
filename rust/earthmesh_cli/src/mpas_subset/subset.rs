use std::io;

use crate::*;

/// Subset a global [`MpasMesh`] to the cells selected by `keep_cell`, producing
/// a topologically-consistent regional (limited-area) MPAS mesh.
pub fn subset_mpas_mesh(global: &MpasMesh, keep_cell: &[bool]) -> io::Result<MpasMesh> {
    let nc = global.lat_cell.len();
    let nv = global.lat_vertex.len();
    let ne = global.lat_edge.len();
    if keep_cell.len() != nc {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "keep_cell length {} must equal nCells rows {nc}",
                keep_cell.len()
            ),
        ));
    }

    let mut new_cell = vec![0usize; nc];
    let mut kept_cells = Vec::new();
    for c in 1..nc {
        if keep_cell[c] {
            new_cell[c] = kept_cells.len() + 1;
            kept_cells.push(c);
        }
    }
    let mut new_vertex = vec![0usize; nv];
    let mut kept_vertices = Vec::new();
    for v in 1..nv {
        if global.cells_on_vertex[v]
            .iter()
            .any(|&c| c > 0 && keep_cell[c as usize])
        {
            new_vertex[v] = kept_vertices.len() + 1;
            kept_vertices.push(v);
        }
    }
    let mut new_edge = vec![0usize; ne];
    let mut kept_edges = Vec::new();
    for e in 1..ne {
        if global.cells_on_edge[e]
            .iter()
            .any(|&c| c > 0 && keep_cell[c as usize])
        {
            new_edge[e] = kept_edges.len() + 1;
            kept_edges.push(e);
        }
    }

    let remap = |v: i32, map: &[usize]| -> i32 {
        if v <= 0 {
            0
        } else {
            map[v as usize] as i32
        }
    };
    let gather_f64 = |src: &[f64], kept: &[usize]| {
        let mut out = Vec::with_capacity(kept.len() + 1);
        out.push(src[0]);
        out.extend(kept.iter().map(|&i| src[i]));
        out
    };

    let cell_f64 = |src: &[f64]| gather_f64(src, &kept_cells);
    let mut cells_on_cell = vec![global.cells_on_cell[0].clone()];
    let mut vertices_on_cell = vec![global.vertices_on_cell[0].clone()];
    let mut edges_on_cell = vec![global.edges_on_cell[0].clone()];
    let mut n_edges_on_cell = vec![global.n_edges_on_cell[0]];
    for &c in &kept_cells {
        cells_on_cell.push(
            global.cells_on_cell[c]
                .iter()
                .map(|&x| remap(x, &new_cell))
                .collect(),
        );
        vertices_on_cell.push(
            global.vertices_on_cell[c]
                .iter()
                .map(|&x| remap(x, &new_vertex))
                .collect(),
        );
        edges_on_cell.push(
            global.edges_on_cell[c]
                .iter()
                .map(|&x| remap(x, &new_edge))
                .collect(),
        );
        n_edges_on_cell.push(global.n_edges_on_cell[c]);
    }

    let mut cells_on_vertex = vec![global.cells_on_vertex[0].clone()];
    let mut edges_on_vertex = vec![global.edges_on_vertex[0].clone()];
    let mut kite_areas_on_vertex = vec![global.kite_areas_on_vertex[0].clone()];
    for &v in &kept_vertices {
        cells_on_vertex.push(
            global.cells_on_vertex[v]
                .iter()
                .map(|&x| remap(x, &new_cell))
                .collect(),
        );
        edges_on_vertex.push(
            global.edges_on_vertex[v]
                .iter()
                .map(|&x| remap(x, &new_edge))
                .collect(),
        );
        kite_areas_on_vertex.push(global.kite_areas_on_vertex[v].clone());
    }

    let mut cells_on_edge = vec![global.cells_on_edge[0]];
    let mut vertices_on_edge = vec![global.vertices_on_edge[0]];
    let mut edges_on_edge = vec![global.edges_on_edge[0].clone()];
    let mut weights_on_edge = vec![global.weights_on_edge[0].clone()];
    let mut n_edges_on_edge = vec![global.n_edges_on_edge[0]];
    for &e in &kept_edges {
        let coe = global.cells_on_edge[e];
        cells_on_edge.push([remap(coe[0], &new_cell), remap(coe[1], &new_cell)]);
        let voe = global.vertices_on_edge[e];
        vertices_on_edge.push([remap(voe[0], &new_vertex), remap(voe[1], &new_vertex)]);
        let eoe: Vec<i32> = global.edges_on_edge[e]
            .iter()
            .map(|&x| remap(x, &new_edge))
            .collect();
        let woe: Vec<f64> = global.weights_on_edge[e]
            .iter()
            .zip(eoe.iter())
            .map(|(&w, &edge)| if edge == 0 { 0.0 } else { w })
            .collect();
        edges_on_edge.push(eoe);
        weights_on_edge.push(woe);
        n_edges_on_edge.push(global.n_edges_on_edge[e]);
    }

    Ok(MpasMesh {
        lat_cell: cell_f64(&global.lat_cell),
        lon_cell: cell_f64(&global.lon_cell),
        x_cell: cell_f64(&global.x_cell),
        y_cell: cell_f64(&global.y_cell),
        z_cell: cell_f64(&global.z_cell),
        lat_vertex: gather_f64(&global.lat_vertex, &kept_vertices),
        lon_vertex: gather_f64(&global.lon_vertex, &kept_vertices),
        x_vertex: gather_f64(&global.x_vertex, &kept_vertices),
        y_vertex: gather_f64(&global.y_vertex, &kept_vertices),
        z_vertex: gather_f64(&global.z_vertex, &kept_vertices),
        lat_edge: gather_f64(&global.lat_edge, &kept_edges),
        lon_edge: gather_f64(&global.lon_edge, &kept_edges),
        x_edge: gather_f64(&global.x_edge, &kept_edges),
        y_edge: gather_f64(&global.y_edge, &kept_edges),
        z_edge: gather_f64(&global.z_edge, &kept_edges),
        n_edges_on_cell,
        cells_on_cell,
        vertices_on_cell,
        edges_on_cell,
        cells_on_vertex,
        edges_on_vertex,
        cells_on_edge,
        vertices_on_edge,
        n_edges_on_edge,
        edges_on_edge,
        area_cell: cell_f64(&global.area_cell),
        area_triangle: gather_f64(&global.area_triangle, &kept_vertices),
        kite_areas_on_vertex,
        dv_edge: gather_f64(&global.dv_edge, &kept_edges),
        dc_edge: gather_f64(&global.dc_edge, &kept_edges),
        angle_edge: gather_f64(&global.angle_edge, &kept_edges),
        weights_on_edge,
        mesh_density: cell_f64(&global.mesh_density),
        nominal_min_dc: global.nominal_min_dc,
        error_segment: gather_f64(&global.error_segment, &kept_edges),
    })
}
