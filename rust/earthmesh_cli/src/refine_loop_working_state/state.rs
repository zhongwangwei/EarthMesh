use crate::*;
use std::fs;
use std::io;

/// In-memory Fortran-indexed working arrays for incrementally replacing
/// `MOD_refine.F90:refine_loop`.
///
/// The top-level executor still owns file scheduling; this state owns the
/// geometry arrays that the migrated Rust adapters mutate in Fortran order.
#[derive(Debug, Clone, PartialEq)]
pub struct RefineLoopWorkingState {
    pub iter: usize,
    pub num_vertex: usize,
    pub num_mp: Vec<usize>,
    pub num_wp: Vec<usize>,
    pub num_sjx: usize,
    pub num_dbx: usize,
    pub num_tranrow_sjx: usize,
    pub mp_new: Vec<LonLatPoint>,
    pub wp_new: Vec<LonLatPoint>,
    pub ngrmw: Vec<Vec<usize>>,
    pub ngrmw_new: Vec<Vec<usize>>,
    pub ngrwm: Vec<Vec<usize>>,
    pub n_ngrwm: Vec<usize>,
    pub mp_f: Vec<LonLatPoint>,
    pub wp_f: Vec<LonLatPoint>,
    pub ngrmw_f: Vec<Vec<usize>>,
    pub ngrwm_f: Vec<Vec<usize>>,
    pub n_ngrwm_f: Vec<usize>,
    pub ref_sjx: Vec<i32>,
    pub ref_lbx: Vec<i32>,
    pub mrl_new: Vec<i32>,
    pub triangle_neighbors: Vec<Vec<usize>>,
    pub segments: Vec<Vec<usize>>,
    pub n_segments: Vec<usize>,
    pub sjx_child: Vec<[usize; 2]>,
    pub weak_concav_pair: Vec<[usize; 2]>,
    pub weak_concav_segment: Vec<Vec<usize>>,
    pub weak_concav_segment_old: Vec<Vec<usize>>,
    pub n_weak_concav_segment: Vec<usize>,
    pub bdy_refine_segment: Vec<Vec<usize>>,
    pub bdy_refine_segment_old: Vec<Vec<usize>>,
    pub n_bdy_refine_segment: Vec<usize>,
    pub ref_sjx_segment_temp: Vec<Vec<usize>>,
    pub n_ref_sjx_segment_temp: Vec<usize>,
    pub ref_sjx_segment: Vec<usize>,
    pub num_ref: usize,
    pub bdy_refine: Vec<usize>,
    pub bdy_refine_tran: Vec<usize>,
}

/// Execute the `refine_loop` prologue and return the Fortran-indexed Rust
/// working state that subsequent migrated geometry adapters can mutate.
pub fn run_mkgrd_refine_loop_working_state_prologue(
    step: &MkgrdRefineLoopStepIoPlan,
) -> io::Result<MkgrdRefineLoopWorkingStatePrologueReport> {
    let mesh = read_unstructured_mesh_netcdf(&step.refine_loop_input_gridfile)?;
    crate::ensure_parent_dir(&step.refine_loop_original_tmpfile)?;
    let copied_bytes = fs::copy(
        &step.refine_loop_input_gridfile,
        &step.refine_loop_original_tmpfile,
    )?;
    let snapshot = MkgrdRefineLoopPrologueSnapshotReport {
        input_gridfile: step.refine_loop_input_gridfile.clone(),
        original_tmpfile: step.refine_loop_original_tmpfile.clone(),
        copied_bytes,
        sjx_points: mesh.m_points.len(),
        lbx_points: mesh.w_points.len(),
    };
    let state = RefineLoopWorkingState::from_unstructured_mesh(&mesh);
    Ok(MkgrdRefineLoopWorkingStatePrologueReport { snapshot, state })
}
