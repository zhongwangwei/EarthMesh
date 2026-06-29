use std::io;

use crate::*;

impl RefineLoopWorkingState {
    /// Apply migrated `OnedivideTwo` and write transition child geometry plus
    /// `sjx_child` mapping back into this state.
    pub fn apply_onedivide_two(&mut self, is_reverse: bool) -> io::Result<OnedivideTwoReport> {
        if let Some(&old_mp) = self
            .iter
            .checked_sub(1)
            .and_then(|idx| self.num_mp.get(idx))
        {
            if self.sjx_child.len() <= old_mp {
                self.sjx_child.resize(old_mp + 1, [0, 0]);
            }
        }
        apply_onedivide_two_fortran_indexed(
            self.iter,
            is_reverse,
            self.num_vertex,
            &self.num_mp,
            &self.num_wp,
            &self.triangle_neighbors,
            &self.ngrmw,
            &self.ref_sjx,
            &self.mrl_new,
            &mut self.mp_new,
            &mut self.wp_new,
            &mut self.ngrmw_new,
            &mut self.sjx_child,
        )
    }

    /// Apply the migrated `NGR_RENEW` adapter to this working state and store
    /// the final compacted arrays in `*_f` fields.
    pub fn apply_ngr_renew(&mut self) -> io::Result<NgrRenewReport> {
        let report = apply_ngr_renew_fortran_indexed(
            self.iter,
            self.num_vertex,
            &self.num_mp,
            &self.num_wp,
            &self.mp_new,
            &self.wp_new,
            &self.ngrmw_new,
            &mut self.mp_f,
            &mut self.wp_f,
            &mut self.ngrmw_f,
            &mut self.ngrwm_f,
            &mut self.n_ngrwm_f,
            &mut self.bdy_refine,
            &mut self.bdy_refine_tran,
        )?;
        self.num_sjx = report.num_sjx;
        self.num_dbx = report.num_dbx;
        Ok(report)
    }
}
