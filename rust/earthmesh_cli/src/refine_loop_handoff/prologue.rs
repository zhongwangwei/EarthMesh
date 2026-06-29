use std::fs;
use std::io;

use crate::*;

/// Execute only the `MOD_refine.F90:refine_loop` prologue that reads the
/// current unstructured grid and copies it to `tmpfile/*_ori.nc4`.
///
/// This intentionally does not perform one-into-four refinement, transition
/// building, LOP, `NGR_RENEW`, or final gridfile writing. It is a safe adapter
/// boundary for wiring the remaining geometry kernels incrementally.
pub fn run_mkgrd_refine_loop_prologue_snapshot(
    step: &MkgrdRefineLoopStepIoPlan,
) -> io::Result<MkgrdRefineLoopPrologueSnapshotReport> {
    let mesh = read_unstructured_mesh_netcdf(&step.refine_loop_input_gridfile)?;
    crate::ensure_parent_dir(&step.refine_loop_original_tmpfile)?;
    let copied_bytes = fs::copy(
        &step.refine_loop_input_gridfile,
        &step.refine_loop_original_tmpfile,
    )?;
    Ok(MkgrdRefineLoopPrologueSnapshotReport {
        input_gridfile: step.refine_loop_input_gridfile.clone(),
        original_tmpfile: step.refine_loop_original_tmpfile.clone(),
        copied_bytes,
        sjx_points: mesh.m_points.len(),
        lbx_points: mesh.w_points.len(),
    })
}
