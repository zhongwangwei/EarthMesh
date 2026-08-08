use super::*;

/// Positions on a small lon/lat grid, keyed the way the carve keys vertices.
fn points(table: Vec<(usize, f64, f64)>) -> impl BoundaryPointSource {
    move |vertex: usize| {
        table
            .iter()
            .find(|(id, _, _)| *id == vertex)
            .map(|(_, lon, lat)| (*lon, *lat))
    }
}

/// An island with a lake: two rings, and the model has to say which is which.
///
/// This is the case `topology_counts` exists for. A builder that called both
/// rings outer would report two islands, and a refinement that later filled the
/// lake would look like it had changed nothing.
#[test]
fn a_ring_inside_another_becomes_its_hole() {
    let source = points(vec![
        (1, 0.0, 0.0),
        (2, 4.0, 0.0),
        (3, 4.0, 4.0),
        (4, 0.0, 4.0),
        (5, 1.0, 1.0),
        (6, 3.0, 1.0),
        (7, 3.0, 3.0),
        (8, 1.0, 3.0),
    ]);
    let curves = vec![
        Vec::new(), // Canonical's placeholder slot
        vec![1, 2, 3, 4],
        vec![5, 6, 7, 8],
    ];

    let model = boundary_model_from_closed_curves(&curves, &source).expect("model");
    model.validate().expect("a valid island with a lake");

    assert_eq!(
        model.topology_counts(),
        (1, 1),
        "one island, one lake: {:?}",
        model.loops
    );
    let hole = model
        .loops
        .iter()
        .find(|ring| ring.loop_type == LoopType::Hole)
        .expect("a hole");
    assert!(hole.parent.is_some(), "a hole names the ring it sits in");
}

/// Two separate islands are two outer loops, neither one's hole.
#[test]
fn disjoint_rings_are_both_outer() {
    let source = points(vec![
        (1, 0.0, 0.0),
        (2, 1.0, 0.0),
        (3, 1.0, 1.0),
        (4, 0.0, 1.0),
        (5, 40.0, 0.0),
        (6, 41.0, 0.0),
        (7, 41.0, 1.0),
        (8, 40.0, 1.0),
    ]);
    let curves = vec![Vec::new(), vec![1, 2, 3, 4], vec![5, 6, 7, 8]];

    let model = boundary_model_from_closed_curves(&curves, &source).expect("model");
    model.validate().expect("valid");
    assert_eq!(model.topology_counts(), (2, 0), "{:?}", model.loops);
}

/// The walker may repeat a ring's first vertex to close it; the model closes
/// implicitly, so the repeat has to go or `validate` reports a pinch.
#[test]
fn a_ring_that_repeats_its_first_vertex_still_validates() {
    let source = points(vec![
        (1, 0.0, 0.0),
        (2, 1.0, 0.0),
        (3, 1.0, 1.0),
        (4, 0.0, 1.0),
    ]);
    let curves = vec![Vec::new(), vec![1, 2, 3, 4, 1]];

    let model = boundary_model_from_closed_curves(&curves, &source).expect("model");
    model.validate().expect("no pinch");
    assert_eq!(model.loops[0].vertices.len(), 4);
}

/// A curve naming a vertex with no position is an error, not a dropped ring.
///
/// Dropping it would change the topology counts silently, which is the one
/// thing this module exists to prevent.
#[test]
fn a_vertex_without_a_position_is_an_error() {
    let source = points(vec![(1, 0.0, 0.0), (2, 1.0, 0.0)]);
    let curves = vec![Vec::new(), vec![1, 2, 99]];
    let error = boundary_model_from_closed_curves(&curves, &source).expect_err("refused");
    assert!(error.to_string().contains("99"), "{error}");
}

/// Orientation is decided, not inherited from the walk.
///
/// The same ring given both ways round must produce the same model, because a
/// ring walked off a mesh has no direction and the direction is what picks a
/// side. Without this the coastline could come back enclosing the ocean.
#[test]
fn a_ring_and_its_reverse_give_the_same_model() {
    let source = points(vec![
        (1, 0.0, 0.0),
        (2, 4.0, 0.0),
        (3, 4.0, 4.0),
        (4, 0.0, 4.0),
        (5, 1.0, 1.0),
        (6, 3.0, 1.0),
        (7, 3.0, 3.0),
        (8, 1.0, 3.0),
    ]);
    let forward = vec![Vec::new(), vec![1, 2, 3, 4], vec![5, 6, 7, 8]];
    let reversed = vec![Vec::new(), vec![4, 3, 2, 1], vec![8, 7, 6, 5]];

    let a = boundary_model_from_closed_curves(&forward, &source).expect("model");
    let b = boundary_model_from_closed_curves(&reversed, &source).expect("model");
    assert_eq!(a.topology_counts(), b.topology_counts());
    assert_eq!(a.topology_counts(), (1, 1), "{:?}", a.loops);
}

