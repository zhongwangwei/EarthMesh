//! Refinement demand read off a continuous threshold field.
//!
//! This covers every criterion in the catalogue that compares a number against
//! a threshold — `sst`, `ssh`, `eke`, `sea_slope` at sea, `lai`, `slope`, `dem`
//! and the soil conductivities on land, `typhoon` in the atmosphere, and
//! bathymetry when it arrives. They differ only in which file and variable to
//! read and which way the comparison runs, so one producer serves all of them.

use std::io;
use std::path::Path;

use earthmesh_mesh::AreaJudgeSourceBounds;

use super::RefinementDemand;
use crate::area_judge_threshold_inputs::data_read_onelayer_one_based;

fn periodic_halo_windows(
    bounds: AreaJudgeSourceBounds,
    gridnum_perdegree: usize,
    cells: usize,
) -> Vec<AreaJudgeSourceBounds> {
    let nlons_source = gridnum_perdegree.saturating_mul(360);
    let nlats_source = gridnum_perdegree.saturating_mul(180);
    let maxlat_source = bounds.maxlat_source.saturating_sub(cells).max(1);
    let minlat_source = bounds.minlat_source.saturating_add(cells).min(nlats_source);
    if cells >= nlons_source {
        return vec![AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: nlons_source,
            maxlat_source,
            minlat_source,
        }];
    }
    let mut windows = vec![AreaJudgeSourceBounds {
        minlon_source: bounds.minlon_source.saturating_sub(cells).max(1),
        maxlon_source: bounds.maxlon_source.saturating_add(cells).min(nlons_source),
        maxlat_source,
        minlat_source,
    }];
    if bounds.minlon_source <= cells {
        let missing = cells - bounds.minlon_source + 1;
        windows.push(AreaJudgeSourceBounds {
            minlon_source: nlons_source - missing + 1,
            maxlon_source: nlons_source,
            maxlat_source,
            minlat_source,
        });
    }
    if bounds.maxlon_source.saturating_add(cells) > nlons_source {
        windows.push(AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: bounds.maxlon_source.saturating_add(cells) - nlons_source,
            maxlat_source,
            minlat_source,
        });
    }
    windows
}

struct PeriodicThresholdLookup {
    windows: Vec<(AreaJudgeSourceBounds, Vec<Vec<f64>>)>,
    nlons_source: usize,
}

impl PeriodicThresholdLookup {
    fn read(
        threshold_file: impl AsRef<Path>,
        var_name: &str,
        gridnum_perdegree: usize,
        bounds: AreaJudgeSourceBounds,
        radius_cells: usize,
    ) -> io::Result<Self> {
        let threshold_file = threshold_file.as_ref();
        let windows = periodic_halo_windows(bounds, gridnum_perdegree, radius_cells)
            .into_iter()
            .map(|halo| {
                data_read_onelayer_one_based(threshold_file, var_name, halo)
                    .map(|field| (halo, field.values))
            })
            .collect::<io::Result<Vec<_>>>()?;
        Ok(Self {
            windows,
            nlons_source: gridnum_perdegree.saturating_mul(360),
        })
    }

    fn value_at_global(&self, lon_index: isize, lat_index: usize) -> Option<f64> {
        let lon_index = if self.nlons_source == 0 {
            lon_index.max(1) as usize
        } else {
            (lon_index - 1).rem_euclid(self.nlons_source as isize) as usize + 1
        };
        for (bounds, values) in &self.windows {
            if lon_index < bounds.minlon_source
                || lon_index > bounds.maxlon_source
                || lat_index < bounds.maxlat_source
                || lat_index > bounds.minlat_source
            {
                continue;
            }
            let lon_offset = lon_index - bounds.minlon_source + 1;
            let lat_offset = lat_index - bounds.maxlat_source + 1;
            return values
                .get(lon_offset)
                .and_then(|column| column.get(lat_offset))
                .copied();
        }
        None
    }
}

