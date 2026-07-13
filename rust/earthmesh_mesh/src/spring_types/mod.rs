use crate::{CartesianPoint, LonLatDegrees};

/// One-edge correction term from `MOD_grid_preprocess:spring_dynamics_global`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringEdgeAdjustment {
    pub displacement: CartesianPoint,
    pub distance: f64,
    pub ratio: f64,
    pub target_distance: f64,
    pub frac_change: f64,
    pub frac_change_squared: f64,
}

/// Output from one `spring_dynamics_global` iteration.
#[derive(Debug, Clone, PartialEq)]
pub struct SpringGlobalIterationOutput {
    pub updated_cell_points: Vec<CartesianPoint>,
    pub edge_displacements: Vec<CartesianPoint>,
    pub frac_change_squared: Vec<f64>,
}

/// Periodic displacement diagnostic printed by `spring_dynamics_global`.
#[derive(Debug, Clone, PartialEq)]
pub struct SpringDiagnosticMaxDisplacement {
    pub iteration: usize,
    pub max_displacement: f64,
}

/// Output from the multi-iteration `spring_dynamics_global` wrapper.
#[derive(Debug, Clone, PartialEq)]
pub struct SpringDynamicsGlobalOutput {
    pub updated_cell_points: Vec<CartesianPoint>,
    pub last_edge_displacements: Vec<CartesianPoint>,
    pub last_frac_change_squared: Vec<f64>,
    pub diagnostic_max_displacements: Vec<SpringDiagnosticMaxDisplacement>,
}

/// Output from the regional move-mask smoother in `spring_dynamics_regionalv2`.
#[derive(Debug, Clone, PartialEq)]
pub struct SpringDynamicsRegionalOutput {
    pub updated_cell_points: Vec<CartesianPoint>,
    pub calculated_cells: Vec<usize>,
    pub moved_cells: Vec<usize>,
    pub diagnostic_max_displacements: Vec<SpringDiagnosticMaxDisplacement>,
}

/// Borrowed inputs for the pure mask-derivation core of
/// `MOD_grid_preprocess:set_dbxMove_regional_step`.
#[derive(Debug, Clone, Copy)]
pub struct RegionalMoveMaskInput<'a> {
    pub set_dis: usize,
    pub refined_triangles: &'a [bool],
    pub cells_on_triangle: &'a [[usize; 3]],
    pub triangles_on_cell: &'a [Vec<usize>],
    pub n_edges_on_cell: &'a [usize],
    pub protected_seed_cells: &'a [usize],
    pub vertex_protect_layers: usize,
}

/// Output from the current `set_dbxMove_regional_step` mask derivation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionalMoveMaskOutput {
    pub move_mask: Vec<bool>,
    pub boundary_mask: Vec<bool>,
    pub expanded_refined_triangles: Vec<bool>,
    pub protected_triangles: Vec<bool>,
}

/// Borrowed inputs for the pure classification core of
/// `MOD_grid_preprocess:refine_sjx_regional_make`.
#[derive(Debug, Clone, Copy)]
pub struct RefineRegionalMaskInput<'a> {
    pub triangle_lonlat: &'a [LonLatDegrees],
    pub source_lon_vertices: &'a [f64],
    pub source_lat_vertices: &'a [f64],
    pub mask_patch: &'a [Vec<bool>],
    pub first_triangle_id: usize,
}
