/// One `bbox_points(i, :)` row: West, East, North, South.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BBoxPoint {
    pub west: f64,
    pub east: f64,
    pub north: f64,
    pub south: f64,
}

/// Parsed `.nml` input for `bbox_mask_make`.
#[derive(Debug, Clone, PartialEq)]
pub struct BBoxMask {
    pub refine_degree: usize,
    pub points: Vec<BBoxPoint>,
}

/// NetCDF payload for `MOD_file_preprocess.F90:bbox_Mesh_Read/Save`.
#[derive(Debug, Clone, PartialEq)]
pub struct BBoxMesh {
    pub points: Vec<BBoxPoint>,
}
