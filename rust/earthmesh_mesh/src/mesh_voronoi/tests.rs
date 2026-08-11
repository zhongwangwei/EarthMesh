use super::*;
use crate::{lonlat_degrees_to_unit_xyz, LonLatDegrees, TriangularMesh};

fn sphere(nxp: usize) -> MeshState {
    let mesh = TriangularMesh::from_icosahedron(nxp, 0, 1.0, 0.25, 0).expect("base mesh");
    MeshState::from_triangular_mesh(&mesh).expect("neutral state")
}

fn on(state: &MeshState, lon: f64, lat: f64) -> CartesianPoint {
    let unit = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(lon, lat));
    let radius = state.sphere_radius();
    CartesianPoint::new(unit.x * radius, unit.y * radius, unit.z * radius)
}

fn angle_between(a: CartesianPoint, b: CartesianPoint) -> f64 {
    let dot = a.x * b.x + a.y * b.y + a.z * b.z;
    let magnitude =
        (a.x * a.x + a.y * a.y + a.z * a.z).sqrt() * (b.x * b.x + b.y * b.y + b.z * b.z).sqrt();
    (dot / magnitude).clamp(-1.0, 1.0).acos()
}

/// An icosahedral mesh has twelve pentagons and nothing else irregular.
///
/// Checked through the fan rather than through `impent`, so it says the walk
/// found the right triangles and not just that the mesh remembers which sites
/// are special.
#[test]
fn the_fan_finds_twelve_pentagons_and_hexagons_everywhere_else() {
    let state = sphere(6);
    let mut pentagons = 0;
    for site in MESH_STATE_FIRST_ID..state.vertices().len() {
        let fan = state.triangle_fan(site).expect("fan");
        match fan.len() {
            5 => pentagons += 1,
            6 => {}
            other => panic!("site {site} has {other} triangles around it"),
        }
    }
    assert_eq!(pentagons, 12);
}

/// Consecutive triangles in a fan share an edge at the site.
///
/// This is what "in rotational order" means, and without it the cell's corners
/// come out in an order that still forms a polygon -- a self-intersecting one,
/// whose area is wrong by an amount too small to look wrong.
#[test]
fn the_fan_is_in_rotational_order() {
    let state = sphere(6);
    for site in MESH_STATE_FIRST_ID..state.vertices().len() {
        let fan = state.triangle_fan(site).expect("fan");
        for step in 0..fan.len() {
            let here = state.triangles()[fan[step]];
            let next = state.triangles()[fan[(step + 1) % fan.len()]];
            let shared: Vec<usize> = here
                .iter()
                .copied()
                .filter(|corner| next.contains(corner))
                .collect();
            assert_eq!(
                shared.len(),
                2,
                "site {site}: triangles {} and {} are consecutive in the fan and share {shared:?}",
                fan[step],
                fan[(step + 1) % fan.len()]
            );
            assert!(shared.contains(&site));
        }
    }
}

/// Any incident triangle is as good a starting point as any other.
#[test]
fn the_fan_is_the_same_cycle_from_any_seed() {
    let state = sphere(6);
    for site in [7usize, 40, 120, 300] {
        let baseline = state.triangle_fan(site).expect("fan");
        for &seed in &baseline {
            let fan = state.triangle_fan_from(site, seed).expect("fan");
            assert_eq!(fan.len(), baseline.len());
            let offset = baseline
                .iter()
                .position(|&triangle| triangle == seed)
                .expect("the seed is in its own fan");
            for step in 0..fan.len() {
                assert_eq!(fan[step], baseline[(offset + step) % baseline.len()]);
            }
        }
    }
}

/// The circumcentre is equidistant from the three sites. The definition.
#[test]
fn a_cell_corner_is_equidistant_from_the_three_sites_that_made_it() {
    let state = sphere(6);
    for triangle in MESH_STATE_FIRST_ID..state.triangles().len() {
        let centre = state.circumcentre(triangle).expect("circumcentre");
        let corners = state.triangles()[triangle];
        let distances = corners.map(|corner| angle_between(centre, state.vertices()[corner]));
        let spread = distances.iter().copied().fold(f64::MIN, f64::max)
            - distances.iter().copied().fold(f64::MAX, f64::min);
        assert!(
            spread < 1.0e-9,
            "triangle {triangle}: {distances:?} differ by {spread:e}"
        );
    }
}

