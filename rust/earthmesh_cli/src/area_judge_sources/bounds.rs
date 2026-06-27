use earthmesh_mesh::AreaJudgeSourceBounds;

pub(crate) fn merge_area_judge_source_bounds(
    current: Option<AreaJudgeSourceBounds>,
    next: AreaJudgeSourceBounds,
) -> AreaJudgeSourceBounds {
    current.map_or(next, |bounds| AreaJudgeSourceBounds {
        minlon_source: bounds.minlon_source.min(next.minlon_source),
        maxlon_source: bounds.maxlon_source.max(next.maxlon_source),
        maxlat_source: bounds.maxlat_source.min(next.maxlat_source),
        minlat_source: bounds.minlat_source.max(next.minlat_source),
    })
}
