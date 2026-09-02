//! Integer-only DQX candidate scoring and atomic acceptance.

use super::{SpatialAngleAtlas, SpatialAngleWitness};
use crate::certificate::{AngleContract, AngleContractId};
use earthmesh_quality::domain::QualityZone;
use std::collections::BTreeMap;

pub const DOMAIN_QUALITY_SCALE: i64 = 1_000_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct DomainQualityVector {
    pub global_hard_violation_count: usize,
    pub global_hard_max_violation_microdeg: i64,
    pub requirement_residuals: usize,
    pub topology_residuals: usize,
    pub dual_residuals: usize,
    pub remap_residuals: usize,
    pub target_preferred_violation_count: usize,
    pub target_worst_preferred_violation_microdeg: i64,
    pub target_preferred_l2_scaled: i64,
    pub target_equilateral_rmse_scaled: i64,
    pub boundary_preferred_violation_count: usize,
    pub boundary_worst_preferred_violation_microdeg: i64,
    pub boundary_preferred_l2_scaled: i64,
    pub transition_faces_in_target: usize,
    pub transition_faces_in_boundary: usize,
    pub export_near_boundary_penalty_scaled: i64,
    pub final_cell_count: usize,
    pub global_preferred_l2_scaled: i64,
    pub geometry_move_scaled: i64,
    pub topology_change_count: usize,
    pub work_units: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DomainQualityCosts {
    pub requirement_residuals: usize,
    pub topology_residuals: usize,
    pub dual_residuals: usize,
    pub remap_residuals: usize,
    pub geometry_move_scaled: i64,
    pub topology_change_count: usize,
    pub work_units: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DomainQualityDamageMetrics {
    pub external_preferred_violation_count: usize,
    pub external_preferred_l2_scaled: i64,
    pub external_minimum_hard_margin_microdeg: Option<i64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DomainQualityEvaluation {
    pub vector: DomainQualityVector,
    pub damage: DomainQualityDamageMetrics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportDamageGuard {
    pub maximum_new_external_preferred_violations: usize,
    pub maximum_external_l2_increase_per_target_l2_decrease_millionths: i64,
    pub minimum_external_hard_margin_microdeg: i64,
}

impl Default for ExportDamageGuard {
    fn default() -> Self {
        Self {
            maximum_new_external_preferred_violations: usize::MAX,
            maximum_external_l2_increase_per_target_l2_decrease_millionths: DOMAIN_QUALITY_SCALE,
            minimum_external_hard_margin_microdeg: 200_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportDamageRejectReason {
    NewPreferredViolations,
    PreferredL2Increase,
    InternalHardMargin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainQualityRejectReason {
    HardCertificateFailed,
    InvalidScore,
    GlobalHardViolation,
    RequirementResidual,
    TopologyResidual,
    DualResidual,
    RemapResidual,
    ExportDamage(ExportDamageRejectReason),
    NotStrictImprovement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainQualityAcceptanceReport {
    pub before: DomainQualityEvaluation,
    pub after: DomainQualityEvaluation,
    pub accepted: bool,
    pub rejection: Option<DomainQualityRejectReason>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TransitionZoneIntrusion {
    pub transition_faces: usize,
    pub preferred_violation_count: usize,
    pub worst_preferred_violation_microdeg: i64,
    pub preferred_l2_scaled: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionIntrusionReport {
    pub target_before: TransitionZoneIntrusion,
    pub target_after: TransitionZoneIntrusion,
    pub boundary_before: TransitionZoneIntrusion,
    pub boundary_after: TransitionZoneIntrusion,
    pub accepted: bool,
    pub rejection: Option<DomainQualityRejectReason>,
}

pub fn transition_intrusion_report(
    before_atlas: &SpatialAngleAtlas,
    after_atlas: &SpatialAngleAtlas,
    acceptance: DomainQualityAcceptanceReport,
) -> Result<TransitionIntrusionReport, String> {
    let (target_before, boundary_before) = transition_intrusions(before_atlas)?;
    let (target_after, boundary_after) = transition_intrusions(after_atlas)?;
    if target_before.transition_faces != acceptance.before.vector.transition_faces_in_target
        || boundary_before.transition_faces != acceptance.before.vector.transition_faces_in_boundary
        || target_after.transition_faces != acceptance.after.vector.transition_faces_in_target
        || boundary_after.transition_faces != acceptance.after.vector.transition_faces_in_boundary
    {
        return Err("transition intrusion report does not match quality evaluation".into());
    }
    Ok(TransitionIntrusionReport {
        target_before,
        target_after,
        boundary_before,
        boundary_after,
        accepted: acceptance.accepted,
        rejection: acceptance.rejection,
    })
}

pub fn transition_intrusion_report_json(report: &TransitionIntrusionReport) -> String {
    format!(
        "{{\"schema_version\":1,\"target_before\":{},\"target_after\":{},\"boundary_before\":{},\"boundary_after\":{},\"accepted\":{},\"rejection\":{}}}",
        transition_zone_json(report.target_before),
        transition_zone_json(report.target_after),
        transition_zone_json(report.boundary_before),
        transition_zone_json(report.boundary_after),
        report.accepted,
        report.rejection.map_or_else(
            || "null".to_string(),
            |reason| format!("\"{}\"", domain_quality_reject_reason(reason))
        ),
    )
}

fn transition_intrusions(
    atlas: &SpatialAngleAtlas,
) -> Result<(TransitionZoneIntrusion, TransitionZoneIntrusion), String> {
    if atlas.contract_id != AngleContractId::DomainQuality38To82V1 {
        return Err("transition intrusion report requires the DQX angle contract".into());
    }
    let mut faces = BTreeMap::new();
    let mut target = TransitionZoneIntrusion::default();
    let mut boundary = TransitionZoneIntrusion::default();
    for witness in &atlas.witnesses {
        if witness.is_transition_face != witness.transition_owner.is_some() {
            return Err(format!(
                "transition intrusion witness for face {} has inconsistent ownership",
                witness.face
            ));
        }
        if !witness.is_transition_face {
            continue;
        }
        let owner = witness
            .transition_owner
            .expect("validated transition owner");
        if let Some(&(existing_zone, existing_owner)) = faces.get(&witness.face) {
            if existing_zone != witness.zone || existing_owner != owner {
                return Err(format!(
                    "transition intrusion face {} has inconsistent context",
                    witness.face
                ));
            }
        } else {
            faces.insert(witness.face, (witness.zone, owner));
        }
        let metrics = match witness.zone {
            QualityZone::TargetCore => &mut target,
            QualityZone::BoundaryProtection => &mut boundary,
            QualityZone::ExportCorridor
            | QualityZone::DeepExterior
            | QualityZone::GlobalNeutral => continue,
        };
        if witness.preferred_violation > 0.0 {
            let violation = scale_nonnegative(
                witness.preferred_violation,
                "transition preferred violation",
            )?;
            metrics.preferred_violation_count = metrics
                .preferred_violation_count
                .checked_add(1)
                .ok_or_else(|| "transition preferred count overflow".to_string())?;
            metrics.worst_preferred_violation_microdeg =
                metrics.worst_preferred_violation_microdeg.max(violation);
            metrics.preferred_l2_scaled = metrics
                .preferred_l2_scaled
                .checked_add(squared_scaled(violation)?)
                .ok_or_else(|| "transition preferred score overflow".to_string())?;
        }
    }
    target.transition_faces = faces
        .values()
        .filter(|&&(zone, _)| zone == QualityZone::TargetCore)
        .count();
    boundary.transition_faces = faces
        .values()
        .filter(|&&(zone, _)| zone == QualityZone::BoundaryProtection)
        .count();
    Ok((target, boundary))
}

fn transition_zone_json(zone: TransitionZoneIntrusion) -> String {
    format!(
        "{{\"transition_faces\":{},\"preferred_violation_count\":{},\"worst_preferred_violation_microdeg\":{},\"preferred_l2_scaled\":{}}}",
        zone.transition_faces,
        zone.preferred_violation_count,
        zone.worst_preferred_violation_microdeg,
        zone.preferred_l2_scaled,
    )
}

fn domain_quality_reject_reason(reason: DomainQualityRejectReason) -> &'static str {
    match reason {
        DomainQualityRejectReason::HardCertificateFailed => "hard_certificate_failed",
        DomainQualityRejectReason::InvalidScore => "invalid_score",
        DomainQualityRejectReason::GlobalHardViolation => "global_hard_violation",
        DomainQualityRejectReason::RequirementResidual => "requirement_residual",
        DomainQualityRejectReason::TopologyResidual => "topology_residual",
        DomainQualityRejectReason::DualResidual => "dual_residual",
        DomainQualityRejectReason::RemapResidual => "remap_residual",
        DomainQualityRejectReason::ExportDamage(
            ExportDamageRejectReason::NewPreferredViolations,
        ) => "export_damage_new_preferred_violations",
        DomainQualityRejectReason::ExportDamage(ExportDamageRejectReason::PreferredL2Increase) => {
            "export_damage_preferred_l2_increase"
        }
        DomainQualityRejectReason::ExportDamage(ExportDamageRejectReason::InternalHardMargin) => {
            "export_damage_internal_hard_margin"
        }
        DomainQualityRejectReason::NotStrictImprovement => "not_strict_improvement",
    }
}

pub fn domain_quality_evaluation_from_atlas(
    atlas: &SpatialAngleAtlas,
    costs: DomainQualityCosts,
) -> Result<DomainQualityEvaluation, String> {
    if atlas.contract_id != AngleContractId::DomainQuality38To82V1 {
        return Err("domain quality scoring requires the DQX angle contract".into());
    }
    if atlas.global.angle_count != atlas.witnesses.len()
        || !atlas.global.angle_count.is_multiple_of(3)
    {
        return Err("domain quality atlas angle count is inconsistent".into());
    }
    if costs.geometry_move_scaled < 0 {
        return Err("domain quality geometry move must be non-negative".into());
    }
    let mut accumulator = DomainQualityAccumulator::default();
    for witness in &atlas.witnesses {
        accumulator.add(witness)?;
    }
    accumulator.evaluation(
        costs,
        atlas.global.angle_count / 3,
        atlas.target.transition_face_count,
        atlas.boundary.transition_face_count,
    )
}

pub fn evaluate_domain_quality_candidate(
    before: DomainQualityEvaluation,
    after: DomainQualityEvaluation,
    hard_certificate_passed: bool,
    guard: ExportDamageGuard,
) -> DomainQualityAcceptanceReport {
    let rejection = if !hard_certificate_passed {
        Some(DomainQualityRejectReason::HardCertificateFailed)
    } else if !valid_evaluation(before) || !valid_evaluation(after) {
        Some(DomainQualityRejectReason::InvalidScore)
    } else if after.vector.global_hard_violation_count != 0
        || after.vector.global_hard_max_violation_microdeg != 0
    {
        Some(DomainQualityRejectReason::GlobalHardViolation)
    } else if after.vector.requirement_residuals != 0 {
        Some(DomainQualityRejectReason::RequirementResidual)
    } else if after.vector.topology_residuals != 0 {
        Some(DomainQualityRejectReason::TopologyResidual)
    } else if after.vector.dual_residuals != 0 {
        Some(DomainQualityRejectReason::DualResidual)
    } else if after.vector.remap_residuals != 0 {
        Some(DomainQualityRejectReason::RemapResidual)
    } else if let Some(reason) = export_damage_rejection(before, after, guard) {
        Some(DomainQualityRejectReason::ExportDamage(reason))
    } else if after.vector >= before.vector {
        Some(DomainQualityRejectReason::NotStrictImprovement)
    } else {
        None
    };
    DomainQualityAcceptanceReport {
        before,
        after,
        accepted: rejection.is_none(),
        rejection,
    }
}

fn valid_evaluation(evaluation: DomainQualityEvaluation) -> bool {
    let vector = evaluation.vector;
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
        evaluation.damage.external_preferred_l2_scaled,
    ]
    .into_iter()
    .all(|value| value >= 0)
}

pub fn commit_domain_quality_candidate<T>(
    state: &mut T,
    candidate: T,
    before: DomainQualityEvaluation,
    after: DomainQualityEvaluation,
    hard_certificate_passed: bool,
    guard: ExportDamageGuard,
) -> DomainQualityAcceptanceReport {
    let report = evaluate_domain_quality_candidate(before, after, hard_certificate_passed, guard);
    if report.accepted {
        *state = candidate;
    }
    report
}

fn export_damage_rejection(
    before: DomainQualityEvaluation,
    after: DomainQualityEvaluation,
    guard: ExportDamageGuard,
) -> Option<ExportDamageRejectReason> {
    let protected_before = before
        .vector
        .target_preferred_violation_count
        .saturating_add(before.vector.boundary_preferred_violation_count);
    let protected_after = after
        .vector
        .target_preferred_violation_count
        .saturating_add(after.vector.boundary_preferred_violation_count);
    let eliminated = protected_before.saturating_sub(protected_after);
    let new_external = after
        .damage
        .external_preferred_violation_count
        .saturating_sub(before.damage.external_preferred_violation_count);
    if new_external > eliminated.min(guard.maximum_new_external_preferred_violations) {
        return Some(ExportDamageRejectReason::NewPreferredViolations);
    }

    let protected_l2_before = before
        .vector
        .target_preferred_l2_scaled
        .saturating_add(before.vector.boundary_preferred_l2_scaled);
    let protected_l2_after = after
        .vector
        .target_preferred_l2_scaled
        .saturating_add(after.vector.boundary_preferred_l2_scaled);
    let protected_l2_decrease = protected_l2_before.saturating_sub(protected_l2_after) as i128;
    let external_l2_increase = after
        .damage
        .external_preferred_l2_scaled
        .saturating_sub(before.damage.external_preferred_l2_scaled)
        as i128;
    if guard.maximum_external_l2_increase_per_target_l2_decrease_millionths < 0
        || external_l2_increase * DOMAIN_QUALITY_SCALE as i128
            > protected_l2_decrease
                * guard.maximum_external_l2_increase_per_target_l2_decrease_millionths as i128
    {
        return Some(ExportDamageRejectReason::PreferredL2Increase);
    }

    if guard.minimum_external_hard_margin_microdeg < 0
        || after
            .damage
            .external_minimum_hard_margin_microdeg
            .is_some_and(|margin| margin < guard.minimum_external_hard_margin_microdeg)
    {
        return Some(ExportDamageRejectReason::InternalHardMargin);
    }
    None
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct DomainQualityAccumulator {
    global_hard_violation_count: usize,
    global_hard_violations: BTreeMap<i64, usize>,
    target_preferred_violation_count: usize,
    target_preferred_violations: BTreeMap<i64, usize>,
    target_preferred_l2_scaled: i64,
    target_equilateral_squared: u128,
    target_angle_count: usize,
    boundary_preferred_violation_count: usize,
    boundary_preferred_violations: BTreeMap<i64, usize>,
    boundary_preferred_l2_scaled: i64,
    export_near_boundary_penalty_scaled: i64,
    global_preferred_l2_scaled: i64,
    external_preferred_violation_count: usize,
    external_preferred_l2_scaled: i64,
    external_hard_margins: BTreeMap<i64, usize>,
}

impl DomainQualityAccumulator {
    pub(super) fn add(&mut self, witness: &SpatialAngleWitness) -> Result<(), String> {
        self.add_angle(DomainQualityAngle::from(witness))
    }

    pub(super) fn add_angle(&mut self, angle: DomainQualityAngle) -> Result<(), String> {
        self.apply(AngleContribution::from_angle(angle)?, true)
    }

    pub(super) fn remove_angle(&mut self, angle: DomainQualityAngle) -> Result<(), String> {
        self.apply(AngleContribution::from_angle(angle)?, false)
    }

    pub(super) fn evaluation(
        &self,
        costs: DomainQualityCosts,
        final_cell_count: usize,
        transition_faces_in_target: usize,
        transition_faces_in_boundary: usize,
    ) -> Result<DomainQualityEvaluation, String> {
        if costs.geometry_move_scaled < 0 {
            return Err("domain quality geometry move must be non-negative".into());
        }
        let target_equilateral_rmse_scaled = self
            .target_equilateral_squared
            .checked_div(self.target_angle_count as u128)
            .map(rounded_integer_sqrt)
            .unwrap_or(0)
            .try_into()
            .map_err(|_| "target equilateral score overflow".to_string())?;
        Ok(DomainQualityEvaluation {
            vector: DomainQualityVector {
                global_hard_violation_count: self.global_hard_violation_count,
                global_hard_max_violation_microdeg: maximum_key(&self.global_hard_violations),
                requirement_residuals: costs.requirement_residuals,
                topology_residuals: costs.topology_residuals,
                dual_residuals: costs.dual_residuals,
                remap_residuals: costs.remap_residuals,
                target_preferred_violation_count: self.target_preferred_violation_count,
                target_worst_preferred_violation_microdeg: maximum_key(
                    &self.target_preferred_violations,
                ),
                target_preferred_l2_scaled: self.target_preferred_l2_scaled,
                target_equilateral_rmse_scaled,
                boundary_preferred_violation_count: self.boundary_preferred_violation_count,
                boundary_worst_preferred_violation_microdeg: maximum_key(
                    &self.boundary_preferred_violations,
                ),
                boundary_preferred_l2_scaled: self.boundary_preferred_l2_scaled,
                transition_faces_in_target,
                transition_faces_in_boundary,
                export_near_boundary_penalty_scaled: self.export_near_boundary_penalty_scaled,
                final_cell_count,
                global_preferred_l2_scaled: self.global_preferred_l2_scaled,
                geometry_move_scaled: costs.geometry_move_scaled,
                topology_change_count: costs.topology_change_count,
                work_units: costs.work_units,
            },
            damage: DomainQualityDamageMetrics {
                external_preferred_violation_count: self.external_preferred_violation_count,
                external_preferred_l2_scaled: self.external_preferred_l2_scaled,
                external_minimum_hard_margin_microdeg: self
                    .external_hard_margins
                    .first_key_value()
                    .map(|(&margin, _)| margin),
            },
        })
    }

    fn apply(&mut self, value: AngleContribution, add: bool) -> Result<(), String> {
        adjust_usize(
            &mut self.global_hard_violation_count,
            value.global_hard_violation,
            add,
        )?;
        adjust_histogram(&mut self.global_hard_violations, value.global_hard, add)?;
        adjust_i64(
            &mut self.global_preferred_l2_scaled,
            value.preferred_l2,
            add,
        )?;
        match value.zone {
            QualityZone::TargetCore => {
                adjust_usize(
                    &mut self.target_preferred_violation_count,
                    value.preferred_violation,
                    add,
                )?;
                adjust_histogram(&mut self.target_preferred_violations, value.preferred, add)?;
                adjust_i64(
                    &mut self.target_preferred_l2_scaled,
                    value.preferred_l2,
                    add,
                )?;
                adjust_u128(
                    &mut self.target_equilateral_squared,
                    value.equilateral_squared,
                    add,
                )?;
                adjust_usize(&mut self.target_angle_count, 1, add)?;
            }
            QualityZone::BoundaryProtection => {
                adjust_usize(
                    &mut self.boundary_preferred_violation_count,
                    value.preferred_violation,
                    add,
                )?;
                adjust_histogram(
                    &mut self.boundary_preferred_violations,
                    value.preferred,
                    add,
                )?;
                adjust_i64(
                    &mut self.boundary_preferred_l2_scaled,
                    value.preferred_l2,
                    add,
                )?;
            }
            QualityZone::ExportCorridor | QualityZone::DeepExterior => {
                adjust_usize(
                    &mut self.external_preferred_violation_count,
                    value.preferred_violation,
                    add,
                )?;
                adjust_i64(
                    &mut self.external_preferred_l2_scaled,
                    value.preferred_l2,
                    add,
                )?;
                adjust_i64(
                    &mut self.export_near_boundary_penalty_scaled,
                    value.export_penalty,
                    add,
                )?;
                adjust_histogram(
                    &mut self.external_hard_margins,
                    value
                        .external_hard_margin
                        .expect("external contribution has a margin"),
                    add,
                )?;
            }
            QualityZone::GlobalNeutral => {}
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub(super) struct DomainQualityAngle {
    pub angle_degrees: f64,
    pub global_hard_violation: f64,
    pub preferred_violation: f64,
    pub zone: QualityZone,
    pub maximum_priority: f64,
}

impl From<&SpatialAngleWitness> for DomainQualityAngle {
    fn from(witness: &SpatialAngleWitness) -> Self {
        Self {
            angle_degrees: witness.angle_degrees,
            global_hard_violation: witness.global_hard_violation,
            preferred_violation: witness.preferred_violation,
            zone: witness.zone,
            maximum_priority: witness.maximum_priority,
        }
    }
}

#[derive(Clone, Copy)]
struct AngleContribution {
    zone: QualityZone,
    global_hard_violation: usize,
    global_hard: i64,
    preferred_violation: usize,
    preferred: i64,
    preferred_l2: i64,
    equilateral_squared: u128,
    export_penalty: i64,
    external_hard_margin: Option<i64>,
}

impl AngleContribution {
    fn from_angle(angle: DomainQualityAngle) -> Result<Self, String> {
        if !angle.angle_degrees.is_finite()
            || !angle.maximum_priority.is_finite()
            || !(0.0..=1.0).contains(&angle.maximum_priority)
        {
            return Err("domain quality atlas contains an invalid angle or priority".into());
        }
        let contract = AngleContract::for_id(AngleContractId::DomainQuality38To82V1);
        let global_hard = scale_nonnegative(angle.global_hard_violation, "global hard violation")?;
        let preferred = scale_nonnegative(angle.preferred_violation, "preferred violation")?;
        let preferred_l2 = squared_scaled(preferred)?;
        let equilateral = scale_signed(angle.angle_degrees - 60.0, "angle")?;
        let priority = scale_nonnegative(angle.maximum_priority, "quality priority")?;
        Ok(Self {
            zone: angle.zone,
            global_hard_violation: usize::from(angle.global_hard_violation > 0.0),
            global_hard,
            preferred_violation: usize::from(angle.preferred_violation > 0.0),
            preferred,
            preferred_l2,
            equilateral_squared: equilateral.unsigned_abs() as u128
                * equilateral.unsigned_abs() as u128,
            export_penalty: multiply_scaled(preferred_l2, priority)?,
            external_hard_margin: matches!(
                angle.zone,
                QualityZone::ExportCorridor | QualityZone::DeepExterior
            )
            .then(|| {
                scale_signed(
                    (angle.angle_degrees - contract.final_delivery.minimum_degrees)
                        .min(contract.final_delivery.maximum_degrees - angle.angle_degrees),
                    "external hard margin",
                )
            })
            .transpose()?,
        })
    }
}

fn maximum_key(histogram: &BTreeMap<i64, usize>) -> i64 {
    histogram.last_key_value().map_or(0, |(&value, _)| value)
}

fn adjust_histogram(
    histogram: &mut BTreeMap<i64, usize>,
    value: i64,
    add: bool,
) -> Result<(), String> {
    if add {
        *histogram.entry(value).or_default() += 1;
        return Ok(());
    }
    let count = histogram
        .get_mut(&value)
        .ok_or_else(|| "domain quality cache histogram underflow".to_string())?;
    *count -= 1;
    if *count == 0 {
        histogram.remove(&value);
    }
    Ok(())
}

fn adjust_usize(target: &mut usize, value: usize, add: bool) -> Result<(), String> {
    *target = if add {
        target.checked_add(value)
    } else {
        target.checked_sub(value)
    }
    .ok_or_else(|| "domain quality cache count overflow".to_string())?;
    Ok(())
}

fn adjust_i64(target: &mut i64, value: i64, add: bool) -> Result<(), String> {
    *target = if add {
        target.checked_add(value)
    } else {
        target.checked_sub(value)
    }
    .ok_or_else(|| "domain quality cache score overflow".to_string())?;
    Ok(())
}

fn adjust_u128(target: &mut u128, value: u128, add: bool) -> Result<(), String> {
    *target = if add {
        target.checked_add(value)
    } else {
        target.checked_sub(value)
    }
    .ok_or_else(|| "domain quality cache squared score overflow".to_string())?;
    Ok(())
}

fn scale_nonnegative(value: f64, label: &str) -> Result<i64, String> {
    if !value.is_finite() || value < 0.0 {
        return Err(format!(
            "domain quality {label} must be finite and non-negative"
        ));
    }
    scale_signed(value, label)
}

fn scale_signed(value: f64, label: &str) -> Result<i64, String> {
    let scaled = value * DOMAIN_QUALITY_SCALE as f64;
    if !scaled.is_finite() || scaled < i64::MIN as f64 || scaled > i64::MAX as f64 {
        return Err(format!("domain quality {label} is not representable"));
    }
    Ok(scaled.round() as i64)
}

fn squared_scaled(value: i64) -> Result<i64, String> {
    let square = value as i128 * value as i128;
    ((square + DOMAIN_QUALITY_SCALE as i128 / 2) / DOMAIN_QUALITY_SCALE as i128)
        .try_into()
        .map_err(|_| "domain quality squared score overflow".to_string())
}

fn multiply_scaled(left: i64, right: i64) -> Result<i64, String> {
    ((left as i128 * right as i128 + DOMAIN_QUALITY_SCALE as i128 / 2)
        / DOMAIN_QUALITY_SCALE as i128)
        .try_into()
        .map_err(|_| "domain quality weighted score overflow".to_string())
}

fn rounded_integer_sqrt(value: u128) -> u128 {
    let floor = value.isqrt();
    let ceil = floor.saturating_add(1);
    if value.saturating_sub(floor * floor) >= ceil.saturating_mul(ceil).saturating_sub(value) {
        ceil
    } else {
        floor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coarsen::{
        EdgeClass, SpatialAngleWitness, SpatialAtlasConclusion, SpatialZoneAngleMetrics,
    };
    use std::collections::BTreeSet;

    fn witness(face: usize, angle: f64, zone: QualityZone, priority: f64) -> SpatialAngleWitness {
        let contract = AngleContract::for_id(AngleContractId::DomainQuality38To82V1);
        let violation = |window: crate::certificate::AngleWindow| {
            (window.minimum_degrees - angle)
                .max(angle - window.maximum_degrees)
                .max(0.0)
        };
        SpatialAngleWitness {
            face,
            corner: 0,
            angle_degrees: angle,
            global_hard_violation: violation(contract.final_delivery),
            preferred_violation: violation(contract.preferred.unwrap()),
            zone,
            maximum_priority: priority,
            distance_to_target: 0.0,
            distance_to_boundary: 0.0,
            is_transition_face: false,
            transition_owner: None,
            component_id: None,
            hierarchy_address: None,
            movable_vertex_count: 3,
            fixed_vertex_count: 0,
            distance_to_pentagon_anchor: None,
            distance_to_seam: None,
            edge_classes: [EdgeClass::MotherGridEdge; 3],
        }
    }

    fn atlas(witnesses: Vec<SpatialAngleWitness>) -> SpatialAngleAtlas {
        let transition_faces = |zone| {
            witnesses
                .iter()
                .filter(|witness| witness.is_transition_face && witness.zone == zone)
                .map(|witness| witness.face)
                .collect::<BTreeSet<_>>()
                .len()
        };
        SpatialAngleAtlas {
            contract_id: AngleContractId::DomainQuality38To82V1,
            global: SpatialZoneAngleMetrics {
                angle_count: witnesses.len(),
                ..SpatialZoneAngleMetrics::default()
            },
            target: SpatialZoneAngleMetrics {
                transition_face_count: transition_faces(QualityZone::TargetCore),
                ..SpatialZoneAngleMetrics::default()
            },
            boundary: SpatialZoneAngleMetrics {
                transition_face_count: transition_faces(QualityZone::BoundaryProtection),
                ..SpatialZoneAngleMetrics::default()
            },
            export: SpatialZoneAngleMetrics::default(),
            deep_exterior: SpatialZoneAngleMetrics::default(),
            global_neutral: SpatialZoneAngleMetrics::default(),
            worst_angle_distance_to_target: None,
            bad_angle_component_count: 0,
            conclusion: SpatialAtlasConclusion::DomainRepairRequired,
            witnesses,
        }
    }

    fn face_witnesses(
        face: usize,
        angles: [f64; 3],
        zone: QualityZone,
        transition_owner: Option<u64>,
    ) -> Vec<SpatialAngleWitness> {
        angles
            .into_iter()
            .enumerate()
            .map(|(corner, angle)| {
                let mut witness = witness(face, angle, zone, 1.0);
                witness.corner = corner;
                witness.is_transition_face = transition_owner.is_some();
                witness.transition_owner = transition_owner;
                witness
            })
            .collect()
    }

    fn evaluation(
        vector: DomainQualityVector,
        external_count: usize,
        external_l2: i64,
    ) -> DomainQualityEvaluation {
        DomainQualityEvaluation {
            vector,
            damage: DomainQualityDamageMetrics {
                external_preferred_violation_count: external_count,
                external_preferred_l2_scaled: external_l2,
                external_minimum_hard_margin_microdeg: Some(1_000_000),
            },
        }
    }

    #[test]
    fn integer_vector_is_order_independent() {
        let mut samples = vec![
            witness(2, 39.5, QualityZone::TargetCore, 1.0),
            witness(3, 80.5, QualityZone::BoundaryProtection, 1.0),
            witness(4, 60.0, QualityZone::ExportCorridor, 0.25),
        ];
        let first = domain_quality_evaluation_from_atlas(
            &atlas(samples.clone()),
            DomainQualityCosts::default(),
        )
        .unwrap();
        samples.reverse();
        let second =
            domain_quality_evaluation_from_atlas(&atlas(samples), DomainQualityCosts::default())
                .unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.vector.target_worst_preferred_violation_microdeg,
            500_000
        );
        assert_eq!(first.vector.target_preferred_l2_scaled, 250_000);
    }

    #[test]
    fn lexicographic_target_improvement_beats_cell_reduction() {
        let baseline = DomainQualityVector {
            target_preferred_violation_count: 1,
            final_cell_count: 100,
            ..DomainQualityVector::default()
        };
        let target_improved = DomainQualityVector {
            target_preferred_violation_count: 0,
            final_cell_count: 120,
            ..baseline
        };
        let only_smaller = DomainQualityVector {
            final_cell_count: 80,
            ..baseline
        };
        assert!(target_improved < baseline);
        assert!(target_improved < only_smaller);

        let boundary_improved = DomainQualityVector {
            boundary_preferred_violation_count: 0,
            export_near_boundary_penalty_scaled: 1_000,
            ..baseline
        };
        let only_export_improved = DomainQualityVector {
            boundary_preferred_violation_count: 1,
            export_near_boundary_penalty_scaled: 0,
            ..baseline
        };
        assert!(boundary_improved < only_export_improved);
    }

    #[test]
    fn export_damage_guard_blocks_target_gain_paid_by_external_damage() {
        let before = evaluation(
            DomainQualityVector {
                target_preferred_violation_count: 2,
                target_preferred_l2_scaled: 2_000_000,
                ..DomainQualityVector::default()
            },
            0,
            0,
        );
        let after = evaluation(DomainQualityVector::default(), 2, 2_000_001);
        assert_eq!(
            evaluate_domain_quality_candidate(before, after, true, ExportDamageGuard::default())
                .rejection,
            Some(DomainQualityRejectReason::ExportDamage(
                ExportDamageRejectReason::PreferredL2Increase
            ))
        );

        let after = evaluation(DomainQualityVector::default(), 3, 0);
        assert_eq!(
            evaluate_domain_quality_candidate(before, after, true, ExportDamageGuard::default())
                .rejection,
            Some(DomainQualityRejectReason::ExportDamage(
                ExportDamageRejectReason::NewPreferredViolations
            ))
        );

        let mut after = evaluation(DomainQualityVector::default(), 0, 0);
        after.damage.external_minimum_hard_margin_microdeg = Some(100_000);
        assert_eq!(
            evaluate_domain_quality_candidate(before, after, true, ExportDamageGuard::default())
                .rejection,
            Some(DomainQualityRejectReason::ExportDamage(
                ExportDamageRejectReason::InternalHardMargin
            ))
        );
    }

    #[test]
    fn transaction_commits_only_after_hard_guard_and_lexicographic_gain() {
        let before = evaluation(
            DomainQualityVector {
                target_preferred_violation_count: 1,
                target_preferred_l2_scaled: 1_000_000,
                ..DomainQualityVector::default()
            },
            0,
            0,
        );
        let mut state = 7_u64;
        let improved = evaluation(DomainQualityVector::default(), 0, 0);
        let uncertified = commit_domain_quality_candidate(
            &mut state,
            8,
            before,
            improved,
            false,
            ExportDamageGuard::default(),
        );
        assert_eq!(
            uncertified.rejection,
            Some(DomainQualityRejectReason::HardCertificateFailed)
        );
        assert_eq!(state, 7);

        let invalid = evaluation(
            DomainQualityVector {
                target_preferred_l2_scaled: -1,
                ..DomainQualityVector::default()
            },
            0,
            0,
        );
        let invalid = commit_domain_quality_candidate(
            &mut state,
            8,
            before,
            invalid,
            true,
            ExportDamageGuard::default(),
        );
        assert_eq!(
            invalid.rejection,
            Some(DomainQualityRejectReason::InvalidScore)
        );
        assert_eq!(state, 7);

        let unchanged = commit_domain_quality_candidate(
            &mut state,
            8,
            before,
            before,
            true,
            ExportDamageGuard::default(),
        );
        assert_eq!(
            unchanged.rejection,
            Some(DomainQualityRejectReason::NotStrictImprovement)
        );
        assert_eq!(state, 7);

        let hard_failure = evaluation(
            DomainQualityVector {
                global_hard_violation_count: 1,
                global_hard_max_violation_microdeg: 1,
                ..DomainQualityVector::default()
            },
            0,
            0,
        );
        let rejected = commit_domain_quality_candidate(
            &mut state,
            8,
            before,
            hard_failure,
            true,
            ExportDamageGuard::default(),
        );
        assert!(!rejected.accepted);
        assert_eq!(
            rejected.rejection,
            Some(DomainQualityRejectReason::GlobalHardViolation)
        );
        assert_eq!(state, 7);

        let accepted = commit_domain_quality_candidate(
            &mut state,
            8,
            before,
            improved,
            true,
            ExportDamageGuard::default(),
        );
        assert!(accepted.accepted);
        assert_eq!(accepted.before, before);
        assert_eq!(state, 8);
    }

    #[test]
    fn transition_report_excludes_non_transition_defects_and_rejects_new_intrusion() {
        let mut before_witnesses =
            face_witnesses(2, [39.5, 60.0, 60.0], QualityZone::TargetCore, None);
        before_witnesses.extend(face_witnesses(
            3,
            [60.0; 3],
            QualityZone::TargetCore,
            Some(7),
        ));
        let mut after_witnesses = before_witnesses[..3].to_vec();
        after_witnesses.extend(face_witnesses(
            3,
            [39.5, 60.0, 60.0],
            QualityZone::TargetCore,
            Some(7),
        ));
        let before_atlas = atlas(before_witnesses);
        let after_atlas = atlas(after_witnesses);
        let before =
            domain_quality_evaluation_from_atlas(&before_atlas, DomainQualityCosts::default())
                .unwrap();
        let after =
            domain_quality_evaluation_from_atlas(&after_atlas, DomainQualityCosts::default())
                .unwrap();
        let mut state = 7_u64;
        let acceptance = commit_domain_quality_candidate(
            &mut state,
            8,
            before,
            after,
            true,
            ExportDamageGuard::default(),
        );
        let report = transition_intrusion_report(&before_atlas, &after_atlas, acceptance).unwrap();

        assert_eq!(state, 7);
        assert!(!report.accepted);
        assert_eq!(
            report.rejection,
            Some(DomainQualityRejectReason::ExportDamage(
                ExportDamageRejectReason::PreferredL2Increase
            ))
        );
        assert_eq!(report.target_before.transition_faces, 1);
        assert_eq!(report.target_before.preferred_violation_count, 0);
        assert_eq!(report.target_after.transition_faces, 1);
        assert_eq!(report.target_after.preferred_violation_count, 1);
        assert_eq!(report.target_after.preferred_l2_scaled, 250_000);
        assert_eq!(
            transition_intrusion_report_json(&report),
            "{\"schema_version\":1,\"target_before\":{\"transition_faces\":1,\"preferred_violation_count\":0,\"worst_preferred_violation_microdeg\":0,\"preferred_l2_scaled\":0},\"target_after\":{\"transition_faces\":1,\"preferred_violation_count\":1,\"worst_preferred_violation_microdeg\":500000,\"preferred_l2_scaled\":250000},\"boundary_before\":{\"transition_faces\":0,\"preferred_violation_count\":0,\"worst_preferred_violation_microdeg\":0,\"preferred_l2_scaled\":0},\"boundary_after\":{\"transition_faces\":0,\"preferred_violation_count\":0,\"worst_preferred_violation_microdeg\":0,\"preferred_l2_scaled\":0},\"accepted\":false,\"rejection\":\"export_damage_preferred_l2_increase\"}"
        );
    }
}
