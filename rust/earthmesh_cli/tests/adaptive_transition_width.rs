//! Does the transition width the adaptive route picks change the mesh?
//!
//! Bare `spawn_nest` hard-codes the surface width, so that route used the
//! surface one for an atmosmesh too. Wiring the spring forced an explicit width
//! and the route now picks by mesh type like every other. Measured through the
//! CLI at the time, the meshes came out bit-identical and only the reported
//! transition-face count moved -- but that run had no spring, and the answer
//! turns out to depend on exactly that.
//!
//! So these ask the question twice, with the spring and without, by calling the
//! route with each width and comparing the meshes it returns. The distinction
//! matters to anyone reading the change: without a spring the width is a
//! classification, and with one it is geometry.

use earthmesh_cli::refinement_demand::{
    nest::{spawn_nest_adaptive_with_named_regions, AdaptiveNestSpring},
    plan::DemandPlanInputs,
    source_bounds_for_bbox,
};
use earthmesh_core::RefineConfig;
use earthmesh_mesh::{LonLatDegrees, RefinementRegion};
use earthmesh_refine_method_c::MethodCMesh;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn temp_root(name: &str) -> PathBuf {
    let sequence = ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "earthmesh_transition_width_{name}_{}_{sequence}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
}

const NLONS: usize = 360;
const NLATS: usize = 180;

fn write_open_ocean(path: &Path) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create landtype file");
    file.add_dimension("longitude", NLONS).expect("lon dim");
    file.add_dimension("latitude", NLATS).expect("lat dim");
    let values = vec![0_i8; NLONS * NLATS];
    let mut var = file
        .add_variable::<i8>("landtype", &["longitude", "latitude"])
        .expect("landtype var");
    var.put_values(&values, (.., ..)).expect("write landtype");
}

fn plan_inputs(landtype: &Path) -> DemandPlanInputs<'_> {
    DemandPlanInputs {
        bounds: source_bounds_for_bbox(90.0, 140.0, 0.0, 45.0, 1).expect("bounds"),
        gridnum_perdegree: 1,
        landtype_file: Some(landtype),
        mesh_type: "earthmesh",
        refine_coastline: false,
    }
}

fn base_cell_meters(nxp: usize) -> f64 {
    2.0 * std::f64::consts::PI * 6_371_229.0 / (5.0 * nxp as f64)
}

/// A run's shape, enough to tell two meshes apart without dumping either.
#[derive(Debug, PartialEq)]
struct Shape {
    faces: usize,
    points: usize,
    transition_rows: usize,
    deepest_level: usize,
}

fn run(
    nxp: usize,
    regions: &[RefinementRegion],
    levels: usize,
    max_mrows: usize,
    spring_iterations: usize,
) -> MethodCMesh {
    let root = temp_root(&format!("n{nxp}_m{max_mrows}_s{spring_iterations}"));
    let landtype = root.join("landtype.nc");
    write_open_ocean(&landtype);
    let mesh = MethodCMesh::from_icosahedron(nxp, 0, 1.0, 0.25, 0).expect("base mesh");
    let (refined, _) = spawn_nest_adaptive_with_named_regions(
        &mesh,
        &RefineConfig::default(),
        &plan_inputs(&landtype),
        regions,
        base_cell_meters(nxp),
        levels,
        Some(AdaptiveNestSpring {
            nxp,
            iterations: spring_iterations,
            max_mrows,
        }),
    )
    .expect("adaptive nest");
    let _ = fs::remove_dir_all(root);
    refined
}

fn shape(mesh: &MethodCMesh) -> Shape {
    Shape {
        faces: mesh.w_faces.len().saturating_sub(2),
        points: mesh.m_points.len().saturating_sub(2),
        transition_rows: mesh.boundary_rows().len(),
        deepest_level: mesh
            .w_faces
            .iter()
            .skip(2)
            .map(|face| face.mrlw)
            .max()
            .unwrap_or(0),
    }
}