fn stddev_row_periodic(
    lookup: &PeriodicThresholdLookup,
    lat_index: usize,
    lon_from: usize,
    lon_to: usize,
    radius_cells: usize,
    out: &mut Vec<bool>,
    threshold: f64,
) {
    let width = lon_to.saturating_sub(lon_from) + 1;
    out.clear();
    out.resize(width, false);
    let Some((first_bounds, _)) = lookup.windows.first() else {
        return;
    };
    let lat_lo = lat_index
        .saturating_sub(radius_cells)
        .max(first_bounds.maxlat_source);
    let lat_hi = lat_index
        .saturating_add(radius_cells)
        .min(first_bounds.minlat_source);
    if lat_lo > lat_hi {
        return;
    }
    let scan_lo = lon_from as isize - radius_cells as isize;
    let scan_hi = lon_to as isize + radius_cells as isize;
    let scan_width = (scan_hi - scan_lo + 1) as usize;
    let mut column_count = vec![0usize; scan_width];
    let mut column_sum = vec![0.0f64; scan_width];
    let mut column_squares = vec![0.0f64; scan_width];
    for logical_lon in scan_lo..=scan_hi {
        let slot = (logical_lon - scan_lo) as usize;
        for lat in lat_lo..=lat_hi {
            let Some(value) = lookup.value_at_global(logical_lon, lat) else {
                continue;
            };
            if !value.is_finite() {
                continue;
            }
            column_count[slot] += 1;
            column_sum[slot] += value;
            column_squares[slot] += value * value;
        }
    }

    let mut count = 0usize;
    let mut sum = 0.0f64;
    let mut sum_squares = 0.0f64;
    let mut covered_lo = scan_lo;
    let mut covered_hi = scan_lo;
    let mut primed = false;
    for lon in lon_from..=lon_to {
        let want_lo = lon as isize - radius_cells as isize;
        let want_hi = lon as isize + radius_cells as isize;
        if !primed {
            for column_index in want_lo..=want_hi {
                let slot = (column_index - scan_lo) as usize;
                count += column_count[slot];
                sum += column_sum[slot];
                sum_squares += column_squares[slot];
            }
            covered_lo = want_lo;
            covered_hi = want_hi;
            primed = true;
        } else {
            while covered_hi < want_hi {
                covered_hi += 1;
                let slot = (covered_hi - scan_lo) as usize;
                count += column_count[slot];
                sum += column_sum[slot];
                sum_squares += column_squares[slot];
            }
            while covered_lo < want_lo {
                let slot = (covered_lo - scan_lo) as usize;
                count -= column_count[slot];
                sum -= column_sum[slot];
                sum_squares -= column_squares[slot];
                covered_lo += 1;
            }
        }
        if count >= 2 {
            let mean = sum / count as f64;
            let variance = (sum_squares / count as f64 - mean * mean).max(0.0);
            out[lon - lon_from] = variance.sqrt() > threshold;
        }
    }
}

fn crosses_periodic_lon_halo(
    bounds: AreaJudgeSourceBounds,
    gridnum_perdegree: usize,
    radius_cells: usize,
) -> bool {
    let nlons_source = gridnum_perdegree.saturating_mul(360);
    bounds.minlon_source <= radius_cells
        || bounds.maxlon_source.saturating_add(radius_cells) > nlons_source
}

struct ThresholdIntegralImage {
    width: usize,
    height: usize,
    count: Vec<u64>,
    sum: Vec<f64>,
    sum_squares: Vec<f64>,
}

impl ThresholdIntegralImage {
    fn new(values: &[Vec<f64>]) -> Self {
        let width = values.len().saturating_sub(1);
        let height = values
            .get(1)
            .map(|column| column.len().saturating_sub(1))
            .unwrap_or(0);
        let stride = height + 1;
        let len = (width + 1).saturating_mul(stride);
        let mut image = Self {
            width,
            height,
            count: vec![0; len],
            sum: vec![0.0; len],
            sum_squares: vec![0.0; len],
        };
        for x in 1..=width {
            for y in 1..=height {
                let value = values.get(x).and_then(|column| column.get(y)).copied();
                let finite = value.filter(|value| value.is_finite());
                let index = x * stride + y;
                let left = index - stride;
                let below = index - 1;
                let diagonal = left - 1;
                image.count[index] = image.count[left] + image.count[below] - image.count[diagonal]
                    + u64::from(finite.is_some());
                image.sum[index] = image.sum[left] + image.sum[below] - image.sum[diagonal]
                    + finite.unwrap_or(0.0);
                image.sum_squares[index] = image.sum_squares[left] + image.sum_squares[below]
                    - image.sum_squares[diagonal]
                    + finite.map_or(0.0, |value| value * value);
            }
        }
        image
    }

