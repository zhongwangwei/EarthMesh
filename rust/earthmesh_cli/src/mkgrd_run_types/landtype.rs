use crate::{MkgrdRefinePrepareSourceGridOptions, V3DataSourceDescriptor};

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

impl LandtypeDataPreprocessReport {
    /// Borrow the data_preprocess source axes as the source-grid options needed
    /// by migrated `mkgrd` refine/restart handoffs.
    pub fn refine_prepare_source_grid(
        &self,
        first_triangle_id: usize,
    ) -> MkgrdRefinePrepareSourceGridOptions<'_> {
        MkgrdRefinePrepareSourceGridOptions {
            lon_vertex: &self.lon_vertex,
            lat_vertex: &self.lat_vertex,
            lon_i: &self.lon_i,
            lat_i: &self.lat_i,
            gridnum_perdegree: self.gridnum_perdegree,
            nlons_source: self.nlons_source,
            nlats_source: self.nlats_source,
            first_triangle_id,
        }
    }
}
