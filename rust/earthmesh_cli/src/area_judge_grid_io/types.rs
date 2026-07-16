use std::path::{Path, PathBuf};

use earthmesh_mesh::AreaJudgeSourceBounds;

#[derive(Debug, Clone, PartialEq)]
pub struct AreaJudgeGridPayload {
    pub bounds: AreaJudgeSourceBounds,
    pub longitude: Vec<f64>,
    pub latitude: Vec<f64>,
    pub is_in_area_select: Vec<Vec<i32>>,
    pub seaorland_select: Option<Vec<Vec<i32>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaJudgeExpandedGridReport {
    pub is_in_domain: Vec<Vec<bool>>,
    pub seaorland: Vec<Vec<bool>>,
    pub bounds: AreaJudgeSourceBounds,
    pub nlons_select: usize,
    pub nlats_select: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct AreaJudgeRestartGridRunConfig<'a> {
    pub input: &'a Path,
    pub nlons_source: usize,
    pub nlats_source: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AreaJudgeRestartGridRunReport {
    pub input: PathBuf,
    pub payload: AreaJudgeGridPayload,
    pub expanded: AreaJudgeExpandedGridReport,
}
