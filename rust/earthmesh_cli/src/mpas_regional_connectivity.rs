use crate::lonlat_degrees_from_points;
use crate::n_edges_on_cell_usize_from_mesh;
use crate::RegionalMpasConnectivity;
use crate::UnstructuredMesh;
use earthmesh_mesh::lonlat_points_to_unit_xyz;
use earthmesh_mesh::order_vertex_arrays_one_based;
use earthmesh_mesh::order_vertices_on_edge_one_based;
use earthmesh_mesh::spherical_centroid_degrees;
use earthmesh_mesh::GetEdgeProductionOutput;
use std::collections::HashMap;
use std::io;

/// Build [`RegionalMpasConnectivity`] from a carved hex [`UnstructuredMesh`].
pub fn build_regional_mpas_connectivity(
    mesh: &UnstructuredMesh,
) -> io::Result<RegionalMpasConnectivity> {
    let n_cells = mesh.w_points.len();
    let n_verts = mesh.m_points.len();
    let nec = n_edges_on_cell_usize_from_mesh(mesh)?;
    let is_real = |id: usize| id >= 2;

    let mut edge_of_pair: HashMap<(usize, usize), usize> = HashMap::new();
    let mut vertices_on_edge: Vec<[usize; 2]> = vec![[0, 0], [0, 0]];
    let mut cells_on_edge: Vec<[usize; 2]> = vec![[0, 0], [0, 0]];
    let mut edges_on_cell: Vec<Vec<usize>> = vec![Vec::new(); n_cells];

    for c in 2..n_cells {
        let ne = nec[c];
        if ne == 0 {
            continue;
        }
        if mesh.w_to_m[c].len() < ne {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("w_to_m row {c} shorter than n_w_to_m {ne}"),
            ));
        }
        let ring: Vec<usize> = mesh.w_to_m[c][..ne].iter().map(|&v| v as usize).collect();
        let mut cell_edges = Vec::with_capacity(ne);
        for i in 0..ne {
            let a = ring[i];
            let b = ring[(i + 1) % ne];
            if !is_real(a) || !is_real(b) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("cell {c} side ({a},{b}) canonicals a placeholder vertex"),
                ));
            }
            let key = (a.min(b), a.max(b));
            let edge_id = match edge_of_pair.get(&key) {
                Some(&eid) => {
                    if cells_on_edge[eid][1] != 0 {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("edge {eid} (vertices {key:?}) shared by more than two cells"),
                        ));
                    }
                    cells_on_edge[eid][1] = c;
                    eid
                }
                None => {
                    let eid = vertices_on_edge.len();
                    vertices_on_edge.push([key.0, key.1]);
                    cells_on_edge.push([c, 0]);
                    edge_of_pair.insert(key, eid);
                    eid
                }
            };
            cell_edges.push(edge_id);
        }
        edges_on_cell[c] = cell_edges;
    }
    let edge_count = vertices_on_edge.len().saturating_sub(2);

    let mut cells_on_cell: Vec<Vec<usize>> = vec![Vec::new(); n_cells];
    for c in 2..n_cells {
        let ne = nec[c];
        if ne == 0 {
            continue;
        }
        let mut nbrs = Vec::with_capacity(ne);
        for &eid in &edges_on_cell[c] {
            let [c0, c1] = cells_on_edge[eid];
            nbrs.push(if c0 == c { c1 } else { c0 });
        }
        cells_on_cell[c] = nbrs;
    }

    let mut vertices_on_cell: Vec<Vec<usize>> = vec![Vec::new(); n_cells];
    for c in 2..n_cells {
        let ne = nec[c];
        vertices_on_cell[c] = mesh.w_to_m[c][..ne].iter().map(|&v| v as usize).collect();
    }

    let mut cells_on_vertex: Vec<[usize; 3]> = vec![[0, 0, 0]; n_verts];
    for v in 2..n_verts {
        let row = mesh.m_to_w[v];
        for (slot, value) in row.iter().enumerate() {
            let id = *value as usize;
            cells_on_vertex[v][slot] = if is_real(id) { id } else { 0 };
        }
    }

    let mut edges_on_vertex: Vec<[usize; 3]> = vec![[0, 0, 0]; n_verts];
    for eid in 2..vertices_on_edge.len() {
        let [v0, v1] = vertices_on_edge[eid];
        for v in [v0, v1] {
            let slots = &mut edges_on_vertex[v];
            if let Some(free) = slots.iter_mut().find(|s| **s == 0) {
                *free = eid;
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("vertex {v} incident to more than three edges"),
                ));
            }
        }
    }

    Ok(RegionalMpasConnectivity {
        edge_count,
        n_edges_on_cell: nec,
        vertices_on_cell,
        edges_on_cell,
        cells_on_cell,
        cells_on_vertex,
        edges_on_vertex,
        cells_on_edge,
        vertices_on_edge,
    })
}

/// Build the edge payload used by the full MPAS writer for a bounded or
/// cell-culled mesh. Interior edge points follow the existing EarthMesh
/// cell-centroid convention; boundary edge points follow MpasMeshConverter.x
/// and use the midpoint of the two edge vertices.
pub(crate) fn build_regional_mpas_edge_output(
    mesh: &UnstructuredMesh,
) -> io::Result<GetEdgeProductionOutput> {
    let connectivity = build_regional_mpas_connectivity(mesh)?;
    let vertex_lonlat = lonlat_degrees_from_points(&mesh.m_points);
    let cell_lonlat = lonlat_degrees_from_points(&mesh.w_points);
    let vertices_on_edge = order_vertices_on_edge_one_based(
        &vertex_lonlat,
        &cell_lonlat,
        &connectivity.cells_on_edge,
        &connectivity.vertices_on_edge,
    )
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to orient regional MPAS verticesOnEdge",
        )
    })?;

    let mut edge_points =
        vec![earthmesh_mesh::LonLatDegrees::new(0.0, 0.0); vertices_on_edge.len()];
    for edge_id in 2..vertices_on_edge.len() {
        let [cell1, cell2] = connectivity.cells_on_edge[edge_id];
        let [vertex1, vertex2] = vertices_on_edge[edge_id];
        let points = if cell2 == 0 {
            [vertex_lonlat[vertex1], vertex_lonlat[vertex2]]
        } else {
            [cell_lonlat[cell1], cell_lonlat[cell2]]
        };
        edge_points[edge_id] = spherical_centroid_degrees(&points).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to compute regional MPAS edge point {edge_id}"),
            )
        })?;
    }

    let ordered = order_vertex_arrays_one_based(
        &lonlat_points_to_unit_xyz(&vertex_lonlat),
        &lonlat_points_to_unit_xyz(&edge_points),
        &connectivity.edges_on_vertex,
        &vertices_on_edge,
        &connectivity.cells_on_edge,
    )
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to order regional MPAS vertex connectivity",
        )
    })?;

    Ok(GetEdgeProductionOutput {
        cells_on_edge: connectivity.cells_on_edge,
        vertices_on_edge,
        edges_on_vertex: ordered.edges_on_vertex,
        cells_on_vertex: ordered.cells_on_vertex,
        edge_points,
    })
}
