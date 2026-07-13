use super::row::ColmCouplingCsvRow;

pub(crate) fn surface_class_code(value: &str) -> i8 {
    match value.trim().to_ascii_uppercase().as_str() {
        "LAND" => 1,
        "OCEAN" => 2,
        "COAST" => 3,
        _ => 0,
    }
}

pub(crate) fn colm_land_fraction(row: &ColmCouplingCsvRow) -> f64 {
    match row.surface_class.trim().to_ascii_uppercase().as_str() {
        "LAND" => 1.0,
        "OCEAN" => 0.0,
        "COAST" => 1.0 - fraction_or_zero(row.coastal_fraction),
        _ => 0.0,
    }
}

pub(crate) const COLM_F64_FILL_VALUE: f64 = 9.969_209_968_386_869e36;

pub(crate) fn finite_or_fill(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        COLM_F64_FILL_VALUE
    }
}

pub(crate) fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

pub(crate) fn fraction_or_fill(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        COLM_F64_FILL_VALUE
    }
}

pub(crate) fn fraction_or_zero(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

pub(crate) fn river_class_code(value: &str) -> i8 {
    match value.trim().to_ascii_uppercase().as_str() {
        "R1" => 1,
        "R2" => 2,
        "R3" => 3,
        _ => 0,
    }
}

pub(crate) fn coast_class_code(value: &str) -> i8 {
    match value.trim().to_ascii_uppercase().as_str() {
        "COAST" => 1,
        "COAST_LAND" => 2,
        "COAST_OCEAN" => 3,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colm_fraction_helpers_clamp_and_use_standard_fill() {
        assert_eq!(fraction_or_fill(1.5), 1.0);
        assert_eq!(fraction_or_zero(-0.5), 0.0);
        assert_eq!(fraction_or_zero(f64::NAN), 0.0);
        assert!(COLM_F64_FILL_VALUE.is_sign_positive());
        assert_eq!(fraction_or_fill(f64::NAN), COLM_F64_FILL_VALUE);
    }
}