fn points_that_differ(a: &MethodCMesh, b: &MethodCMesh) -> usize {
    a.m_points
        .iter()
        .zip(b.m_points.iter())
        .skip(2)
        .filter(|(a, b)| {
            (a.x - b.x).abs() > 1.0e-9 || (a.y - b.y).abs() > 1.0e-9 || (a.z - b.z).abs() > 1.0e-9
        })
        .count()
}

fn circle(lon: f64, lat: f64, radius_m: f64, level: usize) -> RefinementRegion {
    RefinementRegion::Circle {
        center: LonLatDegrees::new(lon, lat),
        radius_meters: radius_m,
        level,
    }
}

/// Without a spring, the width classifies rows and moves no point.
///
/// This is what the CLI comparison measured when the change went in: the two
/// widths produced bit-identical coordinates and differed only in how many
/// faces were counted as transition rows.
#[test]
fn without_a_spring_the_width_changes_only_the_row_count() {
    let regions = vec![circle(114.0, 22.0, 400_000.0, 1)];
    let surface = run(21, &regions, 1, MethodCMesh::MAX_MROWS_SURFACE, 0);
    let atmos = run(21, &regions, 1, MethodCMesh::MAX_MROWS_ATMOS, 0);

    assert_eq!(
        points_that_differ(&surface, &atmos),
        0,
        "no spring, so nothing moves: {:?} vs {:?}",
        shape(&surface),
        shape(&atmos)
    );
    assert_eq!(shape(&surface).faces, shape(&atmos).faces);
    assert_ne!(
        shape(&surface).transition_rows,
        shape(&atmos).transition_rows,
        "the width still decides how far the transition band reaches"
    );
}

/// With a spring, the width is geometry: the rows decide what the spring moves.
///
/// Measured at NXP 21 with a single circle: 599 of 4484 points land somewhere
/// else, with the transition band at 744 rows against 1640. So picking the
/// atmosphere width for an atmosmesh on this route -- which is what every other
/// Method-C path already did -- changes the mesh an atmosmesh run produces, and
/// is not the reporting-only difference the first CLI measurement suggested.
#[test]
fn with_a_spring_the_width_moves_points() {
    let regions = vec![circle(114.0, 22.0, 400_000.0, 1)];
    let surface = run(21, &regions, 1, MethodCMesh::MAX_MROWS_SURFACE, 4);
    let atmos = run(21, &regions, 1, MethodCMesh::MAX_MROWS_ATMOS, 4);

    let moved = points_that_differ(&surface, &atmos);
    assert!(
        moved > 0,
        "the width reaches the spring: {:?} vs {:?}",
        shape(&surface),
        shape(&atmos)
    );
    // The refinement itself is the same either way -- same faces, same depth.
    // Only where the points ended up differs, which is the spring's doing.
    assert_eq!(shape(&surface).faces, shape(&atmos).faces);
    assert_eq!(shape(&surface).points, shape(&atmos).points);
    assert_eq!(shape(&surface).deepest_level, shape(&atmos).deepest_level);
}

/// The same holds where the bound is most likely to bind: neighbouring regions
/// at depth, on a larger base mesh.
///
/// Measured at NXP 30 across five circles over two levels: 1038 of 9236 points
/// differ, 663 transition rows against 1379, and the refinement is otherwise
/// identical -- same faces, same depth, and the same one group refused in both.
#[test]
fn neighbouring_regions_at_depth_agree_on_everything_but_position() {
    let regions = vec![
        circle(114.0, 22.0, 600_000.0, 1),
        circle(122.0, 22.0, 600_000.0, 1),
        circle(118.0, 28.0, 600_000.0, 1),
        circle(114.0, 22.0, 300_000.0, 2),
        circle(122.0, 22.0, 300_000.0, 2),
    ];
    let surface = run(30, &regions, 2, MethodCMesh::MAX_MROWS_SURFACE, 4);
    let atmos = run(30, &regions, 2, MethodCMesh::MAX_MROWS_ATMOS, 4);

    assert_eq!(shape(&surface).faces, shape(&atmos).faces);
    assert_eq!(shape(&surface).deepest_level, shape(&atmos).deepest_level);
    assert!(
        points_that_differ(&surface, &atmos) > 0,
        "{:?} vs {:?}",
        shape(&surface),
        shape(&atmos)
    );
}
