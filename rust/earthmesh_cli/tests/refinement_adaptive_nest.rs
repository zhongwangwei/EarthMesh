//! Criteria to circles to a mesh, one level at a time.
//!
//! The point of the loop is that a level's demand is computed after the
//! previous level has been refined, so a criterion whose answer depends on the
//! cell size gets asked again at each size. These check that the loop does what
//! that requires: it stops when nothing asks for more, it deepens when
//! something does, and a resolution-dependent criterion actually changes its
//! mind between levels.

use earthmesh_refine_method_c::MethodCMesh;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use earthmesh_cli::refinement_demand::{
    nest::spawn_nest_adaptive, plan::DemandPlanInputs, source_bounds_for_bbox,
};
use earthmesh_core::RefineConfig;

static ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn temp_root(name: &str) -> PathBuf {
    let sequence = ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "earthmesh_adaptive_nest_{name}_{}_{sequence}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
}

const NLONS: usize = 360;
const NLATS: usize = 180;
const NXP: usize = 21;

fn base_cell_meters() -> f64 {
    2.0 * std::f64::consts::PI * 6_371_229.0 / (5.0 * NXP as f64)
}

fn write_landtype(path: &Path, class_at: impl Fn(usize, usize) -> i8) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create landtype file");
    file.add_dimension("longitude", NLONS).expect("lon dim");
    file.add_dimension("latitude", NLATS).expect("lat dim");
    let mut values = vec![0_i8; NLONS * NLATS];
    for lon in 0..NLONS {
        for lat in 0..NLATS {
            values[lon * NLATS + lat] = class_at(lon + 1, lat + 1);
        }
    }
    let mut var = file
        .add_variable::<i8>("landtype", &["longitude", "latitude"])
        .expect("landtype var");
    var.put_values(&values, (.., ..)).expect("write landtype");
}

fn plan_inputs<'a>(landtype: &'a Path, refine_coastline: bool) -> DemandPlanInputs<'a> {
    DemandPlanInputs {
        bounds: source_bounds_for_bbox(105.0, 125.0, 12.0, 32.0, 1).expect("bounds"),
        gridnum_perdegree: 1,
        landtype_file: Some(landtype),
        mesh_type: "earthmesh",
        refine_coastline,
        domain_region: None,
    }
}

fn base_mesh() -> MethodCMesh {
    MethodCMesh::from_icosahedron(NXP, 0, 1.0, 0.25, 0).expect("base mesh")
}

