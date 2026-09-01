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
}
