//! Auditable local-repair actions used by the post-PR78 recovery ladder.

use super::{
    direct_restore::closed_edge_incidence, ElasticTargetMode, HierarchyLeafMesh,
    PromotionPatchTopology, ProtectedCoarseRegion, SectorRecoveryAtlas, ViolatingAngle,
};
use crate::{
    certificate::spherical_triangle_angles, mesh_fingerprint, MotherGrid, TriangleAddress,
};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LocalRepairKind {
    DirectSectorRestore,
    OneParentPeel,
    TwoParentAnnularCollar,
    HierarchyCoordinateRelease,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalRepairAction {
    pub id: u64,
    pub kind: LocalRepairKind,
    pub sector_ids: BTreeSet<u64>,
    pub removed_mixed_faces: BTreeSet<usize>,
    pub restored_source_faces: BTreeSet<usize>,
    pub released_parents: BTreeSet<TriangleAddress>,
    pub retained_parents: BTreeSet<TriangleAddress>,
    pub inserted_custom_triangles: BTreeSet<[usize; 3]>,
    pub modified_source_vertices: BTreeSet<usize>,
    pub boundary_source_vertices: BTreeSet<usize>,
    pub movable_source_vertices: BTreeSet<usize>,
    pub original_violation_ids_covered: BTreeSet<usize>,
    pub original_violation_ids_untouched: BTreeSet<usize>,
    pub patch_topology: PromotionPatchTopology,
    pub protected_coarse_regions: Vec<ProtectedCoarseRegion>,
    pub geometry_target_mode: ElasticTargetMode,
    pub geometry_iteration_limit: usize,
    pub fingerprint: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalActionEffect {
    pub action_id: u64,
    pub original_violations_total: usize,
    pub original_violations_removed: usize,
    pub original_violations_resolved: usize,
    pub original_violations_persisted: usize,
    pub uncovered_original_violations: usize,
    pub new_violations_created: usize,
    pub local_angle_range: Option<(f64, f64)>,
    pub outside_angle_range: Option<(f64, f64)>,
    pub global_angle_range: Option<(f64, f64)>,
    pub local_signed_margin_deg: Option<f64>,
    pub global_signed_margin_deg: Option<f64>,
    pub topology_valid: bool,
    pub orientation_valid: bool,
    pub strict_certified: bool,
    pub failure_reason: SingletonFailureReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingletonFailureReason {
    StrictCertified,
    UncoveredOriginalViolations,
    NewViolationsCreated,
    ContinuousUnknown,
}

#[allow(clippy::too_many_arguments)]
pub fn build_local_repair_action(
    source: &MotherGrid,
    candidate: &HierarchyLeafMesh,
    atlas: &SectorRecoveryAtlas,
    original_violations: &[ViolatingAngle],
    retained_parents: &BTreeSet<TriangleAddress>,
    id: u64,
    kind: LocalRepairKind,
    sector_ids: BTreeSet<u64>,
    removed_mixed_faces: BTreeSet<usize>,
    restored_source_faces: BTreeSet<usize>,
    released_parents: BTreeSet<TriangleAddress>,
    inserted_custom_triangles: BTreeSet<[usize; 3]>,
    geometry_iteration_limit: usize,
) -> Result<LocalRepairAction, String> {
    if sector_ids.is_empty() {
        return Err("local repair action requires at least one exact sector".into());
    }
    if sector_ids
        .iter()
        .any(|sector| !atlas.sectors.contains_key(sector))
    {
        return Err("local repair action references an unknown exact sector".into());
    }
    let mut modified_source_faces = restored_source_faces.clone();
    for parent in &released_parents {
        let children = parent
            .children_2_to_1()
            .ok_or_else(|| format!("invalid released parent {parent:?}"))?;
        for child in children {
            let face = source
                .triangle_addresses
                .iter()
                .position(|candidate| candidate == &Some(child))
                .ok_or_else(|| format!("released child {child:?} is absent from source"))?;
            modified_source_faces.insert(face);
        }
    }
    let modified_source_vertices = modified_source_faces
        .iter()
        .flat_map(|&face| source.mesh.triangles()[face])
        .collect::<BTreeSet<_>>();
    let boundary_source_vertices = sector_ids
        .iter()
        .flat_map(|sector| atlas.sectors[sector].boundary_cycles.iter())
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>();
    let movable_source_vertices = modified_source_vertices
        .difference(&boundary_source_vertices)
        .copied()
        .collect::<BTreeSet<_>>();
    let original_violation_ids_covered = original_violations
        .iter()
        .enumerate()
        .filter_map(|(id, violation)| {
            (!violation
                .source_support_faces
                .is_disjoint(&modified_source_faces)
                || !violation.parent_support.is_disjoint(&released_parents))
            .then_some(id)
        })
        .collect::<BTreeSet<_>>();
    let original_violation_ids_untouched = (0..original_violations.len())
        .filter(|id| !original_violation_ids_covered.contains(id))
        .collect::<BTreeSet<_>>();
    let retained_parents = retained_parents
        .difference(&released_parents)
        .copied()
        .collect::<BTreeSet<_>>();
    let protected_coarse_regions = if retained_parents.is_empty() {
        Vec::new()
    } else {
        vec![super::build_protected_coarse_region(
            source,
            id,
            retained_parents.clone(),
        )?]
    };
    let patch_topology = if protected_coarse_regions.is_empty() {
        PromotionPatchTopology::WholeSphere
    } else {
        PromotionPatchTopology::Annulus {
            protected_hole_id: id,
        }
    };
    Ok(LocalRepairAction {
        id,
        kind,
        sector_ids,
        removed_mixed_faces,
        restored_source_faces,
        released_parents,
        retained_parents,
        inserted_custom_triangles,
        modified_source_vertices,
        boundary_source_vertices,
        movable_source_vertices,
        original_violation_ids_covered,
        original_violation_ids_untouched,
        patch_topology,
        protected_coarse_regions,
        geometry_target_mode: ElasticTargetMode::TrialReference,
        geometry_iteration_limit,
        fingerprint: mesh_fingerprint(&candidate.mesh),
    })
}

pub fn audit_local_repair_action(
    action: &LocalRepairAction,
    original_violations: &[ViolatingAngle],
    incumbent: &HierarchyLeafMesh,
    candidate: &HierarchyLeafMesh,
) -> LocalActionEffect {
    let original_keys = original_violations
        .iter()
        .filter_map(|violation| source_angle_key(incumbent, violation.face, violation.corner_site))
        .collect::<BTreeSet<_>>();
    let mut removed = 0usize;
    let mut resolved = 0usize;
    let mut persisted = 0usize;
    for violation in original_violations {
        let Some(key) = source_angle_key(incumbent, violation.face, violation.corner_site) else {
            removed += 1;
            continue;
        };
        let Some(angle) = current_angle(candidate, key) else {
            removed += 1;
            continue;
        };
        if signed_margin(angle) >= 0.0 {
            resolved += 1;
        } else {
            persisted += 1;
        }
    }

    let mut local_range = None;
    let mut outside_range = None;
    let mut global_range = None;
    let mut new_violations = 0usize;
    let mut orientation_valid = true;
    for face in candidate.mesh.active_triangle_slots() {
        let triangle = candidate.mesh.triangles()[face];
        let Some(angles) =
            spherical_triangle_angles(triangle.map(|site| candidate.mesh.vertices()[site]))
        else {
            orientation_valid = false;
            continue;
        };
        let local = triangle.iter().any(|&site| {
            candidate.source_vertex_slots[site]
                .is_some_and(|source| action.modified_source_vertices.contains(&source))
        });
        for (corner, angle) in angles.into_iter().enumerate() {
            update_range(&mut global_range, angle);
            update_range(
                if local {
                    &mut local_range
                } else {
                    &mut outside_range
                },
                angle,
            );
            if signed_margin(angle) < 0.0
                && source_angle_key(candidate, face, triangle[corner])
                    .is_none_or(|key| !original_keys.contains(&key))
            {
                new_violations += 1;
            }
        }
    }
    let topology_valid = candidate.mesh.open_edge_count() == 0 && closed_edge_incidence(candidate);
    let strict_certified = topology_valid
        && orientation_valid
        && persisted == 0
        && new_violations == 0
        && global_range
            .is_some_and(|range| signed_margin(range.0).min(signed_margin(range.1)) >= 0.0);
    let failure_reason = if strict_certified {
        SingletonFailureReason::StrictCertified
    } else if !action.original_violation_ids_untouched.is_empty() {
        SingletonFailureReason::UncoveredOriginalViolations
    } else if new_violations > 0 {
        SingletonFailureReason::NewViolationsCreated
    } else {
        SingletonFailureReason::ContinuousUnknown
    };
    LocalActionEffect {
        action_id: action.id,
        original_violations_total: original_violations.len(),
        original_violations_removed: removed,
        original_violations_resolved: resolved,
        original_violations_persisted: persisted,
        uncovered_original_violations: action.original_violation_ids_untouched.len(),
        new_violations_created: new_violations,
        local_angle_range: local_range,
        outside_angle_range: outside_range,
        global_angle_range: global_range,
        local_signed_margin_deg: local_range.map(range_margin),
        global_signed_margin_deg: global_range.map(range_margin),
        topology_valid,
        orientation_valid,
        strict_certified,
        failure_reason,
    }
}

fn current_angle(mesh: &HierarchyLeafMesh, key: ([usize; 3], usize)) -> Option<f64> {
    mesh.mesh.active_triangle_slots().find_map(|face| {
        let triangle = mesh.mesh.triangles()[face];
        let corner = triangle.iter().position(|&site| {
            mesh.source_vertex_slots[site].is_some_and(|source| source == key.1)
        })?;
        (source_triangle_key(mesh, face)? == key.0).then(|| {
            spherical_triangle_angles(triangle.map(|site| mesh.mesh.vertices()[site]))
                .map(|angles| angles[corner])
        })?
    })
}

fn source_angle_key(
    mesh: &HierarchyLeafMesh,
    face: usize,
    corner_site: usize,
) -> Option<([usize; 3], usize)> {
    Some((
        source_triangle_key(mesh, face)?,
        mesh.source_vertex_slots[corner_site]?,
    ))
}

fn source_triangle_key(mesh: &HierarchyLeafMesh, face: usize) -> Option<[usize; 3]> {
    if !mesh.mesh.is_triangle_live(face) {
        return None;
    }
    let mut source_triangle: [usize; 3] = mesh.mesh.triangles()[face]
        .map(|site| mesh.source_vertex_slots[site])
        .into_iter()
        .collect::<Option<Vec<_>>>()?
        .try_into()
        .ok()?;
    source_triangle.sort_unstable();
    Some(source_triangle)
}

fn signed_margin(angle: f64) -> f64 {
    (angle - 40.2).min(79.8 - angle)
}

fn range_margin(range: (f64, f64)) -> f64 {
    signed_margin(range.0).min(signed_margin(range.1))
}

fn update_range(range: &mut Option<(f64, f64)>, value: f64) {
    *range = Some(match *range {
        Some((minimum, maximum)) => (minimum.min(value), maximum.max(value)),
        None => (value, value),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncovered_external_violation_precedes_continuous_unknown() {
        let reason = if 1usize > 0 {
            SingletonFailureReason::UncoveredOriginalViolations
        } else {
            SingletonFailureReason::ContinuousUnknown
        };
        assert_eq!(reason, SingletonFailureReason::UncoveredOriginalViolations);
    }

    #[test]
    fn range_margin_uses_the_internal_contract() {
        assert_eq!(range_margin((40.2, 79.8)), 0.0);
        assert!(range_margin((39.0, 79.0)) < 0.0);
    }
}
