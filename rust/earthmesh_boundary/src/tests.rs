use super::*;

fn vertex(lon: f64, lat: f64) -> BoundaryVertex {
    BoundaryVertex {
        lon_degrees: lon,
        lat_degrees: lat,
        pinned: false,
    }
}

fn square() -> Vec<BoundaryVertex> {
    vec![
        vertex(0.0, 0.0),
        vertex(1.0, 0.0),
        vertex(1.0, 1.0),
        vertex(0.0, 1.0),
    ]
}

/// An island with a lake: the shape the model exists to hold.
#[test]
fn an_outer_loop_with_a_hole_inside_it_validates() {
    let mut vertices = square();
    vertices.extend([
        vertex(0.25, 0.25),
        vertex(0.75, 0.25),
        vertex(0.75, 0.75),
        vertex(0.25, 0.75),
    ]);
    let model = SphericalBoundaryModel {
        vertices,
        loops: vec![
            BoundaryLoop {
                loop_type: LoopType::Outer,
                role: BoundaryRole::HardDomain,
                vertices: vec![0, 1, 2, 3],
                parent: None,
            },
            BoundaryLoop {
                loop_type: LoopType::Hole,
                role: BoundaryRole::HardDomain,
                vertices: vec![4, 5, 6, 7],
                parent: Some(0),
            },
        ],
    };
    model.validate().expect("an island with a lake is legal");
    assert_eq!(model.topology_counts(), (1, 1));
}

/// A hole with nothing to be inside of is not a hole.
#[test]
fn a_hole_without_a_parent_is_refused() {
    let model = SphericalBoundaryModel {
        vertices: square(),
        loops: vec![BoundaryLoop {
            loop_type: LoopType::Hole,
            role: BoundaryRole::HardDomain,
            vertices: vec![0, 1, 2, 3],
            parent: None,
        }],
    };
    let errors = model.validate().expect_err("an orphan hole is not a hole");
    assert!(errors.contains(&BoundaryError::OrphanHole { loop_index: 0 }));
}

/// A ring that visits a vertex twice pinches, and a pinch is what the
/// perimeter walks cannot close.
#[test]
fn a_ring_that_visits_a_vertex_twice_is_refused() {
    let model = SphericalBoundaryModel {
        vertices: square(),
        loops: vec![BoundaryLoop {
            loop_type: LoopType::Outer,
            role: BoundaryRole::HardDomain,
            vertices: vec![0, 1, 2, 1],
            parent: None,
        }],
    };
    let errors = model.validate().expect_err("a pinched ring is not a ring");
    assert!(errors.contains(&BoundaryError::RepeatedVertex {
        loop_index: 0,
        vertex: 1
    }));
}

/// Fewer than three vertices encloses nothing.
#[test]
fn a_ring_of_two_vertices_is_refused() {
    let model = SphericalBoundaryModel {
        vertices: square(),
        loops: vec![BoundaryLoop {
            loop_type: LoopType::Outer,
            role: BoundaryRole::HardDomain,
            vertices: vec![0, 1],
            parent: None,
        }],
    };
    let errors = model.validate().expect_err("two points enclose nothing");
    assert!(errors.contains(&BoundaryError::DegenerateLoop {
        loop_index: 0,
        vertices: 2
    }));
}

/// Every error is reported, not only the first.
#[test]
fn validation_reports_every_fault_it_finds() {
    let model = SphericalBoundaryModel {
        vertices: square(),
        loops: vec![
            BoundaryLoop {
                loop_type: LoopType::Outer,
                role: BoundaryRole::HardDomain,
                vertices: vec![0, 1],
                parent: Some(1),
            },
            BoundaryLoop {
                loop_type: LoopType::Hole,
                role: BoundaryRole::HardDomain,
                vertices: vec![0, 1, 99],
                parent: None,
            },
        ],
    };
    let errors = model.validate().expect_err("this model has several faults");
    assert!(
        errors.len() >= 4,
        "a caller fixing this should see all of it at once: {errors:?}"
    );
}

