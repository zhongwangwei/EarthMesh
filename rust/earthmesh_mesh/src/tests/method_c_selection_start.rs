use super::*;

#[test]
fn method_c_region_start_prefers_contained_global_pentagon() {
    let mesh = TriangularMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let radius = active_mesh_radius(&mesh).expect("mesh radius");
    let method_c_m_neighbors =
        derive_icosahedron_m_neighbors_canonical_checked(mesh.nmd, &mesh.u_edges, &mesh.w_faces)
            .expect("Method-C test neighbors");
    let pentagon_id = mesh.impent[0];
    let pentagon_lonlat = xyz_to_lonlat_degrees(mesh.m_points[pentagon_id]);

    let mut chosen_region = None;
    for lon_offset in (-40..=40).step_by(4) {
        for lat_offset in (-20..=20).step_by(4) {
            if lon_offset == 0 && lat_offset == 0 {
                continue;
            }
            let region = RefinementRegion::Circle {
                center: LonLatDegrees::new(
                    pentagon_lonlat.lon_degrees + lon_offset as f64,
                    pentagon_lonlat.lat_degrees + lat_offset as f64,
                ),
                radius_meters: 3_000_000.0,
                level: 1,
            };
            if region.contains_cartesian(mesh.m_points[pentagon_id], radius)
                && mesh
                    .closest_m_point_to_region_anchor(&region, false)
                    .expect("closest anchor")
                    != pentagon_id
            {
                chosen_region = Some(region);
                break;
            }
        }
        if chosen_region.is_some() {
            break;
        }
    }
    let region = chosen_region.expect("test region containing pentagon but centered elsewhere");

    let start = mesh
        .method_c_refinement_start_point_with_neighbors(
            &region,
            radius,
            &method_c_m_neighbors,
            false,
        )
        .expect("Method-C start point");

    assert_eq!(
            start, pentagon_id,
            "Method-C spawn_nest should use a contained global impent as IMBEG before falling back to the nearest center point"
        );
}

#[test]
fn method_c_region_start_marches_from_nearby_global_pentagon() {
    let mesh = TriangularMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let radius = active_mesh_radius(&mesh).expect("mesh radius");
    let method_c_m_neighbors =
        derive_icosahedron_m_neighbors_canonical_checked(mesh.nmd, &mesh.u_edges, &mesh.w_faces)
            .expect("Method-C test neighbors");

    let mut selected_case = None;
    'search: for &pentagon_id in &mesh.impent {
        let pentagon_lonlat = xyz_to_lonlat_degrees(mesh.m_points[pentagon_id]);
        for lon_offset in (-36..=36).step_by(3) {
            for lat_offset in (-24..=24).step_by(3) {
                if lon_offset == 0 && lat_offset == 0 {
                    continue;
                }
                for radius_meters in [500_000.0, 750_000.0, 1_000_000.0, 1_250_000.0] {
                    let region = RefinementRegion::Circle {
                        center: LonLatDegrees::new(
                            pentagon_lonlat.lon_degrees + lon_offset as f64,
                            pentagon_lonlat.lat_degrees + lat_offset as f64,
                        ),
                        radius_meters,
                        level: 1,
                    };
                    if region.contains_cartesian(mesh.m_points[pentagon_id], radius)
                        || !region.close_to_cartesian(mesh.m_points[pentagon_id], radius)
                    {
                        continue;
                    }
                    let Some(expected_start) =
                        method_c_impen_march_start_for_test(&mesh, pentagon_id, &region, radius)
                    else {
                        continue;
                    };
                    let closest = mesh
                        .closest_m_point_to_region_anchor(&region, false)
                        .expect("closest anchor");
                    if expected_start != closest {
                        selected_case = Some((region, expected_start));
                        break 'search;
                    }
                }
            }
        }
    }
    let (region, expected_start) =
        selected_case.expect("near-pentagon circle requiring Method-C impen march");

    let start = mesh
        .method_c_refinement_start_point_with_neighbors(
            &region,
            radius,
            &method_c_m_neighbors,
            false,
        )
        .expect("Method-C start point");

    assert_eq!(
            start, expected_start,
            "Method-C spawn_nest should march from a nearby impent toward the nearest inside M point before falling back to the geometric center"
        );
}