/// The cells tile the sphere.
///
/// A global consequence of purely local arithmetic, and the strongest thing
/// available: an order that self-intersects, a circumcentre on the wrong
/// hemisphere, or a fan that skips a triangle all fail here and pass every
/// per-cell check.
#[test]
fn the_cells_cover_the_sphere_exactly_once() {
    let state = sphere(6);
    let mut total = 0.0;
    for site in MESH_STATE_FIRST_ID..state.vertices().len() {
        total += state
            .voronoi_cell(site)
            .expect("cell")
            .area_on_unit_sphere()
            .expect("area");
    }
    let sphere_area = 4.0 * std::f64::consts::PI;
    assert!(
        (total - sphere_area).abs() / sphere_area < 1.0e-9,
        "the cells add up to {total} and the sphere is {sphere_area}"
    );
}

/// A rebuild after a change touches the cells that moved and no others.
///
/// The claim the local rebuild exists to make. Every other cell is compared
/// corner for corner against what it was, not merely for area or degree.
#[test]
fn rebuilding_after_an_insertion_changes_only_the_cells_that_moved() {
    let mut state = sphere(6);
    let before: Vec<Option<VoronoiCell>> = (0..state.vertices().len())
        .map(|site| state.voronoi_cell(site).ok())
        .collect();

    let point = on(&state, 47.0, 23.0);
    let report = state.insert_site(point).expect("insert");
    let changed: BTreeSet<usize> = report.created.iter().copied().collect();
    let rebuilt = state.voronoi_cells_touching(&changed).expect("rebuild");

    let moved: BTreeSet<usize> = rebuilt.iter().map(|cell| cell.site).collect();
    assert!(moved.contains(&report.site), "the new site got a cell");
    assert!(
        moved.len() >= 4,
        "the new site and the ring it displaced, not just the new site"
    );

    for site in MESH_STATE_FIRST_ID..before.len() {
        if moved.contains(&site) {
            continue;
        }
        let now = state.voronoi_cell(site).expect("cell");
        assert_eq!(
            Some(&now),
            before[site].as_ref(),
            "site {site} is outside the change and its cell moved"
        );
    }
}

/// Insertion and rebuild together still tile the sphere.
#[test]
fn the_cells_still_cover_the_sphere_after_inserting() {
    let mut state = sphere(6);
    for (lon, lat) in [(15.0, 12.0), (-95.0, 38.0), (140.0, -47.0)] {
        let point = on(&state, lon, lat);
        state.insert_site(point).expect("insert");
    }
    let total: f64 = (MESH_STATE_FIRST_ID..state.vertices().len())
        .map(|site| {
            state
                .voronoi_cell(site)
                .expect("cell")
                .area_on_unit_sphere()
                .expect("area")
        })
        .sum();
    let sphere_area = 4.0 * std::f64::consts::PI;
    assert!(
        (total - sphere_area).abs() / sphere_area < 1.0e-9,
        "the cells add up to {total} and the sphere is {sphere_area}"
    );
}

/// A seed that does not touch the site is refused, not walked from.
#[test]
fn a_fan_refuses_a_seed_that_does_not_touch_the_site() {
    let state = sphere(6);
    let site = 7;
    let stranger = (MESH_STATE_FIRST_ID..state.triangles().len())
        .find(|&triangle| !state.triangles()[triangle].contains(&site))
        .expect("some triangle is not at this site");
    let error = state
        .triangle_fan_from(site, stranger)
        .expect_err("a fan cannot start where the site is not");
    assert!(
        matches!(error, VoronoiError::SeedDoesNotTouchTheSite { .. }),
        "{error}"
    );
}

/// A site the mesh does not carry has no cell.
#[test]
fn a_site_the_mesh_does_not_carry_is_refused() {
    let state = sphere(6);
    for site in [0usize, 1, state.vertices().len()] {
        let error = state
            .triangle_fan(site)
            .expect_err("this is not a site of this mesh");
        assert!(matches!(error, VoronoiError::UnknownSite { .. }), "{error}");
    }
}
