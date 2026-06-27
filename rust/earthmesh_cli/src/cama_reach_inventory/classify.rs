use std::io;

use crate::cama_binary_io::{
    CamaReachClassification, CamaReachClassificationThresholds, CamaReachRecord,
};

/// Classify one CaMa reach using the same default R0/R1/R2/R3 policy as the Python prototype.
pub fn classify_cama_reach_record(
    record: &CamaReachRecord,
    thresholds: CamaReachClassificationThresholds,
) -> io::Result<CamaReachClassification> {
    if !record.target_dx_km.is_finite() || record.target_dx_km <= 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "target_dx_km must be positive for CaMa reach classification",
        ));
    }
    let effective_width_m = record.width_m.max(record.floodplain_width_m);
    let target_dx_m = record.target_dx_km * 1000.0;

    if record.is_estuary {
        return Ok(CamaReachClassification {
            reach_id: record.reach_id.clone(),
            river_class: "R3".to_string(),
            effective_width_m,
            reasons: vec!["estuary".to_string()],
        });
    }
    if effective_width_m >= thresholds.explicit_2d_width_fraction * target_dx_m {
        return Ok(CamaReachClassification {
            reach_id: record.reach_id.clone(),
            river_class: "R3".to_string(),
            effective_width_m,
            reasons: vec!["effective_width_fraction".to_string()],
        });
    }
    if record.upstream_area_km2 >= thresholds.explicit_2d_upstream_area_km2 {
        return Ok(CamaReachClassification {
            reach_id: record.reach_id.clone(),
            river_class: "R3".to_string(),
            effective_width_m,
            reasons: vec!["upstream_area_r3".to_string()],
        });
    }
    if record.upstream_area_km2 >= thresholds.refine_upstream_area_km2 {
        return Ok(CamaReachClassification {
            reach_id: record.reach_id.clone(),
            river_class: "R2".to_string(),
            effective_width_m,
            reasons: vec!["upstream_area_r2".to_string()],
        });
    }
    if effective_width_m >= thresholds.refine_width_fraction * target_dx_m {
        return Ok(CamaReachClassification {
            reach_id: record.reach_id.clone(),
            river_class: "R2".to_string(),
            effective_width_m,
            reasons: vec!["refine_width_fraction".to_string()],
        });
    }
    if record.upstream_area_km2 >= thresholds.keep_1d_upstream_area_km2 {
        return Ok(CamaReachClassification {
            reach_id: record.reach_id.clone(),
            river_class: "R1".to_string(),
            effective_width_m,
            reasons: vec!["upstream_area_r1".to_string()],
        });
    }
    Ok(CamaReachClassification {
        reach_id: record.reach_id.clone(),
        river_class: "R0".to_string(),
        effective_width_m,
        reasons: vec!["below_explicit_thresholds".to_string()],
    })
}
