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
    let halo = super::landtype::halo_within_source(bounds, gridnum_perdegree, radius_cells);
    let field = data_read_onelayer_one_based(threshold_file, var_name, halo)?;
    let width = field.values.len().saturating_sub(1);
    let height = field
        .values
        .get(1)
        .map(|column| column.len().saturating_sub(1))
        .unwrap_or(0);

    demand.fill_par(|lon_source, lat_source| {
        {
            let mut count = 0usize;
            let mut sum = 0.0_f64;
            let mut sum_squares = 0.0_f64;
            for lon in lon_source.saturating_sub(radius_cells)..=(lon_source + radius_cells) {
                if lon < halo.minlon_source || lon > halo.maxlon_source {
                    continue;
                }
                let lon_offset = lon - halo.minlon_source + 1;
                if lon_offset > width {
                    continue;
                }
                for lat in lat_source.saturating_sub(radius_cells)..=(lat_source + radius_cells) {
                    if lat < halo.maxlat_source || lat > halo.minlat_source {
                        continue;
                    }
                    let lat_offset = lat - halo.maxlat_source + 1;
                    if lat_offset > height {
                        continue;
                    }
                    let value = field.values[lon_offset][lat_offset];
                    if !value.is_finite() {
                        continue;
                    }
                    count += 1;
                    sum += value;
                    sum_squares += value * value;
                }
            }
            if count < 2 {
                return false;
            }
            let mean = sum / count as f64;
            // Population variance, as the h-field's own statistic is, and
            // clamped at zero because rounding can drive it slightly negative
            // over a flat neighbourhood.
            let variance = (sum_squares / count as f64 - mean * mean).max(0.0);
            variance.sqrt() > threshold
        }
    });
    Ok(demand)
}