#[test]
fn method_c_region_start_skips_nearby_global_pentagon_with_different_mrlm() {
    let mut mesh =
        TriangularMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let radius = active_mesh_radius(&mesh).expect("mesh radius");
    let method_c_m_neighbors =
        derive_icosahedron_m_neighbors_canonical_checked(mesh.nmd, &mesh.u_edges, &mesh.w_faces)
            .expect("Method-C test neighbors");

    let mut selected_case = None;
    'search: for &pentagon_id in &mesh.impent {
        let pentagon_lonlat = xyz_to_lonlat_degrees(mesh.m_points[pentagon_id]);
        for lon_offset in (-36..=36).step_by(3) {
            for lat_offset in (-24..=24).step_by(3) {
                if lon_offset == 0 && lat_offset == 0 {
                    continue;
                }
                for radius_meters in [500_000.0, 750_000.0, 1_000_000.0, 1_250_000.0] {
                    let region = RefinementRegion::Circle {
                        center: LonLatDegrees::new(
                            pentagon_lonlat.lon_degrees + lon_offset as f64,
                            pentagon_lonlat.lat_degrees + lat_offset as f64,
                        ),
                        radius_meters,
                        level: 1,
                    };
                    if region.contains_cartesian(mesh.m_points[pentagon_id], radius)
                        || !region.close_to_cartesian(mesh.m_points[pentagon_id], radius)
                    {
                        continue;
                    }
                    let closest = mesh
                        .closest_m_point_to_region_anchor(&region, false)
                        .expect("closest anchor");
                    if closest == pentagon_id {
                        continue;
                    }
                    if method_c_impen_march_start_for_test(&mesh, pentagon_id, &region, radius)
                        .is_some()
                    {
                        selected_case = Some((region, pentagon_id, closest));
                        break 'search;
                    }
                }
            }
        }
    }
    let (region, pentagon_id, closest) =
        selected_case.expect("near-pentagon case that would march with matching mrlm");
    mesh.m_metadata[pentagon_id].mrlm = mesh.m_metadata[closest].mrlm + 1;

    let start = mesh
        .method_c_refinement_start_point_with_neighbors(
            &region,
            radius,
            &method_c_m_neighbors,
            false,
        )
        .expect("Method-C start point");

    assert_eq!(
        start, closest,
        "Canonical only uses the nearby impent march when impent mrlm matches imcent mrlm"
    );
}

