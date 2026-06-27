/// Evidence from applying `MOD_refine.F90:OnedivideFour_connection`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnedivideFourConnectionReport {
    pub marked_triangles: Vec<usize>,
    pub marked_vertices: Vec<usize>,
}

/// Evidence from applying the non-dateline core of
/// `MOD_refine.F90:OnedivideFour_renew`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnedivideFourRenewReport {
    pub refined_triangles: Vec<usize>,
    pub new_triangle_ids: Vec<usize>,
    pub new_vertex_ids: Vec<usize>,
    pub dateline_adjusted: bool,
}

/// Evidence from applying `MOD_refine.F90:OnedivideTwo` through the CLI
/// Fortran-row adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnedivideTwoReport {
    pub split_triangles: Vec<usize>,
    pub new_triangle_ids: Vec<usize>,
    pub new_vertex_ids: Vec<usize>,
    pub dateline_adjusted: bool,
}

/// Evidence from applying `MOD_refine.F90:ref_sjx_isreverse_judge` through
/// the CLI adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsreverseJudgeReport {
    pub ref_sjx: Vec<i32>,
    pub marked_triangles: Vec<usize>,
    pub active_segments: Vec<usize>,
    pub rewritten_segments: Vec<Vec<usize>>,
}

/// Evidence from applying `MOD_refine.F90:Delaunay_Lop` through the CLI
/// Fortran-row adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelaunayLopReport {
    pub flipped_pairs: Vec<(usize, usize)>,
    pub new_triangle_ids: Vec<usize>,
    pub dateline_adjusted: bool,
}

/// Evidence from applying `MOD_refine.F90:NGR_RENEW` through the CLI
/// Fortran-row adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NgrRenewReport {
    pub num_sjx: usize,
    pub num_dbx: usize,
    pub vertex_mapping: Vec<usize>,
    pub adjacency_capacity: usize,
    pub boundary_refine: Vec<usize>,
    pub boundary_refine_transition: Vec<usize>,
}

/// Evidence from applying `MOD_refine.F90:m1w1_to_m11w11` through the CLI
/// Fortran-row adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct M1W1LookupReport {
    pub parent_pair: (usize, usize),
    pub child_pair: Option<(usize, usize)>,
}

/// Evidence from applying `MOD_refine.F90:weak_concav_pair_special` through
/// the CLI Fortran-row adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeakConcavPairSpecialReport {
    pub updated_pairs: Vec<[usize; 2]>,
    pub marked_ref_sjx_triangles: Vec<usize>,
    pub deferred_renew_triangles: Vec<usize>,
    pub segment_first_slots: Vec<(usize, usize)>,
}

/// Evidence from applying `MOD_refine.F90:sharp_concav_lop_judge` through the
/// CLI Fortran-row adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharpConcavLopJudgeReport {
    pub num_ref_added: usize,
    pub segment_lengths: Vec<(usize, usize)>,
    pub written_segments: Vec<(usize, Vec<usize>)>,
}
