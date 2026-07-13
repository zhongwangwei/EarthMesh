use std::path::PathBuf;

use crate::LonLatPoint;

/// Rust data shape written by `MOD_file_preprocess.F90:Unstructured_Mesh_Save`.
#[derive(Debug, Clone, PartialEq)]
pub struct UnstructuredMesh {
    pub m_points: Vec<LonLatPoint>,
    pub w_points: Vec<LonLatPoint>,
    pub m_to_w: Vec<[i32; 3]>,
    pub w_to_m: Vec<Vec<i32>>,
    pub n_w_to_m: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnstructuredMeshTopologyReport {
    pub m_rows: usize,
    pub w_rows: usize,
    pub violations: Vec<String>,
}

impl UnstructuredMeshTopologyReport {
    pub fn is_consistent(&self) -> bool {
        self.violations.is_empty()
    }
}

/// Typed payload returned by `MOD_file_preprocess.F90:IAP_Mesh_Read`.
#[derive(Debug, Clone, PartialEq)]
pub struct IapMeshReadPayload {
    pub w_points: Vec<LonLatPoint>,
    pub triangle_neighbors: Vec<[i32; 3]>,
    pub triangle_vertices: Vec<[i32; 3]>,
}

/// Evidence report from writing an unstructured gridfile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnstructuredMeshWriteReport {
    pub output: PathBuf,
    pub sjx_points: usize,
    pub lbx_points: usize,
    pub dimc: usize,
}

/// Optional Method-C fields carried alongside compact gridfile connectivity.
/// Refinement levels are zero-based at the file boundary; `ngr` retains its
/// native one-based Method-C value (placeholder rows may be zero).
#[derive(Clone, Copy, Debug, Default)]
pub struct MethodCGridfileMetadataSlices<'a> {
    pub m_refine_level: Option<&'a [i32]>,
    pub m_refine_level_orig: Option<&'a [i32]>,
    pub m_ngr: Option<&'a [i32]>,
    pub w_refine_level: Option<&'a [i32]>,
    pub w_refine_level_orig: Option<&'a [i32]>,
    pub w_ngr: Option<&'a [i32]>,
}

/// Mesh node coordinates plus compact connectivity read from an EarthMesh gridfile.
pub struct GridfileMeshPoints {
    pub m_lon: Vec<f64>,
    pub m_lat: Vec<f64>,
    pub w_lon: Vec<f64>,
    pub w_lat: Vec<f64>,
    pub m_to_w: Vec<i32>,
    /// Optional EarthMesh extension: zero-based refinement level per M cell.
    pub m_refine_level: Vec<i32>,
    /// Original zero-based Method-C refinement ownership per M cell.
    pub m_refine_level_orig: Vec<i32>,
    /// Native Method-C nest/grid ownership (`itab_m%ngr`) per M cell.
    pub m_ngr: Vec<i32>,
    /// Flattened `itab_w%im`: the M-points around each W cell.
    pub w_to_m: Vec<i32>,
    pub w_to_m_width: usize,
    /// `n_ngrwm`: how many of each W cell's `w_to_m` entries are valid.
    pub n_w: Vec<i32>,
    /// Optional EarthMesh extension: zero-based refinement level per W cell.
    pub w_refine_level: Vec<i32>,
    /// Original zero-based Method-C refinement ownership per W cell.
    pub w_refine_level_orig: Vec<i32>,
    /// Native Method-C nest/grid ownership (`itab_w%ngr`) per W cell.
    pub w_ngr: Vec<i32>,
}

/// Which connectivity view to render from a gridfile: `Tri` builds one triangle per
/// M cell (`itab_m%iw`); `Hex` builds one polygon per W cell from its surrounding M
/// corners (`itab_w%im`). FVCOM/triangle meshes use `Tri`, MPAS/hex meshes use `Hex`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GridfileCellKind {
    Tri,
    Hex,
}