#[test]
fn method_c_near_pentagon_march_uses_marched_start_mrlm_for_parent_ownership() {
    let mut mesh =
        TriangularMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let radius = active_mesh_radius(&mesh).expect("mesh radius");
    let method_c_m_neighbors =
        derive_icosahedron_m_neighbors_canonical_checked(mesh.nmd, &mesh.u_edges, &mesh.w_faces)
            .expect("Method-C test neighbors");

    let mut selected_case = None;
    'search: for &pentagon_id in &mesh.impent {
        let pentagon_lonlat = xyz_to_lonlat_degrees(mesh.m_points[pentagon_id]);
        for lon_offset in (-36..=36).step_by(3) {
            for lat_offset in (-24..=24).step_by(3) {
                if lon_offset == 0 && lat_offset == 0 {
                    continue;
                }
                for radius_meters in [500_000.0, 750_000.0, 1_000_000.0, 1_250_000.0] {
                    let region = RefinementRegion::Circle {
                        center: LonLatDegrees::new(
                            pentagon_lonlat.lon_degrees + lon_offset as f64,
                            pentagon_lonlat.lat_degrees + lat_offset as f64,
                        ),
                        radius_meters,
                        level: 1,
                    };
                    if region.contains_cartesian(mesh.m_points[pentagon_id], radius)
                        || !region.close_to_cartesian(mesh.m_points[pentagon_id], radius)
                    {
                        continue;
                    }
                    let Some(expected_start) =
                        method_c_impen_march_start_for_test(&mesh, pentagon_id, &region, radius)
                    else {
                        continue;
                    };
                    let closest = mesh
                        .closest_m_point_to_region_anchor(&region, false)
                        .expect("closest anchor");
                    if expected_start != closest && expected_start != pentagon_id {
                        selected_case = Some((region, pentagon_id, closest, expected_start));
                        break 'search;
                    }
                }
            }
        }
    }
    let (region, pentagon_id, closest, expected_start) =
        selected_case.expect("near-pentagon march case with distinct impen/imcent/imbeg");
    mesh.m_metadata[pentagon_id].mrlm = 3;
    mesh.m_metadata[pentagon_id].mrlm_orig = 3;
    mesh.m_metadata[closest].mrlm = 3;
    mesh.m_metadata[closest].mrlm_orig = 3;
    assert_eq!(
        mesh.m_metadata[expected_start].mrlm, 1,
        "test requires marched IMBEG to remain on the parent level"
    );

    let start = mesh
        .method_c_refinement_start_point_with_neighbors(
            &region,
            radius,
            &method_c_m_neighbors,
            false,
        )
        .expect("Method-C start point");
    let selected = mesh
        .selected_region_faces(&region, 1, false)
        .expect("Method-C selection should use marched IMBEG mrlm as mrlo");

    assert_eq!(start, expected_start);
    assert!(
        selected.iter().skip(2).any(|selected| *selected),
        "Canonical sets mrlo from marched IMBEG, not from nearby impen or imcent"
    );
}

#[test]
fn method_c_near_pentagon_march_preserves_canonical_jdone_between_steps() {
    let mesh = TriangularMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let radius = active_mesh_radius(&mesh).expect("mesh radius");
    let method_c_m_neighbors =
        derive_icosahedron_m_neighbors_canonical_checked(mesh.nmd, &mesh.u_edges, &mesh.w_faces)
            .expect("Method-C test neighbors");
    let mut checked = 0usize;

    for &pentagon_id in &mesh.impent {
        let pentagon_lonlat = xyz_to_lonlat_degrees(mesh.m_points[pentagon_id]);
        for lon_offset in (-60..=60).step_by(3) {
            for lat_offset in (-36..=36).step_by(3) {
                if lon_offset == 0 && lat_offset == 0 {
                    continue;
                }
                for radius_meters in [250_000.0, 500_000.0, 750_000.0, 1_000_000.0, 1_250_000.0] {
                    let region = RefinementRegion::Circle {
                        center: LonLatDegrees::new(
                            pentagon_lonlat.lon_degrees + lon_offset as f64,
                            pentagon_lonlat.lat_degrees + lat_offset as f64,
                        ),
                        radius_meters,
                        level: 1,
                    };
                    if region.contains_cartesian(mesh.m_points[pentagon_id], radius)
                        || !region.close_to_cartesian(mesh.m_points[pentagon_id], radius)
                    {
                        continue;
                    }
                    let expected = method_c_impen_march_start_canonical_jdone_for_test(
                        &mesh,
                        pentagon_id,
                        &region,
                        radius,
                    );
                    let actual = mesh
                        .method_c_march_from_nearby_pentagon_to_region_with_neighbors(
                            pentagon_id,
                            &region,
                            radius,
                            &method_c_m_neighbors,
                            false,
                        )
                        .expect("Method-C near-pentagon march");
                    assert_eq!(
                            actual, expected,
                            "Canonical spawn_nest keeps jdone marks between near-pentagon march steps while clearing only the current row"
                        );
                    checked += 1;
                }
            }
        }
    }

    assert!(
        checked > 0,
        "test should exercise at least one near-pentagon march case"
    );
}

