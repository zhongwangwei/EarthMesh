use crate::{
    canonical_quote, namelist_assignments, parse_canonical_string, parse_f64, parse_i32,
    DEFAULT_MIN_ANGLE_WARN_DEG,
};

/// Quality-gate thresholds + on-violation policy, carried in an optional
/// `&quality` namelist block. **Purely additive**: the existing `&mkgrd` /
/// `&mkrefine` parsers ignore this block, and nothing in mesh generation reads
/// it — the CLI/GUI map it to `earthmesh_quality::QualityThresholds` and a
/// Warn/Block policy when judging a finished mesh. Absent block ⇒ `default()`,
/// which mirrors `earthmesh_quality::QualityThresholds::default()`.
#[derive(Clone, Debug, PartialEq)]
pub struct QualityNamelist {
    pub min_angle_warn_deg: f64,
    pub min_angle_fail_deg: f64,
    pub angle_deviation_warn_deg: f64,
    pub aspect_ratio_warn: f64,
    pub aspect_ratio_fail: f64,
    pub cell_edge_cv_warn: f64,
    pub area_cv_warn: f64,
    pub max_adjacent_resolution_ratio_warn: f64,
    pub worst_cells_limit: i32,
    pub repair_batch_limit: i32,
    /// "warn" (report only) or "block" (a Fail verdict aborts the run).
    pub on_violation: String,
}

impl Default for QualityNamelist {
    fn default() -> Self {
        // Keep in lock-step with earthmesh_quality::QualityThresholds::default().
        Self {
            min_angle_warn_deg: DEFAULT_MIN_ANGLE_WARN_DEG,
            min_angle_fail_deg: 5.0,
            angle_deviation_warn_deg: 35.0,
            aspect_ratio_warn: 4.0,
            aspect_ratio_fail: 10.0,
            cell_edge_cv_warn: 0.35,
            area_cv_warn: 1.5,
            max_adjacent_resolution_ratio_warn: 2.0,
            worst_cells_limit: 50,
            repair_batch_limit: 1,
            on_violation: String::from("warn"),
        }
    }
}

impl QualityNamelist {
    /// Parse a `&quality` block. Unknown keys are rejected so misspellings do
    /// not silently select defaults. Missing known keys keep their default, and
    /// old namelists without a `&quality` block still yield `default()`.
    pub fn from_quality_namelist(input: &str) -> Result<Self, String> {
        let mut config = Self::default();
        for assignment in namelist_assignments(input, "quality")? {
            let field = assignment.field.as_str();
            let value = assignment.value.as_str();

            match field.to_ascii_lowercase().as_str() {
                "min_angle_warn_deg" => config.min_angle_warn_deg = parse_f64(field, value)?,
                "min_angle_fail_deg" => config.min_angle_fail_deg = parse_f64(field, value)?,
                "angle_deviation_warn_deg" => {
                    config.angle_deviation_warn_deg = parse_f64(field, value)?
                }
                "aspect_ratio_warn" => config.aspect_ratio_warn = parse_f64(field, value)?,
                "aspect_ratio_fail" => config.aspect_ratio_fail = parse_f64(field, value)?,
                "cell_edge_cv_warn" => config.cell_edge_cv_warn = parse_f64(field, value)?,
                "area_cv_warn" => config.area_cv_warn = parse_f64(field, value)?,
                "max_adjacent_resolution_ratio_warn" => {
                    config.max_adjacent_resolution_ratio_warn = parse_f64(field, value)?
                }
                "worst_cells_limit" => config.worst_cells_limit = parse_i32(field, value)?,
                "repair_batch_limit" => config.repair_batch_limit = parse_i32(field, value)?,
                "on_violation" => config.on_violation = parse_canonical_string(value),
                _ => return Err(format!("unknown &quality field '{field}'")),
            }
        }

        config.validate()?;
        config.on_violation = config.on_violation.trim().to_ascii_lowercase();
        Ok(config)
    }

    fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("min_angle_warn_deg", self.min_angle_warn_deg),
            ("min_angle_fail_deg", self.min_angle_fail_deg),
            ("angle_deviation_warn_deg", self.angle_deviation_warn_deg),
            ("aspect_ratio_warn", self.aspect_ratio_warn),
            ("aspect_ratio_fail", self.aspect_ratio_fail),
            ("cell_edge_cv_warn", self.cell_edge_cv_warn),
            ("area_cv_warn", self.area_cv_warn),
            (
                "max_adjacent_resolution_ratio_warn",
                self.max_adjacent_resolution_ratio_warn,
            ),
        ] {
            if !value.is_finite() {
                return Err(format!("quality {name} must be finite"));
            }
        }
        if !(0.0..180.0).contains(&self.min_angle_warn_deg) {
            return Err("quality min_angle_warn_deg must be between 0 and 180".to_string());
        }
        if !(0.0..180.0).contains(&self.min_angle_fail_deg) {
            return Err("quality min_angle_fail_deg must be between 0 and 180".to_string());
        }
        if self.min_angle_fail_deg > self.min_angle_warn_deg {
            return Err(
                "quality min_angle_fail_deg must not exceed min_angle_warn_deg".to_string(),
            );
        }
        if !(0.0..=180.0).contains(&self.angle_deviation_warn_deg) {
            return Err("quality angle_deviation_warn_deg must be between 0 and 180".to_string());
        }
        if self.aspect_ratio_warn < 1.0 || self.aspect_ratio_fail < 1.0 {
            return Err("quality aspect-ratio thresholds must be at least 1".to_string());
        }
        if self.aspect_ratio_warn > self.aspect_ratio_fail {
            return Err("quality aspect_ratio_warn must not exceed aspect_ratio_fail".to_string());
        }
        if self.cell_edge_cv_warn < 0.0 || self.area_cv_warn < 0.0 {
            return Err("quality coefficient-of-variation thresholds must be non-negative".into());
        }
        if self.max_adjacent_resolution_ratio_warn < 1.0 {
            return Err(
                "quality max_adjacent_resolution_ratio_warn must be at least 1".to_string(),
            );
        }
        if self.worst_cells_limit < 0 {
            return Err("quality worst_cells_limit must be non-negative".to_string());
        }
        if self.repair_batch_limit < 0 {
            return Err("quality repair_batch_limit must be non-negative".to_string());
        }
        if !matches!(
            self.on_violation.trim().to_ascii_lowercase().as_str(),
            "warn" | "block" | "auto_refine"
        ) {
            return Err("quality on_violation must be warn, block, or auto_refine".to_string());
        }
        Ok(())
    }

    /// Serialize to a `&quality` block; `from_quality_namelist(&x.to_quality_namelist())`
    /// reproduces `x`.
    pub fn to_quality_namelist(&self) -> String {
        let mut out = String::new();
        out.push_str("&quality\n");
        out.push_str(&format!(
            "  NL%min_angle_warn_deg = {}\n",
            self.min_angle_warn_deg
        ));
        out.push_str(&format!(
            "  NL%min_angle_fail_deg = {}\n",
            self.min_angle_fail_deg
        ));
        out.push_str(&format!(
            "  NL%angle_deviation_warn_deg = {}\n",
            self.angle_deviation_warn_deg
        ));
        out.push_str(&format!(
            "  NL%aspect_ratio_warn = {}\n",
            self.aspect_ratio_warn
        ));
        out.push_str(&format!(
            "  NL%aspect_ratio_fail = {}\n",
            self.aspect_ratio_fail
        ));
        out.push_str(&format!(
            "  NL%cell_edge_cv_warn = {}\n",
            self.cell_edge_cv_warn
        ));
        out.push_str(&format!("  NL%area_cv_warn = {}\n", self.area_cv_warn));
        out.push_str(&format!(
            "  NL%max_adjacent_resolution_ratio_warn = {}\n",
            self.max_adjacent_resolution_ratio_warn
        ));
        out.push_str(&format!(
            "  NL%worst_cells_limit = {}\n",
            self.worst_cells_limit
        ));
        out.push_str(&format!(
            "  NL%repair_batch_limit = {}\n",
            self.repair_batch_limit
        ));
        out.push_str(&format!(
            "  NL%on_violation = {}\n",
            canonical_quote(&self.on_violation)
        ));
        out.push_str("/\n");
        out
    }
}
