use super::super::*;

const EARTH_RADIUS_METERS: f64 = 6_371_229.0;

fn base_mesh() -> MethodCDelaunayMesh {
    MethodCDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh")
}

/// Great-circle distance in meters between two (lon, lat) degree points on the
/// Earth-radius sphere (matches the Method-C region containment convention).
fn gc_distance_m(lon1: f64, lat1: f64, lon2: f64, lat2: f64) -> f64 {
    let (l1, p1) = (lon1.to_radians(), lat1.to_radians());
    let (l2, p2) = (lon2.to_radians(), lat2.to_radians());
    let cos_angle = p1.sin() * p2.sin() + p1.cos() * p2.cos() * (l1 - l2).cos();
    cos_angle.clamp(-1.0, 1.0).acos() * EARTH_RADIUS_METERS
}

/// Quantized target-level closure for concentric circular demand: level 2
/// inside `inner_m`, level 1 inside `outer_m`, level 0 elsewhere.
fn two_ring_levels(inner_m: f64, outer_m: f64) -> impl Fn(f64, f64) -> u8 {
    move |lon: f64, lat: f64| {
        let d = gc_distance_m(lon, lat, 115.0, 25.0);
        if d <= inner_m {
            2
        } else if d <= outer_m {
            1
        } else {
            0
        }
    }
}

fn face_centroid_lonlat(mesh: &MethodCDelaunayMesh, iw: usize) -> LonLatDegrees {
    let face = mesh.w_faces[iw];
    let [im1, im2, im3] = face.im;
    let p1 = mesh.m_points[im1];
    let p2 = mesh.m_points[im2];
    let p3 = mesh.m_points[im3];
    xyz_to_lonlat_degrees(CartesianPoint::new(
        (p1.x + p2.x + p3.x) / 3.0,
        (p1.y + p2.y + p3.y) / 3.0,
        (p1.z + p2.z + p3.z) / 3.0,
    ))
}

#[test]
fn edge_midpoint_demand_is_not_missed_between_hfield_vertex_samples() {
    let mesh = base_mesh();
    let edge = (2..=mesh.nud)
        .map(|iu| (iu, mesh.u_edges[iu]))
        .find(|(_, edge)| {
            edge.im
                .iter()
                .all(|im| *im > 1 && *im <= mesh.nmd && !mesh.impent.contains(im))
        })
        .expect("non-pentagon active U edge");
    let [im1, im2] = edge.1.im;
    let p1 = mesh.m_points[im1];
    let p2 = mesh.m_points[im2];
    let midpoint = xyz_to_lonlat_degrees(CartesianPoint::new(
        0.5 * (p1.x + p2.x),
        0.5 * (p1.y + p2.y),
        0.5 * (p1.z + p2.z),
    ));
    let nearest_vertex_m = (2..=mesh.nmd)
        .map(|im| {
            let point = xyz_to_lonlat_degrees(mesh.m_points[im]);
            gc_distance_m(
                midpoint.lon_degrees,
                midpoint.lat_degrees,
                point.lon_degrees,
                point.lat_degrees,
            )
        })
        .fold(f64::INFINITY, f64::min);
    let radius_m = 0.25 * nearest_vertex_m;
    let demand = |lon: f64, lat: f64| {
        u8::from(gc_distance_m(lon, lat, midpoint.lon_degrees, midpoint.lat_degrees) <= radius_m)
    };

    assert!(
        (2..=mesh.nmd).all(|im| {
            let point = xyz_to_lonlat_degrees(mesh.m_points[im]);
            demand(point.lon_degrees, point.lat_degrees) == 0
        }),
        "fixture demand must be invisible at every M vertex"
    );

    let selected = mesh
        .selected_faces_from_target_levels(&demand, 1, false)
        .expect("edge-aware h-field selection");
    assert!(
        selected.iter().skip(2).any(|selected| *selected),
        "edge midpoint demand must select a Method-C footprint"
    );
    assert_eq!(
        selected,
        mesh.selected_faces_from_target_levels(&demand, 1, false)
            .expect("deterministic edge-aware selection")
    );
}

