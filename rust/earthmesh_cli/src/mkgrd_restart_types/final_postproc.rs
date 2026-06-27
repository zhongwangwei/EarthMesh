/// Selected land-domain sea/land matrix reconstructed from an Area_judge
/// restart payload for final land `mask_postproc` handoff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedLandDomainMatrix {
    pub minlon_source: usize,
    pub maxlat_source: usize,
    pub nlons: usize,
    pub nlats: usize,
    pub seaorland: Vec<Vec<i32>>,
}

/// Owned context for a restart-refine final land `mask_postproc` request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MkgrdRestartRefineFinalLandPostprocContext {
    pub selected_seaorland: Vec<Vec<i32>>,
    pub minlon_dm_area: i32,
    pub maxlat_dm_area: i32,
    pub nlons_dm_select: usize,
    pub nlats_dm_select: usize,
}

/// Owned context for a restart-refine final earth `mask_postproc` request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MkgrdRestartRefineFinalEarthPostprocContext {
    pub minlon_dm_area: i32,
    pub maxlat_dm_area: i32,
    pub nlons_dm_select: usize,
    pub nlats_dm_select: usize,
}

/// Typed final `mask_postproc` request for migrated Area_judge restart-refine
/// handoffs.
#[derive(Debug, Clone, PartialEq)]
pub enum MkgrdRestartRefineFinalPostprocRequest {
    Earth {
        mask_sea_ratio: f64,
        minlon_dm_area: i32,
        maxlat_dm_area: i32,
        nlons_dm_select: usize,
        nlats_dm_select: usize,
    },
    Land(MkgrdRestartRefineFinalLandPostprocContext),
    Ocean {
        mask_sea_ratio: f64,
        num_vertex: usize,
    },
    Atmos,
}
