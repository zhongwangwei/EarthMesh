use std::path::PathBuf;

/// One polygon/triangle class in `MOD_file_preprocess.F90:quality_save_global`.
#[derive(Debug, Clone, PartialEq)]
pub struct QualityClassMetrics {
    pub length: Vec<Vec<f64>>,
    pub angle: Vec<Vec<f64>>,
    pub extr: [f64; 2],
    pub eavg: [f64; 2],
    pub savg: f64,
    pub less: Vec<i32>,
    pub more: Vec<i32>,
}

/// Rust data shape written by `MOD_file_preprocess.F90:quality_save_global`.
#[derive(Debug, Clone, PartialEq)]
pub struct GlobalQualityMesh {
    pub sjx: QualityClassMetrics,
    pub wbx: QualityClassMetrics,
    pub lbx: QualityClassMetrics,
    pub qbx: Option<QualityClassMetrics>,
}

/// Evidence report from writing `MOD_file_preprocess.F90:quality_save_global`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalQualityWriteReport {
    pub output: PathBuf,
    pub num_sjx: usize,
    pub num_wbx: usize,
    pub num_lbx: usize,
    pub num_qbx: usize,
}
