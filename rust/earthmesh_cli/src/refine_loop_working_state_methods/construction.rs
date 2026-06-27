use std::io;

use crate::refine_loop_state_mesh::{
    derive_triangle_neighbors_from_one_based_membership, state_arrays_to_unstructured_mesh,
};
use crate::*;

impl RefineLoopWorkingState {
    /// Build the initial `refine_loop` working arrays from the current
    /// unstructured gridfile payload.
    pub fn from_unstructured_mesh(mesh: &UnstructuredMesh) -> Self {
        let nma = mesh.m_points.len();
        let nwa = mesh.w_points.len();
        let mut mp_new = vec![LonLatPoint { lon: 0.0, lat: 0.0 }; nma + 1];
        for (idx, point) in mesh.m_points.iter().copied().enumerate() {
            mp_new[idx + 1] = point;
        }
        let mut wp_new = vec![LonLatPoint { lon: 0.0, lat: 0.0 }; nwa + 1];
        for (idx, point) in mesh.w_points.iter().copied().enumerate() {
            wp_new[idx + 1] = point;
        }

        let mut ngrmw = vec![vec![0_usize; nma + 1]; 4];
        for (triangle, vertices) in mesh.m_to_w.iter().enumerate() {
            for row in 1..=3 {
                ngrmw[row][triangle + 1] = vertices[row - 1].max(0) as usize;
            }
        }
        let ngrmw_new = ngrmw.clone();

        let ngrwm_capacity = mesh.w_to_m.iter().map(Vec::len).max().unwrap_or(0);
        let mut ngrwm = vec![vec![0_usize; nwa + 1]; ngrwm_capacity + 1];
        for (vertex, triangles) in mesh.w_to_m.iter().enumerate() {
            for (row, &triangle) in triangles.iter().enumerate() {
                ngrwm[row + 1][vertex + 1] = triangle.max(0) as usize;
            }
        }
        let mut n_ngrwm = vec![0_usize; nwa + 1];
        for (vertex, &count) in mesh.n_w_to_m.iter().enumerate() {
            n_ngrwm[vertex + 1] = count.max(0) as usize;
        }
        let triangle_neighbors =
            derive_triangle_neighbors_from_one_based_membership(nma, nwa, &ngrmw, &ngrwm, &n_ngrwm);

        Self {
            iter: 1,
            num_vertex: 0,
            num_mp: vec![0, nma],
            num_wp: vec![0, nwa],
            num_sjx: 0,
            num_dbx: 0,
            num_tranrow_sjx: 0,
            mp_new,
            wp_new,
            ngrmw,
            ngrmw_new,
            ngrwm,
            n_ngrwm,
            mp_f: Vec::new(),
            wp_f: Vec::new(),
            ngrmw_f: Vec::new(),
            ngrwm_f: Vec::new(),
            n_ngrwm_f: Vec::new(),
            ref_sjx: vec![0; nma + 1],
            ref_lbx: vec![0; nwa + 1],
            mrl_new: vec![1; nma + 1],
            triangle_neighbors,
            segments: Vec::new(),
            n_segments: Vec::new(),
            sjx_child: vec![[0, 0]; nma + 1],
            weak_concav_pair: Vec::new(),
            weak_concav_segment: Vec::new(),
            weak_concav_segment_old: Vec::new(),
            n_weak_concav_segment: Vec::new(),
            bdy_refine_segment: Vec::new(),
            bdy_refine_segment_old: Vec::new(),
            n_bdy_refine_segment: Vec::new(),
            ref_sjx_segment_temp: Vec::new(),
            n_ref_sjx_segment_temp: Vec::new(),
            ref_sjx_segment: Vec::new(),
            num_ref: 0,
            bdy_refine: Vec::new(),
            bdy_refine_tran: Vec::new(),
        }
    }

    /// Export either the renewed final arrays (`*_f`) or the current working
    /// arrays back to the unstructured mesh shape used by gridfile writers.
    pub fn to_unstructured_mesh(&self) -> io::Result<UnstructuredMesh> {
        let has_final = self.num_sjx > 0
            && self.num_dbx > 0
            && !self.mp_f.is_empty()
            && !self.wp_f.is_empty()
            && !self.ngrmw_f.is_empty()
            && !self.ngrwm_f.is_empty()
            && !self.n_ngrwm_f.is_empty();
        if has_final {
            state_arrays_to_unstructured_mesh(
                self.num_sjx,
                self.num_dbx,
                &self.mp_f,
                &self.wp_f,
                &self.ngrmw_f,
                &self.ngrwm_f,
                &self.n_ngrwm_f,
            )
        } else {
            let nma = *self.num_mp.get(self.iter).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "iter must address num_mp")
            })?;
            let nwa = *self.num_wp.get(self.iter).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "iter must address num_wp")
            })?;
            state_arrays_to_unstructured_mesh(
                nma,
                nwa,
                &self.mp_new,
                &self.wp_new,
                &self.ngrmw_new,
                &self.ngrwm,
                &self.n_ngrwm,
            )
        }
    }
}