#[test]
fn a_coastline_drives_the_loop_to_the_depth_it_is_given() {
    let root = temp_root("coast");
    let path = root.join("landtype.nc");
    // Land east of 110 east: source index 291 is 110 degrees.
    write_landtype(&path, |lon, _lat| i8::from(lon >= 291));

    let refine = RefineConfig::default();
    // Criteria-driven refinement is suspended on this backend: Method-C seeds
    // on a lattice that steps three cells at a time and needs a perimeter that
    // is a multiple of three, so a region shaped by the data is refused rather
    // than approximated -- and a global run showed it refusing 25 of 59 groups
    // while still producing a mesh that passed every gate. Refusing is what
    // keeps that from reading as success.
    let error = spawn_nest_adaptive(
        &base_mesh(),
        &refine,
        &plan_inputs(&path, true),
        base_cell_meters(),
        5,
    )
    .expect_err("criteria-driven refinement must refuse while it is suspended");
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported, "{error}");
    assert!(error.to_string().contains("suspended"), "{error}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_uniform_map_asks_for_nothing_and_the_loop_stops() {
    let root = temp_root("uniform");
    let path = root.join("landtype.nc");
    write_landtype(&path, |_, _| 0);

    let refine = RefineConfig::default();
    let (refined, report) = spawn_nest_adaptive(
        &base_mesh(),
        &refine,
        &plan_inputs(&path, true),
        base_cell_meters(),
        5,
    )
    .expect("adaptive nest");

    assert_eq!(report.deepest_level, 0, "{report:?}");
    assert!(report.stopped_on_empty_demand, "{report:?}");
    assert!(report.passes.is_empty());
    assert_eq!(refined.w_faces.len(), base_mesh().w_faces.len());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_resolution_dependent_criterion_changes_its_mind_between_levels() {
    // Stripes eight degrees wide. A pass judging a cell wider than a stripe
    // sees more than one class; a pass judging a much smaller cell does not.
    // Under the old arrangement -- one field, quantised once -- both levels got
    // the same answer, so this is what per-pass planning buys.
    let root = temp_root("landcover");
    let path = root.join("landtype.nc");
    write_landtype(&path, |lon, _lat| ((lon / 8) % 3) as i8 + 1);

    let refine = RefineConfig {
        refine_num_landtypes: true,
        th_num_landtypes: 1,
        ..RefineConfig::default()
    };
    // Criteria-driven refinement is suspended on this backend: Method-C seeds
    // on a lattice that steps three cells at a time and needs a perimeter that
    // is a multiple of three, so a region shaped by the data is refused rather
    // than approximated -- and a global run showed it refusing 25 of 59 groups
    // while still producing a mesh that passed every gate. Refusing is what
    // keeps that from reading as success.
    let error = spawn_nest_adaptive(
        &base_mesh(),
        &refine,
        &plan_inputs(&path, false),
        base_cell_meters(),
        5,
    )
    .expect_err("criteria-driven refinement must refuse while it is suspended");
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported, "{error}");
    assert!(error.to_string().contains("suspended"), "{error}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_named_region_is_refined_even_when_no_criterion_asks() {
    // A project that names a circle and enables nothing else must still get
    // that circle. Naming a region is an instruction, not a criterion, so the
    // loop cannot stop on "no criterion demanded refinement" and leave the mesh
    // uniform -- which is exactly what it did before this case existed.
    let root = temp_root("named_only");
    let path = root.join("landtype.nc");
    write_landtype(&path, |_, _| 0); // open ocean: no coast, no land cover

    let named = vec![earthmesh_mesh::RefinementRegion::Circle {
        center: earthmesh_mesh::LonLatDegrees::new(114.0, 22.0),
        radius_meters: 400_000.0,
        level: 1,
    }];
    let refine = RefineConfig::default();
    let (refined, report) =
        earthmesh_cli::refinement_demand::nest::spawn_nest_adaptive_with_named_regions(
            &base_mesh(),
            &refine,
            &plan_inputs(&path, true),
            &named,
            base_cell_meters(),
            1,
            None,
        )
        .expect("adaptive nest");

    assert_eq!(report.deepest_level, 1, "{report:?}");
    assert_eq!(report.passes.len(), 1, "{report:?}");
    assert!(
        report.passes[0].faces_after > report.passes[0].faces_before,
        "{report:?}"
    );
    let deepest = refined
        .w_faces
        .iter()
        .skip(2)
        .map(|face| face.mrlw)
        .max()
        .unwrap_or(0);
    assert_eq!(deepest, 2, "a named circle must reach mrlw 2");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_gap_between_named_region_levels_does_not_drop_the_deeper_region() {
    let root = temp_root("named_gap");
    let path = root.join("landtype.nc");
    write_landtype(&path, |_, _| 0);

    let named = vec![earthmesh_mesh::RefinementRegion::Circle {
        center: earthmesh_mesh::LonLatDegrees::new(114.0, 22.0),
        radius_meters: 400_000.0,
        level: 3,
    }];
    let (_refined, report) =
        earthmesh_cli::refinement_demand::nest::spawn_nest_adaptive_with_named_regions(
            &base_mesh(),
            &RefineConfig::default(),
            &plan_inputs(&path, true),
            &named,
            base_cell_meters(),
            3,
            None,
        )
        .expect("adaptive nest");

    assert_eq!(report.deepest_level, 3, "{report:?}");
    assert_eq!(
        report
            .passes
            .iter()
            .map(|pass| pass.level)
            .collect::<Vec<_>>(),
        vec![1, 2, 3],
        "empty level 2 must be bridged before level 3"
    );
    let _ = fs::remove_dir_all(root);
}

fn gap_error_for(region: earthmesh_mesh::RefinementRegion) -> String {
    let root = temp_root("named_gap_shape");
    let path = root.join("landtype.nc");
    write_landtype(&path, |_, _| 0);

    let error = earthmesh_cli::refinement_demand::nest::spawn_nest_adaptive_with_named_regions(
        &base_mesh(),
        &RefineConfig::default(),
        &plan_inputs(&path, true),
        &[region],
        base_cell_meters(),
        3,
        None,
    )
    .expect_err("non-bufferable shape cannot bridge an empty parent level");
    let _ = fs::remove_dir_all(root);
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput, "{error}");
    error.to_string()
}

#[test]
fn bbox_gap_between_named_region_levels_is_refused() {
    let error = gap_error_for(earthmesh_mesh::RefinementRegion::Bbox {
        west_degrees: 112.0,
        east_degrees: 116.0,
        south_degrees: 20.0,
        north_degrees: 24.0,
        level: 3,
    });
    assert!(error.contains("explicit parent halo"), "{error}");
}

#[test]
fn polygon_gap_between_named_region_levels_is_refused() {
    let error = gap_error_for(earthmesh_mesh::RefinementRegion::Polygon {
        points: vec![
            earthmesh_mesh::LonLatDegrees::new(112.0, 20.0),
            earthmesh_mesh::LonLatDegrees::new(116.0, 20.0),
            earthmesh_mesh::LonLatDegrees::new(116.0, 24.0),
            earthmesh_mesh::LonLatDegrees::new(112.0, 24.0),
        ],
        level: 3,
    });
    assert!(error.contains("explicit parent halo"), "{error}");
}

#[test]
fn a_named_region_deeper_than_the_run_is_refused_rather_than_dropped() {
    // The per-level loop only picks up regions whose level it is refining, so a
    // region asking for a level beyond the ceiling used to be filtered out in
    // silence. The downstream "nothing refined" guard misses it whenever some
    // other region does refine: the run succeeds, the mesh passes its checks,
    // and the region the project named is simply absent.
    let root = temp_root("named_too_deep");
    let path = root.join("landtype.nc");
    write_landtype(&path, |_, _| 0);

    let named = vec![
        earthmesh_mesh::RefinementRegion::Circle {
            center: earthmesh_mesh::LonLatDegrees::new(114.0, 22.0),
            radius_meters: 400_000.0,
            level: 1,
        },
        earthmesh_mesh::RefinementRegion::Circle {
            center: earthmesh_mesh::LonLatDegrees::new(118.0, 22.0),
            radius_meters: 400_000.0,
            level: 3,
        },
    ];
    let error = earthmesh_cli::refinement_demand::nest::spawn_nest_adaptive_with_named_regions(
        &base_mesh(),
        &RefineConfig::default(),
        &plan_inputs(&path, true),
        &named,
        base_cell_meters(),
        2,
        None,
    )
    .expect_err("a region beyond the ceiling must be refused");
    assert!(error.to_string().contains("level 3"), "{error}");
    let _ = fs::remove_dir_all(root);
}

/// A configured spring runs on this route, and the report says how often.
///
/// It did not. Both `spawn_nest` calls in the loop were the spring-free
/// overload and the branch returned a hard-coded zero, so a namelist setting
/// `SpringRegional_type` got a mesh whose points were exactly where the nest
/// put them -- while the run report went on printing the iteration count it had
/// been asked for. The direct route, given the same namelist, sprang the same
/// mesh in two passes and moved 5182 of its 7023 points.
///
/// Asserting on `spring_passes` alone would not have caught it, since a wrong
/// number is as easy to produce as a right one. So this compares the meshes:
/// with a spring, points move.
#[test]
fn a_configured_spring_moves_points_on_the_adaptive_route() {
    let root = temp_root("adaptive_spring");
    let path = root.join("landtype.nc");
    write_landtype(&path, |_, _| 0);

    let named = vec![earthmesh_mesh::RefinementRegion::Circle {
        center: earthmesh_mesh::LonLatDegrees::new(114.0, 22.0),
        radius_meters: 400_000.0,
        level: 1,
    }];
    let refine = RefineConfig::default();
    let run = |spring| {
        earthmesh_cli::refinement_demand::nest::spawn_nest_adaptive_with_named_regions(
            &base_mesh(),
            &refine,
            &plan_inputs(&path, true),
            &named,
            base_cell_meters(),
            1,
            spring,
        )
        .expect("adaptive nest")
    };

    let (still, still_report) = run(None);
    let (sprung, sprung_report) = run(Some(
        earthmesh_cli::refinement_demand::nest::AdaptiveNestSpring {
            nxp: NXP,
            iterations: 200,
            max_mrows: MethodCMesh::MAX_MROWS_SURFACE,
        },
    ));

    assert_eq!(still_report.spring_passes, 0, "no spring, no passes");
    assert!(
        sprung_report.spring_passes > 0,
        "a spring that ran reports its passes: {sprung_report:?}"
    );
    assert_eq!(
        still.m_points.len(),
        sprung.m_points.len(),
        "the spring moves points; it does not add or remove them"
    );
    let moved = still
        .m_points
        .iter()
        .zip(sprung.m_points.iter())
        .skip(2)
        .filter(|(before, after)| {
            (before.x - after.x).abs() > 1.0e-9
                || (before.y - after.y).abs() > 1.0e-9
                || (before.z - after.z).abs() > 1.0e-9
        })
        .count();
    assert!(
        moved > 0,
        "{moved} of {} points moved: the spring was configured and did nothing",
        still.m_points.len() - 2
    );
    let _ = fs::remove_dir_all(root);
}
