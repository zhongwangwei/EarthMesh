//! The chain red-green exists for: a criterion whose shape came from the data,
//! reduced to circles, refining a mesh.
//!
//! Method-C refuses this outright -- its seed lattice steps three cells at a
//! time and its perimeter must be a multiple of three, so a coastline is
//! refused rather than approximated (`METHOD_C_ADAPTIVE_SUSPENDED`). Red-green
//! takes any marking and grows it until the triangulation closes.
//!
//! Driven at one source cell per degree rather than through the pipeline, which
//! pins `NL%gridnum_perdegree` to 120 or 240: a global raster at 120 is
//! 43200 x 21600, about a gigabyte to write before the first pass. The planner
//! itself takes the sampling as an argument, so the criteria can be exercised
//! for real at a size a test can afford.

use std::fs;
use std::path::{Path, PathBuf};

use earthmesh_cli::refinement_demand::nest::adaptive_demand_circles_for_level;
use earthmesh_cli::refinement_demand::plan::DemandPlanInputs;
use earthmesh_cli::refinement_demand::source_bounds_for_bbox;

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_redgreen_criteria_{name}"));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
}

/// One island, at one cell per degree, so there is a coastline to find and most
/// of the sphere has nothing to ask for.
fn write_island_landtype(path: &Path) {
    let (nlons, nlats) = (360usize, 180usize);
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create landtype file");
    file.add_dimension("longitude", nlons).expect("lon dim");
    file.add_dimension("latitude", nlats).expect("lat dim");
    let mut values = vec![0_i8; nlons * nlats];
    for lon_idx in 0..nlons {
        let lon = -180.0 + (lon_idx as f64 + 0.5);
        for lat_idx in 0..nlats {
            let lat = 90.0 - (lat_idx as f64 + 0.5);
            if (100.0..=140.0).contains(&lon) && (10.0..=40.0).contains(&lat) {
                values[lon_idx * nlats + lat_idx] = 1;
            }
        }
    }
    let mut var = file
        .add_variable::<i8>("landtype", &["longitude", "latitude"])
        .expect("landtype var");
    var.put_values(&values, (.., ..)).expect("write landtype");
}

#[test]
fn a_coastline_the_criteria_found_refines_on_red_green() {
    let root = temp_root("coastline");
    let landtype = root.join("landtype.nc");
    write_island_landtype(&landtype);

    let per_degree = 1usize;
    let inputs = DemandPlanInputs {
        bounds: source_bounds_for_bbox(90.0, 150.0, 0.0, 50.0, per_degree).expect("source bounds"),
        gridnum_perdegree: per_degree,
        landtype_file: Some(&landtype),
        mesh_type: "landmesh",
        refine_coastline: true,
        domain_region: None,
    };
    let refine = earthmesh_core::RefineConfig {
        is_transition: true,
        halo: [3; 10],
        max_transition_row: [3; 10],
        ..earthmesh_core::RefineConfig::default()
    };
    // The cell an NXP 21 mesh starts from, which is the size the criterion is
    // asked over.
    let base_cell_meters =
        2.0 * std::f64::consts::PI * earthmesh_hfield::EARTH_RADIUS_METERS / (5.0 * 21.0);

    let demand = adaptive_demand_circles_for_level(&refine, &inputs, 1, base_cell_meters, 1)
        .expect("plan the coastline demand");

    assert!(demand.demanded, "the island has a coastline to find");
    assert!(
        !demand.circles.is_empty(),
        "and it must survive the reduction to circles"
    );

    let base =
        earthmesh_mesh::TriangularMesh::from_icosahedron(21, 0, 1.0, 0.25).expect("base mesh");
    let neighbors = base.m_neighbors.clone();
    let mesh = earthmesh_refine_redgreen::redgreen_mesh_from_triangular(&base, &neighbors)
        .expect("bridge in");
    let before = mesh.triangle_count();

    let (written, outcome) = earthmesh_cli::redgreen_bridge::refine_redgreen_level(
        &mesh,
        &demand.circles,
        &refine,
        1,
        None,
        false,
    )
    .expect("red-green must build what the criterion asked for");

    assert!(
        outcome.refined_triangle_count > 0,
        "the coastline demand must split triangles: {outcome:?}"
    );
    assert!(
        outcome.mesh.triangle_count() > before,
        "{} vs {before}",
        outcome.mesh.triangle_count()
    );
    let topology =
        earthmesh_cli::unstructured_mesh_support::check_unstructured_mesh_topology(&written);
    assert!(
        topology.is_consistent(),
        "and arrive as a mesh: {:?}",
        &topology.violations[..topology.violations.len().min(4)]
    );
}

