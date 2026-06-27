use std::path::PathBuf;

use crate::MpasGraphInfoWriteReport;

/// Rust data shape written by `MOD_file_preprocess.F90:MPAS_Mesh_Save`.
#[derive(Debug, Clone, PartialEq)]
pub struct MpasMesh {
    pub lat_cell: Vec<f64>,
    pub lon_cell: Vec<f64>,
    pub x_cell: Vec<f64>,
    pub y_cell: Vec<f64>,
    pub z_cell: Vec<f64>,
    pub lat_vertex: Vec<f64>,
    pub lon_vertex: Vec<f64>,
    pub x_vertex: Vec<f64>,
    pub y_vertex: Vec<f64>,
    pub z_vertex: Vec<f64>,
    pub lat_edge: Vec<f64>,
    pub lon_edge: Vec<f64>,
    pub x_edge: Vec<f64>,
    pub y_edge: Vec<f64>,
    pub z_edge: Vec<f64>,
    pub n_edges_on_cell: Vec<i32>,
    pub cells_on_cell: Vec<Vec<i32>>,
    pub vertices_on_cell: Vec<Vec<i32>>,
    pub edges_on_cell: Vec<Vec<i32>>,
    pub cells_on_vertex: Vec<Vec<i32>>,
    pub edges_on_vertex: Vec<Vec<i32>>,
    pub cells_on_edge: Vec<[i32; 2]>,
    pub vertices_on_edge: Vec<[i32; 2]>,
    pub n_edges_on_edge: Vec<i32>,
    pub edges_on_edge: Vec<Vec<i32>>,
    pub area_cell: Vec<f64>,
    pub area_triangle: Vec<f64>,
    pub kite_areas_on_vertex: Vec<Vec<f64>>,
    pub dv_edge: Vec<f64>,
    pub dc_edge: Vec<f64>,
    pub angle_edge: Vec<f64>,
    pub weights_on_edge: Vec<Vec<f64>>,
    pub mesh_density: Vec<f64>,
    pub nominal_min_dc: f64,
    pub error_segment: Vec<f64>,
}

/// Evidence report from writing `MOD_file_preprocess.F90:MPAS_Mesh_Save` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpasMeshWriteReport {
    pub output: PathBuf,
    pub n_cells: usize,
    pub n_vertices: usize,
    pub n_edges: usize,
}

/// Topologically-consistent MPAS connectivity for a regionally-carved hex mesh.
///
/// All ids are Fortran-indexed (index `0` is the reserved placeholder row, real
/// ids start at `2`). Cells outside the region are represented as `0` - the MPAS
/// "no neighbour" marker wherever a carved cell lost a neighbour. The carved
/// gridfile's `w_to_m` corner rings are already in cyclic order, so each cell
/// side `(ring[i], ring[i+1])` is one mesh edge; a side shared by two kept cells
/// is an interior edge, a side touching the removed exterior is a boundary edge
/// with `cells_on_edge = [cell, 0]`.
#[derive(Debug, Clone)]
pub struct RegionalMpasConnectivity {
    pub edge_count: usize,
    pub n_edges_on_cell: Vec<usize>,
    pub vertices_on_cell: Vec<Vec<usize>>,
    pub edges_on_cell: Vec<Vec<usize>>,
    pub cells_on_cell: Vec<Vec<usize>>,
    pub cells_on_vertex: Vec<[usize; 3]>,
    pub edges_on_vertex: Vec<[usize; 3]>,
    pub cells_on_edge: Vec<[usize; 2]>,
    pub vertices_on_edge: Vec<[usize; 2]>,
}

/// Topological self-consistency report for an [`MpasMesh`] (global or regional).
#[derive(Debug, Clone)]
pub struct MeshTopologyReport {
    pub n_cells: usize,
    pub n_vertices: usize,
    pub n_edges: usize,
    /// `nCells + nVertices - nEdges`: 2 for a closed sphere, 1 for a disk/region.
    pub euler_characteristic: i64,
    /// Edges with a `0` (no-neighbour) cell - the region/limited-area boundary.
    pub boundary_edges: usize,
    pub is_closed: bool,
    pub violations: Vec<String>,
}

impl MeshTopologyReport {
    pub fn is_consistent(&self) -> bool {
        self.violations.is_empty()
    }
}

/// Evidence report from the full `MOD_mask_postproc.F90:MPAS_Mesh_Cal` file pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpasFullMeshPipelineReport {
    pub mesh: MpasMeshWriteReport,
    pub graph_info: MpasGraphInfoWriteReport,
}
