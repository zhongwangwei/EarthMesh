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

fn three_ring_levels(inner_m: f64, middle_m: f64, outer_m: f64) -> impl Fn(f64, f64) -> u8 {
    move |lon: f64, lat: f64| {
        let d = gc_distance_m(lon, lat, 115.0, 25.0);
        if d <= inner_m {
            3
        } else if d <= middle_m {
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
fn hfield_vertex_contact_closure_preserves_seed_atomicity() {
    let mesh = base_mesh();
    let neighbors = mesh.method_c_m_neighbors().expect("M neighbors");
    let footprints = (2..=mesh.nmd)
        .filter_map(|im| {
            mesh.method_c_rad3_faces_with_neighbors(im, &neighbors)
                .ok()
                .map(|faces| {
                    (
                        im,
                        faces
                            .into_iter()
                            .filter(|&iw| iw >= 2 && mesh.w_faces[iw].mrlw == 1)
                            .collect::<Vec<_>>(),
                    )
                })
        })
        .filter(|(_, faces)| !faces.is_empty())
        .collect::<Vec<_>>();
    let mut face_owner_seed = vec![0usize; mesh.nwd + 1];
    let mut owner_distance = vec![f64::INFINITY; mesh.nwd + 1];
    for (im, faces) in &footprints {
        let seed = mesh.m_points[*im];
        for &iw in faces {
            let face = mesh.w_faces[iw];
            let center = CartesianPoint::new(
                face.im.iter().map(|&mid| mesh.m_points[mid].x).sum::<f64>() / 3.0,
                face.im.iter().map(|&mid| mesh.m_points[mid].y).sum::<f64>() / 3.0,
                face.im.iter().map(|&mid| mesh.m_points[mid].z).sum::<f64>() / 3.0,
            );
            let distance = (seed.x - center.x).powi(2)
                + (seed.y - center.y).powi(2)
                + (seed.z - center.z).powi(2);
            if distance < owner_distance[iw]
                || (distance == owner_distance[iw] && *im < face_owner_seed[iw])
            {
                owner_distance[iw] = distance;
                face_owner_seed[iw] = *im;
            }
        }
    }
    let (initial_seeds, mut selected) = footprints
        .iter()
        .enumerate()
        .find_map(|(left_index, (left, left_faces))| {
            footprints
                .iter()
                .skip(left_index + 1)
                .find_map(|(right, right_faces)| {
                    let mut selected = vec![false; mesh.nwd + 1];
                    for &iw in left_faces.iter().chain(right_faces) {
                        selected[iw] = true;
                    }
                    (!mesh
                        .method_c_vertex_only_perimeter_contacts(&selected, &neighbors)
                        .ok()?
                        .is_empty())
                    .then_some(([*left, *right], selected))
                })
        })
        .expect("seed-union vertex contact fixture");
    let mut legal_seed = vec![0usize; mesh.nmd + 1];
    for (im, _) in &footprints {
        legal_seed[*im] = 1;
    }
    let mut selected_seeds = vec![0usize; mesh.nmd + 1];
    for im in initial_seeds {
        selected_seeds[im] = 1;
    }

    let added = mesh
        .close_hfield_seed_vertex_contacts(
            &mut selected,
            &mut selected_seeds,
            &legal_seed,
            &face_owner_seed,
            &neighbors,
        )
        .expect("seed-level contact closure");
    assert!(!added.is_empty());
    assert!(mesh
        .method_c_vertex_only_perimeter_contacts(&selected, &neighbors)
        .expect("closed contacts")
        .is_empty());
    let mut reconstructed = vec![false; mesh.nwd + 1];
    for (im, faces) in footprints {
        if selected_seeds[im] == legal_seed[im] {
            for iw in faces {
                reconstructed[iw] = true;
            }
        }
    }
    assert_eq!(selected, reconstructed);
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
fn deeper_point_demand_uses_phase_support_to_reach_a_canonical_seed() {
    let mesh = base_mesh();
    let neighbors = mesh.method_c_m_neighbors().expect("M neighbors");
    let mut canonical = vec![false; mesh.nmd + 1];
    let mut done = vec![[false; 6]; mesh.nmd + 1];
    let mut stack = vec![mesh.impent[0]];
    canonical[mesh.impent[0]] = true;
    while let Some(im) = stack.pop() {
        for next in mesh
            .method_c_thirdm_neighbors_canonical_with_neighbors(im, &mut done, &neighbors)
            .expect("canonical phase")
        {
            if !canonical[next] {
                canonical[next] = true;
                stack.push(next);
            }
        }
    }
    let demanded = (2..=mesh.nmd)
        .filter(|&im| !canonical[im] && !mesh.impent.contains(&im))
        .max_by(|&a, &b| {
            let distance_to_phase = |im| {
                let point = xyz_to_lonlat_degrees(mesh.m_points[im]);
                (2..=mesh.nmd)
                    .filter(|&candidate| canonical[candidate])
                    .map(|candidate| {
                        let phase = xyz_to_lonlat_degrees(mesh.m_points[candidate]);
                        gc_distance_m(
                            point.lon_degrees,
                            point.lat_degrees,
                            phase.lon_degrees,
                            phase.lat_degrees,
                        )
                    })
                    .fold(f64::INFINITY, f64::min)
            };
            distance_to_phase(a).total_cmp(&distance_to_phase(b))
        })
        .expect("non-canonical demand point");
    let center = xyz_to_lonlat_degrees(mesh.m_points[demanded]);
    let nearest_m = (2..=mesh.nmd)
        .filter(|&im| im != demanded)
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
    let nearest_phase = (2..=mesh.nmd)
        .filter(|&im| canonical[im])
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
    let demand = |lon: f64, lat: f64| {
        let distance = gc_distance_m(lon, lat, center.lon_degrees, center.lat_degrees);
        if distance <= 0.1 * nearest_m {
            2
        } else if distance < 0.9 * nearest_phase {
            1
        } else {
            0
        }
    };
    let (selected, diagnostics) = mesh
        .selected_faces_from_target_levels_with_policy_for_test(&demand, 1, false)
        .expect("intermediate pass selection");
    assert!(
        diagnostics.legal_rad3_seeds > 1,
        "phase support did not expose enough aligned owners for deeper demand"
    );
    let neighbors = mesh.m_neighbors[demanded];
    let selected_incident = neighbors
        .iw
        .iter()
        .take(neighbors.npoly)
        .filter(|&&iw| selected[iw])
        .count();
    assert_eq!(
        selected_incident, neighbors.npoly,
        "deeper demand at M point {demanded} was only partially reached from the canonical phase"
    );
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

#[test]
fn face_hard_demand_reuses_aligned_method_c_closure() {
    let mesh = base_mesh();
    let neighbors = mesh.method_c_m_neighbors().expect("M neighbors");
    let seed = (2..=mesh.nmd)
        .find(|&im| {
            !mesh.impent.contains(&im)
                && mesh
                    .method_c_rad3_faces_with_neighbors(im, &neighbors)
                    .is_ok()
        })
        .expect("materializable rad3 seed");
    let mut demand = vec![false; mesh.nwd + 1];
    for iw in mesh
        .method_c_rad3_faces_with_neighbors(seed, &neighbors)
        .expect("rad3 faces")
    {
        if iw >= 2 {
            demand[iw] = true;
        }
    }

    let refined = mesh
        .spawn_nest_pass_from_face_demands(
            &demand,
            1,
            2,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
        )
        .expect("face hard-demand spawn")
        .expect("non-empty face demand");
    assert!(refined.nwd > mesh.nwd);
    refined.validate_topology().expect("refined topology");

    assert!(mesh
        .spawn_nest_pass_from_face_demands(
            &vec![false; mesh.nwd + 1],
            1,
            2,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
        )
        .expect("empty face demand")
        .is_none());
}

#[test]
fn face_hard_demand_selection_checkpoint_is_deterministic() {
    let mesh = base_mesh();
    let neighbors = mesh.method_c_m_neighbors().expect("M neighbors");
    let seed = (2..=mesh.nmd)
        .find(|&im| {
            !mesh.impent.contains(&im)
                && mesh
                    .method_c_rad3_faces_with_neighbors(im, &neighbors)
                    .is_ok()
        })
        .expect("materializable rad3 seed");
    let mut demand = vec![false; mesh.nwd + 1];
    for iw in mesh
        .method_c_rad3_faces_with_neighbors(seed, &neighbors)
        .expect("rad3 faces")
    {
        if iw >= 2 {
            demand[iw] = true;
        }
    }

    let checkpoint = || {
        mesh.selection_checkpoint_from_target_levels_and_face_demands(|_, _| 0, &demand, 1, true)
            .expect("selection checkpoint")
    };
    let first = checkpoint();
    let second = checkpoint();

    assert_eq!(first, second);
    assert_eq!(first.face_demand, demand);
    assert_eq!(first.m_target_levels.len(), mesh.nmd + 1);
    assert_eq!(first.u_target_levels.len(), mesh.nud + 1);
    assert!(first.m_target_levels.iter().all(|&level| level == 0));
    assert!(first.u_target_levels.iter().all(|&level| level == 0));
    assert!(first
        .selected_faces
        .iter()
        .skip(2)
        .any(|&selected| selected));
    assert!(!first.demand_anchors.is_empty());
    first
        .validate_demand_coverage(&first.selected_faces)
        .expect("checkpoint demand coverage");
    let uncovered = first
        .validate_demand_coverage(&vec![false; mesh.nwd + 1])
        .expect_err("reject uncovered demand");
    assert_eq!(
        method_c_hfield_failure_kind(&uncovered),
        MethodCHfieldFailureKind::HardCoverage
    );
    assert!(!first.legal_seed_ids.is_empty());
    assert!(first
        .selected_seed_ids
        .iter()
        .all(|seed| first.legal_seed_ids.binary_search(seed).is_ok()));
    assert!(!first.selected_seed_ids.is_empty());
    assert_eq!(
        mesh.selected_faces_from_method_c_seed_ids(&first.selected_seed_ids)
            .expect("seed assignment"),
        first.selected_faces
    );

    let preflight = mesh
        .legalization_preflight_from_selected_faces(
            &first.selected_faces,
            &first.legal_seed_ids,
            &first.selected_seed_ids,
            2,
        )
        .expect("legalization preflight");
    assert!(preflight
        .perimeter_remainders
        .iter()
        .all(|&remainder| remainder == 0));
    assert_eq!(
        preflight.perimeter_candidate_seed_ids.len(),
        preflight.perimeter_lengths.len()
    );
    assert!(preflight
        .perimeter_candidate_seed_ids
        .iter()
        .all(|candidates| candidates.windows(2).all(|pair| pair[0] < pair[1])));
    let all_perimeter_components = (0..preflight.perimeter_lengths.len()).collect::<Vec<_>>();
    let perimeter_scope = preflight
        .current_perimeter_candidate_scope(&all_perimeter_components)
        .expect("perimeter candidate scope")
        .expect("current perimeter candidate census");
    assert!(perimeter_scope.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(perimeter_scope
        .iter()
        .all(|seed| first.legal_seed_ids.binary_search(seed).is_ok()));
    assert_eq!(
        preflight
            .current_perimeter_candidate_scope(&[])
            .expect("empty perimeter scope"),
        None
    );
    let mut legacy_preflight = preflight.clone();
    legacy_preflight.perimeter_candidate_seed_ids.clear();
    assert_eq!(
        legacy_preflight
            .current_perimeter_candidate_scope(&all_perimeter_components)
            .expect("legacy perimeter scope"),
        None
    );
    assert!(preflight.self_loop_witnesses.is_empty());
    assert!(preflight.witness_dependency_clusters.is_empty());
    assert!(preflight.patches.is_empty());
    let symbolic = mesh
        .legalization_symbolic_check(&first, &first.selected_seed_ids, 2)
        .expect("symbolic legalization check");
    assert_eq!(symbolic.perimeter_lengths, preflight.perimeter_lengths);
    assert_eq!(
        symbolic.perimeter_remainders,
        preflight.perimeter_remainders
    );
    assert_eq!(symbolic.vertex_only_contact_count, 0);
    assert_eq!(symbolic.predicted_transition_self_loop_count, Some(0));
    mesh.legalization_exact_materialization_check(
        &first,
        &first.selected_seed_ids,
        2,
        MethodCDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
    )
    .expect("exact legalization check");
    assert!(mesh
        .required_parent_support_lineages_from_seed_assignment(&first, &first.selected_seed_ids, 2,)
        .expect("seed-assignment parent support")
        .is_empty());

    let mutable_faces = mesh
        .selected_faces_from_method_c_seed_ids(&first.legal_seed_ids)
        .expect("legal seed footprints")
        .iter()
        .enumerate()
        .skip(2)
        .filter_map(|(iw, &selected)| selected.then_some(iw))
        .collect();
    let patch = MethodCHfieldLegalizationPatch {
        cluster_index: 0,
        witness_indices: Vec::new(),
        witness_perimeter_components: Vec::new(),
        perimeter_components: Vec::new(),
        perimeter_interfaces: Vec::new(),
        dependency_faces: Vec::new(),
        dependency_face_lineages: Vec::new(),
        candidate_seed_ids: first.legal_seed_ids.clone(),
        candidate_seed_lineages: Vec::new(),
        selected_candidate_seed_ids: first.selected_seed_ids.clone(),
        mutable_faces,
        mutable_face_lineages: Vec::new(),
    };
    let boundary = mesh
        .legalization_patch_boundary_check(
            &first,
            &preflight,
            &patch,
            &first.selected_seed_ids,
            2,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
        )
        .expect("closed patch boundary");
    assert!(boundary.is_closed());
    assert!(boundary.exact_materializable);
    assert_eq!(boundary.exact_failure_kind, None);
    assert_eq!(
        boundary.selected_face_ids,
        first
            .selected_faces
            .iter()
            .enumerate()
            .skip(2)
            .filter_map(|(iw, &selected)| selected.then_some(iw))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        boundary
            .ordered_perimeter_components
            .iter()
            .map(|component| component.points.len())
            .collect::<Vec<_>>(),
        boundary.perimeter_lengths
    );
    assert_eq!(boundary.vertex_only_contact_count, 0);
    assert_eq!(boundary.predicted_transition_self_loop_count, 0);
    assert!(boundary
        .perimeter_lengths
        .iter()
        .all(|length| length % 3 == 0));

    let mut compiled_patch = patch.clone();
    compiled_patch.candidate_seed_ids = first.selected_seed_ids.clone();
    compiled_patch.selected_candidate_seed_ids = first.selected_seed_ids.clone();
    compiled_patch.mutable_faces = mesh
        .selected_faces_from_method_c_seed_ids(&compiled_patch.candidate_seed_ids)
        .expect("compiled-table candidate footprints")
        .iter()
        .enumerate()
        .skip(2)
        .filter_map(|(iw, &selected)| selected.then_some(iw))
        .collect();
    assert!(compiled_patch.candidate_seed_ids.len() <= 8);
    let compiled = mesh
        .compile_bounded_exact_legalization_patch_table_for_diagnostics(
            &first,
            &preflight,
            &compiled_patch,
            2,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
            8,
        )
        .expect("compile exact legalization table");
    assert_eq!(compiled.status, MethodCHfieldExactPatchTableStatus::Sat);
    assert!(compiled.demand_anchor_count > 0);
    assert!(
        compiled.fixed_direct_covered_demand_anchors
            <= compiled.fixed_closed_covered_demand_anchors
    );
    assert!(
        compiled.fixed_closed_covered_demand_anchors
            <= compiled.maximal_closed_covered_demand_anchors
    );
    assert_eq!(
        compiled.maximal_closed_covered_demand_anchors,
        compiled.demand_anchor_count
    );
    assert_eq!(
        compiled.fixed_uncovered_demand_anchors,
        compiled.demand_anchor_count - compiled.fixed_direct_covered_demand_anchors
    );
    assert!(compiled.direct_unsupported_demand_anchors <= compiled.fixed_uncovered_demand_anchors);
    assert!(
        compiled.distinct_direct_candidate_support_scope_count
            <= compiled.fixed_uncovered_demand_anchors
    );
    if compiled.fixed_uncovered_demand_anchors > 0 {
        assert!(
            compiled.min_direct_candidate_support_count
                <= compiled.max_direct_candidate_support_count
        );
    }
    assert!(
        compiled
            .direct_coverage_clause_satisfying_assignments
            .expect("bounded direct coverage clause count")
            <= compiled
                .total_assignments
                .expect("bounded assignment count")
    );
    assert!(!compiled.covers_current_perimeter_scope);
    assert_eq!(compiled.current_perimeter_scope_candidate_seed_ids, None);
    assert_eq!(
        compiled.total_assignments,
        Some(1usize << compiled_patch.candidate_seed_ids.len())
    );
    assert_eq!(
        compiled.evaluated_assignments,
        compiled
            .total_assignments
            .expect("bounded assignment count")
    );
    assert!(compiled.sat_assignments > 0);
    assert!(compiled.triplet_assignment_count <= compiled.evaluated_assignments);
    assert!(compiled.distinct_exact_state_count <= compiled.triplet_assignment_count);
    assert!(compiled.max_exact_state_multiplicity > 0);
    assert_eq!(compiled.mixed_exact_outcome_state_count, 0);
    for analysis in &compiled.ordered_perimeter_scope_analyses {
        assert!(analysis.projected_interface_face_count > 0);
        assert_eq!(analysis.projected_direct_union_state_cap, 100_000);
        assert!(
            analysis.projected_direct_union_state_count.is_some()
                ^ analysis
                    .projected_direct_union_state_cap_exceeded_after_variables
                    .is_some()
        );
        assert!(analysis.candidate_footprint_face_count > 0);
        assert!(
            analysis.candidate_footprint_union_state_count.is_some()
                ^ analysis
                    .candidate_footprint_union_state_cap_exceeded_after_variables
                    .is_some()
        );
        assert_eq!(
            analysis.closure_prefix_assignment_count,
            1usize << analysis.closure_prefix_variable_count
        );
        assert!(
            analysis.closure_prefix_distinct_closed_mask_count
                <= analysis.closure_prefix_distinct_direct_mask_count
        );
        assert!(analysis.closure_prefix_max_closed_mask_multiplicity > 0);
    }
    assert_eq!(
        compiled.table.as_ref().map(|table| table.row_count()),
        Some(compiled.sat_assignments)
    );
    assert!(
        compiled
            .propagation
            .as_ref()
            .expect("compiled-table propagation")
            .consistent
    );
    let compiled_analysis = compiled
        .system_analysis
        .as_ref()
        .expect("compiled-table system analysis");
    assert!(compiled_analysis.propagation.consistent);
    assert_eq!(compiled_patch.candidate_seed_ids.len(), 4);
    assert_eq!(compiled_analysis.propagation.pruned_values, 1);
    assert_eq!(compiled_analysis.max_residual_component_width, 3);
    let canonical_relation = compiled
        .table
        .as_ref()
        .expect("compiled table")
        .canonical_relation();
    assert_eq!(
        canonical_relation
            .rebind_variables((0..compiled_patch.candidate_seed_ids.len()).collect())
            .expect("rebound canonical relation"),
        compiled
            .table
            .as_ref()
            .expect("compiled table")
            .canonical_relation()
    );
    assert_eq!(
        compiled,
        mesh.compile_bounded_exact_legalization_patch_table_for_diagnostics(
            &first,
            &preflight,
            &compiled_patch,
            2,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
            8,
        )
        .expect("repeat exact legalization table compilation")
    );
    let bounded_out = mesh
        .compile_bounded_exact_legalization_patch_table_for_diagnostics(
            &first,
            &preflight,
            &compiled_patch,
            2,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
            0,
        )
        .expect("bounded-out exact legalization table compilation");
    assert_eq!(
        bounded_out.status,
        MethodCHfieldExactPatchTableStatus::Incomplete
    );
    assert_eq!(bounded_out.evaluated_assignments, 0);

    let mut seed_patch = patch.clone();
    seed_patch.candidate_seed_ids = first.selected_seed_ids.clone();
    let expanded = mesh
        .expand_legalization_patch_one_ring(&first, &preflight, &seed_patch)
        .expect("expand patch");
    assert!(first
        .selected_seed_ids
        .iter()
        .all(|seed| expanded.candidate_seed_ids.binary_search(seed).is_ok()));
    assert_eq!(
        expanded,
        mesh.expand_legalization_patch_one_ring(&first, &preflight, &seed_patch)
            .expect("repeat patch expansion")
    );
    let local_phases = mesh
        .expand_legalization_patch_local_phases(&first, &preflight, &seed_patch)
        .expect("expand local phase candidates");
    assert!(expanded
        .candidate_seed_ids
        .iter()
        .all(|seed| { local_phases.candidate_seed_ids.binary_search(seed).is_ok() }));
    assert_eq!(
        local_phases,
        mesh.expand_legalization_patch_local_phases(&first, &preflight, &seed_patch)
            .expect("repeat local phase expansion")
    );

    let mut leaking_patch = patch;
    leaking_patch.mutable_faces.clear();
    let mut mismatched_preflight = preflight.clone();
    let mismatched_face = mismatched_preflight
        .prepared_selected_faces
        .iter()
        .enumerate()
        .skip(2)
        .find_map(|(iw, &selected)| selected.then_some(iw))
        .expect("selected face");
    mismatched_preflight.prepared_selected_faces[mismatched_face] = false;
    assert!(!mesh
        .legalization_patch_boundary_check(
            &first,
            &mismatched_preflight,
            &leaking_patch,
            &first.selected_seed_ids,
            2,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
        )
        .expect("open patch boundary")
        .is_closed());
    let mut leaking_compiled_patch = compiled_patch;
    leaking_compiled_patch.mutable_faces.clear();
    let leaking_compiled = mesh
        .compile_bounded_exact_legalization_patch_table_for_diagnostics(
            &first,
            &mismatched_preflight,
            &leaking_compiled_patch,
            2,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
            8,
        )
        .expect("compile leaking exact legalization table");
    assert_eq!(
        leaking_compiled.status,
        MethodCHfieldExactPatchTableStatus::Incomplete
    );
    assert!(!leaking_compiled.covers_current_perimeter_scope);
    assert_eq!(
        leaking_compiled.current_perimeter_scope_candidate_seed_ids,
        None
    );
    assert!(leaking_compiled.boundary_incomplete_assignments > 0);
}

#[test]
fn pass_one_sub_lattice_face_demand_reaches_the_global_canonical_phase() {
    let mesh = base_mesh();
    let neighbors = mesh.method_c_m_neighbors().expect("M neighbors");
    let mut canonical = vec![false; mesh.nmd + 1];
    let mut done = vec![[false; 6]; mesh.nmd + 1];
    let mut stack = vec![mesh.impent[0]];
    canonical[mesh.impent[0]] = true;
    while let Some(im) = stack.pop() {
        for next in mesh
            .method_c_thirdm_neighbors_canonical_with_neighbors(im, &mut done, &neighbors)
            .expect("canonical phase")
        {
            if !canonical[next] {
                canonical[next] = true;
                stack.push(next);
            }
        }
    }
    let demanded_face = (2..=mesh.nwd)
        .find(|&iw| mesh.w_faces[iw].im.iter().all(|&im| !canonical[im]))
        .expect("sub-lattice W face");
    let mut demand = vec![false; mesh.nwd + 1];
    demand[demanded_face] = true;

    let refined = mesh
        .spawn_nest_pass_from_target_levels_and_face_demands(
            |_, _| 0,
            &demand,
            1,
            2,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
            false,
        )
        .expect("sub-lattice face demand must reach a canonical owner")
        .expect("non-empty face demand");
    refined.validate_topology().expect("refined topology");
}

#[test]
fn empty_face_demand_skips_parent_support_and_spawn() {
    let mesh = base_mesh();
    let demand = vec![false; mesh.nwd + 1];

    assert!(mesh
        .required_parent_support_lineages_from_target_levels_and_face_demands(
            |_, _| 0,
            &demand,
            3,
            true,
        )
        .expect("empty support request")
        .is_empty());
    assert!(mesh
        .spawn_nest_pass_from_target_levels_and_face_demands(
            |_, _| 0,
            &demand,
            3,
            4,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
            true,
        )
        .expect("empty pass")
        .is_none());
}

#[test]
fn spring_diagnostics_do_not_change_hfield_spawn() {
    let mesh = base_mesh();
    let regular = mesh
        .spawn_nest_from_target_levels_with_spring(
            two_ring_levels(1_000_000.0, 4_000_000.0),
            2,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
            6,
            2,
        )
        .expect("regular h-field spring");
    let measured = mesh
        .spawn_nest_from_target_levels_with_m0_diagnostics(
            two_ring_levels(1_000_000.0, 4_000_000.0),
            2,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
            6,
            2,
            true,
        )
        .expect("measured h-field spring");

    assert_eq!(measured.0, regular.0);
    assert_eq!(measured.1, regular.1);
    assert_eq!(measured.2.len(), 2);
    assert!(measured.2.iter().all(|diagnostics| {
        diagnostics.generation_m_points > 0
            && diagnostics.movable_m_points > 0
            && diagnostics.movable_edges > 0
            && diagnostics.shaped_movable_edges > 0
            && !diagnostics.movable_adjacent_hex_cell_lineages.is_empty()
    }));
    assert_eq!(measured.3.len(), 2);
    assert!(measured.3.iter().all(|diagnostics| {
        let counts = diagnostics.face_reason_mask_counts;
        diagnostics.final_selected_faces > 0
            && counts.iter().sum::<usize>() == diagnostics.final_selected_faces
            && diagnostics.initial_seed_footprint_faces
                == counts[1] + counts[3] + counts[5] + counts[7]
            && diagnostics.demand_tail_faces == counts[2] + counts[3] + counts[6] + counts[7]
            && diagnostics.connectivity_bridge_faces
                == counts[4] + counts[5] + counts[6] + counts[7]
            && diagnostics.unexplained_selected_faces == 0
            && !diagnostics.selected_seed_ids.is_empty()
            && diagnostics.seed_union_vertex_only_contacts == 0
            && diagnostics.seed_union_first_contact_m_point.is_none()
            && diagnostics.seed_reconstruction_matches
            && diagnostics
                .candidate_validation
                .as_ref()
                .is_some_and(|validation| {
                    validation.coverage_valid
                        && validation.parent_level_valid
                        && validation.perimeters_triplets
                        && validation.transition_materializable
                        && validation.materialized_m_valence_census_available
                        && validation.materialized_m_valence_violation_count == 0
                        && validation.failure_kind.is_none()
                })
    }));
}

#[test]
fn intermediate_hfield_pass_expands_local_phase_support() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(18, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let (_, _, _, diagnostics) = mesh
        .spawn_nest_from_target_levels_with_m0_diagnostics(
            three_ring_levels(750_000.0, 2_000_000.0, 5_000_000.0),
            3,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
            18,
            0,
            false,
        )
        .expect("three-level h-field spawn");
    let pass_two = diagnostics
        .iter()
        .find(|diagnostics| diagnostics.pass == 2)
        .expect("intermediate pass diagnostics");

    assert!(!pass_two.preserve_all_demands);
    assert!(
        pass_two.phase_support_m_points > pass_two.hard_demand_m_points,
        "intermediate nested demand needs a shared local stride-3 phase"
    );
    for pass in diagnostics
        .iter()
        .filter(|diagnostics| diagnostics.pass > 1)
    {
        assert!(!pass.component_phases.is_empty());
        assert!(pass.component_phases.iter().all(|component| {
            component.phase_class_count > 1
                && component.phase_starts.len() == component.phase_class_count
                && component
                    .component_m_points
                    .contains(&component.demand_start)
                && component.selected_phase_ordinal == 0
                && component.selected_start == component.demand_start
        }));
        let component_legal = pass
            .component_phases
            .iter()
            .flat_map(|component| component.legal_seed_ids.iter().copied())
            .collect::<BTreeSet<_>>();
        let component_selected = pass
            .component_phases
            .iter()
            .flat_map(|component| component.selected_seed_ids.iter().copied())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            component_legal,
            pass.legal_seed_ids.iter().copied().collect()
        );
        assert_eq!(
            component_selected,
            pass.selected_seed_ids.iter().copied().collect()
        );
    }
}
