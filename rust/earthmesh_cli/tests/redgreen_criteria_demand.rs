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
        earthmesh_mesh::TriangularMesh::from_icosahedron(21, 0, 1.0, 0.25, 0).expect("base mesh");
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