    fn window(&self, x1: usize, x2: usize, y1: usize, y2: usize) -> (u64, f64, f64) {
        let x1 = x1.clamp(1, self.width);
        let x2 = x2.clamp(x1, self.width);
        let y1 = y1.clamp(1, self.height);
        let y2 = y2.clamp(y1, self.height);
        let stride = self.height + 1;
        let corners = [
            x2 * stride + y2,
            (x1 - 1) * stride + y2,
            x2 * stride + y1 - 1,
            (x1 - 1) * stride + y1 - 1,
        ];
        (
            self.count[corners[0]] + self.count[corners[3]]
                - self.count[corners[1]]
                - self.count[corners[2]],
            self.sum[corners[0]] + self.sum[corners[3]]
                - self.sum[corners[1]]
                - self.sum[corners[2]],
            self.sum_squares[corners[0]] + self.sum_squares[corners[3]]
                - self.sum_squares[corners[1]]
                - self.sum_squares[corners[2]],
        )
    }
}

fn stddev_window_reference(
    values: &[Vec<f64>],
    x1: usize,
    x2: usize,
    y1: usize,
    y2: usize,
) -> Option<f64> {
    let mut count = 0usize;
    let mut sum = 0.0;
    let mut sum_squares = 0.0;
    for x in x1..=x2 {
        for y in y1..=y2 {
            let Some(value) = values
                .get(x)
                .and_then(|column| column.get(y))
                .copied()
                .filter(|value| value.is_finite())
            else {
                continue;
            };
            count += 1;
            sum += value;
            sum_squares += value * value;
        }
    }
    (count >= 2).then(|| {
        let mean = sum / count as f64;
        (sum_squares / count as f64 - mean * mean).max(0.0).sqrt()
    })
}

/// Which side of the threshold asks for refinement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThresholdSide {
    /// Refine where the value exceeds the threshold (slope, EKE, LAI).
    Above,
    /// Refine where the value falls below it (bathymetry: shallow water first).
    Below,
}

/// Mark every source cell whose value in `var_name` is on the demanding side of
/// `threshold`.
///
/// A window holding missing or non-finite values is rejected rather than
/// partially answered — that is the engine's existing threshold contract
/// (`reject_invalid_threshold_values`), and it is the safe reading: silently
/// skipping a fill value would under-refine without saying so.
pub fn threshold_demand(
    threshold_file: impl AsRef<Path>,
    var_name: &str,
    gridnum_perdegree: usize,
    bounds: AreaJudgeSourceBounds,
    side: ThresholdSide,
    threshold: f64,
) -> io::Result<RefinementDemand> {
    if !threshold.is_finite() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refinement threshold must be finite",
        ));
    }
    let mut demand = RefinementDemand::new(bounds, gridnum_perdegree)?;
    let field = data_read_onelayer_one_based(threshold_file, var_name, bounds)?;

    for (lon_offset, column) in field.values.iter().enumerate().skip(1) {
        for (lat_offset, value) in column.iter().enumerate().skip(1) {
            let wanted = match side {
                ThresholdSide::Above => *value > threshold,
                ThresholdSide::Below => *value < threshold,
            };
            if wanted {
                demand.set(
                    bounds.minlon_source + lon_offset - 1,
                    bounds.maxlat_source + lat_offset - 1,
                    true,
                );
            }
        }
    }
    Ok(demand)
}