#[test]
fn disconnected_hfield_demands_select_every_component() {
    let mesh = base_mesh();
    let first = (2..=mesh.nmd)
        .find(|im| !mesh.impent.contains(im))
        .expect("regular M point");
    let first_point = xyz_to_lonlat_degrees(mesh.m_points[first]);
    let second = (2..=mesh.nmd)
        .filter(|im| !mesh.impent.contains(im))
        .max_by(|a, b| {
            let distance = |im| {
                let point = xyz_to_lonlat_degrees(mesh.m_points[im]);
                gc_distance_m(
                    first_point.lon_degrees,
                    first_point.lat_degrees,
                    point.lon_degrees,
                    point.lat_degrees,
                )
            };
            distance(*a).total_cmp(&distance(*b))
        })
        .expect("distant regular M point");
    let second_point = xyz_to_lonlat_degrees(mesh.m_points[second]);
    let mut nearest_spacing = f64::INFINITY;
    for center in [first, second] {
        let center = xyz_to_lonlat_degrees(mesh.m_points[center]);
        for im in 2..=mesh.nmd {
            let point = xyz_to_lonlat_degrees(mesh.m_points[im]);
            let distance = gc_distance_m(
                center.lon_degrees,
                center.lat_degrees,
                point.lon_degrees,
                point.lat_degrees,
            );
            if distance > 0.0 {
                nearest_spacing = nearest_spacing.min(distance);
            }
        }
    }
    let radius_m = 0.25 * nearest_spacing;
    let demand = |lon: f64, lat: f64| {
        u8::from(
            gc_distance_m(lon, lat, first_point.lon_degrees, first_point.lat_degrees) <= radius_m
                || gc_distance_m(lon, lat, second_point.lon_degrees, second_point.lat_degrees)
                    <= radius_m,
        )
    };

    let selected = mesh
        .selected_faces_from_target_levels(&demand, 1, false)
        .expect("disconnected h-field selection");
    for center in [first, second] {
        let neighbors = mesh.m_neighbors[center];
        assert!(
            neighbors
                .iw
                .iter()
                .take(neighbors.npoly)
                .any(|iw| selected[*iw]),
            "demand component at M point {center} was dropped"
        );
    }
}

#[test]
fn pentagon_only_hfield_demand_is_not_a_noop() {
    let mesh = base_mesh();
    let pentagon = mesh.impent[0];
    let center = xyz_to_lonlat_degrees(mesh.m_points[pentagon]);
    let nearest_other_m = (2..=mesh.nmd)
        .filter(|im| *im != pentagon)
        .map(|im| {
            let point = xyz_to_lonlat_degrees(mesh.m_points[im]);
            gc_distance_m(
                center.lon_degrees,
                center.lat_degrees,
                point.lon_degrees,
                point.lat_degrees,
            )
        })
        .fold(f64::INFINITY, f64::min);
    let radius_m = 0.25 * nearest_other_m;
    let demand = |lon: f64, lat: f64| {
        u8::from(gc_distance_m(lon, lat, center.lon_degrees, center.lat_degrees) <= radius_m)
    };

    let selected = mesh
        .selected_faces_from_target_levels(&demand, 1, false)
        .expect("pentagon h-field selection");
    assert!(selected.iter().skip(2).any(|selected| *selected));
}

#[test]
fn single_level_field_spawn_matches_region_spawn_footprint() {
    let mesh = base_mesh();
    let radius_m = 2_500_000.0;
    let field = |lon: f64, lat: f64| {
        if gc_distance_m(lon, lat, 115.0, 25.0) <= radius_m {
            1u8
        } else {
            0u8
        }
    };

    let from_field = mesh
        .spawn_nest_from_target_levels(field, 1, MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE)
        .expect("h-field spawn");
    from_field
        .validate_topology()
        .expect("field spawn topology");
    assert!(from_field.nwd > mesh.nwd, "field spawn must refine faces");

    let region = MethodCRefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: radius_m,
        level: 1,
    };
    let from_region = mesh
        .spawn_nest_with_max_mrows(
            &[region],
            1,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
        )
        .expect("region spawn");

    // Same demand expressed two ways: footprints are built differently
    // (centroid mask vs seed BFS + rad3) so counts differ by boundary rows at
    // most, not by scale.
    let added_field = from_field.nwd - mesh.nwd;
    let added_region = from_region.nwd - mesh.nwd;
    assert!(
        added_field * 2 >= added_region && added_region * 2 >= added_field,
        "footprints diverged: field added {added_field}, region added {added_region}"
    );

    // Every refined face stays near the demanded circle: selection radius plus
    // the transition apron (mrow rows of ~coarse-cell size).
    let apron_m = (MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE as f64 + 2.0) * 1_500_000.0;
    let mut refined_faces = 0usize;
    for iw in 2..=from_field.nwd {
        if from_field.w_faces[iw].ngr >= 2 {
            refined_faces += 1;
            let c = face_centroid_lonlat(&from_field, iw);
            let d = gc_distance_m(c.lon_degrees, c.lat_degrees, 115.0, 25.0);
            assert!(
                d <= radius_m + apron_m,
                "refined face {iw} strayed {d} m from the demand circle"
            );
        }
    }
    assert!(
        refined_faces > 0,
        "expected ngr >= 2 faces after field spawn"
    );

    // Determinism: identical closure, identical mesh.
    let again = mesh
        .spawn_nest_from_target_levels(field, 1, MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE)
        .expect("h-field spawn rerun");
    assert_eq!(again.nmd, from_field.nmd);
    assert_eq!(again.nud, from_field.nud);
    assert_eq!(again.nwd, from_field.nwd);
    for iw in 2..=from_field.nwd {
        assert_eq!(
            again.w_faces[iw].ngr, from_field.w_faces[iw].ngr,
            "face {iw}"
        );
    }
}

