use super::*;

use earthmesh_mesh::{MeshState, TriangularMesh, MESH_STATE_FIRST_ID};

fn state(nxp: usize) -> MeshState {
    let mesh = TriangularMesh::from_icosahedron(nxp, 0, 1.0, 0.25).expect("base mesh");
    MeshState::from_triangular_mesh(&mesh).expect("neutral state")
}

fn view<'a>(state: &'a MeshState, cell: &'a earthmesh_mesh::VoronoiCell) -> CellView<'a> {
    CellView {
        site: cell.site,
        cell,
        state,
        radius_m: state.sphere_radius(),
    }
}

/// The cells' areas add to the sphere's, so the scale is measured in the units
/// it claims.
///
/// A cell area computed on the unit sphere and reported as square metres is the
/// same class of mistake as a unit vector inserted into a mesh in metres: every
/// per-cell number looks plausible and the totals are wrong by 10^13.
#[test]
fn cell_areas_are_in_square_metres_and_add_up_to_the_sphere() {
    let state = state(6);
    let radius = state.sphere_radius();
    let total: f64 = (MESH_STATE_FIRST_ID..state.vertices().len())
        .map(|site| {
            let cell = state.voronoi_cell(site).expect("cell");
            view(&state, &cell).area_m2().expect("area")
        })
        .sum();
    let sphere = 4.0 * std::f64::consts::PI * radius * radius;
    assert!(
        (total - sphere).abs() / sphere < 1.0e-9,
        "cells total {total} and the sphere is {sphere}"
    );
}

/// The effective scale is the radius of the disc with the cell's area.
#[test]
fn the_effective_scale_is_the_radius_of_a_disc_with_the_same_area() {
    let state = state(6);
    for site in [7usize, 40, 300] {
        let cell = state.voronoi_cell(site).expect("cell");
        let view = view(&state, &cell);
        let area = view.area_m2().expect("area");
        let scale = view.effective_scale_m().expect("scale");
        assert!((std::f64::consts::PI * scale * scale - area).abs() / area < 1.0e-12);
    }
}

/// A cell already at the target asks for nothing.
#[test]
fn a_cell_that_meets_the_target_is_satisfied() {
    let state = state(6);
    let cell = state.voronoi_cell(40).expect("cell");
    let view = view(&state, &cell);
    let scale = view.effective_scale_m().expect("scale");

    let criterion = TargetScale {
        id: "target".to_string(),
        target_scale_m: scale * 2.0,
        region: TargetRegion::Global,
        source_resolution_m: None,
    };
    let evidence = criterion.evaluate(&view).expect("evaluate");
    assert!(!evidence.demands_work());
}

/// A cell over the target asks, and says by how much.
#[test]
fn a_cell_over_the_target_reports_the_measurement_and_the_threshold() {
    let state = state(6);
    let cell = state.voronoi_cell(40).expect("cell");
    let view = view(&state, &cell);
    let scale = view.effective_scale_m().expect("scale");

    let criterion = TargetScale {
        id: "target".to_string(),
        target_scale_m: scale / 2.0,
        region: TargetRegion::Global,
        source_resolution_m: None,
    };
    let evidence = criterion.evaluate(&view).expect("evaluate");
    assert!(evidence.demands_work());
    assert!((evidence.measured_value - scale).abs() < 1.0e-6);
    assert!((evidence.threshold - scale / 2.0).abs() < 1.0e-6);
    assert!((evidence.normalized_violation - 1.0).abs() < 1.0e-9);
    assert_eq!(evidence.witness, Some(view.centre()));
}

/// A cell outside the region the target applies to is satisfied, whatever its
/// size.
#[test]
fn a_cell_outside_the_region_is_satisfied() {
    let state = state(6);
    let cell = state.voronoi_cell(40).expect("cell");
    let view = view(&state, &cell);

    let criterion = TargetScale {
        id: "target".to_string(),
        target_scale_m: 1.0,
        region: TargetRegion::Circle {
            // Antipodal to nowhere in particular, with a reach far too small
            // to hold the cell being asked about.
            centre: LonLatDegrees::new(
                view.centre().lon_degrees + 180.0,
                -view.centre().lat_degrees,
            ),
            radius_m: 1000.0,
        },
        source_resolution_m: None,
    };
    assert!(!criterion.evaluate(&view).expect("evaluate").demands_work());
}

