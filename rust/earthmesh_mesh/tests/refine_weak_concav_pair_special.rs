use earthmesh_mesh::refine_weak_concav_pair_special_fortran_indexed;

fn base_inputs() -> (
    Vec<Vec<usize>>,
    Vec<[usize; 3]>,
    Vec<i32>,
    Vec<i32>,
    Vec<[usize; 2]>,
    Vec<Vec<usize>>,
) {
    let mut triangle_neighbors = vec![vec![1, 1, 1]; 13];
    let mut cells_on_triangle = vec![[0, 0, 0]; 13];
    let mut mrl_new = vec![0; 13];
    let ref_sjx = vec![0; 13];
    for triangle in 1..=12 {
        mrl_new[triangle] = 1;
        cells_on_triangle[triangle] = [triangle * 10, triangle * 10 + 1, triangle * 10 + 2];
    }

    // Pair 1 weak concavity triangle 2 points outward to m3=5.
    triangle_neighbors[2] = vec![4, 5, 6];
    mrl_new[4] = 4;
    triangle_neighbors[5] = vec![7, 8, 4];
    cells_on_triangle[3] = [100, 101, 102];
    cells_on_triangle[7] = [100, 200, 201]; // shares with paired weak triangle -> segment slot.
    cells_on_triangle[8] = [300, 301, 302]; // disjoint -> deferred mrl_new renewal.

    // Pair 2 weak concavity triangle 3 points outward to m3=10.
    triangle_neighbors[3] = vec![9, 10, 11];
    mrl_new[9] = 4;
    triangle_neighbors[10] = vec![11, 12, 9];
    cells_on_triangle[2] = [500, 501, 502];
    cells_on_triangle[11] = [500, 600, 601]; // shares with paired weak triangle -> segment slot.
    cells_on_triangle[12] = [700, 701, 702]; // disjoint -> deferred mrl_new renewal.

    let weak_concav_pair = vec![[0, 0], [2, 0], [3, 0]];
    let weak_concav_segment = vec![vec![0; 2]; 5];

    (
        triangle_neighbors,
        cells_on_triangle,
        mrl_new,
        ref_sjx,
        weak_concav_pair,
        weak_concav_segment,
    )
}

#[test]
fn weak_concav_pair_special_marks_outward_triangles_segments_and_deferred_refinements() {
    let (
        triangle_neighbors,
        cells_on_triangle,
        mut mrl_new,
        mut ref_sjx,
        mut weak_concav_pair,
        mut weak_concav_segment,
    ) = base_inputs();

    refine_weak_concav_pair_special_fortran_indexed(
        2,
        4,
        &triangle_neighbors,
        &cells_on_triangle,
        &mut mrl_new,
        &mut ref_sjx,
        &mut weak_concav_pair,
        &mut weak_concav_segment,
    )
    .expect("weak concavity special-case state update");

    assert_eq!(weak_concav_pair[1], [2, 5]);
    assert_eq!(weak_concav_pair[2], [3, 10]);
    assert_eq!(ref_sjx[5], 1);
    assert_eq!(ref_sjx[10], 1);
    assert_eq!(weak_concav_segment[3][0], 7);
    assert_eq!(weak_concav_segment[4][0], 11);
    assert_eq!(mrl_new[8], 4);
    assert_eq!(mrl_new[12], 4);
}

#[test]
fn weak_concav_pair_special_uses_even_pair_partner_from_previous_column() {
    let (
        triangle_neighbors,
        mut cells_on_triangle,
        mut mrl_new,
        mut ref_sjx,
        mut weak_concav_pair,
        mut weak_concav_segment,
    ) = base_inputs();
    // If k=2 incorrectly paired with itself or next slot, triangle 11 would be
    // treated as disjoint.  Sharing only with pair-1 triangle proves even k uses k-1.
    cells_on_triangle[11] = [501, 900, 901];

    refine_weak_concav_pair_special_fortran_indexed(
        2,
        4,
        &triangle_neighbors,
        &cells_on_triangle,
        &mut mrl_new,
        &mut ref_sjx,
        &mut weak_concav_pair,
        &mut weak_concav_segment,
    )
    .expect("even pair uses previous weak-concavity triangle");

    assert_eq!(weak_concav_segment[4][0], 11);
}

#[test]
fn weak_concav_pair_special_rejects_odd_pair_without_partner() {
    let (
        triangle_neighbors,
        cells_on_triangle,
        mut mrl_new,
        mut ref_sjx,
        mut weak_concav_pair,
        mut weak_concav_segment,
    ) = base_inputs();

    let err = refine_weak_concav_pair_special_fortran_indexed(
        1,
        3,
        &triangle_neighbors,
        &cells_on_triangle,
        &mut mrl_new,
        &mut ref_sjx,
        &mut weak_concav_pair,
        &mut weak_concav_segment,
    )
    .expect_err("odd k requires the next paired weak concavity triangle");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}
