//! Quality flags shared by geometry overlay and coupling-fraction validation.

/// Geometry conditions surfaced by the production overlay and coupling paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeometryQualityFlag {
    ZeroAreaCell,
    MaskOverlapConflict,
    MissingMask,
    UnresolvedFractionSumError,
    NegativeArea,
    NonFiniteCoordinate,
}

impl GeometryQualityFlag {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ZeroAreaCell => "zero_area_cell",
            Self::MaskOverlapConflict => "mask_overlap_conflict",
            Self::MissingMask => "missing_mask",
            Self::UnresolvedFractionSumError => "unresolved_fraction_sum_error",
            Self::NegativeArea => "negative_area",
            Self::NonFiniteCoordinate => "non_finite_coordinate",
        }
    }
}

/// Validate mutually exclusive fractions that must sum to one within `tolerance`.
pub fn validate_fraction_partition(fractions: &[f64], tolerance: f64) -> Vec<GeometryQualityFlag> {
    let mut flags = Vec::new();
    if fractions.iter().any(|fraction| !fraction.is_finite()) {
        flags.push(GeometryQualityFlag::NonFiniteCoordinate);
        return flags;
    }
    if fractions.iter().any(|fraction| *fraction < -tolerance) {
        flags.push(GeometryQualityFlag::NegativeArea);
    }
    if (fractions.iter().sum::<f64>() - 1.0).abs() > tolerance {
        flags.push(GeometryQualityFlag::UnresolvedFractionSumError);
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fraction_partition_sum_over_one_is_flagged() {
        let flags = validate_fraction_partition(&[0.6, 0.6], 1.0e-6);
        assert!(flags.contains(&GeometryQualityFlag::UnresolvedFractionSumError));
    }

    #[test]
    fn fraction_partition_sum_one_is_clean() {
        assert!(validate_fraction_partition(&[0.4, 0.6], 1.0e-6).is_empty());
    }

    #[test]
    fn fraction_partition_rejects_non_finite_and_negative_values() {
        assert_eq!(
            validate_fraction_partition(&[f64::NAN, 1.0], 1.0e-6),
            vec![GeometryQualityFlag::NonFiniteCoordinate]
        );
        assert!(validate_fraction_partition(&[-0.1, 1.1], 1.0e-6)
            .contains(&GeometryQualityFlag::NegativeArea));
    }

    #[test]
    fn flag_strings_match_overlay_compatibility() {
        assert_eq!(GeometryQualityFlag::ZeroAreaCell.as_str(), "zero_area_cell");
        assert_eq!(GeometryQualityFlag::MissingMask.as_str(), "missing_mask");
    }
}
