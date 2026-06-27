use crate::LonLatPoint;

/// Parsed circle mask input: lon, lat, and radius in kilometers.
#[derive(Debug, Clone, PartialEq)]
pub struct CircleMask {
    pub refine_degree: usize,
    pub points: Vec<LonLatPoint>,
    pub radius_km: Vec<f64>,
}

/// NetCDF payload for `MOD_file_preprocess.F90:circle_Mesh_Read/Save`.
#[derive(Debug, Clone, PartialEq)]
pub struct CircleMesh {
    pub points: Vec<LonLatPoint>,
    pub radius_km: Vec<f64>,
}

/// Parsed close-polygon mask input.
#[derive(Debug, Clone, PartialEq)]
pub struct CloseMask {
    pub refine_degree: usize,
    pub points: Vec<LonLatPoint>,
}