/// A refined region closes wherever it sits -- over a pole, across the
/// antimeridian, at mid-latitude.
///
/// It did not. Each of the three subdivision steps rotated a triangle spanning
/// the antimeridian by 180 degrees of longitude, computed in that frame, and
/// rotated back -- what planar lon/lat averaging needs, and a mathematical
/// identity for the unit-vector centroid that replaced it. Only its arithmetic
/// survived: whether a triangle took the branch was decided per triangle, so of
/// the two triangles sharing an edge one could take it and the other not, and
/// their midpoints for that shared edge came out one ULP apart -- 2.8e-14
/// degrees. New vertices merge by exact equality, so the two did not merge, the
/// shared edge lost its neighbour, and the mesh came out with a hole.
///
/// The test sweeps the places the branch fired: a pole (spuriously, because
/// cells fan out in longitude there), and the seam itself. Mid-latitude is kept
/// as the control, because it always worked and is how this hid.
#[test]
fn a_refined_region_closes_over_a_pole_and_across_the_antimeridian() {
    let refine = earthmesh_core::RefineConfig {
        is_transition: true,
        halo: [3; 10],
        max_transition_row: [3; 10],
        ..earthmesh_core::RefineConfig::default()
    };
    let open_edges = |mesh: &earthmesh_refine_redgreen::RedGreenMesh| {
        let rows = earthmesh_mesh::triangle_neighbors_from_cell_membership_one_based(
            &mesh.cells_on_triangle,
            &mesh.triangles_on_cell,
            &mesh.n_triangles_on_cell,
        )
        .expect("cell membership resolves");
        (mesh.num_vertex + 1..=mesh.triangle_count())
            .filter(|&triangle| rows[triangle].contains(&0))
            .count()
    };

    for (place, lon, lat) in [
        ("north pole", 30.0, 89.0),
        ("south pole", 0.0, -89.0),
        ("antimeridian", 180.0, 0.0),
        ("mid-latitude", 45.0, 45.0),
    ] {
        // NXP 33: the smallest size at which every one of these failed before.
        let base =
            earthmesh_mesh::TriangularMesh::from_icosahedron(33, 0, 1.0, 0.25).expect("base mesh");
        let neighbors = base.m_neighbors.clone();
        let mesh = earthmesh_refine_redgreen::redgreen_mesh_from_triangular(&base, &neighbors)
            .expect("bridge in");
        let regions: Vec<earthmesh_mesh::RefinementRegion> = (1..=2)
            .map(|level| earthmesh_mesh::RefinementRegion::Circle {
                center: earthmesh_mesh::LonLatDegrees::new(lon, lat),
                radius_meters: 600_000.0,
                level,
            })
            .collect();

        let (_, first) = earthmesh_cli::redgreen_bridge::refine_redgreen_level(
            &mesh, &regions, &refine, 1, None, false,
        )
        .unwrap_or_else(|error| panic!("{place} level 1: {error}"));
        assert_eq!(open_edges(&first.mesh), 0, "{place} level 1 left a hole");

        // The second level is what used to report the first level's hole, as
        // "ngrmm row N has invalid neighbor 0" -- so it is half the test.
        let previous =
            earthmesh_cli::redgreen_bridge::redgreen_marking_from_regions(&first.mesh, &regions, 1);
        let (_, second) = earthmesh_cli::redgreen_bridge::refine_redgreen_level(
            &first.mesh,
            &regions,
            &refine,
            2,
            Some(&previous),
            false,
        )
        .unwrap_or_else(|error| panic!("{place} level 2: {error}"));
        assert!(
            second.refined_triangle_count > 0,
            "{place} level 2 refined nothing"
        );
        assert_eq!(open_edges(&second.mesh), 0, "{place} level 2 left a hole");
    }
}

/// The transition rows take back the degree they add, so no cell passes seven.
///
/// Building a transition row splits a triangle in two, which adds one to the
/// degree of the corner it splits away from; the Lawson flips afterwards are
/// what take that back. The flips never ran: the judge read its walk bound from
/// its own output count, which is freshly zeroed every round, so every segment
/// was skipped and `flipped_triangle_count` was zero in every run.
///
/// Cells reached degree 8. Nothing said so until the carve, and only for a mesh
/// that has one -- `center N neighbor count 8 exceeds available width` from the
/// land and ocean examples, while the atmosphere example wrote its mesh and
/// exited clean. Seven is what Method-C guarantees and what every reader below
/// is sized for.
#[test]
fn the_transition_rows_take_back_the_degree_they_add() {
    let refine = earthmesh_core::RefineConfig {
        is_transition: true,
        // The width the shipped examples use.
        halo: [4, 4, 3, 0, 0, 0, 0, 0, 0, 0],
        max_transition_row: [4, 4, 3, 0, 0, 0, 0, 0, 0, 0],
        ..earthmesh_core::RefineConfig::default()
    };
    let base =
        earthmesh_mesh::TriangularMesh::from_icosahedron(33, 0, 1.0, 0.25).expect("base mesh");
    let neighbors = base.m_neighbors.clone();
    let mesh = earthmesh_refine_redgreen::redgreen_mesh_from_triangular(&base, &neighbors)
        .expect("bridge in");
    let widest = |mesh: &earthmesh_refine_redgreen::RedGreenMesh| {
        (mesh.num_center + 1..=mesh.cell_count())
            .map(|cell| mesh.n_triangles_on_cell[cell])
            .max()
            .unwrap_or(0)
    };
    assert_eq!(
        widest(&mesh),
        6,
        "an unrefined mesh is hexagons and pentagons"
    );

    let regions: Vec<earthmesh_mesh::RefinementRegion> = (1..=2)
        .map(|level| earthmesh_mesh::RefinementRegion::Circle {
            center: earthmesh_mesh::LonLatDegrees::new(120.0, 40.0),
            radius_meters: 2_000_000.0,
            level,
        })
        .collect();

    let (_, first) = earthmesh_cli::redgreen_bridge::refine_redgreen_level(
        &mesh, &regions, &refine, 1, None, false,
    )
    .expect("level one");
    assert!(
        first.flipped_triangle_count > 0,
        "a transition band leaves sharp corners; flipping them is not optional"
    );
    assert!(
        widest(&first.mesh) <= 7,
        "level one reached degree {}",
        widest(&first.mesh)
    );

    let previous =
        earthmesh_cli::redgreen_bridge::redgreen_marking_from_regions(&first.mesh, &regions, 1);
    let (_, second) = earthmesh_cli::redgreen_bridge::refine_redgreen_level(
        &first.mesh,
        &regions,
        &refine,
        2,
        Some(&previous),
        false,
    )
    .expect("level two");
    assert!(second.flipped_triangle_count > 0);
    assert!(
        widest(&second.mesh) <= 7,
        "level two reached degree {}",
        widest(&second.mesh)
    );
}
