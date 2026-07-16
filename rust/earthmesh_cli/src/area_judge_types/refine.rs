use earthmesh_mesh::AreaJudgeSourceBounds;

/// Active refine-grid state produced by `Area_judge_refine(iter=0)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaJudgeRefineActivationReport {
    pub is_in_refine: Vec<Vec<bool>>,
    pub bounds: AreaJudgeSourceBounds,
    pub nlons_select: usize,
    pub nlats_select: usize,
    pub selected_cells: usize,
}

/// Unified `Area_judge_refine(iter)` step state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaJudgeRefineStepReport {
    pub is_in_refine: Vec<Vec<bool>>,
    pub bounds: AreaJudgeSourceBounds,
    pub nlons_select: usize,
    pub nlats_select: usize,
    pub selected_cells: usize,
    pub source_numpatch: Option<usize>,
}
