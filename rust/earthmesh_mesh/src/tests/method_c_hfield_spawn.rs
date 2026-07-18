use super::super::*;

fn base_mesh() -> MethodCDelaunayMesh {
    MethodCDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh")
}

/// Great-circle distance in meters between two (lon, lat) degree points.
/// Haversine keeps identical sample points exactly at zero distance.
fn gc_distance_m(lon1: f64, lat1: f64, lon2: f64, lat2: f64) -> f64 {
    let (l1, p1) = (lon1.to_radians(), lat1.to_radians());
    let (l2, p2) = (lon2.to_radians(), lat2.to_radians());
    let sin_dlat = (0.5 * (p2 - p1)).sin();
    let sin_dlon = (0.5 * (l2 - l1)).sin();
    let a = sin_dlat * sin_dlat + p1.cos() * p2.cos() * sin_dlon * sin_dlon;
    2.0 * a.sqrt().asin() * earthmesh_core::EARTH_RADIUS_METERS
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
fn canonical_thirdm_lattice_rad3_footprints_cover_active_base_faces() {
    for nxp in [6, 12, 80] {
        let mesh = MethodCDelaunayMesh::from_icosahedron(nxp, 0, 1.0, 0.25, 100)
            .expect("base Method-C mesh");
        let neighbors = mesh.method_c_m_neighbors().expect("M neighbors");
        let start = mesh.impent[0];
        let mut jdone = vec![[false; 6]; mesh.nmd + 1];
        let mut seen = vec![false; mesh.nmd + 1];
        let mut stack = vec![start];
        seen[start] = true;
        let mut covered = vec![false; mesh.nwd + 1];
        while let Some(im) = stack.pop() {
            mesh.mark_fill_rad3_faces_with_neighbors(im, &mut covered, &neighbors)
                .expect("rad3 footprint");
            for next in mesh
                .method_c_thirdm_neighbors_canonical_with_neighbors(im, &mut jdone, &neighbors)
                .expect("thirdm neighbors")
            {
                if !seen[next] {
                    seen[next] = true;
                    stack.push(next);
                }
            }
        }
        let missing = (2..=mesh.nwd).filter(|&iw| !covered[iw]).count();
        assert_eq!(missing, 0, "NXP {nxp} thirdm/rad3 coverage gap");
    }
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
fn cartesian_hfield_anchors_its_stride_phase_without_a_spherical_pentagon() {
    let mesh = MethodCDelaunayMesh::from_cart_hex(6, 1_000_000.0).expect("Cartesian Method-C mesh");
    assert!(
        mesh.impent.iter().all(|&im| im == 1),
        "cart_hex intentionally has no spherical pentagon anchor"
    );
    let demand = |x: f64, y: f64| {
        u8::from((-2_000_000.0..=2_000_000.0).contains(&x) && y.abs() <= 1_000_000.0)
    };
    let selected = mesh
        .selected_faces_from_target_levels(&demand, 1, true)
        .expect("Cartesian HField should anchor its local stride-3 lattice in the demand");

    assert!(selected.iter().skip(2).any(|&selected| selected));
    assert!((2..=mesh.nwd)
        .filter(|&iw| selected[iw])
        .all(|iw| mesh.method_c_w_face_is_active(iw)));
}

fn first_materializable_cartesian_rad3_seed(
    mesh: &MethodCDelaunayMesh,
    neighbors: &[IcosahedronMPointNeighbors],
) -> (usize, usize, usize, usize, usize) {
    let seed = (2..=mesh.nmd)
        .find(|&im| {
            mesh.m_prognostic[im] == im
                && matches!(
                    mesh.hfield_rad3_faces_for_test(im, neighbors, true),
                    Ok(Some(_))
                )
        })
        .expect("non-seam Cartesian rad3 seed");
    let sector_iw = neighbors[seed].iw[0];
    let sector = mesh.w_faces[sector_iw];
    let (imx, outer_iw, outer_slot) = if seed == sector.im[0] {
        (sector.im[1], sector.iw[3], 3)
    } else if seed == sector.im[1] {
        (sector.im[2], sector.iw[5], 5)
    } else {
        assert_eq!(seed, sector.im[2]);
        (sector.im[0], sector.iw[7], 7)
    };
    (seed, sector_iw, imx, outer_iw, outer_slot)
}

#[test]
fn cartesian_hfield_does_not_hide_non_seam_rad3_corruption() {
    let mut mesh =
        MethodCDelaunayMesh::from_cart_hex(6, 1_000_000.0).expect("Cartesian Method-C mesh");
    let neighbors = mesh.method_c_m_neighbors().expect("M neighbors");
    let (seed, _, imx, outer_iw, _) = first_materializable_cartesian_rad3_seed(&mesh, &neighbors);
    let replacement = (2..=mesh.nmd)
        .find(|&im| mesh.m_prognostic[im] == im && !mesh.w_faces[outer_iw].im.contains(&im))
        .expect("unrelated active M point");
    let vertex = mesh.w_faces[outer_iw]
        .im
        .iter()
        .position(|&im| im == imx)
        .expect("outer face contains sector successor");
    mesh.w_faces[outer_iw].im[vertex] = replacement;

    let error = mesh
        .hfield_rad3_faces_for_test(seed, &neighbors, true)
        .expect_err("non-seam topology corruption must remain fatal");
    assert!(
        error.to_string().contains("fill_rad3"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn cartesian_hfield_rejects_a_non_seam_outer_link_redirected_to_a_ghost() {
    let mut mesh =
        MethodCDelaunayMesh::from_cart_hex(6, 1_000_000.0).expect("Cartesian Method-C mesh");
    let neighbors = mesh.method_c_m_neighbors().expect("M neighbors");
    let (seed, sector_iw, imx, original_outer_iw, outer_slot) =
        first_materializable_cartesian_rad3_seed(&mesh, &neighbors);
    let ghost_iw = (2..=mesh.nwd)
        .find(|&iw| {
            mesh.w_prognostic[iw] != iw
                && iw != original_outer_iw
                && !mesh.w_faces[iw].im.contains(&imx)
        })
        .expect("unrelated Cartesian periodic ghost W face");
    mesh.w_faces[sector_iw].iw[outer_slot] = ghost_iw;

    let error = mesh
        .hfield_rad3_faces_for_test(seed, &neighbors, true)
        .expect_err("an arbitrary ghost outer link must not be classified as a periodic seam");
    assert!(
        error.to_string().contains("fill_rad3"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn cartesian_hfield_rejects_self_consistent_ghost_inner_and_outer_redirects() {
    let mut mesh =
        MethodCDelaunayMesh::from_cart_hex(6, 1_000_000.0).expect("Cartesian Method-C mesh");
    let neighbors = mesh.method_c_m_neighbors().expect("M neighbors");
    let (seed, sector_iw, imx, original_outer_iw, outer_slot) =
        first_materializable_cartesian_rad3_seed(&mesh, &neighbors);
    let original_pair = [
        original_outer_iw,
        mesh.w_faces[sector_iw].iw[outer_slot + 1],
    ];
    let (fake_inner_iw, fake_pair) = (2..=mesh.nwd)
        .filter(|&iw| mesh.w_prognostic[iw] != iw)
        .find_map(|inner_iw| {
            let inner = mesh.w_faces[inner_iw];
            let pair =
                tri_neighbors_outer_w_pair(sector_iw, [inner.iw[0], inner.iw[1], inner.iw[2]]);
            let ids_are_valid = pair
                .iter()
                .all(|&iw| iw > 1 && iw <= mesh.nwd && !mesh.w_faces[iw].im.contains(&imx));
            let touches_periodic_copy = pair.iter().any(|&iw| {
                mesh.w_prognostic[iw] != iw
                    || mesh.w_faces[iw]
                        .im
                        .iter()
                        .any(|&im| mesh.m_prognostic[im] != im)
            });
            (ids_are_valid && touches_periodic_copy && pair != original_pair)
                .then_some((inner_iw, pair))
        })
        .expect("self-consistent but unrelated ghost inner/outer links");
    let inner_slot = (outer_slot - 3) / 2;
    mesh.w_faces[sector_iw].iw[inner_slot] = fake_inner_iw;
    mesh.w_faces[sector_iw].iw[outer_slot] = fake_pair[0];
    mesh.w_faces[sector_iw].iw[outer_slot + 1] = fake_pair[1];

    let error = mesh
        .hfield_rad3_faces_for_test(seed, &neighbors, true)
        .expect_err("stored ghost W links must not override reciprocal U-edge topology");
    assert!(
        error.to_string().contains("fill_rad3"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn mixed_point_and_edge_midpoint_corridor_is_covered_without_truncation() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(18, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let first = (2..=mesh.nmd)
        .find(|im| !mesh.impent.contains(im))
        .expect("regular M point");
    let first_ll = xyz_to_lonlat_degrees(mesh.m_points[first]);
    let last = (2..=mesh.nmd)
        .filter(|im| !mesh.impent.contains(im))
        .max_by(|a, b| {
            let distance = |im| {
                let point = xyz_to_lonlat_degrees(mesh.m_points[im]);
                gc_distance_m(
                    first_ll.lon_degrees,
                    first_ll.lat_degrees,
                    point.lon_degrees,
                    point.lat_degrees,
                )
            };
            distance(*a).total_cmp(&distance(*b))
        })
        .expect("distant regular M point");
    let mut previous = vec![0usize; mesh.nmd + 1];
    let mut queue = std::collections::VecDeque::from([first]);
    previous[first] = first;
    while let Some(im) = queue.pop_front() {
        if im == last {
            break;
        }
        let neighbors = mesh.m_neighbors[im];
        for &iu in neighbors.iu.iter().take(neighbors.npoly) {
            let edge = mesh.u_edges[iu];
            let next = if edge.im[0] == im {
                edge.im[1]
            } else {
                edge.im[0]
            };
            if next > 1 && previous[next] == 0 && !mesh.impent.contains(&next) {
                previous[next] = im;
                queue.push_back(next);
            }
        }
    }
    let mut path = vec![last];
    while *path.last().expect("path point") != first {
        path.push(previous[*path.last().expect("path point")]);
    }
    path.reverse();
    assert!(path.len() > 8);
    let midpoints = path
        .windows(2)
        .map(|pair| {
            let a = mesh.m_points[pair[0]];
            let b = mesh.m_points[pair[1]];
            xyz_to_lonlat_degrees(CartesianPoint::new(
                0.5 * (a.x + b.x),
                0.5 * (a.y + b.y),
                0.5 * (a.z + b.z),
            ))
        })
        .collect::<Vec<_>>();
    let nearest_vertex = midpoints
        .iter()
        .flat_map(|midpoint| {
            (2..=mesh.nmd).map(|im| {
                let point = xyz_to_lonlat_degrees(mesh.m_points[im]);
                gc_distance_m(
                    midpoint.lon_degrees,
                    midpoint.lat_degrees,
                    point.lon_degrees,
                    point.lat_degrees,
                )
            })
        })
        .fold(f64::INFINITY, f64::min);
    let radius_m = 0.15 * nearest_vertex;
    let demand = |lon: f64, lat: f64| {
        u8::from(
            gc_distance_m(lon, lat, first_ll.lon_degrees, first_ll.lat_degrees) <= radius_m
                || midpoints.iter().any(|point| {
                    gc_distance_m(lon, lat, point.lon_degrees, point.lat_degrees) <= radius_m
                }),
        )
    };
    assert_eq!(
        (2..=mesh.nmd)
            .filter(|&im| {
                let point = xyz_to_lonlat_degrees(mesh.m_points[im]);
                demand(point.lon_degrees, point.lat_degrees) > 0
            })
            .count(),
        1
    );

    let refined = mesh
        .spawn_nest_from_target_levels(demand, 1, MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE)
        .expect("a stride-compatible edge corridor must refine atomically");
    for demanded in std::iter::once(first_ll).chain(midpoints.iter().copied()) {
        let nearest = (2..=refined.nmd)
            .min_by(|a, b| {
                let distance = |im| {
                    let point = xyz_to_lonlat_degrees(refined.m_points[im]);
                    gc_distance_m(
                        demanded.lon_degrees,
                        demanded.lat_degrees,
                        point.lon_degrees,
                        point.lat_degrees,
                    )
                };
                distance(*a).total_cmp(&distance(*b))
            })
            .expect("nearest refined M point");
        assert!(
            refined.m_metadata[nearest].mrlm >= 2,
            "corridor demand at ({:.6}, {:.6}) was left at level {}",
            demanded.lon_degrees,
            demanded.lat_degrees,
            refined.m_metadata[nearest].mrlm,
        );
    }
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
    let refined = mesh
        .spawn_nest_from_target_levels(demand, 1, MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE)
        .expect("disconnected h-field components must refine independently");
    refined
        .validate_topology()
        .expect("disconnected h-field topology");
    assert!(refined.nwd > mesh.nwd);
    for demanded in [first_point, second_point] {
        let (nearest, distance_m) = (2..=refined.nmd)
            .map(|im| {
                let point = xyz_to_lonlat_degrees(refined.m_points[im]);
                (
                    im,
                    gc_distance_m(
                        demanded.lon_degrees,
                        demanded.lat_degrees,
                        point.lon_degrees,
                        point.lat_degrees,
                    ),
                )
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .expect("nearest refined M point");
        assert!(
            refined.m_metadata[nearest].mrlm >= 2,
            "HField demand at ({:.6}, {:.6}) was left at refinement level {} (nearest center {distance_m:.1} m away)",
            demanded.lon_degrees,
            demanded.lat_degrees,
            refined.m_metadata[nearest].mrlm,
        );
    }
}

#[test]
fn disconnected_hfield_preserves_every_demand_point() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(12, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let nearest_regular_m = |target_lon: f64, target_lat: f64| {
        (2..=mesh.nmd)
            .filter(|im| !mesh.impent.contains(im))
            .min_by(|a, b| {
                let distance = |im| {
                    let point = xyz_to_lonlat_degrees(mesh.m_points[im]);
                    gc_distance_m(target_lon, target_lat, point.lon_degrees, point.lat_degrees)
                };
                distance(*a).total_cmp(&distance(*b))
            })
            .expect("nearest regular M point")
    };
    let broad_center = xyz_to_lonlat_degrees(mesh.m_points[nearest_regular_m(100.0, 27.0)]);
    let isolated = [
        xyz_to_lonlat_degrees(mesh.m_points[nearest_regular_m(77.25, 43.25)]),
        xyz_to_lonlat_degrees(mesh.m_points[nearest_regular_m(139.75, 39.25)]),
    ];
    let isolated_radius_m = isolated
        .iter()
        .flat_map(|center| {
            (2..=mesh.nmd).filter_map(|im| {
                let point = xyz_to_lonlat_degrees(mesh.m_points[im]);
                let distance = gc_distance_m(
                    center.lon_degrees,
                    center.lat_degrees,
                    point.lon_degrees,
                    point.lat_degrees,
                );
                (distance > 0.0).then_some(distance)
            })
        })
        .fold(f64::INFINITY, f64::min)
        * 0.2;
    let demand = |lon: f64, lat: f64| {
        u8::from(
            gc_distance_m(lon, lat, broad_center.lon_degrees, broad_center.lat_degrees)
                <= 2_000_000.0
                || isolated.iter().any(|center| {
                    gc_distance_m(lon, lat, center.lon_degrees, center.lat_degrees)
                        <= isolated_radius_m
                }),
        )
    };
    let demanded_points = (2..=mesh.nmd)
        .filter_map(|im| {
            let point = xyz_to_lonlat_degrees(mesh.m_points[im]);
            (demand(point.lon_degrees, point.lat_degrees) > 0).then_some(point)
        })
        .collect::<Vec<_>>();
    assert!(
        demanded_points.len() > 20,
        "fixture needs a broad demand component"
    );
    for center in isolated {
        assert!(demand(center.lon_degrees, center.lat_degrees) > 0);
    }

    let refined = mesh
        .spawn_nest_from_target_levels(demand, 1, MethodCDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS)
        .expect("disconnected HField components must share one valid Method-C stride phase");
    for demanded in demanded_points {
        let (nearest, distance_m) = (2..=refined.nmd)
            .map(|im| {
                let point = xyz_to_lonlat_degrees(refined.m_points[im]);
                (
                    im,
                    gc_distance_m(
                        demanded.lon_degrees,
                        demanded.lat_degrees,
                        point.lon_degrees,
                        point.lat_degrees,
                    ),
                )
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .expect("nearest refined M point");
        assert!(
            refined.m_metadata[nearest].mrlm >= 2,
            "HField demand at ({:.6}, {:.6}) was silently left at refinement level {} (nearest center {distance_m:.1} m away)",
            demanded.lon_degrees,
            demanded.lat_degrees,
            refined.m_metadata[nearest].mrlm,
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
fn discontinuous_deeper_hfield_at_parent_boundary_is_rejected() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(18, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    // Equal radii make the level-1 branch unreachable, so this target jumps
    // directly from level 2 to 0. A gradient-limited HField never does this;
    // reject it rather than silently returning a partially satisfied mesh.
    let field = two_ring_levels(4_000_000.0, 4_000_000.0);
    let error = mesh
        .spawn_nest_from_target_levels(&field, 2, MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE)
        .expect_err("a discontinuous level-2 boundary must not be partially accepted");
    assert!(
        error.to_string().contains("parent boundary"),
        "unexpected rejection: {error}"
    );
}

#[test]
fn persisted_parent_advances_to_the_next_absolute_target_level() {
    let mesh = base_mesh();
    let parent = mesh
        .spawn_nest_from_target_levels(
            |lon, lat| u8::from(gc_distance_m(lon, lat, 115.0, 25.0) <= 4_000_000.0),
            1,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
        )
        .expect("level-1 parent");
    assert_eq!(
        (2..=parent.nwd).map(|iw| parent.w_faces[iw].mrlw).max(),
        Some(2)
    );

    let refined = parent
        .spawn_nest_from_target_levels(
            |lon, lat| {
                if gc_distance_m(lon, lat, 115.0, 25.0) <= 1_000_000.0 {
                    2
                } else {
                    0
                }
            },
            2,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
        )
        .expect("absolute level-2 target on persisted parent");
    assert_eq!(
        (2..=refined.nwd).map(|iw| refined.w_faces[iw].mrlw).max(),
        Some(3)
    );
}

#[test]
fn boundary_only_persisted_target_is_rejected_instead_of_silent_noop() {
    let mesh = base_mesh();
    let parent = mesh
        .spawn_nest_from_target_levels(
            |lon, lat| u8::from(gc_distance_m(lon, lat, 115.0, 25.0) <= 4_000_000.0),
            1,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
        )
        .expect("level-1 parent");
    let boundary = (2..=parent.nmd)
        .find(|&im| {
            parent.m_metadata[im].mrlm == 2
                && parent.m_neighbors[im]
                    .iu
                    .iter()
                    .take(parent.m_neighbors[im].npoly)
                    .any(|&iu| parent.u_edges[iu].mrlu != 2)
        })
        .expect("level-2 M point on the parent transition boundary");
    let center = xyz_to_lonlat_degrees(parent.m_points[boundary]);
    let nearest_m = (2..=parent.nmd)
        .filter(|&im| im != boundary)
        .map(|im| {
            let point = xyz_to_lonlat_degrees(parent.m_points[im]);
            gc_distance_m(
                center.lon_degrees,
                center.lat_degrees,
                point.lon_degrees,
                point.lat_degrees,
            )
        })
        .fold(f64::INFINITY, f64::min);
    let radius_m = 0.1 * nearest_m;
    let error = parent
        .spawn_nest_from_target_levels(
            |lon, lat| {
                if gc_distance_m(lon, lat, center.lon_degrees, center.lat_degrees) <= radius_m {
                    2
                } else {
                    0
                }
            },
            2,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
        )
        .expect_err("boundary-only target must not return an unchanged success");
    assert!(
        error.to_string().contains("parent transition boundary"),
        "{error}"
    );
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
