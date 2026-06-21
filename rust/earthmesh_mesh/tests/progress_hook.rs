//! Verifies the Plan 03 progress/cancellation seam: the spring relaxation loop
//! reports progress through `earthmesh_core::progress`, and a callback returning
//! `false` cancels the run.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[test]
fn spring_loop_reports_progress() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    earthmesh_core::progress::set(move |phase, _done, _total| {
        if phase == "spring" {
            counter.fetch_add(1, Ordering::Relaxed);
        }
        true
    });

    // Small in-memory gridinit (no NetCDF I/O); runs the spring loop.
    let state = earthmesh_mesh::gridinit_voronoi_state_fortran(6, 60, 1.0, 0.035, 100_000);
    earthmesh_core::progress::clear();

    assert!(state.is_ok(), "gridinit should succeed");
    assert!(
        calls.load(Ordering::Relaxed) > 0,
        "the spring loop should report progress via the callback"
    );
}

#[test]
fn cancellation_aborts_gridinit() {
    // Returning false on the first report requests cancellation immediately.
    earthmesh_core::progress::set(|_phase, _done, _total| false);
    let state = earthmesh_mesh::gridinit_voronoi_state_fortran(6, 5000, 1.0, 0.035, 100_000);
    earthmesh_core::progress::clear();

    assert!(
        state.is_err(),
        "a cancelling callback should abort the spring loop and fail gridinit"
    );
}

#[test]
fn olam_nest_spring_reports_at_fortran_nprnt_interval() {
    let reports = Arc::new(Mutex::new(Vec::new()));
    let recorded = reports.clone();
    earthmesh_core::progress::set(move |phase, done, _total| {
        if phase == "olam-nest-spring" {
            recorded.lock().expect("record progress").push(done);
        }
        true
    });

    let mesh = earthmesh_mesh::OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100)
        .expect("base OLAM mesh");
    let region = earthmesh_mesh::OlamRefinementRegion::Circle {
        center: earthmesh_mesh::LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let refined = mesh.spawn_nest(&[region], 1).expect("local circle nest");
    let result = refined.spring_nest(6, 120, 2, false);
    earthmesh_core::progress::clear();

    assert!(result.is_ok(), "OLAM nest spring should succeed");
    assert_eq!(
        reports.lock().expect("read progress").as_slice(),
        &[1, 100, 120],
        "Fortran spring_dynamics_nest reports iter 1, iter mod nprnt == 0 with nprnt=100, and final iter"
    );
}