/// The counts a refinement must preserve, on the shape a carve actually makes.
///
/// The ocean runner's fixture carves one connected domain with no lakes, and
/// the model reads it as one outer loop and no holes -- the same pair its
/// stderr line reports. Pinned here because the value of `topology_counts` is
/// that it changes when a run removes an island or closes a channel, and a
/// counter nobody checks against a known shape can drift to reporting anything.
#[test]
fn a_single_carved_domain_reads_as_one_outer_loop_and_no_holes() {
    // A ring walked the way the carve walks one: consecutive vertices, closing
    // back on the first.
    let table: Vec<(usize, f64, f64)> = (0..12)
        .map(|step| {
            let angle = (step as f64) * std::f64::consts::TAU / 12.0;
            (step + 1, 10.0 * angle.cos(), 10.0 * angle.sin())
        })
        .collect();
    let ring: Vec<usize> = (1..=12).collect();
    let source = points(table);

    let model = boundary_model_from_closed_curves(&[Vec::new(), ring], &source).expect("model");
    model.validate().expect("a carved domain is a valid model");
    assert_eq!(model.topology_counts(), (1, 0), "{:?}", model.loops);
}

/// The counts fall when a region goes, which is what the one-sided bound reads.
///
/// The carve is allowed to remove an isolated sea -- that is what it is for --
/// so the check cannot demand the pair come out unchanged. What it can demand
/// is that nothing appears: renewal takes regions away and never adds one. This
/// pins both directions of that reading, so a bound written the other way round
/// would fail here rather than in a production run.
#[test]
fn removing_a_region_lowers_the_count_and_adding_one_raises_it() {
    let mut table: Vec<(usize, f64, f64)> = Vec::new();
    for (island, centre) in [(0usize, 0.0_f64), (1, 60.0)] {
        for step in 0..4 {
            let angle = (step as f64) * std::f64::consts::TAU / 4.0;
            table.push((
                island * 4 + step + 1,
                centre + 5.0 * angle.cos(),
                5.0 * angle.sin(),
            ));
        }
    }
    let source = points(table);
    let both = vec![Vec::new(), vec![1, 2, 3, 4], vec![5, 6, 7, 8]];
    let one = vec![Vec::new(), vec![1, 2, 3, 4]];

    let before = boundary_model_from_closed_curves(&both, &source).expect("model");
    let after = boundary_model_from_closed_curves(&one, &source).expect("model");

    assert_eq!(before.topology_counts(), (2, 0));
    assert_eq!(after.topology_counts(), (1, 0));
    let (outer_before, holes_before) = before.topology_counts();
    let (outer_after, holes_after) = after.topology_counts();
    assert!(
        outer_after <= outer_before && holes_after <= holes_before,
        "losing an island is allowed"
    );
    assert!(
        outer_before > outer_after,
        "and it is visible: the bound would catch the reverse"
    );
}

/// `robust_spherical_area_unit` is negative on a right-hand-rule ring.
///
/// The other half of the check `earthmesh_boundary` makes on the winding. Both
/// were only ever verified as a composition -- orient a ring, ask what contains
/// what, get the right answer -- which two mistakes that cancel would also
/// pass. This one asserts the area function's sign against the same outside
/// reference the winding was checked against, so `orient_counter_clockwise`
/// rests on two independently pinned facts rather than on their product.
///
/// The convention is worth stating because the name does not: **negative area
/// means counter-clockwise seen from outside the sphere**, the opposite of the
/// winding's sign. Reading it the other way produces a coastline that encloses
/// the ocean, consistently, which is the hardest kind of wrong to notice.
#[test]
fn the_area_sign_is_the_opposite_of_the_winding_sign() {
    let corners = [(0.0_f64, 0.0_f64), (10.0, 0.0), (5.0, 8.0)];
    let unit = |lon: f64, lat: f64| {
        let (lon, lat) = (lon.to_radians(), lat.to_radians());
        [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
    };
    let [a, b, c] = corners.map(|(lon, lat)| unit(lon, lat));
    let cross = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let counter_clockwise = cross[0] * c[0] + cross[1] * c[1] + cross[2] * c[2] > 0.0;
    assert!(counter_clockwise, "the fixture must be counter-clockwise");

    let points: Vec<earthmesh_mesh::LonLatDegrees> = corners
        .iter()
        .map(|&(lon, lat)| earthmesh_mesh::LonLatDegrees::new(lon, lat))
        .collect();
    let forward = earthmesh_mesh::robust_spherical_area_unit(&points).expect("area");
    let reversed: Vec<_> = points.iter().rev().copied().collect();
    let backward = earthmesh_mesh::robust_spherical_area_unit(&reversed).expect("area");

    assert!(
        forward < 0.0,
        "counter-clockwise gives a negative area: {forward}"
    );
    assert!(backward > 0.0, "and its reverse a positive one: {backward}");
    assert!(
        (forward + backward).abs() < 1.0e-12,
        "the two are the same magnitude: {forward} and {backward}"
    );
}
