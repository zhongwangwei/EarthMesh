use earthmesh_mesh::order_vertices_on_cell_by_shared_edges_fortran_indexed;

#[test]
fn topology_order_restores_cycle_when_last_two_vertices_are_swapped() {
    let mut edges_on_vertex = vec![[0usize; 3]; 61];
    edges_on_vertex[10] = [1, 6, 0];
    edges_on_vertex[20] = [1, 2, 0];
    edges_on_vertex[30] = [2, 3, 0];
    edges_on_vertex[40] = [3, 4, 0];
    edges_on_vertex[50] = [4, 5, 0];
    edges_on_vertex[60] = [5, 6, 0];
    let vertices_on_cell = vec![vec![], vec![], vec![10, 20, 30, 40, 60, 50, 1]];
    let n_edges_on_cell = vec![0, 0, 6];

    let ordered = order_vertices_on_cell_by_shared_edges_fortran_indexed(
        &vertices_on_cell,
        &n_edges_on_cell,
        &edges_on_vertex,
    )
    .expect("topological order");

    assert_eq!(ordered[2], vec![10, 20, 30, 40, 50, 60, 1]);
}
