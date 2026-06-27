use std::io;
use std::path::Path;

use super::MkgrdRefineLoopWorkingStateExecutor;
use crate::refine_loop_transition_helpers::{
    apply_previous_refine_region_prefilter, apply_transition_onedivide_two,
    empty_refine_array_length_report, fortran_index_segments, identity_ngr_renew_report,
    marked_triangles_have_valid_neighbors, refresh_working_vertex_membership_from_ngrmw_new,
    remove_isolated_one_into_four_markers, transition_cell_views,
};
use crate::*;

impl MkgrdRefineLoopWorkingStateExecutor {
    pub(super) fn run_configured_one_into_two_transition_pipeline(
        &self,
        state: &mut RefineLoopWorkingState,
        ref_sjx: &[i32],
        mrl_new: &[i32],
    ) -> io::Result<OnedivideTwoReport> {
        let old_mp = *state
            .num_mp
            .get(1)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "num_mp[1] is required"))?;
        let old_wp = *state
            .num_wp
            .get(1)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "num_wp[1] is required"))?;
        if ref_sjx.len() <= old_mp || mrl_new.len() <= old_mp {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "one-into-two transition requires placeholder plus {old_mp} ref_sjx/mrl_new entries"
                ),
            ));
        }

        let num_vertex = self.num_vertex.max(1).min(old_mp);
        state.num_vertex = num_vertex;
        state.ref_sjx = ref_sjx[..=old_mp]
            .iter()
            .map(|&marker| if marker > 1 { 1 } else { marker })
            .collect();
        state.mrl_new = mrl_new[..=old_mp].to_vec();
        let marked_triangles = state.ref_sjx[num_vertex + 1..=old_mp]
            .iter()
            .filter(|&&marker| marker != 0)
            .count();

        state.iter = 2;
        if state.num_mp.len() <= state.iter {
            state.num_mp.resize(state.iter + 1, 0);
        }
        if state.num_wp.len() <= state.iter {
            state.num_wp.resize(state.iter + 1, 0);
        }
        state.num_mp[state.iter] = old_mp + marked_triangles * 2;
        state.num_wp[state.iter] = old_wp + marked_triangles;
        state.mp_new.resize(
            state.num_mp[state.iter] + 1,
            LonLatPoint { lon: 0.0, lat: 0.0 },
        );
        state.wp_new.resize(
            state.num_wp[state.iter] + 1,
            LonLatPoint { lon: 0.0, lat: 0.0 },
        );
        if state.ngrmw_new.len() <= 3 {
            state.ngrmw_new.resize_with(4, Vec::new);
        }
        for row in &mut state.ngrmw_new[1..=3] {
            row.resize(state.num_mp[state.iter] + 1, 0);
        }
        state.sjx_child.resize(old_mp + 1, [0, 0]);
        if let Some(triangle_neighbors) = &self.one_into_two_triangle_neighbors {
            state.triangle_neighbors = triangle_neighbors.clone();
        }

        let report = state.apply_onedivide_two(self.one_into_two_is_reverse)?;
        refresh_working_vertex_membership_from_ngrmw_new(state)?;
        Ok(report)
    }

    pub(super) fn run_configured_one_into_four_pipeline(
        &self,
        step: &MkgrdRefineLoopStepIoPlan,
        state: &mut RefineLoopWorkingState,
        ref_sjx: &[i32],
    ) -> io::Result<(
        OnedivideFourConnectionReport,
        RefineArrayLengthCalculationRunReport,
        OnedivideFourRenewReport,
        NgrRenewReport,
        Option<(usize, usize)>,
    )> {
        let old_mp = *state
            .num_mp
            .get(1)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "num_mp[1] is required"))?;
        let old_wp = *state
            .num_wp
            .get(1)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "num_wp[1] is required"))?;
        if ref_sjx.len() <= old_mp {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("one-into-four ref_sjx must include placeholder plus {old_mp} triangles"),
            ));
        }
        let file_dir = step
            .refine_loop_input_gridfile
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "refine_loop_input_gridfile must live under <file_dir>/gridfile",
                )
            })?;
        let set_dis_in = if self.set_dis_in > 0 {
            self.set_dis_in
        } else {
            step.max_transition_row
        };
        let num_vertex = self.num_vertex.max(1).min(old_mp);
        state.num_vertex = num_vertex;
        state.ref_sjx = ref_sjx[..=old_mp]
            .iter()
            .map(|&marker| if marker > 1 { 1 } else { marker })
            .collect();
        if step.step > 1 {
            apply_previous_refine_region_prefilter(
                file_dir,
                step.step - 1,
                set_dis_in + 1,
                old_mp,
                old_wp,
                state,
            )?;
        }
        if set_dis_in == 0 {
            remove_isolated_one_into_four_markers(
                num_vertex,
                old_mp,
                &state.triangle_neighbors,
                &mut state.ref_sjx,
            )?;
        }
        let active_refine_start = num_vertex + 1;
        if active_refine_start > old_mp
            || !state.ref_sjx[active_refine_start..=old_mp]
                .iter()
                .any(|&marker| marker != 0)
        {
            return Ok((
                OnedivideFourConnectionReport {
                    marked_triangles: Vec::new(),
                    marked_vertices: Vec::new(),
                },
                empty_refine_array_length_report(),
                OnedivideFourRenewReport {
                    refined_triangles: Vec::new(),
                    new_triangle_ids: Vec::new(),
                    new_vertex_ids: Vec::new(),
                    dateline_adjusted: false,
                },
                identity_ngr_renew_report(old_mp, old_wp, state),
                None,
            ));
        }
        let mut ref_sjx_segment = state.ref_sjx.clone();
        state.num_tranrow_sjx = ref_sjx_segment[num_vertex + 1..=old_mp]
            .iter()
            .filter(|&&marker| marker != 0)
            .count();

        let mut connection = state.apply_onedivide_four_connection()?;
        let can_expand_transition = marked_triangles_have_valid_neighbors(
            num_vertex,
            old_mp,
            &state.triangle_neighbors,
            &state.ref_sjx,
        );
        if can_expand_transition {
            self.expand_one_into_four_transition_band(
                state,
                num_vertex,
                old_mp,
                old_wp,
                set_dis_in,
                &mut ref_sjx_segment,
                &mut connection,
            )?;
        }
        state.ref_sjx = ref_sjx_segment;
        state.num_tranrow_sjx = state.ref_sjx[num_vertex + 1..=old_mp]
            .iter()
            .filter(|&&marker| marker != 0)
            .count();
        let length = state.apply_array_length_calculation(file_dir, step.step, set_dis_in)?;

        state.iter = 2;
        if state.num_mp.len() <= state.iter {
            state.num_mp.resize(state.iter + 1, 0);
        }
        if state.num_wp.len() <= state.iter {
            state.num_wp.resize(state.iter + 1, 0);
        }
        let reserved_capacity = state.num_tranrow_sjx * 4;
        state.num_mp[state.iter] = old_mp + reserved_capacity;
        state.num_wp[state.iter] = old_wp + reserved_capacity;
        state.mp_new.resize(
            state.num_mp[state.iter] + 1,
            LonLatPoint { lon: 0.0, lat: 0.0 },
        );
        state.wp_new.resize(
            state.num_wp[state.iter] + 1,
            LonLatPoint { lon: 0.0, lat: 0.0 },
        );
        if state.ngrmw_new.len() <= 3 {
            state.ngrmw_new.resize_with(4, Vec::new);
        }
        for row in &mut state.ngrmw_new[1..=3] {
            row.resize(state.num_mp[state.iter] + 1, 1);
        }

        let renew = state.apply_onedivide_four_renew()?;
        state.num_mp[state.iter] = old_mp + renew.new_triangle_ids.len();
        state.num_wp[state.iter] = old_wp + renew.new_vertex_ids.len();
        let num_lop = self.apply_forward_transition_rows(state, set_dis_in, &length)?;
        let ngr = state.apply_ngr_renew()?;
        let output_mesh = state.to_unstructured_mesh()?;
        let post_counts = Some(refine_loop_post_counts_fortran_indexed(
            old_mp,
            old_wp,
            *state.num_mp.get(state.iter).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "refine working state is missing expanded num_mp(iter)",
                )
            })?,
            &output_mesh,
            num_lop,
        )?);
        Ok((connection, length, renew, ngr, post_counts))
    }

    pub(super) fn expand_one_into_four_transition_band(
        &self,
        state: &mut RefineLoopWorkingState,
        num_vertex: usize,
        old_mp: usize,
        old_wp: usize,
        set_dis_in: usize,
        ref_sjx_segment: &mut [i32],
        connection: &mut OnedivideFourConnectionReport,
    ) -> io::Result<()> {
        if set_dis_in == 0 {
            return Ok(());
        }

        loop {
            let mut changed_outer = false;

            loop {
                let markers = refine_iter_b_judge_fortran_indexed(
                    set_dis_in,
                    num_vertex,
                    &state.triangle_neighbors,
                    &state.mrl_new,
                )?;
                if !self.apply_transition_markers(
                    state,
                    num_vertex,
                    old_mp,
                    &markers,
                    ref_sjx_segment,
                    connection,
                )? {
                    break;
                }
                changed_outer = true;
            }

            loop {
                let (cells_on_triangle, triangles_on_cell) =
                    transition_cell_views(state, old_mp, old_wp)?;
                let markers = refine_iter_c_judge_fortran_indexed(
                    set_dis_in,
                    num_vertex,
                    num_vertex,
                    old_wp,
                    &state.triangle_neighbors,
                    &triangles_on_cell,
                    &state.n_ngrwm,
                    &state.mrl_new,
                    &state.ref_lbx,
                )?;
                drop(cells_on_triangle);
                if !self.apply_transition_markers(
                    state,
                    num_vertex,
                    old_mp,
                    &markers,
                    ref_sjx_segment,
                    connection,
                )? {
                    break;
                }
                changed_outer = true;
            }

            let (cells_on_triangle, triangles_on_cell) =
                transition_cell_views(state, old_mp, old_wp)?;
            let markers = refine_iter_e_judge_fortran_indexed(
                num_vertex,
                old_wp,
                &cells_on_triangle,
                &triangles_on_cell,
                &state.n_ngrwm,
                &state.mrl_new,
                &state.ref_lbx,
            )?;
            if self.apply_transition_markers(
                state,
                num_vertex,
                old_mp,
                &markers,
                ref_sjx_segment,
                connection,
            )? {
                changed_outer = true;
            }

            if changed_outer {
                continue;
            }

            loop {
                let (_, triangles_on_cell) = transition_cell_views(state, old_mp, old_wp)?;
                let markers = refine_iter_g_judge_fortran_indexed(
                    num_vertex,
                    old_wp,
                    &triangles_on_cell,
                    &state.n_ngrwm,
                    &state.mrl_new,
                )?;
                if !self.apply_transition_markers(
                    state,
                    num_vertex,
                    old_mp,
                    &markers,
                    ref_sjx_segment,
                    connection,
                )? {
                    break;
                }
                changed_outer = true;
            }

            if !changed_outer {
                break;
            }
        }

        Ok(())
    }

    pub(super) fn apply_transition_markers(
        &self,
        state: &mut RefineLoopWorkingState,
        num_vertex: usize,
        old_mp: usize,
        markers: &[i32],
        ref_sjx_segment: &mut [i32],
        connection: &mut OnedivideFourConnectionReport,
    ) -> io::Result<bool> {
        if markers.len() <= old_mp {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("transition markers must include placeholder plus {old_mp} triangles"),
            ));
        }
        state.ref_sjx.fill(0);
        let mut any = false;
        for triangle in (num_vertex + 1)..=old_mp {
            if markers[triangle] == 0 || state.mrl_new[triangle] == 4 {
                continue;
            }
            state.ref_sjx[triangle] = 1;
            ref_sjx_segment[triangle] = 1;
            any = true;
        }
        if !any {
            return Ok(false);
        }
        let added = state.apply_onedivide_four_connection()?;
        for triangle in added.marked_triangles {
            if !connection.marked_triangles.contains(&triangle) {
                connection.marked_triangles.push(triangle);
            }
        }
        for vertex in added.marked_vertices {
            if !connection.marked_vertices.contains(&vertex) {
                connection.marked_vertices.push(vertex);
            }
        }
        Ok(true)
    }

    pub(super) fn apply_forward_transition_rows(
        &self,
        state: &mut RefineLoopWorkingState,
        set_dis_in: usize,
        length: &RefineArrayLengthCalculationRunReport,
    ) -> io::Result<usize> {
        if set_dis_in == 0 || length.calculation.boundary.curves.num_closed_curve == 0 {
            return Ok(0);
        }

        let old_mp = *state
            .num_mp
            .get(1)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "num_mp[1] is required"))?;
        let old_wp = *state
            .num_wp
            .get(1)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "num_wp[1] is required"))?;
        let (_, triangles_on_cell) = transition_cell_views(state, old_mp, old_wp)?;
        let close_curves = length.calculation.boundary.curves.close_curves
            [1..=length.calculation.boundary.curves.num_closed_curve]
            .to_vec();
        let boundary_segments = refine_boundary_segments_make_fortran_indexed(
            set_dis_in,
            &close_curves,
            &triangles_on_cell,
            &state.n_ngrwm,
            &state.mrl_new,
        )?;
        if boundary_segments.num_bdy_refine_segment == 0 {
            return Ok(0);
        }

        let mut segments = boundary_segments.bdy_refine_segment;
        for segment in &mut segments {
            segment.resize(set_dis_in, 1);
        }
        let mut segment_lengths = boundary_segments.n_bdy_refine_segment;
        state.sjx_child.resize(old_mp + 1, [0, 0]);
        let mut num_lop = 0usize;

        for transition_row in 1..=set_dis_in {
            let segments_old = segments.clone();
            state.ref_sjx.fill(0);
            for (segment, remaining) in segments.iter().zip(segment_lengths.iter_mut()) {
                if *remaining == 0 {
                    continue;
                }
                for &triangle in segment.iter().take(*remaining) {
                    if triangle == 1 {
                        break;
                    }
                    if triangle != 0
                        && triangle < state.ref_sjx.len()
                        && state.mrl_new[triangle] != 4
                    {
                        state.ref_sjx[triangle] = 1;
                    }
                }
                *remaining = remaining.saturating_sub(1);
            }

            let num_ref = state.ref_sjx.iter().filter(|&&marker| marker != 0).count();
            if num_ref == 0 {
                if transition_row == 1 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "transition row 1 has no forward one-into-two triangles",
                    ));
                }
                break;
            }

            apply_transition_onedivide_two(state, num_ref, false)?;

            if transition_row >= set_dis_in {
                continue;
            }

            let reverse = apply_isreverse_judge_fortran_indexed(
                set_dis_in,
                segments.len(),
                &state.triangle_neighbors,
                &state.mrl_new,
                &mut segments,
                &segment_lengths,
            )?;
            state.ref_sjx = reverse.ref_sjx;
            let reverse_ref = state.ref_sjx.iter().filter(|&&marker| marker != 0).count();
            if reverse_ref != 0 {
                apply_transition_onedivide_two(state, reverse_ref, true)?;
            }

            let num_end = 4 * (set_dis_in - 1);
            if num_end == 0 {
                continue;
            }
            let num_segment = segments.len();
            let mut segment_temp = vec![vec![1_usize; num_end + 1]; num_segment + 1];
            let mut n_segment_temp = Vec::with_capacity(num_segment + 1);
            n_segment_temp.push(0);
            n_segment_temp.extend(segment_lengths.iter().copied());
            let bdy_segment = fortran_index_segments(&segments);
            let bdy_segment_old = fortran_index_segments(&segments_old);
            let mut n_bdy_segment = Vec::with_capacity(num_segment + 1);
            n_bdy_segment.push(0);
            n_bdy_segment.extend(segment_lengths.iter().copied());
            let mut lop_ref = 0usize;
            apply_sharp_concav_lop_judge_fortran_indexed(
                &mut lop_ref,
                num_segment,
                state.num_mp[state.iter],
                &state.mrl_new,
                &state.triangle_neighbors,
                &state.ngrmw_new,
                &state.sjx_child,
                &bdy_segment,
                &bdy_segment_old,
                &n_bdy_segment,
                &mut segment_temp,
                &mut n_segment_temp,
            )?;
            if lop_ref == 0 {
                continue;
            }
            num_lop += lop_ref;
            let mut lop_segment = vec![0usize];
            for segment_id in 1..=num_segment {
                let n = n_segment_temp[segment_id];
                if n == 0 {
                    continue;
                }
                lop_segment.extend_from_slice(&segment_temp[segment_id][1..=n]);
            }
            state.iter += 1;
            if state.num_mp.len() <= state.iter {
                state.num_mp.resize(state.iter + 1, 0);
            }
            if state.num_wp.len() <= state.iter {
                state.num_wp.resize(state.iter + 1, 0);
            }
            state.num_mp[state.iter] = state.num_mp[state.iter - 1] + lop_ref;
            state.num_wp[state.iter] = state.num_wp[state.iter - 1];
            state.mp_new.resize(
                state.num_mp[state.iter] + 1,
                LonLatPoint { lon: 0.0, lat: 0.0 },
            );
            if state.ngrmw_new.len() <= 3 {
                state.ngrmw_new.resize_with(4, Vec::new);
            }
            for row in &mut state.ngrmw_new[1..=3] {
                row.resize(state.num_mp[state.iter] + 1, 1);
            }
            state.num_ref = lop_ref;
            state.ref_sjx_segment = lop_segment;
            state.apply_delaunay_lop()?;
        }

        Ok(num_lop)
    }
}