/// Mark every source cell whose neighbourhood standard deviation exceeds
/// `threshold`.
///
/// The catalogue's threshold flags come in mean/std pairs — even slots compare
/// the value itself, odd slots compare how much it varies — and the h-field
/// honours both. The point+radius route read only the mean half, so a project
/// that asked for refinement where a field is *rough* (steep terrain, a sharp
/// SST front) got a uniform mesh there and no message saying why. This is the
/// other half.
///
/// The neighbourhood is `radius_cells` of source grid either side, matching the
/// cell the current pass is judging — the same scale the land-type criteria use,
/// so a criterion's answer changes with resolution exactly as they do.
pub fn threshold_stddev_demand(
    threshold_file: impl AsRef<Path>,
    var_name: &str,
    gridnum_perdegree: usize,
    bounds: AreaJudgeSourceBounds,
    radius_cells: usize,
    threshold: f64,
) -> io::Result<RefinementDemand> {
    if !threshold.is_finite() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refinement threshold must be finite",
        ));
    }
    let mut demand = RefinementDemand::new(bounds, gridnum_perdegree)?;
    // Read a haloed window so a cell on the edge of the domain still has a full
    // neighbourhood; without it the rim would be judged against a truncated
    // sample and read as smoother than it is.
    let periodic = crosses_periodic_lon_halo(bounds, gridnum_perdegree, radius_cells).then(|| {
        PeriodicThresholdLookup::read(
            threshold_file.as_ref(),
            var_name,
            gridnum_perdegree,
            bounds,
            radius_cells,
        )
    });
    let halo = super::landtype::halo_within_source(bounds, gridnum_perdegree, radius_cells);
    let field = if periodic.is_none() {
        Some(data_read_onelayer_one_based(
            threshold_file,
            var_name,
            halo,
        )?)
    } else {
        None
    };
    let periodic = periodic.transpose()?;
    let width = field
        .as_ref()
        .map(|field| field.values.len().saturating_sub(1))
        .unwrap_or(0);
    let height = field
        .as_ref()
        .and_then(|field| field.values.get(1))
        .map(|column| column.len().saturating_sub(1))
        .unwrap_or(0);

    if let Some(periodic) = &periodic {
        demand.fill_rows_par(|lat, lon_from, lon_to, row| {
            stddev_row_periodic(
                periodic,
                lat,
                lon_from,
                lon_to,
                radius_cells,
                row,
                threshold,
            );
        });
        return Ok(demand);
    }

    let values = &field.as_ref().unwrap().values;
    let integral = ThresholdIntegralImage::new(values);
    demand.fill_par(|lon_source, lat_source| {
        let lon_lo = lon_source
            .saturating_sub(radius_cells)
            .max(halo.minlon_source);
        let lon_hi = lon_source
            .saturating_add(radius_cells)
            .min(halo.maxlon_source);
        let lat_lo = lat_source
            .saturating_sub(radius_cells)
            .max(halo.maxlat_source);
        let lat_hi = lat_source
            .saturating_add(radius_cells)
            .min(halo.minlat_source);
        let x1 = lon_lo - halo.minlon_source + 1;
        let x2 = lon_hi - halo.minlon_source + 1;
        let y1 = lat_lo - halo.maxlat_source + 1;
        let y2 = lat_hi - halo.maxlat_source + 1;
        debug_assert!(x2 <= width && y2 <= height);
        let (count, sum, sum_squares) = integral.window(x1, x2, y1, y2);
        if count < 2 {
            return false;
        }
        let count_f64 = count as f64;
        let mean = sum / count_f64;
        let stddev = (sum_squares / count_f64 - mean * mean).max(0.0).sqrt();
        let comparison_scale = stddev.abs().max(threshold.abs()).max(1.0);
        let borderline =
            (stddev - threshold).abs() <= 256.0 * f64::EPSILON * comparison_scale * count_f64;
        if borderline {
            return stddev_window_reference(values, x1, x2, y1, y2)
                .is_some_and(|reference| reference > threshold);
        }
        stddev > threshold
    });
    Ok(demand)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integral_stddev_matches_the_reference_window_by_window() {
        let mut values = vec![vec![0.0; 8]; 9];
        for (x, column) in values.iter_mut().enumerate().skip(1) {
            for (y, value) in column.iter_mut().enumerate().skip(1) {
                *value = ((x * 17 + y * 11 + x * y) % 23) as f64 - 7.0;
            }
        }
        let integral = ThresholdIntegralImage::new(&values);
        for x1 in 1..=8 {
            for x2 in x1..=8 {
                for y1 in 1..=7 {
                    for y2 in y1..=7 {
                        let (count, sum, sum_squares) = integral.window(x1, x2, y1, y2);
                        let actual = (count >= 2).then(|| {
                            let mean = sum / count as f64;
                            (sum_squares / count as f64 - mean * mean).max(0.0).sqrt()
                        });
                        let expected = stddev_window_reference(&values, x1, x2, y1, y2);
                        assert_eq!(actual, expected, "window {x1}..={x2}, {y1}..={y2}");
                    }
                }
            }
        }
    }
}
