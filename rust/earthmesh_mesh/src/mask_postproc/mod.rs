use super::*;

/// Result of `MOD_refine.F90:bdy_refine_segment_make`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefineBoundarySegments {
    pub num_bdy_refine_segment: usize,
    pub bdy_refine_segment: Vec<Vec<usize>>,
    pub n_bdy_refine_segment: Vec<usize>,
    /// Exclusive segment-array end for each closed curve.
    ///
    /// A flat segment table alone cannot say which first segment the last
    /// segment of a curve wraps to. Canonical carries the same information as
    /// `num_bdy_refine_segment_curve`.
    pub curve_segment_ends: Vec<usize>,
}

/// Result of `MOD_refine.F90:weak_concav_segment_make`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefineWeakConcavitySegments {
    pub num_ref_weak_concav: usize,
    pub num_weak_concav_segment: usize,
    pub num_weak_concav_pair: usize,
    pub bdy_refine_segment: Vec<Vec<usize>>,
    pub n_bdy_refine_segment: Vec<usize>,
    pub weak_concav_segment: Vec<Vec<usize>>,
    pub n_weak_concav_segment: Vec<usize>,
    pub weak_concav_pair: Vec<[usize; 2]>,
}

pub(crate) fn require_vertex_count(
    vertex_id: usize,
    vertex_neighbor_counts: &[usize],
) -> io::Result<()> {
    if vertex_id >= vertex_neighbor_counts.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("vertex {vertex_id} is outside vertex count array"),
        ));
    }
    Ok(())
}
