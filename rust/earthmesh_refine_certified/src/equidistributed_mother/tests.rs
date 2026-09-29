use super::*;
use crate::adapted_certified_mother;
use earthmesh_mesh::MeshState;

fn lonlat(lon: f64, lat: f64) -> P {
    let (lon, lat) = (lon.to_radians(), lat.to_radians());
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}

/// A lon/lat raster with `level` within `radius_km` of each centre.
fn raster(nlon: usize, nlat: usize, spots: &[((f64, f64), f64, usize)]) -> RasterLevelField {
    let mut levels = vec![0usize; nlon * nlat];
    for j in 0..nlat {
        for i in 0..nlon {
            let p = centre(nlon, nlat, i, j);
            for &((lon, lat), radius_km, level) in spots {
                if arc(p, lonlat(lon, lat)) * 6371.0 <= radius_km {
                    levels[j * nlon + i] = levels[j * nlon + i].max(level);
                }
            }
        }
    }
    RasterLevelField::new(nlon, nlat, levels).unwrap()
}

fn mesh_arrays(n: usize) -> (Vec<P>, Vec<[usize; 3]>) {
    let mother = MotherGrid::generate(n).unwrap().mesh;
    let slots = mother.active_vertex_slots().collect::<Vec<_>>();
    let mut index = vec![usize::MAX; mother.vertices().len()];
    for (i, &s) in slots.iter().enumerate() {
        index[s] = i;
    }
    let points = slots
        .iter()
        .map(|&v| unit(xyz(mother.vertices()[v])).unwrap())
        .collect::<Vec<_>>();
    let triangles = mother
        .active_triangle_slots()
        .map(|t| {
            let mut tri = mother.triangles()[t].map(|v| index[v]);
            let (a, b, c) = (points[tri[0]], points[tri[1]], points[tri[2]]);
            if dot(cross(sub(b, a), sub(c, a)), add(add(a, b), c)) < 0.0 {
                tri.swap(1, 2);
            }
            tri
        })
        .collect::<Vec<_>>();
    (points, triangles)
}

fn degrees(mesh: &MeshState) -> Vec<usize> {
    let mut degree = vec![0usize; mesh.vertices().len()];
    for t in mesh.active_triangle_slots() {
        for v in mesh.triangles()[t] {
            degree[v] += 1;
        }
    }
    mesh.active_vertex_slots().map(|v| degree[v]).collect()
}

#[test]
fn the_gradient_matches_finite_differences() {
    let (mut points, triangles) = mesh_arrays(4);
    // A non-trivial state: nudge every point, targets unequal.
    for (i, p) in points.iter_mut().enumerate() {
        let d = [0.01 * (i as f64).sin(), 0.01 * (i as f64 * 1.7).cos(), 0.0];
        *p = unit(add(*p, d)).unwrap();
    }
    let targets = (0..triangles.len())
        .map(|t| 0.05 + 0.02 * (t as f64).sin().abs())
        .collect::<Vec<_>>();
    let mut gradient = vec![[0.0; 3]; points.len()];
    energy(&points, &triangles, &targets, 0.3, Some(&mut gradient)).unwrap();
    let h = 1e-6;
    for v in [0usize, 7, 23] {
        // A tangent direction at v.
        let x = points[v];
        let t = unit(cross(x, [0.3, 0.5, 0.8])).unwrap();
        let moved = |s: f64| {
            let mut q = points.clone();
            q[v] = add(x, scale(t, s));
            energy(&q, &triangles, &targets, 0.3, None).unwrap()
        };
        let numeric = (moved(h) - moved(-h)) / (2.0 * h);
        let analytic = dot(gradient[v], t);
        assert!(
            (numeric - analytic).abs() <= 1e-5 * numeric.abs().max(1.0),
            "v={v}: {numeric} vs {analytic}"
        );
    }
}

