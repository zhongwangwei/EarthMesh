use crate::{fortran_quote, parse_f64, parse_fortran_string, parse_i32, strip_fortran_comment};

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
    pub aspect_ratio_warn: f64,
    pub aspect_ratio_fail: f64,
    pub area_cv_warn: f64,
    pub max_adjacent_resolution_ratio_warn: f64,
    pub worst_cells_limit: i32,
    /// "warn" (report only) or "block" (a Fail verdict aborts the run).
    pub on_violation: String,
}

impl Default for QualityNamelist {
    fn default() -> Self {
        // Keep in lock-step with earthmesh_quality::QualityThresholds::default().
        Self {
            min_angle_warn_deg: 20.0,
            min_angle_fail_deg: 5.0,
            aspect_ratio_warn: 4.0,
            aspect_ratio_fail: 10.0,
            area_cv_warn: 1.5,
            max_adjacent_resolution_ratio_warn: 2.0,
            worst_cells_limit: 50,
            on_violation: String::from("warn"),
        }
    }
}

impl QualityNamelist {
    /// Parse a `&quality` block. Lenient like the `&mkgrd` parser: unknown keys
    /// are ignored and missing keys keep their default, so old namelists (no
    /// `&quality` block) yield `default()`.
    pub fn from_quality_namelist(input: &str) -> Result<Self, String> {
        let mut config = Self::default();
        let mut in_block = false;

        for raw_line in input.lines() {
            let line = strip_fortran_comment(raw_line).trim().trim_end_matches(',');
            if line.is_empty() {
                continue;
            }
            if line.starts_with('&') {
                in_block = line.eq_ignore_ascii_case("&quality");
                continue;
            }
            if line == "/" {
                in_block = false;
                continue;
            }
            if !in_block {
                continue;
            }

            let Some((left, right)) = line.split_once('=') else {
                continue;
            };
            let Some(field) = left.trim().split_once('%').map(|(_, field)| field.trim()) else {
                continue;
            };
            let value = right.trim().trim_end_matches(',');

            match field.to_ascii_lowercase().as_str() {
                "min_angle_warn_deg" => config.min_angle_warn_deg = parse_f64(field, value)?,
                "min_angle_fail_deg" => config.min_angle_fail_deg = parse_f64(field, value)?,
                "aspect_ratio_warn" => config.aspect_ratio_warn = parse_f64(field, value)?,
                "aspect_ratio_fail" => config.aspect_ratio_fail = parse_f64(field, value)?,
                "area_cv_warn" => config.area_cv_warn = parse_f64(field, value)?,
                "max_adjacent_resolution_ratio_warn" => {
                    config.max_adjacent_resolution_ratio_warn = parse_f64(field, value)?
                }
                "worst_cells_limit" => config.worst_cells_limit = parse_i32(field, value)?,
                "on_violation" => config.on_violation = parse_fortran_string(value),
                _ => {}
            }
        }

        Ok(config)
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
            "  NL%aspect_ratio_warn = {}\n",
            self.aspect_ratio_warn
        ));
        out.push_str(&format!(
            "  NL%aspect_ratio_fail = {}\n",
            self.aspect_ratio_fail
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
            "  NL%on_violation = {}\n",
            fortran_quote(&self.on_violation)
        ));
        out.push_str("/\n");
        out
    }
}
