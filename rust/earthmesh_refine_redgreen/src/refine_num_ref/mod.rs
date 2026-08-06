use std::io;

/// Port of `MOD_refine.F90:num_ref_cal`.
///
/// Counts the triangles the last judge pass newly asked for, and folds them
/// into the running segment marking.
///
/// The two markings do different jobs and that is why both exist. `ref_sjx` is
/// what a single judge produced and is overwritten by the next one; the segment
/// marking accumulates every triangle this refinement round will split, and is
/// what the subdivision reads. A triangle already in the segment marking
/// contributes nothing to the count, so the count is exactly "how much did this
/// pass add" -- which is what the driver's loops test for zero to decide they
/// have converged.
///
/// Original vertices (`1..=num_vertex`) are never counted or marked: they are
/// the icosahedron's own points, and the judge chain protects them separately.
pub fn refine_num_ref_cal_one_based(
    num_vertex: usize,
    sjx_points: usize,
    ref_sjx: &[i32],
    ref_sjx_segment: &mut [i32],
) -> io::Result<usize> {
    if sjx_points >= ref_sjx.len() || sjx_points >= ref_sjx_segment.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "sjx_points {sjx_points} must be addressable in ref_sjx ({}) and \
                 ref_sjx_segment ({})",
                ref_sjx.len(),
                ref_sjx_segment.len()
            ),
        ));
    }
    if num_vertex > sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_vertex {num_vertex} exceeds sjx_points {sjx_points}"),
        ));
    }

    let mut num_ref = 0usize;
    for triangle in num_vertex + 1..=sjx_points {
        if ref_sjx[triangle] == 0 {
            continue;
        }
        if ref_sjx_segment[triangle] == 0 {
            num_ref += 1;
            ref_sjx_segment[triangle] = 1;
        }
    }
    Ok(num_ref)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_triangles_the_segment_marking_does_not_already_hold_are_counted() {
        // The count is what the driver's convergence tests read, so a triangle
        // asked for twice must count once -- otherwise the loop that waits for
        // zero never reaches it.
        let ref_sjx = [0, 0, 1, 1, 1, 0, 1];
        let mut segment = [0, 0, 1, 0, 0, 0, 0];

        let added = refine_num_ref_cal_one_based(1, 5, &ref_sjx, &mut segment).expect("count");

        assert_eq!(added, 2, "triangles 3 and 4; 2 was already in the segment");
        assert_eq!(
            segment,
            [0, 0, 1, 1, 1, 0, 0],
            "triangle 6 is past sjx_points and stays untouched"
        );
    }

    #[test]
    fn original_vertices_are_never_counted_or_marked() {
        // The icosahedron's own points are protected by the judge chain; the
        // count must not disagree with it.
        let ref_sjx = [0, 1, 1, 1];
        let mut segment = [0, 0, 0, 0];

        let added = refine_num_ref_cal_one_based(2, 3, &ref_sjx, &mut segment).expect("count");

        assert_eq!(added, 1);
        assert_eq!(segment, [0, 0, 0, 1]);
    }

    #[test]
    fn a_marking_shorter_than_the_mesh_is_an_error_rather_than_a_short_count() {
        let ref_sjx = [0, 0, 1];
        let mut segment = [0, 0, 0];
        let error = refine_num_ref_cal_one_based(1, 9, &ref_sjx, &mut segment)
            .expect_err("a marking that cannot address the mesh must not silently count less");
        assert!(error.to_string().contains("sjx_points 9"), "{error}");
    }
}