#[test]
fn method_c_thirdm_walks_straight_opposite_edges_and_marks_reciprocal_done_like_canonical() {
    let mesh =
        TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base mesh should build");
    let method_c_m_neighbors = mesh
        .derive_icosahedron_m_neighbors_canonical()
        .expect("Method-C M-neighbor table should derive");
    let start = (2..=mesh.nmd)
        .find(|&im| method_c_m_neighbors[im].npoly == 6)
        .expect("base mesh should contain a 6-edge M point");

    let iu = method_c_m_neighbors[start].iu[0];
    let imm = mesh
        .other_m_endpoint(iu, start)
        .expect("first U edge should have opposite M endpoint");
    let iuu = mesh
        .opposite_ring_u_edge_with_neighbors(imm, iu, &method_c_m_neighbors)
        .expect("Canonical thirdm should choose opposite edge at first M");
    let immm = mesh
        .other_m_endpoint(iuu, imm)
        .expect("second U edge should have opposite M endpoint");
    let iuuu = mesh
        .opposite_ring_u_edge_with_neighbors(immm, iuu, &method_c_m_neighbors)
        .expect("Canonical thirdm should choose opposite edge at second M");
    let expected_immmm = mesh
        .other_m_endpoint(iuuu, immm)
        .expect("third U edge should have opposite M endpoint");

    let mut jdone = vec![[false; 6]; mesh.nmd + 1];
    let thirdm_neighbors = mesh
        .method_c_thirdm_neighbors_canonical_with_neighbors(
            start,
            &mut jdone,
            &method_c_m_neighbors,
        )
        .expect("thirdm should traverse ordinary 6-edge topology");

    assert_eq!(thirdm_neighbors.first().copied(), Some(expected_immmm));
    assert!(jdone[start][0]);
    let reciprocal_edge = method_c_m_neighbors[expected_immmm]
        .iu
        .iter()
        .take(method_c_m_neighbors[expected_immmm].npoly.min(6))
        .position(|&far_iu| far_iu == iuuu)
        .expect("far M point should contain the incoming third U edge");
    assert!(jdone[expected_immmm][reciprocal_edge]);
}

#[test]
fn method_c_thirdm_rejects_broken_topology_instead_of_skipping_path() {
    let mesh = TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let mut method_c_m_neighbors = mesh
        .derive_icosahedron_m_neighbors_canonical()
        .expect("Method-C M neighbors");
    let start = (2..=mesh.nmd)
        .find(|&im| method_c_m_neighbors[im].npoly >= 2)
        .expect("M point with multiple U edges");
    let non_incident_iu = (2..=mesh.nud)
        .find(|&iu| !mesh.u_edges[iu].im.contains(&start))
        .expect("U edge not incident on selected M point");
    method_c_m_neighbors[start].iu[0] = non_incident_iu;
    let mut jdone = vec![[false; 6]; mesh.nmd + 1];

    let err = mesh
        .method_c_thirdm_neighbors_canonical_with_neighbors(
            start,
            &mut jdone,
            &method_c_m_neighbors,
        )
        .expect_err("Canonical thirdm should not silently skip an invalid straight path");
    assert!(
        err.to_string().contains("not incident") || err.to_string().contains("not in M point"),
        "unexpected thirdm topology error: {err}"
    );
}

#[test]
fn method_c_thirdm_skips_intermediate_zero_npoly_path() {
    let mesh = TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let mut method_c_m_neighbors = mesh
        .derive_icosahedron_m_neighbors_canonical()
        .expect("Method-C M neighbors");
    let start = (2..=mesh.nmd)
        .find(|&im| {
            let mut jdone = vec![[false; 6]; mesh.nmd + 1];
            if method_c_m_neighbors[im].npoly < 2 {
                return false;
            }
            mesh.method_c_thirdm_neighbors_canonical_with_neighbors(
                im,
                &mut jdone,
                &method_c_m_neighbors,
            )
            .map(|neighbors| !neighbors.is_empty())
            .unwrap_or(false)
        })
        .expect("M point with at least one computed third-m path");
    let iu = method_c_m_neighbors[start].iu[0];
    let imm = mesh
        .other_m_endpoint(iu, start)
        .expect("neighbor on valid edge");
    method_c_m_neighbors[imm].npoly = 0;
    let mut jdone = vec![[false; 6]; mesh.nmd + 1];
    let neighbors = mesh
        .method_c_thirdm_neighbors_canonical_with_neighbors(
            start,
            &mut jdone,
            &method_c_m_neighbors,
        )
        .expect("thirdm should ignore malformed intermediate npoly entries");
    assert!(
        !neighbors.is_empty(),
        "zero-npoly intermediate should still allow at least one straight third-m path"
    );
}

