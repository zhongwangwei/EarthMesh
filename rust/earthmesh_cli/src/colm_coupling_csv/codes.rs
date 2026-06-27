use super::row::ColmCouplingCsvRow;

pub(crate) fn surface_class_code(value: &str) -> i8 {
    match value {
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
        "COAST" => (1.0 - row.coastal_fraction).clamp(0.0, 1.0),
        _ => 0.0,
    }
}

pub(crate) fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

pub(crate) fn river_class_code(value: &str) -> i8 {
    match value {
        "R1" => 1,
        "R2" => 2,
        "R3" => 3,
        _ => 0,
    }
}

pub(crate) fn coast_class_code(value: &str) -> i8 {
    match value {
        "COAST" => 1,
        "COAST_LAND" => 2,
        "COAST_OCEAN" => 3,
        _ => 0,
    }
}
