//! Verifies the Plan 03 progress/cancellation seam: the spring relaxation loop
//! reports progress through `earthmesh_core::progress`, and a callback returning
//! `false` cancels the run.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

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
