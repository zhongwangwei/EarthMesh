use crate::LonLatPoint;

/// Rectilinear Lambert-style vertex grid consumed by `lamb_mask_make`.
#[derive(Debug, Clone, PartialEq)]
pub struct LambertVertices {
    pub xi_vert: usize,
    pub eta_vert: usize,
    pub lon_vert: Vec<f64>,
    pub lat_vert: Vec<f64>,
}

/// Mode4 mesh payload written by `Mode4_Mesh_Save`.
#[derive(Debug, Clone, PartialEq)]
pub struct Mode4Mesh {
    pub lonlat_bound: Vec<LonLatPoint>,
    pub ngr_bound: Vec<[i32; 4]>,
    pub n_ngr: Vec<i32>,
}

impl Mode4Mesh {
    pub fn bound_points(&self) -> usize {
        self.lonlat_bound.len()
    }

    pub fn mode_points(&self) -> usize {
        self.ngr_bound.len()
    }
}
