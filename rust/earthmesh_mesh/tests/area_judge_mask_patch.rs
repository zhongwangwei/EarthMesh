use earthmesh_mesh::{area_judge_apply_mask_patch_one_based, AreaJudgeSourceBounds};

fn one_based_grid(nx: usize, ny: usize, fill: bool) -> Vec<Vec<bool>> {
    vec![vec![fill; ny + 1]; nx + 1]
}

fn one_based_mask(nx: usize, ny: usize, fill: bool) -> Vec<Vec<bool>> {
    vec![vec![fill; ny + 1]; nx + 1]
}

#[test]
fn mask_patch_modify_zeroes_land_where_patch_mask_is_nonzero_inside_bounds() {
    let mut seaorland = one_based_grid(4, 4, true);
    let mut patch_mask = one_based_mask(4, 4, false);
    patch_mask[2][2] = true;
    patch_mask[3][2] = true;
    patch_mask[2][3] = false;
    patch_mask[3][3] = true;
    patch_mask[4][4] = true;

    let report = area_judge_apply_mask_patch_one_based(
        &mut seaorland,
        &patch_mask,
        AreaJudgeSourceBounds {
            minlon_source: 2,
            maxlon_source: 3,
            maxlat_source: 2,
            minlat_source: 3,
        },
    )
    .expect("valid one-based patch grid");

    assert_eq!(report.patched_cells, 3);
    assert!(!seaorland[2][2]);
    assert!(!seaorland[3][2]);
    assert!(seaorland[2][3]);
    assert!(!seaorland[3][3]);
    assert!(seaorland[4][4], "outside bounds must not be modified");
}

#[test]
fn mask_patch_modify_rejects_bounds_or_masks_that_do_not_cover_canonical_indices() {
    let mut seaorland = one_based_grid(2, 2, true);
    let patch_mask = one_based_mask(2, 2, true);

    assert!(area_judge_apply_mask_patch_one_based(
        &mut seaorland,
        &patch_mask,
        AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: 3,
            maxlat_source: 1,
            minlat_source: 2,
        },
    )
    .is_none());

    let ragged_patch = vec![vec![false; 3], vec![false; 2], vec![false; 3]];
    assert!(area_judge_apply_mask_patch_one_based(
        &mut seaorland,
        &ragged_patch,
        AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: 2,
            maxlat_source: 1,
            minlat_source: 2,
        },
    )
    .is_none());
}