/// The roles differ in what they permit, and the difference is the point.
#[test]
fn only_a_guide_may_be_flipped_away_and_only_hard_curves_block_the_mesh() {
    assert!(BoundaryRole::RefinementGuide.permits_edge_flip());
    assert!(!BoundaryRole::HardDomain.permits_edge_flip());
    assert!(!BoundaryRole::EmbeddedFeature.permits_edge_flip());

    assert!(BoundaryRole::HardDomain.is_impassable());
    assert!(BoundaryRole::PeriodicSeam.is_impassable());
    assert!(
        !BoundaryRole::MaterialInterface.is_impassable(),
        "cells live on both sides of an interface"
    );
}

fn ring(loop_type: LoopType, vertices: Vec<usize>, parent: Option<usize>) -> BoundaryLoop {
    BoundaryLoop {
        loop_type,
        role: BoundaryRole::HardDomain,
        vertices,
        parent,
    }
}

/// The island-with-a-lake case, asked the question it exists to answer.
///
/// Three places, three different answers: the sea outside the island, the land
/// between coast and lake shore, and the lake itself. A model that joined the
/// hole to the outer ring by a cut would have to call the lake land or the
/// island sea; keeping the hole as its own loop is what makes all three
/// answerable.
#[test]
fn a_lake_inside_an_island_is_outside_the_domain() {
    let mut vertices = square();
    vertices.extend([
        vertex(0.25, 0.25),
        vertex(0.75, 0.25),
        vertex(0.75, 0.75),
        vertex(0.25, 0.75),
    ]);
    let model = SphericalBoundaryModel {
        vertices,
        loops: vec![
            ring(LoopType::Outer, vec![0, 1, 2, 3], None),
            ring(LoopType::Hole, vec![4, 5, 6, 7], Some(0)),
        ],
    };
    model.validate().expect("valid");

    assert!(!model.contains(2.0, 2.0), "the sea outside the island");
    assert!(
        model.contains(0.1, 0.5),
        "land between coast and lake shore"
    );
    assert!(!model.contains(0.5, 0.5), "the lake");
}

/// A coastline that crosses the dateline is inside-out to a planar ray cast.
///
/// This is the case a longitude-interval test gets wrong and the reason the
/// winding is summed on the sphere: the ring below spans 170 east to 170 west,
/// which as plain numbers looks like almost the whole globe rather than a
/// twenty-degree strip.
#[test]
fn a_ring_across_the_dateline_still_knows_its_inside() {
    let model = SphericalBoundaryModel {
        vertices: vec![
            vertex(170.0, -10.0),
            vertex(-170.0, -10.0),
            vertex(-170.0, 10.0),
            vertex(170.0, 10.0),
        ],
        loops: vec![ring(LoopType::Outer, vec![0, 1, 2, 3], None)],
    };
    model.validate().expect("valid");

    assert!(model.contains(180.0, 0.0), "the middle of the strip");
    assert!(model.contains(-179.0, 5.0), "east of the seam");
    assert!(model.contains(179.0, -5.0), "west of the seam");
    assert!(!model.contains(0.0, 0.0), "the far side of the globe");
    assert!(!model.contains(160.0, 0.0), "just outside the western edge");
}

/// A ring around the pole encloses the pole, which a latitude test denies.
#[test]
fn a_ring_around_the_pole_contains_it() {
    let model = SphericalBoundaryModel {
        vertices: vec![
            vertex(0.0, 80.0),
            vertex(90.0, 80.0),
            vertex(180.0, 80.0),
            vertex(-90.0, 80.0),
        ],
        loops: vec![ring(LoopType::Outer, vec![0, 1, 2, 3], None)],
    };
    model.validate().expect("valid");

    assert!(model.contains(0.0, 89.9), "the pole cap is inside");
    assert!(
        model.contains(137.0, 85.0),
        "and so is any longitude above 80"
    );
    assert!(!model.contains(0.0, 70.0), "below the ring is outside");
}

/// Every ring contributes its edges, and the last one closes it.
#[test]
fn segments_close_each_ring() {
    let mut vertices = square();
    vertices.extend([
        vertex(0.25, 0.25),
        vertex(0.75, 0.25),
        vertex(0.75, 0.75),
        vertex(0.25, 0.75),
    ]);
    let model = SphericalBoundaryModel {
        vertices,
        loops: vec![
            ring(LoopType::Outer, vec![0, 1, 2, 3], None),
            ring(LoopType::Hole, vec![4, 5, 6, 7], Some(0)),
        ],
    };

    let segments = model.segments();
    assert_eq!(segments.len(), 8, "four edges per ring: {segments:?}");
    assert!(segments.contains(&(3, 0)), "the outer ring closes");
    assert!(segments.contains(&(7, 4)), "and so does the hole");
}

