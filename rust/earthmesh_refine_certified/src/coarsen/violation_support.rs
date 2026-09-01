//! Deterministic source-hierarchy support for active angle constraints.

use super::full_polygon_reachability::effective_sector_polygons;
use super::{
    build_worst_angle_atlas, ElasticPatch, FullPolygonTopologyKey, GlobalExactSelectedEar,
    HierarchyLeafMesh, StratifiedAnnulus,
};
use crate::{mother_grid::TriangleAddress, MotherGrid};
use std::collections::{BTreeMap, BTreeSet};

const MAX_NEAR_BOUNDARY_GUARDS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AngleBoundKind {
    Lower,
    Upper,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvenancePrecision {
    ExactSourceFace,
    ExactHierarchyLeaf,
    ConservativeSector,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomTriangleProvenance {
    pub triangle: [usize; 3],
    pub sector_id: u64,
    pub face_band: Option<u8>,
    pub precision: ProvenancePrecision,
    pub covered_source_faces: BTreeSet<usize>,
    pub source_parents: BTreeSet<TriangleAddress>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViolatingAngle {
    pub face: usize,
    pub corner_site: usize,
    pub angle_deg: f64,
    pub signed_margin_deg: f64,
    pub bound: AngleBoundKind,
    pub source_support_faces: BTreeSet<usize>,
    pub parent_support: BTreeSet<TriangleAddress>,
    pub topology_sector: Option<u64>,
    pub face_band: Option<u8>,
    pub movable_vertices: Vec<usize>,
    pub fixed_vertices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViolationComponent {
    pub id: u64,
    pub angles: Vec<ViolatingAngle>,
    pub source_faces: BTreeSet<usize>,
    pub parent_faces: BTreeSet<TriangleAddress>,
    pub support_vertices: BTreeSet<usize>,
    pub active_constraint_vertices: BTreeSet<usize>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AngleEvidenceSets {
    pub strict_violations: Vec<ViolatingAngle>,
    pub near_boundary_guards: Vec<ViolatingAngle>,
    pub optimization_active: Vec<ViolatingAngle>,
    pub promotion_violation_seeds: Vec<ViolatingAngle>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SupportInflationReport {
    pub actual_violation_angles: usize,
    pub guard_angles: usize,
    pub promotion_seed_angles: usize,
    pub strict_violation_mesh_faces: usize,
    pub legacy_component_count: usize,
    pub strict_only_component_count: usize,
    pub old_source_support_faces: usize,
    pub strict_seed_source_faces: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViolationSupportAtlas {
    pub total_angles: usize,
    pub evidence_sets: AngleEvidenceSets,
    pub custom_triangle_provenance: Vec<CustomTriangleProvenance>,
    pub components: Vec<ViolationComponent>,
    pub patch_expansion_graph: BTreeMap<usize, BTreeSet<usize>>,
    pub support_inflation: SupportInflationReport,
}

pub fn build_violation_support_atlas(
    source: &MotherGrid,
    mesh: &HierarchyLeafMesh,
    patch: &ElasticPatch,
    stratified: &StratifiedAnnulus,
    topology_keys: &[FullPolygonTopologyKey],
    selected_ears: &[GlobalExactSelectedEar],
) -> Result<ViolationSupportAtlas, String> {
    build_violation_support_atlas_with_promotion_seed_margin(
        source,
        mesh,
        patch,
        stratified,
        topology_keys,
        selected_ears,
        0.0,
    )
}

pub fn build_violation_support_atlas_with_promotion_seed_margin(
    source: &MotherGrid,
    mesh: &HierarchyLeafMesh,
    patch: &ElasticPatch,
    stratified: &StratifiedAnnulus,
    topology_keys: &[FullPolygonTopologyKey],
    selected_ears: &[GlobalExactSelectedEar],
    promotion_seed_margin_deg: f64,
) -> Result<ViolationSupportAtlas, String> {
    if !promotion_seed_margin_deg.is_finite() || promotion_seed_margin_deg > 0.0 {
        return Err("promotion seed margin must be finite and non-positive".into());
    }
    let worst = build_worst_angle_atlas(
        source,
        mesh,
        patch,
        stratified,
        topology_keys,
        selected_ears,
        usize::MAX,
    )?;
    let provenance = custom_triangle_provenance(source, stratified, topology_keys)?;
    let provenance_by_sector = provenance.iter().fold(
        BTreeMap::<u64, Vec<&CustomTriangleProvenance>>::new(),
        |mut by_sector, item| {
            by_sector.entry(item.sector_id).or_default().push(item);
            by_sector
        },
    );
    let fixed = patch
        .fixed_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let movable = patch
        .movable_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let supported_angle = |angle: &super::AngleWitness| -> Result<ViolatingAngle, String> {
        let triangle = mesh.mesh.triangles()[angle.face];
        let source_triangle = triangle.map(|site| mesh.source_vertex_slots[site]);
        let mut support = angle
            .sector_id
            .and_then(|sector| provenance_by_sector.get(&sector))
            .into_iter()
            .flatten()
            .flat_map(|item| item.covered_source_faces.iter().copied())
            .collect::<BTreeSet<_>>();
        support.extend(source_faces_incident_to_vertices(
            source,
            &source_triangle.into_iter().flatten().collect(),
        ));
        if support.is_empty() {
            return Err(format!(
                "angle at face {} corner {} has no finite source support",
                angle.face, angle.corner
            ));
        }
        let parent_support = source_parents(source, &support)?;
        let face_band = angle.band_id.and_then(|band| u8::try_from(band).ok());
        Ok(ViolatingAngle {
            face: angle.face,
            corner_site: triangle[angle.corner],
            angle_deg: angle.angle_deg,
            signed_margin_deg: angle.signed_margin_deg,
            bound: if angle.angle_deg <= 60.0 {
                AngleBoundKind::Lower
            } else {
                AngleBoundKind::Upper
            },
            source_support_faces: support,
            parent_support,
            topology_sector: angle.sector_id,
            face_band,
            movable_vertices: triangle
                .into_iter()
                .filter(|site| movable.contains(site))
                .collect(),
            fixed_vertices: triangle
                .into_iter()
                .filter(|site| fixed.contains(site))
                .collect(),
        })
    };
    let strict_violations = worst
        .worst_angles
        .iter()
        .filter(|angle| angle.signed_margin_deg < 0.0)
        .map(supported_angle)
        .collect::<Result<Vec<_>, _>>()?;
    let near_boundary_guards = worst
        .worst_angles
        .iter()
        .filter(|angle| angle.signed_margin_deg >= 0.0)
        .take(MAX_NEAR_BOUNDARY_GUARDS)
        .map(supported_angle)
        .collect::<Result<Vec<_>, _>>()?;
    let promotion_violation_seeds = strict_violations
        .iter()
        .filter(|angle| {
            angle.signed_margin_deg < 0.0 && angle.signed_margin_deg <= promotion_seed_margin_deg
        })
        .cloned()
        .collect::<Vec<_>>();
    let optimization_active = strict_violations
        .iter()
        .chain(&near_boundary_guards)
        .cloned()
        .collect::<Vec<_>>();
    let graph = source_face_graph(source);
    let legacy_components = merge_violation_components(source, &optimization_active, &graph);
    let components = merge_violation_components(source, &promotion_violation_seeds, &graph);
    let support_inflation = SupportInflationReport {
        actual_violation_angles: strict_violations.len(),
        guard_angles: near_boundary_guards.len(),
        promotion_seed_angles: promotion_violation_seeds.len(),
        strict_violation_mesh_faces: strict_violations
            .iter()
            .map(|angle| angle.face)
            .collect::<BTreeSet<_>>()
            .len(),
        legacy_component_count: legacy_components.len(),
        strict_only_component_count: components.len(),
        old_source_support_faces: union_source_support(&optimization_active).len(),
        strict_seed_source_faces: union_source_support(&promotion_violation_seeds).len(),
    };
    Ok(ViolationSupportAtlas {
        total_angles: worst.total_angles,
        evidence_sets: AngleEvidenceSets {
            strict_violations,
            near_boundary_guards,
            optimization_active,
            promotion_violation_seeds,
        },
        custom_triangle_provenance: provenance,
        components,
        patch_expansion_graph: graph,
        support_inflation,
    })
}

fn union_source_support(angles: &[ViolatingAngle]) -> BTreeSet<usize> {
    angles
        .iter()
        .flat_map(|angle| angle.source_support_faces.iter().copied())
        .collect()
}

fn custom_triangle_provenance(
    source: &MotherGrid,
    stratified: &StratifiedAnnulus,
    topology_keys: &[FullPolygonTopologyKey],
) -> Result<Vec<CustomTriangleProvenance>, String> {
    let sectors = effective_sector_polygons(stratified)?
        .into_iter()
        .map(|sector| {
            (
                sector.id,
                sector.vertices.into_iter().collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let band_by_face = stratified
        .band_face_labels
        .iter()
        .map(|label| (label.face_slot, label.band_id))
        .collect::<BTreeMap<_, _>>();
    let annulus = stratified
        .coupled
        .annulus_face_slots
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut result = Vec::new();
    for key in topology_keys {
        let vertices = sectors
            .get(&key.sector_id)
            .cloned()
            .unwrap_or_else(|| key.triangles.iter().flatten().copied().collect());
        let mut sector_faces = annulus
            .iter()
            .copied()
            .filter(|&face| {
                source.mesh.triangles()[face]
                    .into_iter()
                    .any(|site| vertices.contains(&site))
            })
            .collect::<BTreeSet<_>>();
        if sector_faces.is_empty() {
            sector_faces.extend(source_faces_incident_to_vertices(source, &vertices));
        }
        let band = majority_band(&sector_faces, &band_by_face);
        let parents = source_parents(source, &sector_faces)?;
        for &triangle in &key.triangles {
            if sector_faces.is_empty() {
                return Err(format!(
                    "custom triangle {triangle:?} in sector {} has no source support",
                    key.sector_id
                ));
            }
            result.push(CustomTriangleProvenance {
                triangle,
                sector_id: key.sector_id,
                face_band: band,
                precision: ProvenancePrecision::ConservativeSector,
                covered_source_faces: sector_faces.clone(),
                source_parents: parents.clone(),
            });
        }
    }
    result.sort_by(|left, right| {
        left.sector_id
            .cmp(&right.sector_id)
            .then(left.triangle.cmp(&right.triangle))
    });
    Ok(result)
}

fn source_faces_incident_to_vertices(
    source: &MotherGrid,
    vertices: &BTreeSet<usize>,
) -> BTreeSet<usize> {
    source
        .mesh
        .active_triangle_slots()
        .filter(|&face| {
            source.mesh.triangles()[face]
                .into_iter()
                .any(|site| vertices.contains(&site))
        })
        .collect()
}

fn source_parents(
    source: &MotherGrid,
    faces: &BTreeSet<usize>,
) -> Result<BTreeSet<TriangleAddress>, String> {
    faces
        .iter()
        .map(|&face| {
            let address = source
                .triangle_addresses
                .get(face)
                .copied()
                .flatten()
                .ok_or_else(|| format!("source support face {face} has no hierarchy address"))?;
            Ok(address.parent_2_to_1().unwrap_or(address))
        })
        .collect()
}

fn majority_band(faces: &BTreeSet<usize>, band_by_face: &BTreeMap<usize, usize>) -> Option<u8> {
    let mut counts = BTreeMap::<usize, usize>::new();
    for face in faces {
        if let Some(&band) = band_by_face.get(face) {
            *counts.entry(band).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .max_by_key(|(band, count)| (*count, usize::MAX - *band))
        .and_then(|(band, _)| u8::try_from(band).ok())
}

fn source_face_graph(source: &MotherGrid) -> BTreeMap<usize, BTreeSet<usize>> {
    source
        .mesh
        .active_triangle_slots()
        .map(|face| {
            let neighbours = source.mesh.neighbours()[face]
                .into_iter()
                .filter(|&neighbour| source.mesh.is_triangle_live(neighbour))
                .collect();
            (face, neighbours)
        })
        .collect()
}

fn merge_violation_components(
    source: &MotherGrid,
    angles: &[ViolatingAngle],
    graph: &BTreeMap<usize, BTreeSet<usize>>,
) -> Vec<ViolationComponent> {
    // ponytail: O(n²) is deliberate for the bounded active set; use a spatial
    // support index only if real fixtures exceed a few thousand constraints.
    let mut groups = (0..angles.len()).collect::<Vec<_>>();
    for left in 0..angles.len() {
        for right in left + 1..angles.len() {
            if supports_overlap(&angles[left], &angles[right], graph) {
                let old = groups[right];
                let new = groups[left];
                for group in &mut groups {
                    if *group == old {
                        *group = new;
                    }
                }
            }
        }
    }
    let mut grouped = BTreeMap::<usize, Vec<ViolatingAngle>>::new();
    for (index, angle) in angles.iter().cloned().enumerate() {
        grouped.entry(groups[index]).or_default().push(angle);
    }
    grouped
        .into_values()
        .enumerate()
        .map(|(id, angles)| {
            let source_faces = angles
                .iter()
                .flat_map(|angle| angle.source_support_faces.iter().copied())
                .collect::<BTreeSet<_>>();
            let parent_faces = angles
                .iter()
                .flat_map(|angle| angle.parent_support.iter().copied())
                .collect();
            let support_vertices = source_faces
                .iter()
                .flat_map(|&face| source.mesh.triangles()[face])
                .collect();
            let active_constraint_vertices = angles
                .iter()
                .flat_map(|angle| {
                    angle
                        .movable_vertices
                        .iter()
                        .chain(&angle.fixed_vertices)
                        .copied()
                })
                .collect();
            ViolationComponent {
                id: id as u64,
                angles,
                source_faces,
                parent_faces,
                support_vertices,
                active_constraint_vertices,
            }
        })
        .collect()
}

fn supports_overlap(
    left: &ViolatingAngle,
    right: &ViolatingAngle,
    graph: &BTreeMap<usize, BTreeSet<usize>>,
) -> bool {
    if left.corner_site == right.corner_site
        || left
            .movable_vertices
            .iter()
            .any(|site| right.movable_vertices.contains(site))
    {
        return true;
    }
    let expand = |faces: &BTreeSet<usize>| {
        faces
            .iter()
            .copied()
            .chain(
                faces
                    .iter()
                    .flat_map(|face| graph.get(face).into_iter().flatten().copied()),
            )
            .collect::<BTreeSet<_>>()
    };
    !expand(&left.source_support_faces).is_disjoint(&expand(&right.source_support_faces))
}

pub fn violation_support_atlas_json(atlas: &ViolationSupportAtlas) -> String {
    let strict = atlas
        .evidence_sets
        .strict_violations
        .iter()
        .map(violating_angle_json)
        .collect::<Vec<_>>()
        .join(",");
    let guards = atlas
        .evidence_sets
        .near_boundary_guards
        .iter()
        .map(violating_angle_json)
        .collect::<Vec<_>>()
        .join(",");
    let active = atlas
        .evidence_sets
        .optimization_active
        .iter()
        .map(violating_angle_json)
        .collect::<Vec<_>>()
        .join(",");
    let promotion_seeds = atlas
        .evidence_sets
        .promotion_violation_seeds
        .iter()
        .map(violating_angle_json)
        .collect::<Vec<_>>()
        .join(",");
    let provenance = atlas
        .custom_triangle_provenance
        .iter()
        .map(provenance_json)
        .collect::<Vec<_>>()
        .join(",");
    let components = atlas
        .components
        .iter()
        .map(component_json)
        .collect::<Vec<_>>()
        .join(",");
    let graph = atlas
        .patch_expansion_graph
        .iter()
        .map(|(face, neighbours)| format!("\"{face}\":{}", usize_set_json(neighbours)))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"total_angles\":{},\"evidence_sets\":{{\"strict_violations\":[{}],\"near_boundary_guards\":[{}],\"optimization_active\":[{}],\"promotion_violation_seeds\":[{}]}},\"support_inflation\":{},\"custom_triangle_provenance\":[{}],\"components\":[{}],\"patch_expansion_graph\":{{{}}}}}",
        atlas.total_angles,
        strict,
        guards,
        active,
        promotion_seeds,
        support_inflation_json(&atlas.support_inflation),
        provenance,
        components,
        graph,
    )
}

fn violating_angle_json(angle: &ViolatingAngle) -> String {
    format!(
        "{{\"face\":{},\"corner_site\":{},\"angle_deg\":{:.12},\"signed_margin_deg\":{:.12},\"bound\":\"{:?}\",\"source_support_faces\":{},\"parent_support\":{},\"topology_sector\":{},\"face_band\":{},\"movable_vertices\":{},\"fixed_vertices\":{}}}",
        angle.face,
        angle.corner_site,
        angle.angle_deg,
        angle.signed_margin_deg,
        angle.bound,
        usize_set_json(&angle.source_support_faces),
        address_set_json(&angle.parent_support),
        angle.topology_sector.map_or_else(|| "null".into(), |value| value.to_string()),
        angle.face_band.map_or_else(|| "null".into(), |value| value.to_string()),
        usize_slice_json(&angle.movable_vertices),
        usize_slice_json(&angle.fixed_vertices),
    )
}

fn provenance_json(item: &CustomTriangleProvenance) -> String {
    format!(
        "{{\"triangle\":[{},{},{}],\"sector_id\":{},\"face_band\":{},\"precision\":\"{:?}\",\"diagnostic_only\":{},\"covered_source_faces\":{},\"source_parents\":{}}}",
        item.triangle[0],
        item.triangle[1],
        item.triangle[2],
        item.sector_id,
        item.face_band.map_or_else(|| "null".into(), |value| value.to_string()),
        item.precision,
        item.precision == ProvenancePrecision::ConservativeSector,
        usize_set_json(&item.covered_source_faces),
        address_set_json(&item.source_parents),
    )
}

fn support_inflation_json(report: &SupportInflationReport) -> String {
    format!(
        "{{\"actual_violation_angles\":{},\"guard_angles\":{},\"promotion_seed_angles\":{},\"strict_violation_mesh_faces\":{},\"legacy_component_count\":{},\"strict_only_component_count\":{},\"old_source_support_faces\":{},\"strict_seed_source_faces\":{}}}",
        report.actual_violation_angles,
        report.guard_angles,
        report.promotion_seed_angles,
        report.strict_violation_mesh_faces,
        report.legacy_component_count,
        report.strict_only_component_count,
        report.old_source_support_faces,
        report.strict_seed_source_faces,
    )
}

fn component_json(component: &ViolationComponent) -> String {
    let angles = component
        .angles
        .iter()
        .map(|angle| format!("[{},{}]", angle.face, angle.corner_site))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"id\":{},\"angles\":[{}],\"source_faces\":{},\"parent_faces\":{},\"support_vertices\":{},\"active_constraint_vertices\":{}}}",
        component.id,
        angles,
        usize_set_json(&component.source_faces),
        address_set_json(&component.parent_faces),
        usize_set_json(&component.support_vertices),
        usize_set_json(&component.active_constraint_vertices),
    )
}

fn usize_slice_json(values: &[usize]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn usize_set_json(values: &BTreeSet<usize>) -> String {
    usize_slice_json(&values.iter().copied().collect::<Vec<_>>())
}

fn address_set_json(values: &BTreeSet<TriangleAddress>) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|address| {
                format!(
                    "{{\"base_face\":{},\"i\":{},\"j\":{},\"n\":{},\"orientation\":\"{:?}\"}}",
                    address.base_face, address.i, address.j, address.n, address.orientation,
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coarsen::{
        build_stratified_annulus, n6_legacy_mixed_fixture_with_source_levels, ElasticTargetField,
        GeometryDomainId, TransitionTopologyCandidate,
    };

    fn fixture() -> (
        MotherGrid,
        HierarchyLeafMesh,
        ElasticPatch,
        StratifiedAnnulus,
        Vec<FullPolygonTopologyKey>,
    ) {
        let (source, component, _) = n6_legacy_mixed_fixture_with_source_levels().unwrap();
        let stratified = build_stratified_annulus(&source, &component).unwrap();
        let sector = effective_sector_polygons(&stratified).unwrap().remove(0);
        let triangles = (1..sector.vertices.len() - 1)
            .map(|index| {
                [
                    sector.vertices[0],
                    sector.vertices[index],
                    sector.vertices[index + 1],
                ]
            })
            .collect::<Vec<_>>();
        let keys = vec![FullPolygonTopologyKey {
            sector_id: sector.id,
            triangles,
        }];
        let mesh = HierarchyLeafMesh {
            mesh: source.mesh.clone(),
            triangle_addresses: source.triangle_addresses.clone(),
            source_vertex_slots: (0..source.mesh.vertices().len()).map(Some).collect(),
        };
        let guard_faces = source.mesh.active_triangle_slots().collect::<Vec<_>>();
        let patch = ElasticPatch {
            domain_id: GeometryDomainId::PlusTwoOrdinaryRings,
            topology: TransitionTopologyCandidate {
                component_id: component.id,
                topology_id: 0,
                core_parents: Vec::new(),
                custom_transition_triangles: BTreeMap::new(),
                source_triangles: Vec::new(),
                source_active_vertices: (2..source.mesh.vertices().len()).collect(),
                source_degree_forecast: BTreeMap::new(),
            },
            reference_positions: source.mesh.vertices().to_vec(),
            fixed_compact_vertices: Vec::new(),
            movable_compact_vertices: (2..source.mesh.vertices().len()).collect(),
            guard_faces,
            target_mode: super::super::ElasticTargetMode::TrialReference,
            target_field: ElasticTargetField::default(),
        };
        (source, mesh, patch, stratified, keys)
    }

    #[test]
    fn every_custom_triangle_has_source_support() {
        let (source, _, _, stratified, keys) = fixture();
        let provenance = custom_triangle_provenance(&source, &stratified, &keys).unwrap();
        assert!(!provenance.is_empty());
        assert!(provenance
            .iter()
            .all(|item| !item.covered_source_faces.is_empty() && !item.source_parents.is_empty()));
    }

    #[test]
    fn conservative_sector_support_is_not_exact() {
        let (source, _, _, stratified, keys) = fixture();
        let provenance = custom_triangle_provenance(&source, &stratified, &keys).unwrap();
        assert!(provenance
            .iter()
            .all(|item| item.precision == ProvenancePrecision::ConservativeSector));
    }

    #[test]
    fn violation_support_is_deterministic() {
        let (source, mesh, patch, stratified, keys) = fixture();
        let first =
            build_violation_support_atlas(&source, &mesh, &patch, &stratified, &keys, &[]).unwrap();
        let second =
            build_violation_support_atlas(&source, &mesh, &patch, &stratified, &keys, &[]).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            violation_support_atlas_json(&first),
            violation_support_atlas_json(&second)
        );
    }

    #[test]
    fn nonviolating_guard_angles_never_seed_promotion() {
        let (source, mesh, patch, stratified, keys) = fixture();
        let atlas =
            build_violation_support_atlas(&source, &mesh, &patch, &stratified, &keys, &[]).unwrap();
        let guards = atlas
            .evidence_sets
            .near_boundary_guards
            .iter()
            .map(|angle| (angle.face, angle.corner_site))
            .collect::<BTreeSet<_>>();
        assert!(atlas
            .evidence_sets
            .promotion_violation_seeds
            .iter()
            .all(|angle| !guards.contains(&(angle.face, angle.corner_site))));
        assert!(atlas
            .evidence_sets
            .promotion_violation_seeds
            .iter()
            .all(|angle| angle.signed_margin_deg < 0.0));
    }

    #[test]
    fn strict_seed_count_excludes_64_guards() {
        let (source, mesh, patch, stratified, keys) = fixture();
        let atlas =
            build_violation_support_atlas(&source, &mesh, &patch, &stratified, &keys, &[]).unwrap();
        assert_eq!(atlas.support_inflation.guard_angles, 64);
        assert_eq!(
            atlas.support_inflation.promotion_seed_angles,
            atlas.support_inflation.actual_violation_angles
        );
        assert_eq!(
            atlas.evidence_sets.optimization_active.len(),
            atlas.support_inflation.actual_violation_angles + 64
        );
    }

    #[test]
    fn support_components_merge_on_overlap() {
        let (source, mesh, patch, stratified, keys) = fixture();
        let atlas =
            build_violation_support_atlas(&source, &mesh, &patch, &stratified, &keys, &[]).unwrap();
        let angle = atlas.evidence_sets.optimization_active[0].clone();
        let components = merge_violation_components(
            &source,
            &[angle.clone(), angle],
            &atlas.patch_expansion_graph,
        );
        assert_eq!(components.len(), 1);
        assert_eq!(components[0].angles.len(), 2);
    }
}
