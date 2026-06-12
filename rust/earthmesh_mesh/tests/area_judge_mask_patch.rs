use earthmesh_mesh::{area_judge_apply_mask_patch_fortran_indexed, AreaJudgeSourceBounds};

fn one_based_grid(nx: usize, ny: usize, fill: i32) -> Vec<Vec<i32>> {
    vec![vec![fill; ny + 1]; nx + 1]
}

#[test]
fn mask_patch_modify_zeroes_land_where_patch_mask_is_nonzero_inside_bounds() {
    let mut seaorland = one_based_grid(4, 4, 1);
    let mut patch_mask = one_based_grid(4, 4, 0);
    patch_mask[2][2] = 1;
    patch_mask[3][2] = 7;
    patch_mask[2][3] = 0;
    patch_mask[3][3] = 1;
    patch_mask[4][4] = 1;

    let report = area_judge_apply_mask_patch_fortran_indexed(
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
    assert_eq!(seaorland[2][2], 0);
    assert_eq!(seaorland[3][2], 0);
    assert_eq!(seaorland[2][3], 1);
    assert_eq!(seaorland[3][3], 0);
    assert_eq!(seaorland[4][4], 1, "outside bounds must not be modified");
}

#[test]
fn mask_patch_modify_rejects_bounds_or_masks_that_do_not_cover_fortran_indices() {
    let mut seaorland = one_based_grid(2, 2, 1);
    let patch_mask = one_based_grid(2, 2, 1);

    assert!(area_judge_apply_mask_patch_fortran_indexed(
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

    let ragged_patch = vec![vec![0; 3], vec![0; 2], vec![0; 3]];
    assert!(area_judge_apply_mask_patch_fortran_indexed(
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