#[test]
fn two_level_field_spawn_nests_and_deeper_passes_stop_cleanly() {
    let mesh = base_mesh();
    let inner_m = 1_000_000.0;
    let outer_m = 4_000_000.0;

    // Feasibility oracle: the identical demand expressed as regions must be
    // spawnable by the standard path on this fixture. If THIS expect fires, the
    // fixture geometry (not the h-field selection) is infeasible at NXP 6.
    let regions = [
        MethodCRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: outer_m,
            level: 1,
        },
        MethodCRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: inner_m,
            level: 2,
        },
    ];
    let from_region = mesh
        .spawn_nest_with_max_mrows(&regions, 2, MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE)
        .expect("region-mode two-level spawn must be feasible on this fixture");

    let field = two_ring_levels(inner_m, outer_m);
    let two = mesh
        .spawn_nest_from_target_levels(&field, 2, MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE)
        .expect("two-level h-field spawn");

    // Same demand, two expressions: footprints agree to within boundary rows.
    let added_field = two.nwd - mesh.nwd;
    let added_region = from_region.nwd - mesh.nwd;
    assert!(
        added_field * 2 >= added_region && added_region * 2 >= added_field,
        "two-level footprints diverged: field added {added_field}, region added {added_region}"
    );
    two.validate_topology().expect("two-level topology");

    let max_ngr = (2..=two.nwd).map(|iw| two.w_faces[iw].ngr).max().unwrap();
    assert_eq!(max_ngr, 3, "level-2 demand must produce ngr == 3 faces");

    // Inner-generation faces hug the inner circle (child cells are ~half the
    // coarse size, so the apron is tighter).
    let child_apron_m = (MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE as f64 + 2.0) * 800_000.0;
    for iw in 2..=two.nwd {
        if two.w_faces[iw].ngr == 3 {
            let c = face_centroid_lonlat(&two, iw);
            let d = gc_distance_m(c.lon_degrees, c.lat_degrees, 115.0, 25.0);
            assert!(
                d <= 1_000_000.0 + child_apron_m,
                "ngr-3 face {iw} strayed {d} m from the inner demand"
            );
        }
    }

    // Asking for deeper passes than the field demands stops cleanly after the
    // demanded depth: identical output tables.
    let five = mesh
        .spawn_nest_from_target_levels(&field, 5, MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE)
        .expect("max_level 5 h-field spawn");
    assert_eq!(five.nmd, two.nmd);
    assert_eq!(five.nud, two.nud);
    assert_eq!(five.nwd, two.nwd);
}

#[test]
fn empty_field_is_identity_and_zero_level_rejected_paths() {
    let mesh = base_mesh();
    let silent = |_lon: f64, _lat: f64| 0u8;
    let out = mesh
        .spawn_nest_from_target_levels(silent, 3, MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE)
        .expect("empty field spawn");
    assert_eq!(out.nmd, mesh.nmd);
    assert_eq!(out.nud, mesh.nud);
    assert_eq!(out.nwd, mesh.nwd);
    for im in 2..=mesh.nmd {
        assert_eq!(
            out.m_points[im].x.to_bits(),
            mesh.m_points[im].x.to_bits(),
            "identity spawn must not move point {im}"
        );
    }

    assert!(
        mesh.spawn_nest_from_target_levels(|_, _| 1u8, 1, 0)
            .is_err(),
        "max_mrows == 0 must be rejected"
    );
}
