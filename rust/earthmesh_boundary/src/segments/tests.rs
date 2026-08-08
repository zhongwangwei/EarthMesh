use super::*;

/// Only the edges crossing the domain edge become segments.
#[test]
fn a_segment_is_an_edge_with_one_end_on_each_side() {
    // 0 and 1 inside, 2 and 3 outside.
    let edges = [(0, 1), (1, 2), (2, 3), (3, 0), (0, 2)];
    let list = SegmentList::from_straddling_edges(edges, |v| v < 2);

    assert!(!list.contains(0, 1), "both ends inside");
    assert!(!list.contains(2, 3), "both ends outside");
    assert!(list.contains(1, 2), "straddles");
    assert!(list.contains(3, 0), "straddles the other way");
    assert!(list.contains(0, 2), "and so does the diagonal");
    assert_eq!(list.len(), 3);
}

/// The same edge either way round is one segment.
#[test]
fn direction_and_repetition_do_not_multiply_segments() {
    let list = SegmentList::from_straddling_edges([(1, 2), (2, 1), (1, 2)], |v| v < 2);
    assert_eq!(list.len(), 1);
    assert!(list.contains(2, 1));
}

/// Splitting replaces one segment with two, which is Ruppert's induction.
#[test]
fn a_split_segment_is_two_segments() {
    let mut list = SegmentList::from_pairs([(1, 2)]);
    assert!(list.split(1, 2, 9));

    assert!(!list.contains(1, 2), "the original is gone");
    assert!(
        list.contains(1, 9) && list.contains(9, 2),
        "both halves are in"
    );
    assert_eq!(list.len(), 2);
}

/// Splitting an edge that is not a segment changes nothing, and says so.
///
/// The unsound version of this could not tell the two cases apart, so every
/// insertion looked like boundary work and the refinement multiplied its own
/// list. Guide 11.28.
#[test]
fn splitting_a_non_segment_is_a_no_op_that_reports_itself() {
    let mut list = SegmentList::from_pairs([(1, 2)]);
    assert!(!list.split(3, 4, 9), "not a segment");
    assert_eq!(list.len(), 1, "and nothing was added: {list:?}");
}

/// A midpoint that is one of the ends would leave a zero-length half.
#[test]
fn a_midpoint_that_is_an_endpoint_is_refused() {
    let mut list = SegmentList::from_pairs([(1, 2)]);
    assert!(!list.split(1, 2, 1));
    assert!(list.contains(1, 2), "and the segment survives");
}

/// A predicate is asked once per vertex, not once per edge it appears in.
#[test]
fn the_inside_predicate_is_asked_once_per_vertex() {
    let mut asked = Vec::new();
    let edges = [(0, 1), (1, 2), (2, 0), (0, 3), (3, 1)];
    let _ = SegmentList::from_straddling_edges(edges, |v| {
        asked.push(v);
        v < 2
    });
    let mut unique = asked.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(asked.len(), unique.len(), "asked more than once: {asked:?}");
}
