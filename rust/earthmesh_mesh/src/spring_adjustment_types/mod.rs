use crate::{
    DistanceLayerSpacing, GlobalDistanceStep, LonLatDegrees, RegionalMoveMaskOutput,
    SpringDynamicsGlobalOutput, SpringDynamicsRegionalOutput,
};

/// Borrowed inputs for the pure in-memory calculation side of
/// `MOD_grid_preprocess:Springjustment_global`.
#[derive(Debug, Clone, Copy)]
pub struct SpringjustmentGlobalCoreInput<'a> {
    pub triangle_lonlat: &'a [LonLatDegrees],
    pub cell_lonlat: &'a [LonLatDegrees],
    pub cells_on_triangle: &'a [[usize; 3]],
    pub triangles_on_cell: &'a [Vec<usize>],
    pub n_edges_on_cell: &'a [usize],
    pub base_dists_on_edge: f64,
    pub base_cellwidth: Option<f64>,
    pub distance_num_rc: usize,
    pub distance_spacing: DistanceLayerSpacing,
    pub distance_steps: &'a [GlobalDistanceStep<'a>],
    pub niter_refine: usize,
    pub relax: f64,
    pub radius: f64,
    pub diagnostic_every: usize,
}

/// Output from the pure in-memory `Springjustment_global` adapter.
#[derive(Debug, Clone, PartialEq)]
pub struct SpringjustmentGlobalCoreOutput {
    pub updated_triangle_lonlat: Vec<LonLatDegrees>,
    pub updated_cell_lonlat: Vec<LonLatDegrees>,
    pub triangle_neighbors: Vec<[usize; 3]>,
    pub cells_on_edge: Vec<[usize; 2]>,
    pub vertices_on_edge: Vec<[usize; 2]>,
    pub edges_on_vertex: Vec<[usize; 3]>,
    pub cells_on_vertex: Vec<[usize; 3]>,
    pub edges_on_cell: Vec<Vec<usize>>,
    pub cells_on_cell: Vec<Vec<usize>>,
    pub edges_on_edge_tri: Vec<[usize; 4]>,
    pub dists_on_edge: Vec<f64>,
    pub cellwidth: Option<Vec<f64>>,
    pub edge_lonlat: Vec<LonLatDegrees>,
    pub spring: SpringDynamicsGlobalOutput,
}

/// Borrowed inputs for the pure in-memory calculation side of
/// `MOD_grid_preprocess:Springjustment_regional_step`.
#[derive(Debug, Clone, Copy)]
pub struct SpringjustmentRegionalCoreInput<'a> {
    pub triangle_lonlat: &'a [LonLatDegrees],
    pub cell_lonlat: &'a [LonLatDegrees],
    pub cells_on_triangle: &'a [[usize; 3]],
    pub triangles_on_cell: &'a [Vec<usize>],
    pub n_edges_on_cell: &'a [usize],
    pub move_mask: &'a [bool],
    pub niter_refine: usize,
    pub radius: f64,
    pub diagnostic_every: usize,
}

/// Output from the pure in-memory `Springjustment_regional_step` adapter.
#[derive(Debug, Clone, PartialEq)]
pub struct SpringjustmentRegionalCoreOutput {
    pub updated_triangle_lonlat: Vec<LonLatDegrees>,
    pub updated_cell_lonlat: Vec<LonLatDegrees>,
    pub triangle_neighbors: Vec<[usize; 3]>,
    pub cells_on_edge: Vec<[usize; 2]>,
    pub vertices_on_edge: Vec<[usize; 2]>,
    pub edges_on_vertex: Vec<[usize; 3]>,
    pub edges_on_cell: Vec<Vec<usize>>,
    pub cells_on_cell: Vec<Vec<usize>>,
    pub regional: SpringDynamicsRegionalOutput,
}

/// Borrowed inputs for the pure in-memory calculation side of
/// `Springjustment_regional_step` when the upstream refinement source has
/// already been resolved to triangle flags.
#[derive(Debug, Clone, Copy)]
pub struct SpringjustmentRegionalFromRefinementInput<'a> {
    pub triangle_lonlat: &'a [LonLatDegrees],
    pub cell_lonlat: &'a [LonLatDegrees],
    pub cells_on_triangle: &'a [[usize; 3]],
    pub triangles_on_cell: &'a [Vec<usize>],
    pub n_edges_on_cell: &'a [usize],
    pub refined_triangles: &'a [bool],
    pub set_dis: usize,
    pub protected_seed_cells: &'a [usize],
    pub vertex_protect_layers: usize,
    pub niter_refine: usize,
    pub radius: f64,
    pub diagnostic_every: usize,
}

/// Output from `Springjustment_regional_step` after mask derivation and the
/// migrated regional spring core have both run.
#[derive(Debug, Clone, PartialEq)]
pub struct SpringjustmentRegionalFromRefinementOutput {
    pub mask: RegionalMoveMaskOutput,
    pub core: SpringjustmentRegionalCoreOutput,
}

/// Borrowed inputs for the pure in-memory regional Springjustment path when
/// the upstream refinement source is an already-loaded source mask grid.
#[derive(Debug, Clone, Copy)]
pub struct SpringjustmentRegionalFromSourceMaskInput<'a> {
    pub triangle_lonlat: &'a [LonLatDegrees],
    pub cell_lonlat: &'a [LonLatDegrees],
    pub cells_on_triangle: &'a [[usize; 3]],
    pub triangles_on_cell: &'a [Vec<usize>],
    pub n_edges_on_cell: &'a [usize],
    pub source_lon_vertices: &'a [f64],
    pub source_lat_vertices: &'a [f64],
    pub mask_patch: &'a [Vec<bool>],
    pub first_triangle_id: usize,
    pub set_dis: usize,
    pub protected_seed_cells: &'a [usize],
    pub vertex_protect_layers: usize,
    pub niter_refine: usize,
    pub radius: f64,
    pub diagnostic_every: usize,
}

/// Output from the source-mask regional Springjustment adapter.
#[derive(Debug, Clone, PartialEq)]
pub struct SpringjustmentRegionalFromSourceMaskOutput {
    pub refined_triangles: Vec<bool>,
    pub regional: SpringjustmentRegionalFromRefinementOutput,
}
