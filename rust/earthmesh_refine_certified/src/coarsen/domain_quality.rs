//! Integer-only DQX candidate scoring and atomic acceptance.

use super::SpatialAngleAtlas;
use crate::certificate::{AngleContract, AngleContractId};
use earthmesh_quality::domain::QualityZone;

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
    let contract = AngleContract::for_id(atlas.contract_id);
    let mut vector = DomainQualityVector {
        requirement_residuals: costs.requirement_residuals,
        topology_residuals: costs.topology_residuals,
        dual_residuals: costs.dual_residuals,
        remap_residuals: costs.remap_residuals,
        transition_faces_in_target: atlas.target.transition_face_count,
        transition_faces_in_boundary: atlas.boundary.transition_face_count,
        final_cell_count: atlas.global.angle_count / 3,
        geometry_move_scaled: costs.geometry_move_scaled,
        topology_change_count: costs.topology_change_count,
        work_units: costs.work_units,
        ..DomainQualityVector::default()
    };
    let mut damage = DomainQualityDamageMetrics::default();
    let mut target_equilateral_squared = 0_u128;
    let mut target_angle_count = 0_u128;

    for witness in &atlas.witnesses {
        if !witness.angle_degrees.is_finite()
            || !witness.maximum_priority.is_finite()
            || !(0.0..=1.0).contains(&witness.maximum_priority)
        {
            return Err("domain quality atlas contains an invalid angle or priority".into());
        }
        let hard = scale_nonnegative(witness.global_hard_violation, "global hard violation")?;
        let preferred = scale_nonnegative(witness.preferred_violation, "preferred violation")?;
        let preferred_l2 = squared_scaled(preferred)?;
        vector.global_hard_violation_count += usize::from(witness.global_hard_violation > 0.0);
        vector.global_hard_max_violation_microdeg =
            vector.global_hard_max_violation_microdeg.max(hard);
        add_scaled(&mut vector.global_preferred_l2_scaled, preferred_l2)?;

        match witness.zone {
            QualityZone::TargetCore => {
                vector.target_preferred_violation_count +=
                    usize::from(witness.preferred_violation > 0.0);
                vector.target_worst_preferred_violation_microdeg = vector
                    .target_worst_preferred_violation_microdeg
                    .max(preferred);
                add_scaled(&mut vector.target_preferred_l2_scaled, preferred_l2)?;
                let equilateral = scale_signed(witness.angle_degrees - 60.0, "angle")?;
                target_equilateral_squared = target_equilateral_squared
                    .checked_add(
                        equilateral.unsigned_abs() as u128 * equilateral.unsigned_abs() as u128,
                    )
                    .ok_or_else(|| "target equilateral score overflow".to_string())?;
                target_angle_count += 1;
            }
            QualityZone::BoundaryProtection => {
                vector.boundary_preferred_violation_count +=
                    usize::from(witness.preferred_violation > 0.0);
                vector.boundary_worst_preferred_violation_microdeg = vector
                    .boundary_worst_preferred_violation_microdeg
                    .max(preferred);
                add_scaled(&mut vector.boundary_preferred_l2_scaled, preferred_l2)?;
            }
            QualityZone::ExportCorridor | QualityZone::DeepExterior => {
                damage.external_preferred_violation_count +=
                    usize::from(witness.preferred_violation > 0.0);
                add_scaled(&mut damage.external_preferred_l2_scaled, preferred_l2)?;
                let priority = scale_nonnegative(witness.maximum_priority, "quality priority")?;
                add_scaled(
                    &mut vector.export_near_boundary_penalty_scaled,
                    multiply_scaled(preferred_l2, priority)?,
                )?;
                let margin = scale_signed(
                    (witness.angle_degrees - contract.final_delivery.minimum_degrees)
                        .min(contract.final_delivery.maximum_degrees - witness.angle_degrees),
                    "external hard margin",
                )?;
                damage.external_minimum_hard_margin_microdeg = Some(
                    damage
                        .external_minimum_hard_margin_microdeg
                        .map_or(margin, |current| current.min(margin)),
                );
            }
            QualityZone::GlobalNeutral => {}
        }
    }

    if let Some(mean) = target_equilateral_squared.checked_div(target_angle_count) {
        vector.target_equilateral_rmse_scaled = rounded_integer_sqrt(mean)
            .try_into()
            .map_err(|_| "target equilateral score overflow".to_string())?;
    }
    Ok(DomainQualityEvaluation { vector, damage })
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

fn add_scaled(target: &mut i64, value: i64) -> Result<(), String> {
    *target = target
        .checked_add(value)
        .ok_or_else(|| "domain quality accumulated score overflow".to_string())?;
    Ok(())
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
        SpatialAngleAtlas {
            contract_id: AngleContractId::DomainQuality38To82V1,
            global: SpatialZoneAngleMetrics {
                angle_count: witnesses.len(),
                ..SpatialZoneAngleMetrics::default()
            },
            target: SpatialZoneAngleMetrics::default(),
            boundary: SpatialZoneAngleMetrics::default(),
            export: SpatialZoneAngleMetrics::default(),
            deep_exterior: SpatialZoneAngleMetrics::default(),
            global_neutral: SpatialZoneAngleMetrics::default(),
            worst_angle_distance_to_target: None,
            bad_angle_component_count: 0,
            conclusion: SpatialAtlasConclusion::DomainRepairRequired,
            witnesses,
        }
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
}