/// The unit vector of a lon/lat point, written out rather than borrowed.
///
/// `earthmesh_mesh` has one, and this crate deliberately does not depend on it
/// -- the layout doc puts the two side by side, both feeding `refine`. Six
/// lines here is the price of that, and it also means the yardstick below is
/// independent of the code it is checking.
fn unit(lon_degrees: f64, lat_degrees: f64) -> [f64; 3] {
    let (lon, lat) = (lon_degrees.to_radians(), lat_degrees.to_radians());
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}

/// `(a x b) . c > 0` exactly when a, b, c wind counter-clockwise seen from
/// outside the sphere. The textbook right-hand rule, and nothing this crate
/// computes goes into it.
fn winds_counter_clockwise(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> bool {
    let cross = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    cross[0] * c[0] + cross[1] * c[1] + cross[2] * c[2] > 0.0
}

/// `contains` reads the direction the right-hand rule defines, not some other one.
///
/// Everything else about orientation in this crate was checked as a
/// *composition*: build a model, ask whether a lake is outside it, and the
/// answer came out right. That leaves the possibility that the winding's
/// handedness and the sign of whatever orients the ring are both wrong and
/// cancel -- the pair works and either half alone misleads the next caller.
///
/// So this pins the half that lives here against an outside yardstick. A
/// triangle the right-hand rule calls counter-clockwise must contain its own
/// centroid, and reversed it must not.
#[test]
fn the_winding_convention_is_the_right_hand_rule() {
    let corners = [(0.0, 0.0), (10.0, 0.0), (5.0, 8.0)];
    let [a, b, c] = corners.map(|(lon, lat)| unit(lon, lat));
    assert!(
        winds_counter_clockwise(a, b, c),
        "the fixture itself must be counter-clockwise, or this proves nothing"
    );
    assert!(
        !winds_counter_clockwise(c, b, a),
        "and its reverse must not be"
    );

    let vertices: Vec<BoundaryVertex> =
        corners.iter().map(|&(lon, lat)| vertex(lon, lat)).collect();
    let model = |order: Vec<usize>| SphericalBoundaryModel {
        vertices: vertices.clone(),
        loops: vec![BoundaryLoop {
            loop_type: LoopType::Outer,
            role: BoundaryRole::HardDomain,
            vertices: order,
            parent: None,
        }],
    };
    // Inside the triangle by construction: a point near its centroid.
    let (inside_lon, inside_lat) = (5.0, 2.5);

    assert!(
        model(vec![0, 1, 2]).contains(inside_lon, inside_lat),
        "a counter-clockwise ring encloses what is on its left"
    );
    assert!(
        !model(vec![2, 1, 0]).contains(inside_lon, inside_lat),
        "and the same ring reversed encloses the rest of the sphere instead"
    );
}

/// `enclosing` supplies the orientation the ring itself cannot carry.
///
/// This is what `closed_rings` hands off to. Given the same vertices either way
/// round and a point the loop must contain, both come back oriented the same --
/// which is the property the caller wanted and the ring alone could never give.
#[test]
fn enclosing_orients_a_ring_whichever_way_it_arrives() {
    let vertices = square();
    let inside = (0.5, 0.5);

    let forward = BoundaryLoop::enclosing(
        LoopType::Outer,
        BoundaryRole::HardDomain,
        vec![0, 1, 2, 3],
        None,
        &vertices,
        inside,
    )
    .expect("a square encloses its middle");
    let backward = BoundaryLoop::enclosing(
        LoopType::Outer,
        BoundaryRole::HardDomain,
        vec![3, 2, 1, 0],
        None,
        &vertices,
        inside,
    )
    .expect("and so does the same square reversed");

    assert_eq!(
        forward.vertices(),
        backward.vertices(),
        "the same loop either way in"
    );
    let model = SphericalBoundaryModel {
        vertices,
        loops: vec![forward],
    };
    assert!(model.contains(inside.0, inside.1));
    assert!(!model.contains(40.0, 40.0), "and not the far side");
}

/// A point on the ring orients nothing, so the loop is refused.
///
/// Both directions "contain" a vertex of the ring -- the winding is undefined
/// there -- so there is no answer to give, and inventing one would pick a side
/// the caller never chose.
#[test]
fn enclosing_refuses_a_point_that_lies_on_the_ring() {
    let vertices = square();
    assert!(BoundaryLoop::enclosing(
        LoopType::Outer,
        BoundaryRole::HardDomain,
        vec![0, 1, 2, 3],
        None,
        &vertices,
        (0.0, 0.0),
    )
    .is_none());
}

/// A ring of fewer than three vertices encloses nothing, whatever the point.
#[test]
fn enclosing_refuses_a_degenerate_ring() {
    let vertices = square();
    assert!(BoundaryLoop::enclosing(
        LoopType::Outer,
        BoundaryRole::HardDomain,
        vec![0, 1],
        None,
        &vertices,
        (0.5, 0.5),
    )
    .is_none());
}

/// This crate's signed area is positive on a right-hand-rule ring.
///
/// Pinned against `(a x b) . c > 0` and not against the other area function in
/// the workspace, because the two disagree: `earthmesh_mesh`'s is negative
/// where this one is positive. Checking them against each other would make
/// either one's convention depend on the other staying put.
#[test]
fn the_area_sign_matches_the_winding_sign_here() {
    let corners = [(0.0_f64, 0.0_f64), (10.0, 0.0), (5.0, 8.0)];
    let [a, b, c] = corners.map(|(lon, lat)| unit(lon, lat));
    assert!(winds_counter_clockwise(a, b, c), "fixture");

    let vertices: Vec<BoundaryVertex> =
        corners.iter().map(|&(lon, lat)| vertex(lon, lat)).collect();
    let forward = signed_area_on_unit_sphere(&[0, 1, 2], &vertices).expect("area");
    let backward = signed_area_on_unit_sphere(&[2, 1, 0], &vertices).expect("area");

    assert!(
        forward > 0.0,
        "counter-clockwise is positive here: {forward}"
    );
    assert!(backward < 0.0, "and its reverse negative: {backward}");
    assert!((forward + backward).abs() < 1.0e-12, "same magnitude");
}

/// `bounding_smaller_side` picks the island, not the ocean around it.
///
/// The constructor a ring walked off a mesh reaches for. Both ways in must give
/// the same loop, and it must be the one that contains the small patch rather
/// than the rest of the globe -- which is what makes a lake inside it nest.
#[test]
fn bounding_smaller_side_picks_the_island_either_way_in() {
    let vertices = square();
    let build = |order: Vec<usize>| {
        BoundaryLoop::bounding_smaller_side(
            LoopType::Outer,
            BoundaryRole::HardDomain,
            order,
            None,
            &vertices,
        )
        .expect("a square has a smaller side")
    };
    let forward = build(vec![0, 1, 2, 3]);
    let backward = build(vec![3, 2, 1, 0]);
    assert_eq!(forward.vertices(), backward.vertices());

    let model = SphericalBoundaryModel {
        vertices: vertices.clone(),
        loops: vec![forward],
    };
    assert!(model.contains(0.5, 0.5), "the square's own middle");
    assert!(
        !model.contains(120.0, -40.0),
        "and not the far side of the globe"
    );
}

/// It agrees with `enclosing` where both apply.
///
/// Two constructors that answer the same question differently would be worse
/// than one: a caller picking by which name reads better would get a different
/// boundary. Where the caller has an interior point *and* the region is the
/// smaller side, both must produce the same loop.
#[test]
fn the_two_orienting_constructors_agree_where_both_apply() {
    let vertices = square();
    let by_area = BoundaryLoop::bounding_smaller_side(
        LoopType::Outer,
        BoundaryRole::HardDomain,
        vec![3, 2, 1, 0],
        None,
        &vertices,
    )
    .expect("area");
    let by_point = BoundaryLoop::enclosing(
        LoopType::Outer,
        BoundaryRole::HardDomain,
        vec![3, 2, 1, 0],
        None,
        &vertices,
        (0.5, 0.5),
    )
    .expect("point");
    assert_eq!(by_area.vertices(), by_point.vertices());
}
