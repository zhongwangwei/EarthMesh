/// Result of `MOD_grid_preprocess:find_frac_index`.
///
/// `index` is intentionally 1-based to preserve the Fortran caller contract.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FortranFracIndex {
    pub index: usize,
    pub frac: f64,
}

/// Port of `MOD_grid_preprocess:find_frac_index` with explicit failure.
///
/// The Fortran subroutine supports monotonic ascending longitude grids and
/// monotonic descending latitude grids. The original error path is unreachable
/// after `return`; this Rust port returns `None` when the point is outside the
/// provided bounds or a zero-width cell is encountered.
pub fn find_frac_index_fortran(grid: &[f64], point: f64) -> Option<FortranFracIndex> {
    if grid.len() < 2 {
        return None;
    }

    let ascending = grid[0] < *grid.last()?;
    for i in 0..(grid.len() - 1) {
        let in_cell = if ascending {
            point >= grid[i] && point <= grid[i + 1]
        } else {
            point <= grid[i] && point >= grid[i + 1]
        };
        if !in_cell {
            continue;
        }

        let dx = grid[i + 1] - grid[i];
        if dx == 0.0 {
            return None;
        }
        let frac = ((point - grid[i]) / dx).clamp(0.0, 1.0);
        return Some(FortranFracIndex { index: i + 1, frac });
    }

    None
}

/// Rust representation of `refine_vars:set_dis_type` choices used by
/// `MOD_grid_preprocess:dist_layers_make`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceLayerSpacing {
    Linear,
    Power,
    Exponential,
    Logarithmic,
}

/// Port of `MOD_grid_preprocess:dist_layers_make`.
pub fn distance_layers(
    dist_len: usize,
    dist_select: f64,
    spacing: DistanceLayerSpacing,
) -> Option<Vec<f64>> {
    if dist_len == 0 {
        return None;
    }

    let mindist_select = dist_select / 2.0;
    let dist_len_f = dist_len as f64;
    let mut layers = Vec::with_capacity(dist_len);

    match spacing {
        DistanceLayerSpacing::Linear => {
            let a = mindist_select / dist_len_f;
            let b = mindist_select - a;
            for i in 1..=dist_len {
                layers.push(a * (i as f64 + 1.0) + b);
            }
        }
        DistanceLayerSpacing::Power => {
            let a = mindist_select;
            let b = 2.0_f64.ln() / (dist_len_f + 1.0).ln();
            for i in 1..=dist_len {
                layers.push(a * (i as f64 + 1.0).powf(b));
            }
        }
        DistanceLayerSpacing::Exponential => {
            let b = 2.0_f64.powf(1.0 / dist_len_f);
            let a = mindist_select / b;
            for i in 1..=dist_len {
                layers.push(a * b.powf(i as f64 + 1.0));
            }
        }
        DistanceLayerSpacing::Logarithmic => {
            let b = mindist_select;
            let a = b / (dist_len_f + 1.0).ln();
            for i in 1..=dist_len {
                layers.push(a * (i as f64 + 1.0).ln() + b);
            }
        }
    }

    Some(layers)
}
