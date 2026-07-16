use super::*;

/// Summary from the pure `mask_patch_modify` sea/land update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AreaJudgeMaskPatchReport {
    pub patched_cells: usize,
}

fn area_judge_grid_covers_bounds_one_based<T>(
    grid: &[Vec<T>],
    bounds: AreaJudgeSourceBounds,
) -> bool {
    if bounds.minlon_source > bounds.maxlon_source
        || bounds.maxlat_source > bounds.minlat_source
        || bounds.maxlon_source >= grid.len()
    {
        return false;
    }
    (bounds.minlon_source..=bounds.maxlon_source)
        .all(|lon_index| bounds.minlat_source < grid[lon_index].len())
}

/// Pure Rust core of `MOD_Area_judge:mask_patch_modify`.
///
/// Canonical first builds an `IsInPaArea_grid` patch mask, then scans the
/// inclusive patch bounds and sets `seaorland(i, j) = 0` wherever that patch
/// mask is nonzero.  This helper keeps the same one-based array convention and
/// returns the number of nonzero patch cells applied; area construction and
/// NetCDF restart I/O remain in the higher-level orchestration layer.
pub fn area_judge_apply_mask_patch_one_based<T>(
    seaorland: &mut [Vec<bool>],
    patch_mask: &[Vec<T>],
    bounds: AreaJudgeSourceBounds,
) -> Option<AreaJudgeMaskPatchReport>
where
    T: Copy + Into<i32>,
{
    if !area_judge_grid_covers_bounds_one_based(seaorland, bounds)
        || !area_judge_grid_covers_bounds_one_based(patch_mask, bounds)
    {
        return None;
    }

    let mut patched_cells = 0usize;
    for lat_index in bounds.maxlat_source..=bounds.minlat_source {
        for lon_index in bounds.minlon_source..=bounds.maxlon_source {
            if patch_mask[lon_index][lat_index].into() != 0 {
                seaorland[lon_index][lat_index] = false;
                patched_cells += 1;
            }
        }
    }

    Some(AreaJudgeMaskPatchReport { patched_cells })
}
