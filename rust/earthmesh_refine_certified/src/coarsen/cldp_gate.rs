//! Frozen CLDP product gate and delivery classification.

use super::{HierarchyLeafMesh, PromotionLevel, PromotionTrial};
use crate::{FinalCertificateReport, MotherGrid, VertexAddress};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq)]
pub struct FrozenCldpGateEvidence {
    pub internal_angle_range_deg: (f64, f64),
    pub final_angle_range_deg: (f64, f64),
    pub anchors_degree_five: bool,
    pub ordinary_degree_window: bool,
    pub links_are_cycles: bool,
    pub edge_incidence_two: bool,
    pub vertices: usize,
    pub edges: usize,
    pub faces: usize,
    pub euler: isize,
    pub charge: isize,
    pub delaunay_violations: usize,
    pub voronoi_invalid_cells: usize,
    pub voronoi_reciprocal_errors: usize,
    pub physical_residuals: usize,
    pub balance_residuals: usize,
    pub remap_closure_errors: usize,
    pub mixed_levels_delivered: bool,
    pub promoted_source_faces: usize,
    pub retained_coarse_parents: usize,
    pub compression_ratio: f64,
    pub promotion_level: PromotionLevel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrozenCldpGateOutcome {
    CertifiedAdaptive,
    CertifiedSafeFallback,
    Failed(Vec<&'static str>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrozenN6MixedExistenceStatus {
    CertifiedAdaptive,
    ContinuousSearchIncomplete,
    ScopedNoGo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrozenN6IntervalStatus {
    NotAttempted,
    ScopedInfeasible,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PostPr78FamilyEvidence {
    pub original_violation_count: usize,
    pub original_violating_faces: usize,
    pub singleton_actions: usize,
    pub strict_singleton_candidates: usize,
    pub compatible_action_sets: usize,
    pub pruned_action_sets: usize,
    pub combined_topologies_closed: usize,
    pub combined_geometry_attempted: usize,
    pub strict_combined_candidates: usize,
    pub best_mixed_angle_range_deg: Option<(f64, f64)>,
    pub best_combined_angle_range_deg: Option<(f64, f64)>,
    pub best_combined_margin_deg: Option<f64>,
    pub best_combined_retained_parents: usize,
    pub retained_core_subsets_tested: usize,
    pub retained_core_families_tested: usize,
    pub retained_core_topologies_closed: usize,
    pub retained_core_geometry_attempted: usize,
    pub retained_core_exact_no_solution: usize,
    pub retained_core_search_incomplete: usize,
    pub strict_retained_core_candidates: usize,
    pub combined_continuous_search_incomplete: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PostPr78CombinedRecoveryReport {
    pub fixture_fingerprint: u64,
    pub families: PostPr78FamilyEvidence,
    pub interval_status: FrozenN6IntervalStatus,
    pub mixed_existence_status: FrozenN6MixedExistenceStatus,
    pub product_outcome: FrozenCldpGateOutcome,
    pub safe_fallback: FrozenCldpGateEvidence,
    pub next_scale_unlocked: bool,
}

pub fn build_post_pr78_combined_recovery_report(
    fixture_fingerprint: u64,
    families: PostPr78FamilyEvidence,
    strict_candidate: Option<&FrozenCldpGateEvidence>,
    safe_fallback: FrozenCldpGateEvidence,
    interval_status: FrozenN6IntervalStatus,
) -> Result<PostPr78CombinedRecoveryReport, String> {
    if families.original_violation_count == 0
        || families.original_violating_faces == 0
        || families.singleton_actions == 0
        || families.compatible_action_sets == 0
        || families.retained_core_subsets_tested == 0
        || families.retained_core_families_tested == 0
    {
        return Err("post-PR78 report requires evidence from every registered family".into());
    }
    if families.retained_core_families_tested != families.retained_core_subsets_tested * 6
        || families.retained_core_exact_no_solution
            + families.retained_core_search_incomplete
            + families.retained_core_topologies_closed
            != families.retained_core_families_tested
    {
        return Err("retained-core F0-F5 family accounting is incomplete".into());
    }
    let fallback_outcome = evaluate_frozen_cldp_gate(&safe_fallback);
    if fallback_outcome != FrozenCldpGateOutcome::CertifiedSafeFallback {
        return Err(format!(
            "Frozen N6 report lacks its certified safe fallback: {fallback_outcome:?}"
        ));
    }
    let adaptive = strict_candidate
        .map(evaluate_frozen_cldp_gate)
        .is_some_and(|outcome| outcome == FrozenCldpGateOutcome::CertifiedAdaptive);
    let product_outcome = if adaptive {
        FrozenCldpGateOutcome::CertifiedAdaptive
    } else {
        fallback_outcome
    };
    let incomplete = families.combined_continuous_search_incomplete
        || families.retained_core_search_incomplete > 0;
    let mixed_existence_status = if adaptive {
        FrozenN6MixedExistenceStatus::CertifiedAdaptive
    } else if incomplete || interval_status == FrozenN6IntervalStatus::NotAttempted {
        FrozenN6MixedExistenceStatus::ContinuousSearchIncomplete
    } else {
        FrozenN6MixedExistenceStatus::ScopedNoGo
    };
    let next_scale_unlocked = matches!(
        mixed_existence_status,
        FrozenN6MixedExistenceStatus::CertifiedAdaptive | FrozenN6MixedExistenceStatus::ScopedNoGo
    );
    Ok(PostPr78CombinedRecoveryReport {
        fixture_fingerprint,
        families,
        interval_status,
        mixed_existence_status,
        product_outcome,
        safe_fallback,
        next_scale_unlocked,
    })
}

pub fn post_pr78_combined_recovery_report_json(report: &PostPr78CombinedRecoveryReport) -> String {
    let families = &report.families;
    let fallback = &report.safe_fallback;
    format!(
        "{{\"fixture_fingerprint\":{},\"original_violation_count\":{},\"original_violating_faces\":{},\"singleton_actions\":{},\"strict_singleton_candidates\":{},\"compatible_action_sets\":{},\"pruned_action_sets\":{},\"combined_topologies_closed\":{},\"combined_geometry_attempted\":{},\"strict_combined_candidates\":{},\"best_mixed_angle_range_deg\":{},\"best_combined_angle_range_deg\":{},\"best_combined_margin_deg\":{},\"best_combined_retained_parents\":{},\"retained_core_subsets_tested\":{},\"retained_core_families_tested\":{},\"retained_core_topologies_closed\":{},\"retained_core_geometry_attempted\":{},\"retained_core_exact_no_solution\":{},\"retained_core_search_incomplete\":{},\"strict_retained_core_candidates\":{},\"combined_continuous_search_incomplete\":{},\"interval_status\":\"{:?}\",\"mixed_existence_status\":\"{:?}\",\"product_outcome\":\"{:?}\",\"safe_fallback_internal_angle_range_deg\":[{:.12},{:.12}],\"safe_fallback_final_angle_range_deg\":[{:.12},{:.12}],\"safe_fallback_retained_parents\":{},\"safe_fallback_compression_ratio\":{:.12},\"next_scale_unlocked\":{}}}",
        report.fixture_fingerprint,
        families.original_violation_count,
        families.original_violating_faces,
        families.singleton_actions,
        families.strict_singleton_candidates,
        families.compatible_action_sets,
        families.pruned_action_sets,
        families.combined_topologies_closed,
        families.combined_geometry_attempted,
        families.strict_combined_candidates,
        optional_range_json(families.best_mixed_angle_range_deg),
        optional_range_json(families.best_combined_angle_range_deg),
        families
            .best_combined_margin_deg
            .map_or_else(|| "null".into(), |value| format!("{value:.12}")),
        families.best_combined_retained_parents,
        families.retained_core_subsets_tested,
        families.retained_core_families_tested,
        families.retained_core_topologies_closed,
        families.retained_core_geometry_attempted,
        families.retained_core_exact_no_solution,
        families.retained_core_search_incomplete,
        families.strict_retained_core_candidates,
        families.combined_continuous_search_incomplete,
        report.interval_status,
        report.mixed_existence_status,
        report.product_outcome,
        fallback.internal_angle_range_deg.0,
        fallback.internal_angle_range_deg.1,
        fallback.final_angle_range_deg.0,
        fallback.final_angle_range_deg.1,
        fallback.retained_coarse_parents,
        fallback.compression_ratio,
        report.next_scale_unlocked,
    )
}

pub fn build_frozen_cldp_gate_evidence(
    source: &MotherGrid,
    trial: &PromotionTrial,
    final_report: &FinalCertificateReport,
) -> Result<FrozenCldpGateEvidence, String> {
    if trial.mesh.source_vertex_slots.len() != trial.mesh.mesh.vertices().len() {
        return Err("CLDP source-slot map does not match the final mesh".into());
    }
    let degrees = vertex_degrees(&trial.mesh);
    let mut anchors_degree_five = true;
    let mut ordinary_degree_window = true;
    for (compact, source_slot) in trial.mesh.source_vertex_slots.iter().copied().enumerate() {
        if !trial.mesh.mesh.is_vertex_live(compact) {
            continue;
        }
        let degree = degrees[compact];
        if source_slot.is_some_and(|source_slot| {
            matches!(
                source.addresses[source_slot],
                Some(VertexAddress::IcosahedronVertex(_))
            )
        }) {
            anchors_degree_five &= degree == 5;
        } else {
            ordinary_degree_window &= (5..=7).contains(&degree);
        }
    }
    let retained_coarse_parents = trial
        .mesh
        .mesh
        .active_triangle_slots()
        .filter_map(|face| trial.mesh.triangle_addresses[face])
        .filter(|address| address.n < source.subdivision)
        .collect::<BTreeSet<_>>()
        .len();
    let has_fine = trial.mesh.mesh.active_triangle_slots().any(|face| {
        trial.mesh.triangle_addresses[face].is_some_and(|address| address.n == source.subdivision)
    });
    let has_compressed = trial.mesh.mesh.active_triangle_slots().any(|face| {
        trial.mesh.triangle_addresses[face].is_none_or(|address| address.n < source.subdivision)
    });
    let faces = final_report.geometry.faces;
    Ok(FrozenCldpGateEvidence {
        internal_angle_range_deg: (
            trial.geometry.min_angle_degrees,
            trial.geometry.max_angle_degrees,
        ),
        final_angle_range_deg: (
            final_report.geometry.min_angle_degrees,
            final_report.geometry.max_angle_degrees,
        ),
        anchors_degree_five,
        ordinary_degree_window,
        links_are_cycles: final_report.geometry.topology_errors == 0,
        edge_incidence_two: final_report.geometry.open_edges == 0
            && final_report.geometry.topology_errors == 0,
        vertices: final_report.geometry.vertices,
        edges: final_report.geometry.edges,
        faces,
        euler: final_report.geometry.euler,
        charge: final_report.geometry.charge,
        delaunay_violations: final_report.geometry.delaunay_violations,
        voronoi_invalid_cells: final_report.geometry.voronoi_invalid_cells,
        voronoi_reciprocal_errors: final_report.geometry.voronoi_reciprocal_errors,
        physical_residuals: final_report.physical_residuals,
        balance_residuals: final_report.balance_residuals,
        remap_closure_errors: final_report.remap_closure_errors,
        mixed_levels_delivered: has_fine && has_compressed,
        promoted_source_faces: trial.promotion_patch.source_faces.len(),
        retained_coarse_parents,
        compression_ratio: faces as f64 / source.mesh.triangle_count() as f64,
        promotion_level: trial.promotion_patch.level,
    })
}

pub fn evaluate_frozen_cldp_gate(evidence: &FrozenCldpGateEvidence) -> FrozenCldpGateOutcome {
    let mut failed = Vec::new();
    let (internal_min, internal_max) = evidence.internal_angle_range_deg;
    if internal_min < 40.2 || internal_max > 79.8 {
        failed.push("internal_angle_window");
    }
    let (final_min, final_max) = evidence.final_angle_range_deg;
    if final_min < 40.0 || final_max > 80.0 {
        failed.push("final_angle_window");
    }
    if !evidence.anchors_degree_five {
        failed.push("anchor_degree");
    }
    if !evidence.ordinary_degree_window {
        failed.push("ordinary_degree_window");
    }
    if !evidence.links_are_cycles {
        failed.push("vertex_links");
    }
    if !evidence.edge_incidence_two {
        failed.push("edge_incidence");
    }
    if evidence.euler != 2 || evidence.charge != 12 {
        failed.push("euler_charge");
    }
    if evidence.delaunay_violations != 0 {
        failed.push("delaunay");
    }
    if evidence.voronoi_invalid_cells != 0 || evidence.voronoi_reciprocal_errors != 0 {
        failed.push("voronoi");
    }
    if evidence.physical_residuals != 0 {
        failed.push("physical");
    }
    if evidence.balance_residuals != 0 {
        failed.push("balance");
    }
    if evidence.remap_closure_errors != 0 {
        failed.push("remap");
    }
    if !evidence.mixed_levels_delivered {
        failed.push("mixed_levels_delivered");
    }
    if evidence.retained_coarse_parents == 0 {
        failed.push("retained_coarse_parents");
    }
    if evidence.compression_ratio >= 1.0 {
        failed.push("compression_ratio");
    }
    if failed.is_empty() {
        FrozenCldpGateOutcome::CertifiedAdaptive
    } else if failed
        == [
            "mixed_levels_delivered",
            "retained_coarse_parents",
            "compression_ratio",
        ]
        && evidence.promotion_level == PromotionLevel::P5SafeMotherFallback
    {
        FrozenCldpGateOutcome::CertifiedSafeFallback
    } else {
        FrozenCldpGateOutcome::Failed(failed)
    }
}

fn optional_range_json(range: Option<(f64, f64)>) -> String {
    range.map_or_else(
        || "null".into(),
        |(minimum, maximum)| format!("[{minimum:.12},{maximum:.12}]"),
    )
}

fn vertex_degrees(mesh: &HierarchyLeafMesh) -> Vec<usize> {
    let mut degrees = vec![0; mesh.mesh.vertices().len()];
    for face in mesh.mesh.active_triangle_slots() {
        for site in mesh.mesh.triangles()[face] {
            degrees[site] += 1;
        }
    }
    degrees
}

#[cfg(test)]
mod tests {
    use super::*;

    fn passing(mixed: bool) -> FrozenCldpGateEvidence {
        FrozenCldpGateEvidence {
            internal_angle_range_deg: (40.2, 79.8),
            final_angle_range_deg: (40.0, 80.0),
            anchors_degree_five: true,
            ordinary_degree_window: true,
            links_are_cycles: true,
            edge_incidence_two: true,
            vertices: 10,
            edges: 24,
            faces: 16,
            euler: 2,
            charge: 12,
            delaunay_violations: 0,
            voronoi_invalid_cells: 0,
            voronoi_reciprocal_errors: 0,
            physical_residuals: 0,
            balance_residuals: 0,
            remap_closure_errors: 0,
            mixed_levels_delivered: mixed,
            promoted_source_faces: 4,
            retained_coarse_parents: usize::from(mixed),
            compression_ratio: if mixed { 0.75 } else { 1.0 },
            promotion_level: if mixed {
                PromotionLevel::P2RestoreOneParentRing
            } else {
                PromotionLevel::P5SafeMotherFallback
            },
        }
    }

    fn families(search_incomplete: usize) -> PostPr78FamilyEvidence {
        PostPr78FamilyEvidence {
            original_violation_count: 109,
            original_violating_faces: 89,
            singleton_actions: 14,
            strict_singleton_candidates: 0,
            compatible_action_sets: 16_383,
            pruned_action_sets: 0,
            combined_topologies_closed: 122,
            combined_geometry_attempted: 10,
            strict_combined_candidates: 0,
            best_mixed_angle_range_deg: Some((39.278499430048, 80.721500570507)),
            best_combined_angle_range_deg: Some((30.000000000281, 90.000000000509)),
            best_combined_margin_deg: Some(-10.200000000509),
            best_combined_retained_parents: 7,
            retained_core_subsets_tested: 154,
            retained_core_families_tested: 924,
            retained_core_topologies_closed: 0,
            retained_core_geometry_attempted: 0,
            retained_core_exact_no_solution: 265,
            retained_core_search_incomplete: search_incomplete,
            strict_retained_core_candidates: 0,
            combined_continuous_search_incomplete: true,
        }
    }

    #[test]
    fn strict_mixed_returns_certified_adaptive() {
        assert_eq!(
            evaluate_frozen_cldp_gate(&passing(true)),
            FrozenCldpGateOutcome::CertifiedAdaptive
        );
    }

    #[test]
    fn all_fine_returns_safe_fallback_not_adaptive_success() {
        assert_eq!(
            evaluate_frozen_cldp_gate(&passing(false)),
            FrozenCldpGateOutcome::CertifiedSafeFallback
        );
    }

    #[test]
    fn mixed_output_without_compression_fails_the_adaptive_gate() {
        let mut evidence = passing(true);
        evidence.compression_ratio = 1.0;
        assert_eq!(
            evaluate_frozen_cldp_gate(&evidence),
            FrozenCldpGateOutcome::Failed(vec!["compression_ratio"])
        );
    }

    #[test]
    fn incomplete_mixed_search_keeps_the_certified_safe_fallback() {
        let report = build_post_pr78_combined_recovery_report(
            7,
            families(659),
            None,
            passing(false),
            FrozenN6IntervalStatus::NotAttempted,
        )
        .unwrap();
        assert_eq!(
            report.product_outcome,
            FrozenCldpGateOutcome::CertifiedSafeFallback
        );
        assert_eq!(
            report.mixed_existence_status,
            FrozenN6MixedExistenceStatus::ContinuousSearchIncomplete
        );
        assert!(!report.next_scale_unlocked);
    }

    #[test]
    fn strict_mixed_witness_overrides_incomplete_searches() {
        let report = build_post_pr78_combined_recovery_report(
            7,
            families(659),
            Some(&passing(true)),
            passing(false),
            FrozenN6IntervalStatus::NotAttempted,
        )
        .unwrap();
        assert_eq!(
            report.mixed_existence_status,
            FrozenN6MixedExistenceStatus::CertifiedAdaptive
        );
        assert_eq!(
            report.product_outcome,
            FrozenCldpGateOutcome::CertifiedAdaptive
        );
        assert!(report.next_scale_unlocked);
    }

    #[test]
    fn scoped_no_go_requires_closed_search_and_interval_proof() {
        let mut evidence = families(0);
        evidence.combined_continuous_search_incomplete = false;
        evidence.retained_core_exact_no_solution = evidence.retained_core_families_tested;
        let report = build_post_pr78_combined_recovery_report(
            7,
            evidence,
            None,
            passing(false),
            FrozenN6IntervalStatus::ScopedInfeasible,
        )
        .unwrap();
        assert_eq!(
            report.mixed_existence_status,
            FrozenN6MixedExistenceStatus::ScopedNoGo
        );
        assert!(report.next_scale_unlocked);
    }
}
