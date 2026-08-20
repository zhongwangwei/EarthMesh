use super::*;
use crate::ContainMesh;

/// `GetRef_Lnd`'s two land-cover criteria, transcribed.
///
/// The oracle is deliberately naive and shaped like the Fortran rather than
/// like the code it checks: a tally indexed by class, counted the way `nlaa` is
/// and divided the way `f_mainarea` is. Agreeing with a tidier rewrite of the
/// same idea would prove nothing -- guide 11.3 records a rewrite whose whole
/// suite stayed green while results moved, and only a comparison against the
/// implementation being replaced caught it.
fn oracle(
    contain: &ContainMesh,
    fill_class: i32,
    classes: usize,
    classify: impl Fn(usize, usize) -> Option<i32>,
) -> Vec<Option<(usize, f64)>> {
    let mut out = vec![None; contain.ustr_id.len()];
    for cell in 0..contain.ustr_id.len() {
        if contain.is_in_area_ustr[cell] != 1 {
            continue;
        }
        let count = contain.ustr_id[cell][0] as usize;
        let offset = contain.ustr_id[cell][1] as usize;
        if count == 0 {
            continue;
        }
        let mut nlaa = vec![0usize; classes + 1];
        for j in 0..count {
            let row = contain.ustr_ii[offset + j][0] as usize;
            let col = contain.ustr_ii[offset + j][1] as usize;
            let Some(l) = classify(row, col) else {
                continue;
            };
            if l != fill_class {
                nlaa[l as usize] += 1;
            }
        }
        let present = nlaa.iter().filter(|&&t| t > 0).count();
        let dominant = *nlaa.iter().max().unwrap_or(&0);
        out[cell] = Some((present, (dominant as f64 / count as f64).min(1.0)));
    }
    out
}

fn contain_from(cells: &[(&[(i32, i32)], i32)]) -> ContainMesh {
    let mut ustr_id = Vec::new();
    let mut ustr_ii = Vec::new();
    let mut is_in_area_ustr = Vec::new();
    for (sources, in_area) in cells {
        ustr_id.push(vec![sources.len() as i32, ustr_ii.len() as i32]);
        is_in_area_ustr.push(*in_area);
        for (row, col) in sources.iter() {
            ustr_ii.push(vec![*row, *col]);
        }
    }
    ContainMesh {
        ustr_id,
        ustr_ii,
        is_in_area_ustr,
    }
}

fn checkerboard(row: usize, col: usize) -> Option<i32> {
    Some(((row * 7 + col * 13) % 5) as i32)
}

#[test]
fn matches_the_reference_on_a_mixed_index() {
    let contain = contain_from(&[
        (&[(1, 1), (1, 2), (2, 1)][..], 1),
        (&[(3, 3), (3, 4), (4, 3), (4, 4), (5, 5)][..], 1),
        (&[(6, 6)][..], 0),
        (&[][..], 1),
        (&[(7, 7), (7, 7), (7, 7), (7, 7)][..], 1),
    ]);
    let got = cell_landcover_stats(&contain, 4, checkerboard);
    let want = oracle(&contain, 4, 4, checkerboard);
    assert_eq!(got.len(), want.len());
    for (cell, (got, want)) in got.iter().zip(want.iter()).enumerate() {
        match (got, want) {
            (None, None) => {}
            (Some(got), Some((classes, fraction))) => {
                assert_eq!(got.class_count, *classes, "cell {cell} class count");
                assert!(
                    (got.dominant_fraction - fraction).abs() < 1e-12,
                    "cell {cell} dominant fraction {} vs {fraction}",
                    got.dominant_fraction
                );
            }
            _ => panic!("cell {cell}: {got:?} vs {want:?}"),
        }
    }
}

/// A cell containing nothing is not a uniform cell.
#[test]
fn an_empty_cell_reports_nothing_rather_than_uniformity() {
    let contain = contain_from(&[(&[][..], 1)]);
    assert_eq!(cell_landcover_stats(&contain, 4, checkerboard)[0], None);
}

/// The fill class is out of the numerator and still in the denominator.
///
/// This is what makes the dominant-fraction criterion fire on a coastline: a
/// triangle that is half ocean reports its land as holding half the cell.
/// Reading `Lnd_id(i, 1)` as "land cells" rather than "contained cells" would
/// turn a coastal criterion into a no-op exactly where it matters.
#[test]
fn ocean_counts_against_the_dominant_fraction() {
    let contain = contain_from(&[(&[(1, 1), (1, 2), (1, 3), (1, 4)][..], 1)]);
    let stats = cell_landcover_stats(&contain, 9, |_row, col| Some(if col <= 2 { 9 } else { 3 }))
        [0]
    .expect("cell measured");
    assert_eq!(stats.class_count, 1);
    assert_eq!(stats.contained_cells, 4);
    assert!(
        (stats.dominant_fraction - 0.5).abs() < 1e-12,
        "two land cells out of four contained is half, got {}",
        stats.dominant_fraction
    );
}
