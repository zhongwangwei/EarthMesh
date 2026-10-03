//! The resolution ladder: a base mesh of NXP, its cells halved at every level
//! below it. One rule for the engine, the project and what they report.

use crate::EARTH_RADIUS_METERS;

/// The base cell at NXP 1, metres: a fifth of a great circle. A base of NXP
/// has cells of this over NXP -- the size the h-field, the project and the
/// Studio quote for an NXP.
pub const NXP1_CELL_METRES: f64 = 2.0 * std::f64::consts::PI * EARTH_RADIUS_METERS / 5.0;

/// The base cell of NXP `nxp`, metres.
pub fn base_cell_metres(nxp: usize) -> f64 {
    NXP1_CELL_METRES / nxp.max(1) as f64
}

/// The levels below a base of NXP `nxp` whose cell is nearest
/// `finest_metres` -- the power of two nearest the ratio of the two -- or
/// `None` when that is no level below the base, or the size is not a
/// positive number.
pub fn levels_to_cell(nxp: usize, finest_metres: f64) -> Option<usize> {
    if nxp == 0 || !(finest_metres.is_finite() && finest_metres > 0.0) {
        return None;
    }
    let levels = (base_cell_metres(nxp) / finest_metres).log2().round();
    (levels >= 1.0).then_some(levels as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_finest_size_snaps_to_the_nearest_level_below_the_base() {
        // A 1 km base: 30 m is five halvings (31 m) away.
        let nxp = (NXP1_CELL_METRES / 1000.0).round() as usize;
        assert!((base_cell_metres(nxp) - 1000.0).abs() < 0.1);
        assert_eq!(levels_to_cell(nxp, 30.0), Some(5));
        assert_eq!(levels_to_cell(nxp, 500.0), Some(1));
        // Nearest by ratio: 1000/354 is just under 2^1.5.
        assert_eq!(levels_to_cell(nxp, 354.0), Some(1));
        assert_eq!(levels_to_cell(nxp, 353.0), Some(2));
        // Not below the base, or not a size.
        for finest in [800.0, 1000.0, 5000.0, 0.0, -30.0, f64::NAN] {
            assert_eq!(levels_to_cell(nxp, finest), None, "{finest}");
        }
        assert_eq!(levels_to_cell(0, 30.0), None);
    }
}
