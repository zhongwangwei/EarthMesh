use earthmesh_project::{
    transform_close_boundary, CloseBoundaryGeometry, CloseBoundaryMode, GeometryPoint, RegionShape,
};

fn square() -> Vec<GeometryPoint> {
    vec![
        GeometryPoint::new(100.0, 10.0),
        GeometryPoint::new(102.0, 10.0),
        GeometryPoint::new(102.0, 12.0),
        GeometryPoint::new(100.0, 12.0),
    ]
}

#[test]
fn close_boundary_mode_defaults_to_compatibility_polyline() {
    #[derive(serde::Deserialize)]
    struct Wrapper {
        #[serde(default)]
        boundary: CloseBoundaryMode,
    }

    let parsed: Wrapper = serde_yaml::from_str("{}").expect("default boundary parses");
    assert_eq!(parsed.boundary, CloseBoundaryMode::Polyline);

    let transformed =
        transform_close_boundary(&square(), &parsed.boundary).expect("polyline is unchanged");
    assert_eq!(
        transformed.geometry,
        CloseBoundaryGeometry::Polygon(square())
    );

    let shape: RegionShape =
        serde_yaml::from_str("!Close\npath: ./masks/domain_close.nml\nformat: Nml\n")
            .expect("compatibility close shape parses without boundary");
    let RegionShape::Close { boundary, .. } = shape else {
        panic!("expected close shape");
    };
    assert_eq!(boundary, CloseBoundaryMode::Polyline);
}

#[test]
fn close_boundary_engine_specs_round_trip() {
    let modes = [
        CloseBoundaryMode::Polyline,
        CloseBoundaryMode::SphericalChaikin {
            iterations: 2,
            max_segment_angle_deg: 0.25,
        },
        CloseBoundaryMode::EnclosingCap {
            margin_km: 20.0,
            max_radius_deg: 80.0,
            max_segment_angle_deg: 0.25,
        },
    ];

    for mode in modes {
        assert_eq!(
            CloseBoundaryMode::from_engine_spec(&mode.to_engine_spec())
                .expect("engine spec parses"),
            mode
        );
    }
}

#[test]
fn spherical_chaikin_smooths_and_densifies_a_closed_ring() {
    let transformed = transform_close_boundary(
        &square(),
        &CloseBoundaryMode::SphericalChaikin {
            iterations: 1,
            max_segment_angle_deg: 0.5,
        },
    )
    .expect("local square smooths");

    let CloseBoundaryGeometry::Polygon(points) = transformed.geometry else {
        panic!("Chaikin must remain a polygon");
    };
    assert!(points.len() >= 8);
    assert!(points
        .iter()
        .all(|point| point.lon.is_finite() && point.lat.is_finite()));
    assert!(transformed.report.output_area_km2 < transformed.report.input_area_km2);
}

#[test]
fn spherical_chaikin_is_antimeridian_safe() {
    let ring = vec![
        GeometryPoint::new(179.0, 10.0),
        GeometryPoint::new(-179.0, 10.0),
        GeometryPoint::new(-179.0, 12.0),
        GeometryPoint::new(179.0, 12.0),
    ];
    let transformed = transform_close_boundary(
        &ring,
        &CloseBoundaryMode::SphericalChaikin {
            iterations: 1,
            max_segment_angle_deg: 0.5,
        },
    )
    .expect("dateline ring smooths");
    let CloseBoundaryGeometry::Polygon(points) = transformed.geometry else {
        panic!("Chaikin must remain a polygon");
    };
    assert!(points
        .iter()
        .all(|point| (-180.0..=180.0).contains(&point.lon)));
}

#[test]
fn enclosing_cap_covers_every_original_vertex_with_margin() {
    let transformed = transform_close_boundary(
        &square(),
        &CloseBoundaryMode::EnclosingCap {
            margin_km: 10.0,
            max_radius_deg: 80.0,
            max_segment_angle_deg: 0.25,
        },
    )
    .expect("local square fits a cap");

    let CloseBoundaryGeometry::EnclosingCap { center, radius_km } = transformed.geometry else {
        panic!("enclosing-cap mode must produce a cap");
    };
    for point in square() {
        assert!(
            earthmesh_geometry::haversine_km(
                earthmesh_geometry::Point::new(point.lon, point.lat),
                earthmesh_geometry::Point::new(center.lon, center.lat),
            ) <= radius_km + 1.0e-8
        );
    }
    assert_eq!(transformed.report.radius_km, Some(radius_km));
    assert!(transformed.report.output_area_km2 > transformed.report.input_area_km2);
}

#[test]
fn transformed_modes_reject_self_intersections_and_antipodal_edges() {
    let bow_tie = vec![
        GeometryPoint::new(0.0, 0.0),
        GeometryPoint::new(2.0, 2.0),
        GeometryPoint::new(0.0, 2.0),
        GeometryPoint::new(2.0, 0.0),
    ];
    let smooth = CloseBoundaryMode::SphericalChaikin {
        iterations: 1,
        max_segment_angle_deg: 0.5,
    };
    assert!(transform_close_boundary(&bow_tie, &smooth)
        .unwrap_err()
        .contains("self-intersect"));

    let antipodal = vec![
        GeometryPoint::new(0.0, 0.0),
        GeometryPoint::new(180.0, 0.0),
        GeometryPoint::new(90.0, 20.0),
    ];
    assert!(transform_close_boundary(&antipodal, &smooth)
        .unwrap_err()
        .contains("antipodal"));
}
