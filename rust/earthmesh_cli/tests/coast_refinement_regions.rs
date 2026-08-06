//! Land-type raster -> refinement circles -> Method-C, end to end.
//!
//! This is the path that would let a project ask for coastal refinement without
//! an h-field raster at all: the same land/sea rule the carve uses, reduced to
//! circles that are materializable by construction.

use earthmesh_cli::coast_refinement_regions::{
    coastal_refinement_circles, materializable_radius_meters, CoastRefinementRequest,
};
use earthmesh_mesh::{MethodCDelaunayMesh, RefinementRegion};

/// Production land-type raster; skipped when the data is not mounted.
fn landtype_path() -> Option<std::path::PathBuf> {
    let path = std::env::var("EARTHMESH_LANDTYPE")
        .map(std::path::PathBuf::from)
        .ok()?;
    path.is_file().then_some(path)
}

fn south_china_sea(radius_m: f64, level: usize) -> CoastRefinementRequest {
    CoastRefinementRequest {
        west_degrees: 108.0,
        east_degrees: 120.0,
        south_degrees: 18.0,
        north_degrees: 26.0,
        level,
        radius_meters: radius_m,
    }
}

#[test]
fn derived_coastal_circles_refine_a_method_c_mesh() {
    let Some(landtype) = landtype_path() else {
        eprintln!("EARTHMESH_LANDTYPE not set to a real file; skipping");
        return;
    };
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(21, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
    // NXP 21 base cells are ~381 km.
    let base_cell_m = 2.0 * std::f64::consts::PI * 6_371_229.0 / (5.0 * 21.0);
    let radius_m = materializable_radius_meters(base_cell_m);

    let regions = coastal_refinement_circles(&landtype, 240, south_china_sea(radius_m, 1))
        .expect("derive coastal circles");

    assert!(
        !regions.is_empty(),
        "the South China Sea coast must yield circles"
    );
    assert!(
        regions
            .iter()
            .all(|r| matches!(r, RefinementRegion::Circle { .. })),
        "only circles are emitted"
    );

    let before = mesh.nwd;
    let refined = mesh
        .spawn_nest(&regions, 1)
        .expect("derived circles must be materializable");
    assert!(
        refined.nwd > before,
        "refinement must add faces: {before} -> {}",
        refined.nwd
    );
}

#[test]
fn a_land_locked_box_yields_no_circles() {
    let Some(landtype) = landtype_path() else {
        eprintln!("EARTHMESH_LANDTYPE not set to a real file; skipping");
        return;
    };
    // Interior Asia: land on all sides, so no land/ocean block anywhere.
    let request = CoastRefinementRequest {
        west_degrees: 90.0,
        east_degrees: 95.0,
        south_degrees: 40.0,
        north_degrees: 45.0,
        level: 1,
        radius_meters: 150_000.0,
    };
    let regions =
        coastal_refinement_circles(&landtype, 240, request).expect("derive over land-locked box");
    assert!(
        regions.is_empty(),
        "a box with no coastline must ask for nothing, got {} circles",
        regions.len()
    );
}

#[test]
fn invalid_requests_are_rejected() {
    let request = CoastRefinementRequest {
        west_degrees: 120.0,
        east_degrees: 108.0, // inverted
        south_degrees: 18.0,
        north_degrees: 26.0,
        level: 1,
        radius_meters: 150_000.0,
    };
    assert!(coastal_refinement_circles("unused.nc", 240, request).is_err());

    let mut bad_radius = south_china_sea(0.0, 1);
    bad_radius.radius_meters = 0.0;
    assert!(coastal_refinement_circles("unused.nc", 240, bad_radius).is_err());
}
