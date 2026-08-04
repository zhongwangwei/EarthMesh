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