#[test]
fn target_scale_exposes_the_same_size_field_the_criterion_evaluates() {
    let state = state(6);
    let cell = state.voronoi_cell(40).expect("cell");
    let view = view(&state, &cell);
    let centre = view.centre();
    let criterion = TargetScale {
        id: "target".to_string(),
        target_scale_m: 12_500.0,
        region: TargetRegion::Circle {
            centre,
            radius_m: 1_000.0,
        },
        source_resolution_m: None,
    };

    assert_eq!(
        criterion.target_scale_m_at(centre, state.sphere_radius()),
        Some(12_500.0)
    );
    assert_eq!(
        criterion.target_scale_m_at(
            LonLatDegrees::new(centre.lon_degrees + 180.0, -centre.lat_degrees),
            state.sphere_radius(),
        ),
        None
    );
}

#[test]
fn an_unsatisfiable_target_is_not_exposed_to_the_optimizer() {
    let criterion = TargetScale {
        id: "below-source-resolution".to_string(),
        target_scale_m: 100.0,
        region: TargetRegion::Global,
        source_resolution_m: Some(200.0),
    };
    assert_eq!(
        criterion.target_scale_m_at(LonLatDegrees::new(0.0, 0.0), 1.0),
        None
    );
}

#[test]
fn triangle_eta_is_one_for_an_equilateral_triangle_and_falls_with_distortion() {
    let latitude_ring =
        |longitude: f64| lonlat_degrees_to_unit_xyz(LonLatDegrees::new(longitude, 60.0));
    let equilateral = triangle_eta([
        latitude_ring(0.0),
        latitude_ring(120.0),
        latitude_ring(-120.0),
    ])
    .expect("eta");
    let distorted = triangle_eta([
        latitude_ring(0.0),
        latitude_ring(25.0),
        latitude_ring(155.0),
    ])
    .expect("eta");

    assert!((equilateral - 1.0).abs() < 1.0e-12);
    assert!(distorted < equilateral);
}

#[test]
fn indexed_circles_are_one_target_with_the_same_great_circle_membership() {
    let state = state(6);
    let cell = state.voronoi_cell(40).expect("cell");
    let view = view(&state, &cell);
    let centre = view.centre();
    let criterion = TargetScale {
        id: "adaptive-level".to_string(),
        target_scale_m: 1.0,
        region: TargetRegion::circles(vec![
            RefinementRegion::Circle {
                center: LonLatDegrees::new(centre.lon_degrees + 180.0, -centre.lat_degrees),
                radius_meters: 1_000.0,
                level: 1,
            },
            RefinementRegion::Circle {
                center: centre,
                radius_meters: 1_000.0,
                level: 1,
            },
        ]),
        source_resolution_m: None,
    };

    assert!(criterion.evaluate(&view).expect("evaluate").demands_work());
}

/// A target finer than the data behind it is reported unsatisfiable, not
/// pursued.
///
/// Refining past what a source can resolve gives a finer mesh carrying no more
/// information -- and it succeeds, which is what makes it worth naming.
#[test]
fn a_target_below_the_source_resolution_is_unsatisfiable() {
    let state = state(6);
    let cell = state.voronoi_cell(40).expect("cell");
    let view = view(&state, &cell);
    let scale = view.effective_scale_m().expect("scale");

    let criterion = TargetScale {
        id: "target".to_string(),
        target_scale_m: scale / 4.0,
        region: TargetRegion::Global,
        source_resolution_m: Some(scale / 2.0),
    };
    let evidence = criterion.evaluate(&view).expect("evaluate");
    assert!(!evidence.satisfiable);
    assert_eq!(
        evidence.stop_reason,
        Some(EvidenceStopReason::SourceResolutionReached)
    );
    assert!(
        !evidence.demands_work(),
        "an unsatisfiable demand is not work to be scheduled"
    );
}
