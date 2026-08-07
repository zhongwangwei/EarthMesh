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