fn method_c_impen_march_start_canonical_jdone_for_test(
    mesh: &TriangularMesh,
    pentagon_id: usize,
    region: &RefinementRegion,
    radius: f64,
) -> Option<usize> {
    let method_c_m_neighbors =
        derive_icosahedron_m_neighbors_canonical_checked(mesh.nmd, &mesh.u_edges, &mesh.w_faces)
            .ok()?;
    let nearest_inside = mesh
        .nearest_inside_m_point_to(pentagon_id, region, radius, false)
        .ok()??;
    let mut current = pentagon_id;
    let mut visited = BTreeSet::new();
    let mut jdone = vec![[false; 6]; mesh.nmd + 1];
    for _ in 0..mesh.nmd {
        if !visited.insert(current) {
            return None;
        }
        jdone[current] = [false; 6];
        let neighbors = mesh
            .method_c_thirdm_neighbors_canonical_with_neighbors(
                current,
                &mut jdone,
                &method_c_m_neighbors,
            )
            .ok()?;
        let mut best_neighbor = None;
        let mut best_distance = f64::INFINITY;
        for neighbor in neighbors {
            if region.contains_cartesian(mesh.m_points[neighbor], radius) {
                return Some(neighbor);
            }
            let distance =
                cartesian_distance(mesh.m_points[neighbor], mesh.m_points[nearest_inside]);
            if distance < best_distance {
                best_distance = distance;
                best_neighbor = Some(neighbor);
            }
        }
        current = best_neighbor?;
    }
    None
}

fn method_c_impen_march_start_for_test(
    mesh: &TriangularMesh,
    pentagon_id: usize,
    region: &RefinementRegion,
    radius: f64,
) -> Option<usize> {
    let method_c_m_neighbors =
        derive_icosahedron_m_neighbors_canonical_checked(mesh.nmd, &mesh.u_edges, &mesh.w_faces)
            .ok()?;
    let mut nearest_inside = None;
    let mut nearest_distance = f64::INFINITY;
    for im in 2..=mesh.nmd {
        if !region.contains_cartesian(mesh.m_points[im], radius) {
            continue;
        }
        let distance = cartesian_distance(mesh.m_points[im], mesh.m_points[pentagon_id]);
        if distance < nearest_distance {
            nearest_distance = distance;
            nearest_inside = Some(im);
        }
    }
    let nearest_inside = nearest_inside?;
    let mut current = pentagon_id;
    let mut visited = BTreeSet::new();
    let mut jdone = vec![[false; 6]; mesh.nmd + 1];
    for _ in 0..mesh.nmd {
        if !visited.insert(current) {
            return None;
        }
        jdone[current] = [false; 6];
        let neighbors = mesh
            .method_c_thirdm_neighbors_canonical_with_neighbors(
                current,
                &mut jdone,
                &method_c_m_neighbors,
            )
            .ok()?;
        let mut best_neighbor = None;
        let mut best_distance = f64::INFINITY;
        for neighbor in neighbors {
            if region.contains_cartesian(mesh.m_points[neighbor], radius) {
                return Some(neighbor);
            }
            let distance =
                cartesian_distance(mesh.m_points[neighbor], mesh.m_points[nearest_inside]);
            if distance < best_distance {
                best_distance = distance;
                best_neighbor = Some(neighbor);
            }
        }
        current = best_neighbor?;
    }
    None
}

fn cartesian_distance(a: CartesianPoint, b: CartesianPoint) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}
