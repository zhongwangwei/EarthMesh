/// Axis selector for the `MOD_Area_judge:Source_Find` source-grid lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AreaJudgeAxis {
    Longitude,
    Latitude,
}

/// One-based source-grid bounds returned by
/// `MOD_Area_judge:minmax_range_make`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AreaJudgeSourceBounds {
    pub minlon_source: usize,
    pub maxlon_source: usize,
    pub maxlat_source: usize,
    pub minlat_source: usize,
}

fn area_judge_source_window_fortran_indexed(
    temp: f64,
    axis: AreaJudgeAxis,
    gridnum_perdegree: usize,
    n_source: usize,
    max_index: usize,
) -> Option<(usize, usize)> {
    if !temp.is_finite() || gridnum_perdegree == 0 || n_source == 0 || max_index < 1 {
        return None;
    }

    let gridnum = gridnum_perdegree as isize;
    let (minsource, maxsource) = match axis {
        AreaJudgeAxis::Longitude => (
            ((temp.floor() as isize) + 180) * gridnum,
            ((temp.ceil() as isize) + 180) * gridnum,
        ),
        AreaJudgeAxis::Latitude => (
            (90 - temp.ceil() as isize) * gridnum,
            (90 - temp.floor() as isize) * gridnum,
        ),
    };

    let start = (minsource - 10).max(1) as usize;
    let end = (maxsource + 10).min((1 + n_source) as isize) as usize;
    if start > end {
        return None;
    }
    Some((start.min(max_index), end.min(max_index)))
}

/// Pure Rust port of `MOD_Area_judge:Source_Find`.
///
/// The routine keeps the Fortran one-based indexing convention: callers pass
/// a placeholder at index 0, source vertices occupy `1..=n_source+1`, longitude
/// vertices ascend from -180 to 180, and latitude vertices descend from 90 to
/// -90.  The search is bounded by the same degree-derived ±10-cell window used
/// in Fortran before scanning for the first matching vertex.
pub fn area_judge_source_find_fortran_indexed(
    temp: f64,
    seq_lonlat: &[f64],
    axis: AreaJudgeAxis,
    gridnum_perdegree: usize,
    n_source: usize,
) -> Option<usize> {
    let max_index = seq_lonlat.len().checked_sub(1)?;
    let (start, end) = area_judge_source_window_fortran_indexed(
        temp,
        axis,
        gridnum_perdegree,
        n_source,
        max_index,
    )?;
    match axis {
        AreaJudgeAxis::Longitude => (start..=end).find(|&index| temp <= seq_lonlat[index]),
        AreaJudgeAxis::Latitude => (start..=end).find(|&index| temp >= seq_lonlat[index]),
    }
}

/// Pure Rust return-value form of `MOD_Area_judge:minmax_range_make`.
///
/// The Fortran subroutine also mutates one of three global range accumulators
/// depending on `type_select`.  This kernel intentionally returns just the
/// source bounds; the later `Area_judge` orchestration can merge these bounds
/// into domain/refine/patch accumulators without reimplementing the lookup.
pub fn area_judge_minmax_range_make_fortran_indexed(
    edgew_temp: f64,
    edgee_temp: f64,
    edgen_temp: f64,
    edges_temp: f64,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> Option<AreaJudgeSourceBounds> {
    let minlon_source = area_judge_source_find_fortran_indexed(
        edgew_temp,
        lon_vertex,
        AreaJudgeAxis::Longitude,
        gridnum_perdegree,
        nlons_source,
    )?;
    let mut maxlon_source = area_judge_source_find_fortran_indexed(
        edgee_temp,
        lon_vertex,
        AreaJudgeAxis::Longitude,
        gridnum_perdegree,
        nlons_source,
    )?
    .checked_sub(2)?;
    let maxlat_source = area_judge_source_find_fortran_indexed(
        edgen_temp,
        lat_vertex,
        AreaJudgeAxis::Latitude,
        gridnum_perdegree,
        nlats_source,
    )?;
    let mut minlat_source = area_judge_source_find_fortran_indexed(
        edges_temp,
        lat_vertex,
        AreaJudgeAxis::Latitude,
        gridnum_perdegree,
        nlats_source,
    )?
    .checked_sub(2)?;

    if maxlon_source == nlons_source.saturating_sub(1) {
        maxlon_source += 1;
    }
    if minlat_source == nlats_source.saturating_sub(1) {
        minlat_source += 1;
    }

    Some(AreaJudgeSourceBounds {
        minlon_source,
        maxlon_source,
        maxlat_source,
        minlat_source,
    })
}
