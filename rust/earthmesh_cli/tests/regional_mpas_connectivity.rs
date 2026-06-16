//! Regional (limited-area) MPAS connectivity must remain topologically
//! consistent after a carve: every interior edge borders exactly two kept
//! cells that both list it, boundary edges carry `[cell, 0]`, neighbour
//! relations are symmetric, and the Euler characteristic of the carved patch
//! is that of a disk.

use earthmesh_cli::{build_regional_mpas_connectivity, LonLatPoint, UnstructuredMesh};

/// Two triangular cells sharing one edge — the smallest carved patch with both
/// an interior edge and boundary edges. Index 0/1 are the Fortran placeholders;
/// a `1` in `m_to_w` marks a cell that was carved away (exterior).
fn two_cell_patch() -> UnstructuredMesh {
    let p = |lon: f64, lat: f64| LonLatPoint { lon, lat };
    UnstructuredMesh {
        // idx: 0,1 placeholder; 2,3,4,5 real vertices (triangle circumcenters)
        m_points: vec![p(0.0, 0.0), p(0.0, 0.0), p(0.0, 0.0), p(1.0, 0.0), p(0.5, 1.0), p(0.5, -1.0)],
        // idx: 0,1 placeholder; 2,3 real cells (hexagon centres, here triangles)
        w_points: vec![p(0.0, 0.0), p(0.0, 0.0), p(0.4, 0.3), p(0.6, -0.3)],
        // cellsOnVertex: v2,v3 shared by cells 2,3; v4 only cell 2; v5 only cell 3
        m_to_w: vec![[1, 1, 1], [1, 1, 1], [2, 3, 1], [2, 3, 1], [2, 1, 1], [3, 1, 1]],
        // verticesOnCell rings (cyclic): cell2=[2,3,4], cell3 shares edge (2,3)
        w_to_m: vec![vec![1], vec![1], vec![2, 3, 4], vec![3, 2, 5]],
        n_w_to_m: vec![1, 1, 3, 3],
    }
}

#[test]
fn carved_patch_connectivity_is_topologically_consistent() {
    let mesh = two_cell_patch();
    let conn = build_regional_mpas_connectivity(&mesh).expect("build connectivity");

    // 5 distinct edges: (2,3) shared interior + (3,4),(4,2),(2,5),(5,3) boundary.
    assert_eq!(conn.edge_count, 5);
    let interior = (2..conn.cells_on_edge.len())
        .filter(|&e| conn.cells_on_edge[e][1] != 0)
        .count();
    let boundary = (2..conn.cells_on_edge.len())
        .filter(|&e| conn.cells_on_edge[e][1] == 0)
        .count();
    assert_eq!((interior, boundary), (1, 4));

    // cellsOnEdge ↔ edgesOnCell agreement.
    for e in 2..conn.cells_on_edge.len() {
        for &cell in &conn.cells_on_edge[e] {
            if cell >= 2 {
                assert!(conn.edges_on_cell[cell].contains(&e), "edge {e} not on cell {cell}");
            }
        }
    }
    // cellsOnCell symmetry; the shared neighbour is mutual.
    for cell in 2..mesh.w_points.len() {
        for &nb in &conn.cells_on_cell[cell] {
            if nb >= 2 {
                assert!(conn.cells_on_cell[nb].contains(&cell), "{cell}->{nb} not symmetric");
            }
        }
    }
    assert!(conn.cells_on_cell[2].contains(&3) && conn.cells_on_cell[3].contains(&2));

    // Per-cell array lengths all equal nEdgesOnCell.
    for cell in 2..mesh.w_points.len() {
        let ne = conn.n_edges_on_cell[cell];
        assert_eq!(conn.vertices_on_cell[cell].len(), ne);
        assert_eq!(conn.edges_on_cell[cell].len(), ne);
        assert_eq!(conn.cells_on_cell[cell].len(), ne);
    }

    // cellsOnVertex maps carved cells to 0.
    assert_eq!(conn.cells_on_vertex[4], [2, 0, 0]);
    assert_eq!(conn.cells_on_vertex[2], [2, 3, 0]);

    // Euler characteristic of a disk: V - E + F = 1.
    let v = (2..mesh.m_points.len())
        .filter(|&v| conn.edges_on_vertex[v].iter().any(|&e| e > 0))
        .count() as i64;
    let f = (2..mesh.w_points.len())
        .filter(|&c| conn.n_edges_on_cell[c] > 0)
        .count() as i64;
    assert_eq!(v - conn.edge_count as i64 + f, 1);
}
