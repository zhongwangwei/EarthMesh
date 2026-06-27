use crate::{
    AreaJudgeSeaOrLandReport, LandtypeDataPreprocessReport, MkgrdRefinePrepareSourceGridOptions,
    V3DataSourceDescriptor,
};

/// Combined `MOD_data_preprocess` + `MOD_Area_judge` source-grid state.
#[derive(Debug, Clone, PartialEq)]
pub struct DataPreprocessAreaJudgeSourceReport {
    pub preprocess: LandtypeDataPreprocessReport,
    pub seaorland: AreaJudgeSeaOrLandReport,
}

/// Owned source state derived from `MOD_data_preprocess` and `Area_judge`
/// that can be passed into the migrated `mkgrd` refine stack without an
/// external source-state text file or Fortran module globals.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdDataPreprocessSourceState {
    pub lon_vertex: Vec<f64>,
    pub lat_vertex: Vec<f64>,
    pub lon_i: Vec<f64>,
    pub lat_i: Vec<f64>,
    pub gridnum_perdegree: usize,
    pub nlons_source: usize,
    pub nlats_source: usize,
    pub first_triangle_id: usize,
    pub num_vertex: usize,
    pub sources: Vec<V3DataSourceDescriptor>,
    pub is_in_domain: Vec<Vec<i32>>,
    pub seaorland: Vec<Vec<i32>>,
    pub landtypes_global: Vec<Vec<i32>>,
    pub maxlc: i32,
}

impl MkgrdDataPreprocessSourceState {
    pub fn refine_prepare_source_grid(&self) -> MkgrdRefinePrepareSourceGridOptions<'_> {
        MkgrdRefinePrepareSourceGridOptions {
            lon_vertex: &self.lon_vertex,
            lat_vertex: &self.lat_vertex,
            lon_i: &self.lon_i,
            lat_i: &self.lat_i,
            gridnum_perdegree: self.gridnum_perdegree,
            nlons_source: self.nlons_source,
            nlats_source: self.nlats_source,
            first_triangle_id: self.first_triangle_id,
        }
    }
}

/// Owned context required to construct land final-domain postprocessing from a
/// data_preprocess-derived source-state bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MkgrdDataPreprocessSourceStateLandPostprocContext {
    pub selected_seaorland: Vec<Vec<i32>>,
    pub minlon_dm_area: i32,
    pub maxlat_dm_area: i32,
    pub nlons_dm_select: usize,
    pub nlats_dm_select: usize,
}

/// Owned context required to construct earth final-domain postprocessing from a
/// data_preprocess-derived source-state bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MkgrdDataPreprocessSourceStateEarthPostprocContext {
    pub minlon_dm_area: i32,
    pub maxlat_dm_area: i32,
    pub nlons_dm_select: usize,
    pub nlats_dm_select: usize,
}

/// Typed final-domain postprocess request inferred from a data_preprocess
/// source-state bundle plus the target `NL%mesh_type`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MkgrdDataPreprocessSourceStateFinalPostprocRequest {
    Earth(MkgrdDataPreprocessSourceStateEarthPostprocContext),
    Land(MkgrdDataPreprocessSourceStateLandPostprocContext),
    Atmos,
    Ocean { num_vertex: usize },
}
