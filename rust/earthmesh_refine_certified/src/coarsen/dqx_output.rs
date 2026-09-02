//! Stable DQX certificate, outcome, and export metadata contracts.

use super::{
    domain_quality_evaluation_from_atlas, DomainQualityCosts, DomainQualityVector,
    SpatialAngleAtlas,
};
use crate::certificate::{AngleContract, AngleContractId};
use earthmesh_quality::domain::QualityZone;

pub const DQX_OUTPUT_SCHEMA_VERSION: u32 = 1;

pub const DQX_NETCDF_GLOBAL_ATTRIBUTES: [&str; 8] = [
    "angle_contract_id",
    "global_angle_min_required",
    "global_angle_max_required",
    "target_preferred_angle_min",
    "target_preferred_angle_max",
    "quality_domain_kind",
    "quality_priority_field_hash",
    "quality_optimization_status",
];

pub const DQX_NETCDF_CELL_VARIABLES: [&str; 10] = [
    "cell_quality_zone",
    "cell_quality_priority_max",
    "cell_quality_priority_mean",
    "cell_distance_to_target",
    "cell_distance_to_boundary",
    "cell_is_transition",
    "cell_min_angle",
    "cell_max_angle",
    "cell_preferred_violation",
    "cell_quality_action",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityCellAction {
    None,
    Coarsened,
    QualityProtected,
    FrontShiftSource,
    FrontShiftDestination,
    VertexRelocated,
    EdgeFlipped,
    PromotionRestore,
}

impl QualityCellAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Coarsened => "coarsened",
            Self::QualityProtected => "quality_protected",
            Self::FrontShiftSource => "front_shift_source",
            Self::FrontShiftDestination => "front_shift_destination",
            Self::VertexRelocated => "vertex_relocated",
            Self::EdgeFlipped => "edge_flipped",
            Self::PromotionRestore => "promotion_restore",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityOptimizationStatus {
    NotRequested,
    PreferredWindowSatisfied,
    Improved,
    BudgetLimited,
    TopologyLimited,
    ExportCapacityInsufficient,
    NoImprovingCandidate,
}

impl QualityOptimizationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotRequested => "not_requested",
            Self::PreferredWindowSatisfied => "preferred_window_satisfied",
            Self::Improved => "improved",
            Self::BudgetLimited => "budget_limited",
            Self::TopologyLimited => "topology_limited",
            Self::ExportCapacityInsufficient => "export_capacity_insufficient",
            Self::NoImprovingCandidate => "no_improving_candidate",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpatialQualityFulfillmentReport {
    pub contract_id: AngleContractId,
    pub domain_kind: String,
    pub global_min_angle: f64,
    pub global_max_angle: f64,
    pub global_hard_violations: usize,
    pub target_min_angle: Option<f64>,
    pub target_max_angle: Option<f64>,
    pub target_preferred_violations: usize,
    pub target_fraction_in_40_80: f64,
    pub target_fraction_in_42_78: f64,
    pub target_equilateral_rmse: f64,
    pub boundary_preferred_violations: usize,
    pub export_preferred_violations: usize,
    pub deep_exterior_preferred_violations: usize,
    pub worst_angle_distance_to_target: Option<f64>,
    pub transition_faces_in_target: usize,
    pub transition_faces_in_boundary: usize,
    pub extra_cells_for_quality: usize,
    pub moves_committed: usize,
    pub flips_committed: usize,
    pub front_shifts_committed: usize,
    pub coarsenings_rejected_for_quality: usize,
    pub status: QualityOptimizationStatus,
}

impl SpatialQualityFulfillmentReport {
    pub fn validate(&self) -> Result<(), String> {
        if self.contract_id != AngleContractId::DomainQuality38To82V1 {
            return Err("DQX fulfillment requires the DQX angle contract".into());
        }
        if !matches!(
            self.domain_kind.as_str(),
            "global" | "region" | "watershed" | "land" | "ocean" | "custom_mask"
        ) {
            return Err("DQX fulfillment has an unknown quality domain kind".into());
        }
        let hard = AngleContract::for_id(self.contract_id).final_delivery;
        if !self.global_min_angle.is_finite()
            || !self.global_max_angle.is_finite()
            || self.global_min_angle > self.global_max_angle
            || !hard.contains_range(self.global_min_angle, self.global_max_angle)
            || self.global_hard_violations != 0
        {
            return Err("DQX fulfillment does not carry a passing global hard certificate".into());
        }
        match (self.target_min_angle, self.target_max_angle) {
            (Some(minimum), Some(maximum))
                if minimum.is_finite()
                    && maximum.is_finite()
                    && self.global_min_angle <= minimum
                    && minimum <= maximum
                    && maximum <= self.global_max_angle => {}
            (None, None) => {}
            _ => return Err("DQX fulfillment target angle range is invalid".into()),
        }
        if ![self.target_fraction_in_40_80, self.target_fraction_in_42_78]
            .into_iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(&value))
            || !self.target_equilateral_rmse.is_finite()
            || self.target_equilateral_rmse < 0.0
            || self
                .worst_angle_distance_to_target
                .is_some_and(|value| !value.is_finite() || value < 0.0)
        {
            return Err("DQX fulfillment contains invalid quality metrics".into());
        }
        if self.status == QualityOptimizationStatus::PreferredWindowSatisfied
            && (self.target_preferred_violations != 0 || self.boundary_preferred_violations != 0)
        {
            return Err("DQX preferred-window status contradicts the quality metrics".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DqxRunEvidence {
    pub taskbook_id: String,
    pub taskbook_sha: String,
    pub angle_contract: AngleContractId,
    pub domain_key: String,
    pub priority_field_hash: String,
    pub orientation_candidates: usize,
    pub selected_orientation_key: String,
    pub initial_quality: DomainQualityVector,
    pub final_quality: DomainQualityVector,
    pub components_considered: usize,
    pub components_committed: usize,
    pub components_rejected_for_global_hard: usize,
    pub components_rejected_for_target_quality: usize,
    pub front_shifts_attempted: usize,
    pub front_shifts_committed: usize,
    pub moves_attempted: usize,
    pub moves_committed: usize,
    pub flips_attempted: usize,
    pub flips_committed: usize,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub full_scans: usize,
    pub work_units: u64,
    pub final_status: QualityOptimizationStatus,
}

impl DqxRunEvidence {
    pub fn validate(&self) -> Result<(), String> {
        if self.taskbook_id.is_empty() || !is_sha256(&self.taskbook_sha) {
            return Err("DQX evidence has invalid taskbook identity".into());
        }
        if self.angle_contract != AngleContractId::DomainQuality38To82V1 {
            return Err("DQX evidence requires the DQX angle contract".into());
        }
        if !is_sha256(&self.domain_key) || !is_sha256(&self.priority_field_hash) {
            return Err("DQX evidence has an invalid domain or priority-field hash".into());
        }
        let accounted_components = self
            .components_committed
            .checked_add(self.components_rejected_for_global_hard)
            .and_then(|value| value.checked_add(self.components_rejected_for_target_quality))
            .ok_or_else(|| "DQX evidence component telemetry overflow".to_string())?;
        if self.selected_orientation_key.is_empty()
            || accounted_components > self.components_considered
            || self.front_shifts_committed > self.front_shifts_attempted
            || self.moves_committed > self.moves_attempted
            || self.flips_committed > self.flips_attempted
        {
            return Err("DQX evidence has inconsistent action telemetry".into());
        }
        if self.final_quality.global_hard_violation_count != 0
            || self.final_quality.global_hard_max_violation_microdeg != 0
            || self.final_quality.requirement_residuals != 0
            || self.final_quality.topology_residuals != 0
            || self.final_quality.dual_residuals != 0
            || self.final_quality.remap_residuals != 0
        {
            return Err("DQX evidence does not carry publishable final quality".into());
        }
        if !quality_vector_is_valid(self.initial_quality)
            || !quality_vector_is_valid(self.final_quality)
        {
            return Err("DQX evidence contains an invalid quality vector".into());
        }
        if self.final_status == QualityOptimizationStatus::PreferredWindowSatisfied
            && (self.final_quality.target_preferred_violation_count != 0
                || self.final_quality.boundary_preferred_violation_count != 0)
        {
            return Err("DQX evidence final status contradicts final quality".into());
        }
        Ok(())
    }
}

pub fn build_spatial_quality_fulfillment_report(
    atlas: &SpatialAngleAtlas,
    domain_kind: impl Into<String>,
    evidence: &DqxRunEvidence,
    extra_cells_for_quality: usize,
) -> Result<SpatialQualityFulfillmentReport, String> {
    evidence.validate()?;
    if atlas.contract_id != evidence.angle_contract
        || atlas.global.angle_count != atlas.witnesses.len()
        || !atlas.global.angle_count.is_multiple_of(3)
    {
        return Err("DQX fulfillment atlas does not match the run evidence".into());
    }
    let final_quality = evidence.final_quality;
    let recomputed = domain_quality_evaluation_from_atlas(
        atlas,
        DomainQualityCosts {
            requirement_residuals: final_quality.requirement_residuals,
            topology_residuals: final_quality.topology_residuals,
            dual_residuals: final_quality.dual_residuals,
            remap_residuals: final_quality.remap_residuals,
            geometry_move_scaled: final_quality.geometry_move_scaled,
            topology_change_count: final_quality.topology_change_count,
            work_units: final_quality.work_units,
        },
    )?;
    if recomputed.vector != final_quality {
        return Err("DQX fulfillment atlas and final quality vector disagree".into());
    }
    let target_angles = atlas
        .witnesses
        .iter()
        .filter(|witness| witness.zone == QualityZone::TargetCore)
        .collect::<Vec<_>>();
    if target_angles.len() != atlas.target.angle_count {
        return Err("DQX fulfillment target atlas count is inconsistent".into());
    }
    let target_fraction_in_40_80 = fraction(
        atlas
            .target
            .angle_count
            .saturating_sub(atlas.target.preferred_violation_count),
        atlas.target.angle_count,
    );
    let target_fraction_in_42_78 = fraction(
        target_angles
            .iter()
            .filter(|witness| (42.0..=78.0).contains(&witness.angle_degrees))
            .count(),
        target_angles.len(),
    );
    let target_equilateral_rmse = if target_angles.is_empty() {
        0.0
    } else {
        (target_angles
            .iter()
            .map(|witness| (witness.angle_degrees - 60.0).powi(2))
            .sum::<f64>()
            / target_angles.len() as f64)
            .sqrt()
    };
    let report = SpatialQualityFulfillmentReport {
        contract_id: atlas.contract_id,
        domain_kind: domain_kind.into(),
        global_min_angle: atlas
            .global
            .minimum_angle_degrees
            .ok_or_else(|| "DQX fulfillment atlas has no global minimum angle".to_string())?,
        global_max_angle: atlas
            .global
            .maximum_angle_degrees
            .ok_or_else(|| "DQX fulfillment atlas has no global maximum angle".to_string())?,
        global_hard_violations: atlas.global.global_hard_violation_count,
        target_min_angle: atlas.target.minimum_angle_degrees,
        target_max_angle: atlas.target.maximum_angle_degrees,
        target_preferred_violations: atlas.target.preferred_violation_count,
        target_fraction_in_40_80,
        target_fraction_in_42_78,
        target_equilateral_rmse,
        boundary_preferred_violations: atlas.boundary.preferred_violation_count,
        export_preferred_violations: atlas.export.preferred_violation_count,
        deep_exterior_preferred_violations: atlas.deep_exterior.preferred_violation_count,
        worst_angle_distance_to_target: atlas.worst_angle_distance_to_target,
        transition_faces_in_target: atlas.target.transition_face_count,
        transition_faces_in_boundary: atlas.boundary.transition_face_count,
        extra_cells_for_quality,
        moves_committed: evidence.moves_committed,
        flips_committed: evidence.flips_committed,
        front_shifts_committed: evidence.front_shifts_committed,
        coarsenings_rejected_for_quality: evidence.components_rejected_for_target_quality,
        status: evidence.final_status,
    };
    report.validate()?;
    Ok(report)
}

pub fn spatial_quality_fulfillment_report_json(
    report: &SpatialQualityFulfillmentReport,
) -> Result<String, String> {
    report.validate()?;
    Ok(format!(
        "{{\"schema_version\":{},\"contract_id\":{},\"domain_kind\":{},\"global_min_angle\":{},\"global_max_angle\":{},\"global_hard_violations\":{},\"target_min_angle\":{},\"target_max_angle\":{},\"target_preferred_violations\":{},\"target_fraction_in_40_80\":{},\"target_fraction_in_42_78\":{},\"target_equilateral_rmse\":{},\"boundary_preferred_violations\":{},\"export_preferred_violations\":{},\"deep_exterior_preferred_violations\":{},\"worst_angle_distance_to_target\":{},\"transition_faces_in_target\":{},\"transition_faces_in_boundary\":{},\"extra_cells_for_quality\":{},\"moves_committed\":{},\"flips_committed\":{},\"front_shifts_committed\":{},\"coarsenings_rejected_for_quality\":{},\"status\":{}}}",
        DQX_OUTPUT_SCHEMA_VERSION,
        json_string(report.contract_id.as_str()),
        json_string(&report.domain_kind),
        report.global_min_angle,
        report.global_max_angle,
        report.global_hard_violations,
        option_f64_json(report.target_min_angle),
        option_f64_json(report.target_max_angle),
        report.target_preferred_violations,
        report.target_fraction_in_40_80,
        report.target_fraction_in_42_78,
        report.target_equilateral_rmse,
        report.boundary_preferred_violations,
        report.export_preferred_violations,
        report.deep_exterior_preferred_violations,
        option_f64_json(report.worst_angle_distance_to_target),
        report.transition_faces_in_target,
        report.transition_faces_in_boundary,
        report.extra_cells_for_quality,
        report.moves_committed,
        report.flips_committed,
        report.front_shifts_committed,
        report.coarsenings_rejected_for_quality,
        json_string(report.status.as_str()),
    ))
}

pub fn dqx_run_evidence_json(evidence: &DqxRunEvidence) -> Result<String, String> {
    evidence.validate()?;
    Ok(format!(
        "{{\"schema_version\":{},\"taskbook_id\":{},\"taskbook_sha\":{},\"angle_contract\":{},\"domain_key\":{},\"priority_field_hash\":{},\"orientation_candidates\":{},\"selected_orientation_key\":{},\"initial_quality\":{},\"final_quality\":{},\"components_considered\":{},\"components_committed\":{},\"components_rejected_for_global_hard\":{},\"components_rejected_for_target_quality\":{},\"front_shifts_attempted\":{},\"front_shifts_committed\":{},\"moves_attempted\":{},\"moves_committed\":{},\"flips_attempted\":{},\"flips_committed\":{},\"cache_hits\":{},\"cache_misses\":{},\"full_scans\":{},\"work_units\":{},\"final_status\":{}}}",
        DQX_OUTPUT_SCHEMA_VERSION,
        json_string(&evidence.taskbook_id),
        json_string(&evidence.taskbook_sha),
        json_string(evidence.angle_contract.as_str()),
        json_string(&evidence.domain_key),
        json_string(&evidence.priority_field_hash),
        evidence.orientation_candidates,
        json_string(&evidence.selected_orientation_key),
        domain_quality_vector_json(evidence.initial_quality),
        domain_quality_vector_json(evidence.final_quality),
        evidence.components_considered,
        evidence.components_committed,
        evidence.components_rejected_for_global_hard,
        evidence.components_rejected_for_target_quality,
        evidence.front_shifts_attempted,
        evidence.front_shifts_committed,
        evidence.moves_attempted,
        evidence.moves_committed,
        evidence.flips_attempted,
        evidence.flips_committed,
        evidence.cache_hits,
        evidence.cache_misses,
        evidence.full_scans,
        evidence.work_units,
        json_string(evidence.final_status.as_str()),
    ))
}

fn domain_quality_vector_json(vector: DomainQualityVector) -> String {
    format!(
        "{{\"global_hard_violation_count\":{},\"global_hard_max_violation_microdeg\":{},\"requirement_residuals\":{},\"topology_residuals\":{},\"dual_residuals\":{},\"remap_residuals\":{},\"target_preferred_violation_count\":{},\"target_worst_preferred_violation_microdeg\":{},\"target_preferred_l2_scaled\":{},\"target_equilateral_rmse_scaled\":{},\"boundary_preferred_violation_count\":{},\"boundary_worst_preferred_violation_microdeg\":{},\"boundary_preferred_l2_scaled\":{},\"transition_faces_in_target\":{},\"transition_faces_in_boundary\":{},\"export_near_boundary_penalty_scaled\":{},\"final_cell_count\":{},\"global_preferred_l2_scaled\":{},\"geometry_move_scaled\":{},\"topology_change_count\":{},\"work_units\":{}}}",
        vector.global_hard_violation_count,
        vector.global_hard_max_violation_microdeg,
        vector.requirement_residuals,
        vector.topology_residuals,
        vector.dual_residuals,
        vector.remap_residuals,
        vector.target_preferred_violation_count,
        vector.target_worst_preferred_violation_microdeg,
        vector.target_preferred_l2_scaled,
        vector.target_equilateral_rmse_scaled,
        vector.boundary_preferred_violation_count,
        vector.boundary_worst_preferred_violation_microdeg,
        vector.boundary_preferred_l2_scaled,
        vector.transition_faces_in_target,
        vector.transition_faces_in_boundary,
        vector.export_near_boundary_penalty_scaled,
        vector.final_cell_count,
        vector.global_preferred_l2_scaled,
        vector.geometry_move_scaled,
        vector.topology_change_count,
        vector.work_units,
    )
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn quality_vector_is_valid(vector: DomainQualityVector) -> bool {
    [
        vector.global_hard_max_violation_microdeg,
        vector.target_worst_preferred_violation_microdeg,
        vector.target_preferred_l2_scaled,
        vector.target_equilateral_rmse_scaled,
        vector.boundary_worst_preferred_violation_microdeg,
        vector.boundary_preferred_l2_scaled,
        vector.export_near_boundary_penalty_scaled,
        vector.global_preferred_l2_scaled,
        vector.geometry_move_scaled,
    ]
    .into_iter()
    .all(|value| value >= 0)
}

fn fraction(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn option_f64_json(value: Option<f64>) -> String {
    value.map_or_else(|| "null".into(), |value| value.to_string())
}

fn json_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character < ' ' => {
                use std::fmt::Write as _;
                write!(output, "\\u{:04x}", character as u32).expect("write to String");
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coarsen::{
        EdgeClass, SpatialAngleWitness, SpatialAtlasConclusion, SpatialZoneAngleMetrics,
    };

    fn report() -> SpatialQualityFulfillmentReport {
        SpatialQualityFulfillmentReport {
            contract_id: AngleContractId::DomainQuality38To82V1,
            domain_kind: "region".into(),
            global_min_angle: 38.5,
            global_max_angle: 81.5,
            global_hard_violations: 0,
            target_min_angle: Some(40.5),
            target_max_angle: Some(79.5),
            target_preferred_violations: 0,
            target_fraction_in_40_80: 1.0,
            target_fraction_in_42_78: 0.75,
            target_equilateral_rmse: 4.25,
            boundary_preferred_violations: 0,
            export_preferred_violations: 2,
            deep_exterior_preferred_violations: 1,
            worst_angle_distance_to_target: Some(3.0),
            transition_faces_in_target: 4,
            transition_faces_in_boundary: 6,
            extra_cells_for_quality: 8,
            moves_committed: 2,
            flips_committed: 1,
            front_shifts_committed: 3,
            coarsenings_rejected_for_quality: 5,
            status: QualityOptimizationStatus::PreferredWindowSatisfied,
        }
    }

    fn evidence() -> DqxRunEvidence {
        DqxRunEvidence {
            taskbook_id: "CMRC-DQX-2026-09-03-R1".into(),
            taskbook_sha: "a".repeat(64),
            angle_contract: AngleContractId::DomainQuality38To82V1,
            domain_key: "b".repeat(64),
            priority_field_hash: "c".repeat(64),
            orientation_candidates: 12,
            selected_orientation_key: "orientation-03".into(),
            initial_quality: DomainQualityVector {
                target_preferred_violation_count: 4,
                final_cell_count: 100,
                work_units: 10,
                ..DomainQualityVector::default()
            },
            final_quality: DomainQualityVector {
                final_cell_count: 104,
                work_units: 20,
                ..DomainQualityVector::default()
            },
            components_considered: 8,
            components_committed: 3,
            components_rejected_for_global_hard: 1,
            components_rejected_for_target_quality: 2,
            front_shifts_attempted: 4,
            front_shifts_committed: 2,
            moves_attempted: 6,
            moves_committed: 3,
            flips_attempted: 5,
            flips_committed: 1,
            cache_hits: 9,
            cache_misses: 2,
            full_scans: 1,
            work_units: 20,
            final_status: QualityOptimizationStatus::Improved,
        }
    }

    fn target_witness(corner: usize, angle_degrees: f64) -> SpatialAngleWitness {
        SpatialAngleWitness {
            face: 0,
            corner,
            angle_degrees,
            global_hard_violation: 0.0,
            preferred_violation: 0.0,
            zone: QualityZone::TargetCore,
            maximum_priority: 1.0,
            distance_to_target: 0.0,
            distance_to_boundary: 1.0,
            is_transition_face: true,
            transition_owner: Some(7),
            component_id: None,
            hierarchy_address: None,
            movable_vertex_count: 3,
            fixed_vertex_count: 0,
            distance_to_pentagon_anchor: None,
            distance_to_seam: None,
            edge_classes: [EdgeClass::CoarseInterface; 3],
        }
    }

    #[test]
    fn output_schema_names_and_actions_are_frozen() {
        assert_eq!(
            DQX_NETCDF_GLOBAL_ATTRIBUTES,
            [
                "angle_contract_id",
                "global_angle_min_required",
                "global_angle_max_required",
                "target_preferred_angle_min",
                "target_preferred_angle_max",
                "quality_domain_kind",
                "quality_priority_field_hash",
                "quality_optimization_status",
            ]
        );
        assert_eq!(
            [
                QualityCellAction::None,
                QualityCellAction::Coarsened,
                QualityCellAction::QualityProtected,
                QualityCellAction::FrontShiftSource,
                QualityCellAction::FrontShiftDestination,
                QualityCellAction::VertexRelocated,
                QualityCellAction::EdgeFlipped,
                QualityCellAction::PromotionRestore,
            ]
            .map(QualityCellAction::as_str),
            [
                "none",
                "coarsened",
                "quality_protected",
                "front_shift_source",
                "front_shift_destination",
                "vertex_relocated",
                "edge_flipped",
                "promotion_restore",
            ]
        );
    }

    #[test]
    fn fulfillment_json_is_deterministic_and_fail_closed() {
        let report = report();
        let json = spatial_quality_fulfillment_report_json(&report).unwrap();
        assert_eq!(
            json,
            spatial_quality_fulfillment_report_json(&report).unwrap()
        );
        assert_eq!(json, "{\"schema_version\":1,\"contract_id\":\"domain_quality_38_to_82_v1\",\"domain_kind\":\"region\",\"global_min_angle\":38.5,\"global_max_angle\":81.5,\"global_hard_violations\":0,\"target_min_angle\":40.5,\"target_max_angle\":79.5,\"target_preferred_violations\":0,\"target_fraction_in_40_80\":1,\"target_fraction_in_42_78\":0.75,\"target_equilateral_rmse\":4.25,\"boundary_preferred_violations\":0,\"export_preferred_violations\":2,\"deep_exterior_preferred_violations\":1,\"worst_angle_distance_to_target\":3,\"transition_faces_in_target\":4,\"transition_faces_in_boundary\":6,\"extra_cells_for_quality\":8,\"moves_committed\":2,\"flips_committed\":1,\"front_shifts_committed\":3,\"coarsenings_rejected_for_quality\":5,\"status\":\"preferred_window_satisfied\"}");

        let mut invalid = report;
        invalid.global_hard_violations = 1;
        assert!(spatial_quality_fulfillment_report_json(&invalid).is_err());
    }

    #[test]
    fn run_evidence_json_is_deterministic_and_rejects_unpublishable_quality() {
        let valid = evidence();
        let json = dqx_run_evidence_json(&valid).unwrap();
        assert_eq!(json, dqx_run_evidence_json(&valid).unwrap());
        assert!(json.contains("\"initial_quality\":{\"global_hard_violation_count\":0"));
        assert!(json.contains("\"final_status\":\"improved\""));

        let mut invalid = valid;
        invalid.final_quality.topology_residuals = 1;
        assert!(dqx_run_evidence_json(&invalid).is_err());

        let mut invalid = evidence();
        invalid.components_considered = 1;
        invalid.components_committed = 1;
        invalid.components_rejected_for_global_hard = 1;
        invalid.components_rejected_for_target_quality = 0;
        assert!(dqx_run_evidence_json(&invalid).is_err());

        let mut invalid = evidence();
        invalid.final_status = QualityOptimizationStatus::PreferredWindowSatisfied;
        invalid.final_quality.target_preferred_violation_count = 1;
        assert!(dqx_run_evidence_json(&invalid).is_err());
    }

    #[test]
    fn fulfillment_is_built_from_the_final_atlas_and_evidence() {
        let target = SpatialZoneAngleMetrics {
            angle_count: 3,
            minimum_angle_degrees: Some(40.5),
            maximum_angle_degrees: Some(79.5),
            transition_face_count: 1,
            ..SpatialZoneAngleMetrics::default()
        };
        let atlas = SpatialAngleAtlas {
            contract_id: AngleContractId::DomainQuality38To82V1,
            global: target,
            target,
            boundary: SpatialZoneAngleMetrics::default(),
            export: SpatialZoneAngleMetrics::default(),
            deep_exterior: SpatialZoneAngleMetrics::default(),
            global_neutral: SpatialZoneAngleMetrics::default(),
            worst_angle_distance_to_target: Some(0.0),
            bad_angle_component_count: 0,
            conclusion: SpatialAtlasConclusion::NoGlobalTopologySearchRequired,
            witnesses: [40.5, 60.0, 79.5]
                .into_iter()
                .enumerate()
                .map(|(corner, angle)| target_witness(corner, angle))
                .collect(),
        };
        let mut evidence = evidence();
        evidence.final_quality =
            domain_quality_evaluation_from_atlas(&atlas, DomainQualityCosts::default())
                .unwrap()
                .vector;
        evidence.final_status = QualityOptimizationStatus::PreferredWindowSatisfied;

        let report =
            build_spatial_quality_fulfillment_report(&atlas, "region", &evidence, 4).unwrap();
        assert_eq!(report.target_fraction_in_40_80, 1.0);
        assert_eq!(report.target_fraction_in_42_78, 1.0 / 3.0);
        assert_eq!(report.transition_faces_in_target, 1);
        assert_eq!(report.extra_cells_for_quality, 4);
        assert!(report.target_equilateral_rmse > 15.0);

        evidence.final_quality.target_equilateral_rmse_scaled += 1;
        assert!(build_spatial_quality_fulfillment_report(&atlas, "region", &evidence, 4).is_err());
    }
}
