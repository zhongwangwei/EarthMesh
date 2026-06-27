use earthmesh_mesh::AreaJudgeSourceBounds;

pub(super) fn count_area_judge_selected_cells_fortran_indexed(
    grid: &[Vec<i32>],
    bounds: AreaJudgeSourceBounds,
) -> usize {
    (bounds.maxlat_source..=bounds.minlat_source)
        .flat_map(|lat_index| {
            (bounds.minlon_source..=bounds.maxlon_source)
                .map(move |lon_index| (lon_index, lat_index))
        })
        .filter(|(lon_index, lat_index)| grid[*lon_index][*lat_index] != 0)
        .count()
}
