use std::io;
use std::path::Path;

use crate::refine_loop_adapters::fortran_rows_to_triangle_major;
use crate::*;

impl RefineLoopWorkingState {
    /// Apply migrated `Array_length_calculation` using the state's base
    /// connectivity, update transition-row and boundary lists, and write the
    /// legacy close-mesh scratch files.
    pub fn apply_array_length_calculation(
        &mut self,
        file_dir: impl AsRef<Path>,
        step: usize,
        set_dis_in: usize,
    ) -> io::Result<RefineArrayLengthCalculationRunReport> {
        let sjx_points = *self
            .num_mp
            .get(1)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "num_mp[1] is required"))?;
        let lbx_points = *self
            .num_wp
            .get(1)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "num_wp[1] is required"))?;
        let cells_on_triangle = fortran_rows_to_triangle_major(&self.ngrmw, sjx_points)?;
        if self.n_ngrwm.len() <= lbx_points {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("num_wp[1] {lbx_points} requires one-based n_ngrwm storage"),
            ));
        }
        let mut triangles_on_cell = vec![Vec::<usize>::new(); lbx_points + 1];
        for (cell, target) in triangles_on_cell
            .iter_mut()
            .enumerate()
            .take(lbx_points + 1)
            .skip(1)
        {
            let count = self.n_ngrwm[cell];
            if self.ngrwm.len() <= count
                || self.ngrwm[1..=count].iter().any(|row| row.len() <= cell)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("cell {cell} requires ngrwm rows 1..={count}"),
                ));
            }
            target.extend((1..=count).map(|row| self.ngrwm[row][cell]));
        }
        let report = run_refine_array_length_calculation_fortran_indexed(
            file_dir,
            step,
            set_dis_in,
            self.num_vertex,
            self.num_vertex,
            sjx_points,
            lbx_points,
            &self.mrl_new,
            &self.triangle_neighbors,
            &cells_on_triangle,
            &triangles_on_cell,
            &self.n_ngrwm,
            self.num_tranrow_sjx,
            &self.wp_new,
        )?;
        self.num_tranrow_sjx = report.calculation.halo.num_transition_row_triangles;
        self.bdy_refine = report.calculation.halo.boundary_refine.clone();
        self.bdy_refine_tran = report.calculation.halo.boundary_refine_transition.clone();
        Ok(report)
    }

    /// Apply migrated `OnedivideFour_connection` using the state's current
    /// refinement markers and base triangle connectivity.
    pub fn apply_onedivide_four_connection(&mut self) -> io::Result<OnedivideFourConnectionReport> {
        let sjx_points = *self
            .num_mp
            .get(1)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "num_mp[1] is required"))?;
        apply_onedivide_four_connection_fortran_indexed(
            self.num_vertex,
            sjx_points,
            &self.ref_sjx,
            &self.ngrmw,
            &mut self.ref_lbx,
            &mut self.mrl_new,
        )
    }

    /// Apply migrated `OnedivideFour_renew` and write child points/connectivity
    /// back into this state's `*_new` arrays.
    pub fn apply_onedivide_four_renew(&mut self) -> io::Result<OnedivideFourRenewReport> {
        apply_onedivide_four_renew_fortran_indexed(
            self.num_vertex,
            self.iter,
            &self.ngrmw,
            &self.ref_sjx,
            &self.num_mp,
            &self.num_wp,
            &mut self.mp_new,
            &mut self.wp_new,
            &mut self.ngrmw_new,
        )
    }

    /// Look up the child triangle pair corresponding to a parent triangle/cell
    /// pair using the state's current `sjx_child` and renewed connectivity.
    pub fn lookup_m1w1_to_m11w11(&self, m1: usize, w1: usize) -> io::Result<M1W1LookupReport> {
        let max_triangle = self.num_mp.get(self.iter).copied().unwrap_or_else(|| {
            self.ngrmw_new
                .get(1)
                .map_or(0, |row| row.len().saturating_sub(1))
        });
        lookup_m1w1_to_m11w11_fortran_indexed(
            m1,
            w1,
            &self.sjx_child,
            &self.ngrmw_new,
            max_triangle,
        )
    }
}
