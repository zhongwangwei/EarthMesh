use crate::{AreaJudgeSeaOrLandReport, LandtypeDataPreprocessReport, V3DataSourceDescriptor};

/// Combined `MOD_data_preprocess` + `MOD_Area_judge` source-grid state.
#[derive(Debug, Clone, PartialEq)]
pub struct DataPreprocessAreaJudgeSourceReport {
    pub preprocess: LandtypeDataPreprocessReport,
    pub seaorland: AreaJudgeSeaOrLandReport,
}

/// Owned source state derived from `MOD_data_preprocess` and `Area_judge`
/// that can be passed into the current `mkgrd` refine stack without an
/// external source-state text file or Canonical module globals.
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
    pub is_in_domain: Vec<Vec<bool>>,
    pub seaorland: Vec<Vec<bool>>,
    pub landtypes_global: Vec<Vec<i32>>,
    pub maxlc: i32,
}

/// Owned context required to construct land final-domain postprocessing from a
/// data_preprocess-derived source-state bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MkgrdDataPreprocessSourceStateLandPostprocContext {
    pub selected_seaorland: Vec<Vec<bool>>,
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
