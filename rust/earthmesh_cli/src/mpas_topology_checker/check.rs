use crate::*;

/// Check an [`MpasMesh`] for topological self-consistency: index ranges, the
/// `cellsOnCell`/`cellsOnEdge`/`edgesOnCell`/`edgesOnVertex`/`verticesOnEdge`
/// cross-reference symmetries, and the Euler characteristic. Works for a global
/// (closed, chi=2) or a regional/limited-area (open disk, chi=1) mesh.
pub fn check_mpas_mesh_topology(mesh: &MpasMesh) -> MeshTopologyReport {
    let mut v: Vec<String> = Vec::new();
    let n_cells = mesh.lat_cell.len().saturating_sub(1);
    let n_vertices = mesh.lat_vertex.len().saturating_sub(1);
    let n_edges = mesh.lat_edge.len().saturating_sub(1);

    let check_len = |v: &mut Vec<String>, name: &str, got: usize, want: usize| {
        if got != want {
            v.push(format!("{name} length {got} != {want}"));
        }
    };
    check_len(
        &mut v,
        "cells_on_cell",
        mesh.cells_on_cell.len().saturating_sub(1),
        n_cells,
    );
    check_len(
        &mut v,
        "vertices_on_cell",
        mesh.vertices_on_cell.len().saturating_sub(1),
        n_cells,
    );
    check_len(
        &mut v,
        "edges_on_cell",
        mesh.edges_on_cell.len().saturating_sub(1),
        n_cells,
    );
    check_len(
        &mut v,
        "cells_on_vertex",
        mesh.cells_on_vertex.len().saturating_sub(1),
        n_vertices,
    );
    check_len(
        &mut v,
        "cells_on_edge",
        mesh.cells_on_edge.len().saturating_sub(1),
        n_edges,
    );
    check_len(
        &mut v,
        "vertices_on_edge",
        mesh.vertices_on_edge.len().saturating_sub(1),
        n_edges,
    );
    if !v.is_empty() {
        return MeshTopologyReport {
            n_cells,
            n_vertices,
            n_edges,
            euler_characteristic: n_cells as i64 + n_vertices as i64 - n_edges as i64,
            boundary_edges: 0,
            is_closed: false,
            violations: v,
        };
    }

    let cap = 60usize;
    for c in 1..=n_cells {
        let ne = mesh.n_edges_on_cell[c].max(0) as usize;
        if ne > mesh.vertices_on_cell[c].len() {
            v.push(format!(
                "cell {c}: nEdges {ne} exceeds verticesOnCell width"
            ));
            continue;
        }
        for k in 0..ne {
            let vv = mesh.vertices_on_cell[c][k];
            if (vv <= 0 || vv as usize > n_vertices) && v.len() < cap {
                v.push(format!(
                    "cell {c}: verticesOnCell[{k}]={vv} out of 1..={n_vertices}"
                ));
            }
            let ee = mesh.edges_on_cell[c][k];
            if (ee <= 0 || ee as usize > n_edges) && v.len() < cap {
                v.push(format!(
                    "cell {c}: edgesOnCell[{k}]={ee} out of 1..={n_edges}"
                ));
            }
            let cc = mesh.cells_on_cell[c][k];
            if (cc < 0 || cc as usize > n_cells) && v.len() < cap {
                v.push(format!(
                    "cell {c}: cellsOnCell[{k}]={cc} out of 0..={n_cells}"
                ));
            }
        }
    }

    for c in 1..=n_cells {
        let ne = mesh.n_edges_on_cell[c].max(0) as usize;
        for k in 0..ne.min(mesh.cells_on_cell[c].len()) {
            let nb = mesh.cells_on_cell[c][k];
            if nb > 0 && (nb as usize) <= n_cells {
                let back = mesh.cells_on_cell[nb as usize].contains(&(c as i32));
                if !back && v.len() < cap {
                    v.push(format!("cellsOnCell asymmetry: {c}->{nb} not mirrored"));
                }
            }
        }
    }

    let mut boundary_edges = 0usize;
    for e in 1..=n_edges {
        let [c0, c1] = mesh.cells_on_edge[e];
        if c0 == 0 || c1 == 0 {
            boundary_edges += 1;
        }
        for &c in &[c0, c1] {
            if c > 0 && (c as usize) <= n_cells {
                if !mesh.edges_on_cell[c as usize].contains(&(e as i32)) && v.len() < cap {
                    v.push(format!(
                        "edge {e}: cell {c} does not list it in edgesOnCell"
                    ));
                }
            } else if (c < 0 || c as usize > n_cells) && v.len() < cap {
                v.push(format!("edge {e}: cellsOnEdge {c} out of 0..={n_cells}"));
            }
        }
        for &vv in &mesh.vertices_on_edge[e] {
            if (vv <= 0 || vv as usize > n_vertices) && v.len() < cap {
                v.push(format!(
                    "edge {e}: verticesOnEdge {vv} out of 1..={n_vertices}"
                ));
            }
        }
    }

    for vert in 1..=n_vertices {
        for &e in &mesh.edges_on_vertex[vert] {
            if e > 0 && (e as usize) <= n_edges {
                let [a, b] = mesh.vertices_on_edge[e as usize];
                if a != vert as i32 && b != vert as i32 && v.len() < cap {
                    v.push(format!(
                        "vertex {vert}: edge {e} does not list it in verticesOnEdge"
                    ));
                }
            }
        }
    }

    let euler = n_cells as i64 + n_vertices as i64 - n_edges as i64;
    let is_closed = boundary_edges == 0 && euler == 2;
    if euler != 1 && euler != 2 {
        v.push(format!(
            "Euler characteristic {euler} is neither 2 (closed sphere) nor 1 (disk/region)"
        ));
    }

    MeshTopologyReport {
        n_cells,
        n_vertices,
        n_edges,
        euler_characteristic: euler,
        boundary_edges,
        is_closed,
        violations: v,
    }
}
