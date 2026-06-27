use std::io;

use crate::*;

impl RefineLoopWorkingState {
    /// Apply migrated `weak_concav_pair_special` to the state's weak-concavity
    /// pair and temporary segment workspace.
    pub fn apply_weak_concav_pair_special(
        &mut self,
        num_weak_concav_pair: usize,
        num_ref_weak_concav: usize,
    ) -> io::Result<WeakConcavPairSpecialReport> {
        let max_triangle = self
            .iter
            .checked_sub(1)
            .and_then(|idx| self.num_mp.get(idx))
            .copied()
            .filter(|&value| value > 0)
            .unwrap_or_else(|| {
                self.ngrmw
                    .get(1)
                    .map_or(0, |row| row.len().saturating_sub(1))
            });
        apply_weak_concav_pair_special_fortran_indexed(
            num_weak_concav_pair,
            num_ref_weak_concav,
            max_triangle,
            &self.triangle_neighbors,
            &self.ngrmw,
            &mut self.mrl_new,
            &mut self.ref_sjx,
            &mut self.weak_concav_pair,
            &mut self.weak_concav_segment,
        )
    }

    /// Apply migrated `Delaunay_Lop` to the state's queued LOP reference
    /// segment and renewed geometry/connectivity arrays.
    pub fn apply_delaunay_lop(&mut self) -> io::Result<DelaunayLopReport> {
        apply_delaunay_lop_fortran_indexed(
            self.iter,
            self.num_ref,
            &self.num_mp,
            &self.num_wp,
            &mut self.mp_new,
            &mut self.wp_new,
            &mut self.ngrmw_new,
            &self.ref_sjx_segment,
        )
    }

    /// Apply migrated `sharp_concav_lop_judge` to the state's boundary-refine
    /// segment work arrays and accumulated LOP reference count.
    pub fn apply_sharp_concav_lop_judge(
        &mut self,
        num_bdy_refine_segment: usize,
    ) -> io::Result<SharpConcavLopJudgeReport> {
        let max_triangle = self.num_mp.get(self.iter).copied().unwrap_or_else(|| {
            self.ngrmw_new
                .get(1)
                .map_or(0, |row| row.len().saturating_sub(1))
        });
        apply_sharp_concav_lop_judge_fortran_indexed(
            &mut self.num_ref,
            num_bdy_refine_segment,
            max_triangle,
            &self.mrl_new,
            &self.triangle_neighbors,
            &self.ngrmw_new,
            &self.sjx_child,
            &self.bdy_refine_segment,
            &self.bdy_refine_segment_old,
            &self.n_bdy_refine_segment,
            &mut self.ref_sjx_segment_temp,
            &mut self.n_ref_sjx_segment_temp,
        )
    }

    /// Apply migrated `weak_concav_lop_judge` to the state's weak-concavity
    /// LOP work arrays and accumulated LOP reference count.
    pub fn apply_weak_concav_lop_judge(
        &mut self,
        num_bdy_refine_segment: usize,
        num_ref_weak_concav: usize,
        num_weak_concav_segment: usize,
        num_weak_concav_pair: usize,
    ) -> io::Result<SharpConcavLopJudgeReport> {
        let max_triangle = self.num_mp.get(self.iter).copied().unwrap_or_else(|| {
            self.ngrmw_new
                .get(1)
                .map_or(0, |row| row.len().saturating_sub(1))
        });
        apply_weak_concav_lop_judge_fortran_indexed(
            &mut self.num_ref,
            num_bdy_refine_segment,
            num_ref_weak_concav,
            num_weak_concav_segment,
            num_weak_concav_pair,
            max_triangle,
            &self.mrl_new,
            &self.triangle_neighbors,
            &self.ngrmw_new,
            &self.sjx_child,
            &mut self.weak_concav_segment,
            &self.weak_concav_segment_old,
            &self.n_weak_concav_segment,
            &self.weak_concav_pair,
            &mut self.ref_sjx_segment_temp,
            &mut self.n_ref_sjx_segment_temp,
        )
    }

    /// Apply migrated `ref_sjx_isreverse_judge` to the state's transition
    /// segment workspace and replace `ref_sjx` with the returned markers.
    pub fn apply_isreverse_judge(&mut self, set_dis_in: usize) -> io::Result<IsreverseJudgeReport> {
        let report = apply_isreverse_judge_fortran_indexed(
            set_dis_in,
            self.n_segments.len(),
            &self.triangle_neighbors,
            &self.mrl_new,
            &mut self.segments,
            &self.n_segments,
        )?;
        self.ref_sjx = report.ref_sjx.clone();
        Ok(report)
    }
}
