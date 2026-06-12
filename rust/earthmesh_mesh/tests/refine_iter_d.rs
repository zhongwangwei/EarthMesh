use earthmesh_mesh::refine_iter_d_judge_fortran_indexed;

fn fixture_one_plus_n_weak_concavity() -> (
    Vec<Vec<usize>>,
    Vec<[usize; 3]>,
    Vec<Vec<usize>>,
    Vec<usize>,
    Vec<i32>,
) {
    let sjx_points = 31;
    let mut triangle_neighbors = vec![vec![1, 1, 1]; sjx_points + 1];
    let mut cells_on_triangle = vec![[0, 0, 0]; sjx_points + 1];
    let mut mrl_new = vec![0; sjx_points + 1];

    for triangle in 2..=sjx_points {
        mrl_new[triangle] = 1;
        triangle_neighbors[triangle] = vec![2, 3, 4];
        cells_on_triangle[triangle] = [70 + triangle, 80 + triangle, 90 + triangle];
    }

    // Four unrefined boundary triangles, each adjacent to exactly one refined
    // triangle, define the closed boundary curve 10 -> 11 -> 12 -> 13 -> 10.
    triangle_neighbors[2] = vec![20, 6, 7];
    triangle_neighbors[3] = vec![21, 8, 9];
    triangle_neighbors[4] = vec![22, 11, 12];
    triangle_neighbors[5] = vec![23, 13, 14];
    for triangle in [20, 21, 22, 23, 30, 31] {
        mrl_new[triangle] = 4;
        triangle_neighbors[triangle] = vec![2, 3, 4];
    }

    cells_on_triangle[2] = [10, 11, 99];
    cells_on_triangle[3] = [11, 12, 99];
    cells_on_triangle[4] = [12, 13, 98];
    cells_on_triangle[5] = [13, 10, 98];
    cells_on_triangle[20] = [10, 11, 90];
    cells_on_triangle[21] = [11, 12, 91];
    cells_on_triangle[22] = [12, 13, 92];
    cells_on_triangle[23] = [13, 10, 93];
    cells_on_triangle[30] = [12, 50, 51];
    cells_on_triangle[31] = [13, 52, 53];

    let mut triangles_on_cell = vec![vec![]; 14];
    let mut edge_counts = vec![0; 14];
    triangles_on_cell[10] = vec![2, 5, 20, 23];
    edge_counts[10] = 4; // turn: two refined triangles, not a straight 3-refined section.
    triangles_on_cell[11] = vec![2, 3, 20, 21];
    edge_counts[11] = 4; // turn: closes the first one-triangle segment.
    triangles_on_cell[12] = vec![3, 4, 21, 22, 30];
    edge_counts[12] = 5; // straight: three refined triangles.
    triangles_on_cell[13] = vec![4, 5, 22, 23, 31];
    edge_counts[13] = 5; // straight: three refined triangles.

    (
        triangle_neighbors,
        cells_on_triangle,
        triangles_on_cell,
        edge_counts,
        mrl_new,
    )
}

#[test]
fn iter_d_marks_one_plus_n_weak_concavity_segment_pair() {
    let (triangle_neighbors, cells_on_triangle, triangles_on_cell, edge_counts, mrl_new) =
        fixture_one_plus_n_weak_concavity();

    let ref_sjx = refine_iter_d_judge_fortran_indexed(
        3,
        1,
        31,
        9,
        13,
        &triangle_neighbors,
        &cells_on_triangle,
        &triangles_on_cell,
        &edge_counts,
        &mrl_new,
    )
    .expect("calculate iterD one-plus-n weak concavity marks");

    assert_eq!(ref_sjx[2], 1, "last triangle of the one-length segment");
    assert_eq!(
        ref_sjx[3], 1,
        "first triangle of the neighboring longer segment"
    );
    assert_eq!(ref_sjx.iter().sum::<i32>(), 2);
}

#[test]
fn iter_d_returns_zero_when_transition_distance_is_one() {
    let ref_sjx = refine_iter_d_judge_fortran_indexed(
        1,
        1,
        3,
        1,
        1,
        &[vec![], vec![], vec![2, 2, 2], vec![2, 2, 2]],
        &[[0, 0, 0]; 4],
        &[vec![], vec![]],
        &[0, 0],
        &[0, 1, 1, 1],
    )
    .expect("set_dis_in=1 short-circuits like Fortran iterD");

    assert_eq!(ref_sjx, vec![0, 0, 0, 0]);
}

#[test]
fn iter_d_rejects_open_boundary_connections() {
    let (mut triangle_neighbors, cells_on_triangle, triangles_on_cell, edge_counts, mrl_new) =
        fixture_one_plus_n_weak_concavity();
    // Remove the refined neighbor from one boundary triangle, producing a graph
    // vertex with only one boundary connection.  Fortran stops here; Rust
    // surfaces it as InvalidInput.
    triangle_neighbors[5] = vec![13, 14, 15];

    let err = refine_iter_d_judge_fortran_indexed(
        3,
        1,
        31,
        9,
        13,
        &triangle_neighbors,
        &cells_on_triangle,
        &triangles_on_cell,
        &edge_counts,
        &mrl_new,
    )
    .expect_err("open boundary graph should be rejected");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}
