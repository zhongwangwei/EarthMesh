use earthmesh_mesh::AreaJudgeSourceBounds;

/// Source-mask state produced by an `IsInArea_*_Calculation` input file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaJudgeAreaSourceReport {
    pub is_in_area: Vec<Vec<i32>>,
    pub bounds: AreaJudgeSourceBounds,
    pub numpatch: usize,
}

/// Sparse source-mask state for close-curve sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaJudgeSparseAreaSourceReport {
    pub cells: Vec<(usize, usize)>,
    pub bounds: AreaJudgeSourceBounds,
    pub numpatch: usize,
}

/// Summary from building and applying a patch-source mask to `seaorland`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AreaJudgePatchSourceReport {
    pub bounds: AreaJudgeSourceBounds,
    pub patched_cells: usize,
}

/// Summary from applying the `mask_patch_modify` source loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaJudgePatchModifyReport {
    pub source_reports: Vec<AreaJudgePatchSourceReport>,
    pub bounds: Option<AreaJudgeSourceBounds>,
    pub patched_cells: usize,
}

/// Domain mask state produced by `Area_judge` when `mask_domain_global` is true.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaJudgeDomainInitializationReport {
    pub is_in_domain: Vec<Vec<i32>>,
    pub bounds: AreaJudgeSourceBounds,
    pub numpatch: usize,
    pub nlons_select: usize,
    pub nlats_select: usize,
}

/// Result of the `Area_judge` sea/land classification over the domain bounds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaJudgeSeaOrLandReport {
    pub seaorland: Vec<Vec<i32>>,
    pub sum_land_grid: i32,
}

/// Binary landtype class used by `MOD_Area_judge.F90` when building `seaorland`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AreaJudgeLandtypeClass {
    Ocean,
    Land,
}

/// Base `Area_judge` state after domain construction and sea/land classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaJudgeBaseStateReport {
    pub domain: AreaJudgeDomainInitializationReport,
    pub seaorland: AreaJudgeSeaOrLandReport,
}

/// Optional `mask_patch_modify` configuration for non-restart `Area_judge`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AreaJudgePatchConfig<'a> {
    pub mask_patch_type: &'a str,
    pub mask_patch_ndm: usize,
}

/// Optional calculated-refine configuration for non-restart `Area_judge`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AreaJudgeCalculatedRefineConfig<'a> {
    pub refine_setting: &'a str,
    pub mask_refine_cal_type: &'a str,
    pub mask_refine_ndm: usize,
}

/// Non-restart `Area_judge` state after domain, sea/land, optional patch, and
/// optional calculated-refine source construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaJudgeNonRestartReport {
    pub domain: AreaJudgeDomainInitializationReport,
    pub seaorland: AreaJudgeSeaOrLandReport,
    pub patch: Option<AreaJudgePatchModifyReport>,
    pub calculated_refine: Option<AreaJudgeAreaSourceReport>,
}

/// Restart `Area_judge` state restored from selected-grid files, then optionally patched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaJudgeRestartReport {
    pub domain: AreaJudgeDomainInitializationReport,
    pub seaorland: AreaJudgeSeaOrLandReport,
    pub patch: Option<AreaJudgePatchModifyReport>,
    pub calculated_refine: Option<AreaJudgeAreaSourceReport>,
}
