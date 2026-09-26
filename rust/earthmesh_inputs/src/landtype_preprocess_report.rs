use crate::V3DataSourceDescriptor;

/// Data-preprocess source-grid report from `MOD_data_preprocess.F90:data_preprocess`.
#[derive(Debug, Clone, PartialEq)]
pub struct LandtypeDataPreprocessReport {
    pub source: V3DataSourceDescriptor,
    pub gridnum_perdegree: usize,
    pub nlons_source: usize,
    pub nlats_source: usize,
    pub lon_i: Vec<f64>,
    pub lat_i: Vec<f64>,
    pub lon_vertex: Vec<f64>,
    pub lat_vertex: Vec<f64>,
    pub landtypes_global: Vec<Vec<i32>>,
    pub maxlc: i32,
}