#[test]
fn minimising_lowers_the_energy_and_never_inverts_a_triangle() {
    let (points, triangles) = mesh_arrays(8);
    // Ask one cap of the sphere for much smaller triangles.
    let focus = lonlat(40.0, 10.0);
    let targets = triangles
        .iter()
        .map(|&t| {
            let c = centroid(&points, t);
            if arc(c, focus) < 0.6 {
                0.004
            } else {
                0.012
            }
        })
        .collect::<Vec<_>>();
    let before = energy(&points, &triangles, &targets, 0.3, None).unwrap();
    let after_points = minimise(points.clone(), &triangles, &targets, 0.3, 150);
    let after =
        energy(&after_points, &triangles, &targets, 0.3, None).expect("no inverted triangle");
    assert!(after < before, "{after} >= {before}");
    assert!(after_points
        .iter()
        .all(|p| (dot(*p, *p) - 1.0).abs() < 1e-12));
}

#[test]
fn the_size_field_is_gradient_limited_and_fine_at_the_demand() {
    let demand = raster(180, 90, &[((115.0, 23.0), 500.0, 2)]);
    let base = 0.06;
    let field = SizeField::new(&demand, base);
    let (nlon, nlat) = (field.nlon, field.nlat);
    for j in 0..nlat {
        for i in 0..nlon {
            let h = field.size[j * nlon + i];
            assert!(h <= base + 1e-15);
            if demand.levels()[j * nlon + i] == 2 {
                assert!((h - base / 4.0).abs() < 1e-15);
            }
            let ii = (i + 1) % nlon;
            let d = arc(centre(nlon, nlat, i, j), centre(nlon, nlat, ii, j));
            assert!((h - field.size[j * nlon + ii]).abs() <= GRADATION * d + 1e-12);
        }
    }
}

#[test]
fn two_far_regions_are_served_without_a_heptagon_by_fewer_cells_than_the_safe_mother() {
    // Level 2 in two regions on opposite sides of the globe: one focus
    // cannot serve both, the equidistribution can (guide 11.88).
    let demand = raster(
        360,
        180,
        &[((115.0, 23.0), 500.0, 2), ((-60.0, -15.0), 500.0, 2)],
    );
    let adapted = adapted_certified_mother(
        20,
        2,
        &demand,
        AngleContractId::LegacyStrict40To80,
        10_000_000,
    )
    .unwrap_or_else(|why| panic!("{why:?}"));
    assert_eq!(
        adapted.strategy, "equidistribution",
        "{:?}",
        adapted.rejected
    );
    assert!(
        adapted.subdivision < 80,
        "safe mother is n=80, got {}",
        adapted.subdivision
    );
    let degree = degrees(adapted.geometry.primal());
    assert!(degree.iter().all(|&d| d == 5 || d == 6));
    assert_eq!(degree.iter().filter(|&&d| d == 5).count(), 12);
    assert_eq!(adapted.final_requirements.physical_residuals(), 0);
    assert_eq!(adapted.final_requirements.balance_residuals(), 0);
    assert!(adapted.delivered_levels.iter().max() >= Some(&2));
}

#[test]
fn one_concentrated_region_goes_to_the_schmidt_stretch() {
    let demand = raster(360, 180, &[((115.0, 23.0), 500.0, 3)]);
    let adapted = adapted_certified_mother(
        20,
        3,
        &demand,
        AngleContractId::LegacyStrict40To80,
        10_000_000,
    )
    .unwrap_or_else(|why| panic!("{why:?}"));
    assert_eq!(
        adapted.strategy, "schmidt_stretch",
        "{:?}",
        adapted.rejected
    );
    // The stretch takes n=64 (3.2 times the base), below the n=80 a power of
    // two would have needed; equidistribution only tries mothers coarser
    // than that.
    assert_eq!(adapted.subdivision, 64);
}

#[test]
fn a_level_one_demand_has_no_mother_between_the_base_and_the_safe_one() {
    // Nothing is supported between n=20 and the safe n=40.
    let demand = raster(360, 180, &[((115.0, 23.0), 500.0, 1)]);
    assert!(adapted_certified_mother(
        20,
        1,
        &demand,
        AngleContractId::LegacyStrict40To80,
        10_000_000
    )
    .is_err());
}
