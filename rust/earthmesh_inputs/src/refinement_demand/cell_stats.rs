//! Land-cover criteria measured on the mesh cell, not on a block of raster.
//!
//! The criteria in [`super::landtype`] ask their question of a square
//! neighbourhood of source cells: right size, wrong shape, and positioned by the
//! raster's grid rather than by the cell being judged. The reference
//! implementation asks it of the triangle -- `MOD_GetRef.F90:GetRef_Lnd` walks
//! the source cells a triangle contains, through the index `Get_Contain` builds
//! -- and "how much variation is left inside this cell" is a question about the
//! cell. Guide 11.5 and section 3 record the gap.
//!
//! This module is that measurement and nothing else. It takes an already-built
//! containment index and returns per-cell statistics; it does not read files,
//! decide thresholds, or emit demand. Wiring it into the refinement route is a
//! separate step, and until that happens nothing here changes any output.

use crate::ContainMesh;

/// What a cell contains, in the terms the land-cover criteria are written in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellLandcoverStats {
    /// Distinct land classes present, ignoring the fill class.
    ///
    /// `n_landtypes` in the reference.
    pub class_count: usize,
    /// Share of the cell held by its most common class, capped at one.
    ///
    /// `f_mainarea` in the reference. The denominator is every source cell the
    /// triangle contains, including the fill-class ones the numerator skips, so
    /// a cell that is mostly ocean reports a low fraction for its land -- which
    /// is what makes the criterion fire on coastlines.
    pub dominant_fraction: f64,
    /// Source cells the triangle contains, fill class included.
    pub contained_cells: usize,
}

/// Measure every cell the containment index marks as in the refinement area.
///
/// `classify` maps a source cell's `(row, col)` to its land class, both
/// one-based as the index stores them. Cells outside the refinement area, and
/// rows the index leaves empty, come back `None` rather than as zeroed
/// statistics -- a cell with no contained source is not a cell with no
/// variation, and collapsing the two is how a criterion comes to report that
/// every cell is uniform.
pub fn cell_landcover_stats(
    contain: &ContainMesh,
    fill_class: i32,
    mut classify: impl FnMut(usize, usize) -> Option<i32>,
) -> Vec<Option<CellLandcoverStats>> {
    let mut out = vec![None; contain.ustr_id.len()];
    let mut counts: Vec<(i32, usize)> = Vec::new();
    for (cell, row) in contain.ustr_id.iter().enumerate() {
        if contain.is_in_area_ustr.get(cell).copied().unwrap_or(0) != 1 {
            continue;
        }
        let (Some(&count), Some(&offset)) = (row.first(), row.get(1)) else {
            continue;
        };
        let (Ok(count), Ok(offset)) = (usize::try_from(count), usize::try_from(offset)) else {
            continue;
        };
        if count == 0 {
            continue;
        }
        counts.clear();
        for entry in offset..offset.saturating_add(count) {
            let Some(pair) = contain.ustr_ii.get(entry) else {
                continue;
            };
            let (Some(&source_row), Some(&source_col)) = (pair.first(), pair.get(1)) else {
                continue;
            };
            let (Ok(source_row), Ok(source_col)) =
                (usize::try_from(source_row), usize::try_from(source_col))
            else {
                continue;
            };
            let Some(class) = classify(source_row, source_col) else {
                continue;
            };
            if class == fill_class {
                continue;
            }
            match counts.iter_mut().find(|(seen, _)| *seen == class) {
                Some((_, tally)) => *tally += 1,
                None => counts.push((class, 1)),
            }
        }
        let dominant = counts.iter().map(|(_, tally)| *tally).max().unwrap_or(0);
        out[cell] = Some(CellLandcoverStats {
            class_count: counts.len(),
            dominant_fraction: (dominant as f64 / count as f64).min(1.0),
            contained_cells: count,
        });
    }
    out
}

#[cfg(test)]
mod tests;
