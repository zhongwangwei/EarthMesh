//! Subsetting a global MPAS mesh to a region must re-index connectivity
//! topologically consistently: dropped cells/edges/vertices collapse to the
//! MPAS `0` no-neighbour marker, a once-interior edge on the new boundary keeps
//! one real cell, flux-stencil weights for dropped neighbour edges are zeroed,
//! and per-cell/vertex/edge geometry is copied verbatim for the kept rows.

use earthmesh_cli::{subset_mpas_mesh, MpasMesh};

/// Two triangular cells sharing edge `e1`. Index 0 is the placeholder row.
/// Cells 1,2 share vertices 1,2 (edge e1) and have unique vertices 3,4.
fn two_cell_global() -> MpasMesh {
    MpasMesh {
        // distinguishable dummy geometry so gather() can be checked by value
        lat_cell: vec![0.0, 11.0, 22.0],
        lon_cell: vec![0.0, 11.0, 22.0],
        x_cell: vec![0.0, 11.0, 22.0],
        y_cell: vec![0.0, 11.0, 22.0],
        z_cell: vec![0.0, 11.0, 22.0],
        lat_vertex: vec![0.0, 101.0, 102.0, 103.0, 104.0],
        lon_vertex: vec![0.0, 101.0, 102.0, 103.0, 104.0],
        x_vertex: vec![0.0, 101.0, 102.0, 103.0, 104.0],
        y_vertex: vec![0.0, 101.0, 102.0, 103.0, 104.0],
        z_vertex: vec![0.0, 101.0, 102.0, 103.0, 104.0],
        lat_edge: vec![0.0, 201.0, 202.0, 203.0, 204.0, 205.0],
        lon_edge: vec![0.0, 201.0, 202.0, 203.0, 204.0, 205.0],
        x_edge: vec![0.0, 201.0, 202.0, 203.0, 204.0, 205.0],
        y_edge: vec![0.0, 201.0, 202.0, 203.0, 204.0, 205.0],
        z_edge: vec![0.0, 201.0, 202.0, 203.0, 204.0, 205.0],
        n_edges_on_cell: vec![0, 3, 3],
        cells_on_cell: vec![vec![0, 0, 0], vec![2, 0, 0], vec![1, 0, 0]],
        vertices_on_cell: vec![vec![0, 0, 0], vec![1, 2, 3], vec![1, 2, 4]],
        edges_on_cell: vec![vec![0, 0, 0], vec![1, 2, 3], vec![1, 4, 5]],
        // cellsOnVertex / edgesOnVertex per vertex (idx 0 placeholder, 1..4)
        cells_on_vertex: vec![vec![0, 0, 0], vec![1, 2, 0], vec![1, 2, 0], vec![1, 0, 0], vec![2, 0, 0]],
        edges_on_vertex: vec![vec![0, 0, 0], vec![1, 2, 4], vec![1, 3, 5], vec![2, 3, 0], vec![4, 5, 0]],
        // per edge (idx 0 placeholder, 1..5)
        cells_on_edge: vec![[0, 0], [1, 2], [1, 0], [1, 0], [2, 0], [2, 0]],
        vertices_on_edge: vec![[0, 0], [1, 2], [1, 3], [2, 3], [1, 4], [2, 4]],
        n_edges_on_edge: vec![0, 4, 0, 0, 0, 0],
        edges_on_edge: vec![
            vec![],
            vec![2, 3, 4, 5],
            vec![],
            vec![],
            vec![],
            vec![],
        ],
        area_cell: vec![0.0, 1.1, 2.2],
        area_triangle: vec![0.0, 1.0, 2.0, 3.0, 4.0],
        kite_areas_on_vertex: vec![vec![0.0; 3], vec![1.0; 3], vec![2.0; 3], vec![3.0; 3], vec![4.0; 3]],
        dv_edge: vec![0.0, 21.0, 22.0, 23.0, 24.0, 25.0],
        dc_edge: vec![0.0, 31.0, 32.0, 33.0, 34.0, 35.0],
        angle_edge: vec![0.0, 0.1, 0.2, 0.3, 0.4, 0.5],
        weights_on_edge: vec![
            vec![],
            vec![0.1, 0.2, 0.3, 0.4],
            vec![],
            vec![],
            vec![],
            vec![],
        ],
        mesh_density: vec![0.0, 1.0, 1.0],
        nominal_min_dc: 0.5,
        error_segment: vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    }
}

#[test]
fn keeping_one_cell_collapses_dropped_references_to_zero() {
    let g = two_cell_global();
    // keep cell 1, drop cell 2
    let keep = vec![false, true, false];
    let r = subset_mpas_mesh(&g, &keep).expect("subset");

    // 1 cell, 3 vertices (1,2,3 — v4 only touched cell 2), 3 edges (1,2,3).
    assert_eq!(r.lat_cell.len() - 1, 1);
    assert_eq!(r.lat_vertex.len() - 1, 3);
    assert_eq!(r.lat_edge.len() - 1, 3);

    // Geometry copied verbatim for kept rows.
    assert_eq!(r.lat_cell, vec![0.0, 11.0]);
    assert_eq!(r.lat_vertex, vec![0.0, 101.0, 102.0, 103.0]);
    assert_eq!(r.lat_edge, vec![0.0, 201.0, 202.0, 203.0]);
    assert_eq!(r.area_cell, vec![0.0, 1.1]);

    // Cell 1's only neighbour (cell 2) is gone → all-boundary cellsOnCell.
    assert_eq!(r.cells_on_cell[1], vec![0, 0, 0]);

    // The once-interior shared edge e1 keeps its real cell, the other → 0.
    assert_eq!(r.cells_on_edge[1], [1, 0]);
    // Its stencil neighbours e4,e5 dropped → 0; e2,e3 kept and renumbered.
    assert_eq!(r.edges_on_edge[1], vec![2, 3, 0, 0]);
    // Weights for dropped stencil edges zeroed; kept ones preserved.
    assert_eq!(r.weights_on_edge[1], vec![0.1, 0.2, 0.0, 0.0]);

    // Boundary vertex: cellsOnVertex maps dropped cell 2 → 0.
    assert_eq!(r.cells_on_vertex[1], vec![1, 0, 0]);
    // Vertex 1's edge e4 (dropped) → 0.
    assert_eq!(r.edges_on_vertex[1], vec![1, 2, 0]);

    // Every connectivity id is within the new index range.
    let (nc, nv, ne) = (r.lat_cell.len(), r.lat_vertex.len(), r.lat_edge.len());
    for row in &r.cells_on_cell {
        for &x in row {
            assert!(x >= 0 && (x as usize) < nc);
        }
    }
    for row in &r.vertices_on_cell {
        for &x in row {
            assert!(x >= 0 && (x as usize) < nv);
        }
    }
    for &[a, b] in &r.cells_on_edge {
        assert!(a >= 0 && (a as usize) < nc && b >= 0 && (b as usize) < nc);
    }

    // cellsOnEdge ↔ edgesOnCell agreement for kept cells.
    for e in 1..ne {
        for &c in &r.cells_on_edge[e] {
            if c > 0 {
                assert!(r.edges_on_cell[c as usize].contains(&(e as i32)));
            }
        }
    }
}
