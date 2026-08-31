//! Exact transfer of a saved geometry witness into a strictly larger movable domain.

use super::{
    ElasticPatch, FullPolygonTopologyKey, GeometryDomainId, GeometryFailureWitness,
    HierarchyLeafMesh,
};
use crate::{certificate::spherical_triangle_angles, mother_grid::MotherGrid};
use earthmesh_mesh::CartesianPoint;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq)]
pub struct GeometryDomainWitness {
    pub fixture_fingerprint: u64,
    // A closed global candidate contains one key per stratified sector.
    pub topology_keys: Vec<FullPolygonTopologyKey>,
    pub domain_id: GeometryDomainId,
    pub source_positions: BTreeMap<usize, CartesianPoint>,
    pub signed_margin_deg: f64,
    pub global_angle_range_deg: (f64, f64),
    pub guard_angle_range_deg: (f64, f64),
    mesh: HierarchyLeafMesh,
    patch: ElasticPatch,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NestedDomainEmbeddingReport {
    pub source_domain: GeometryDomainId,
    pub target_domain: GeometryDomainId,
    pub common_vertices: usize,
    pub newly_released_vertices: usize,
    pub common_positions_bitwise_equal: bool,
    pub topology_equal: bool,
    pub source_signed_margin_deg: f64,
    pub embedded_signed_margin_deg: f64,
    pub embedded_global_angle_range_deg: (f64, f64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeometryEmbeddingError {
    InvalidWitness(String),
    DomainNotNested,
    TopologyChanged,
    MissingSourcePosition(usize),
    NewlyReleasedPositionDrift(usize),
    AngleScanFailed,
}

impl GeometryDomainWitness {
    pub fn from_failure(
        fixture_fingerprint: u64,
        topology_keys: Vec<FullPolygonTopologyKey>,
        domain_id: GeometryDomainId,
        failure: &GeometryFailureWitness,
        global_angle_range_deg: (f64, f64),
        guard_angle_range_deg: (f64, f64),
    ) -> Result<Self, GeometryEmbeddingError> {
        if topology_keys.is_empty() {
            return Err(GeometryEmbeddingError::InvalidWitness(
                "geometry witness has no full-polygon topology keys".into(),
            ));
        }
        if failure.patch.domain_id != domain_id {
            return Err(GeometryEmbeddingError::InvalidWitness(format!(
                "patch domain {} does not match witness domain {}",
                failure.patch.domain_id.as_str(),
                domain_id.as_str()
            )));
        }
        validate_angle_range(global_angle_range_deg, "global")?;
        validate_angle_range(guard_angle_range_deg, "guard")?;
        if failure.mesh.source_vertex_slots.len() != failure.mesh.mesh.vertices().len() {
            return Err(GeometryEmbeddingError::InvalidWitness(
                "source-slot map does not match witness mesh".into(),
            ));
        }
        let actual_global = angle_range(&failure.mesh)?;
        let actual_guard =
            angle_range_faces(&failure.mesh, failure.patch.guard_faces.iter().copied())?;
        if !range_bits_equal(actual_global, global_angle_range_deg)
            || !range_bits_equal(actual_guard, guard_angle_range_deg)
        {
            return Err(GeometryEmbeddingError::InvalidWitness(
                "recorded angle ranges do not match the witness mesh".into(),
            ));
        }
        let mut source_positions = BTreeMap::new();
        for (compact, source_slot) in failure.mesh.source_vertex_slots.iter().copied().enumerate() {
            let Some(source_slot) = source_slot else {
                continue;
            };
            let point = failure.mesh.mesh.vertices()[compact];
            if source_positions.insert(source_slot, point).is_some() {
                return Err(GeometryEmbeddingError::InvalidWitness(format!(
                    "source slot {source_slot} appears more than once"
                )));
            }
        }
        let signed_margin_deg =
            (global_angle_range_deg.0 - 40.2).min(79.8 - global_angle_range_deg.1);
        Ok(Self {
            fixture_fingerprint,
            topology_keys,
            domain_id,
            source_positions,
            signed_margin_deg,
            global_angle_range_deg,
            guard_angle_range_deg,
            mesh: failure.mesh.clone(),
            patch: failure.patch.clone(),
        })
    }

    pub fn expanded_patch(
        &self,
        source: &MotherGrid,
        source_levels: &[Option<usize>],
        physical_fixed_sources: &BTreeSet<usize>,
        target_domain: GeometryDomainId,
    ) -> Result<ElasticPatch, GeometryEmbeddingError> {
        self.patch
            .expanded_nested_domain(
                source,
                &self.mesh,
                source_levels,
                physical_fixed_sources,
                self.domain_id,
                target_domain,
            )
            .map_err(GeometryEmbeddingError::InvalidWitness)
    }

    pub fn mesh(&self) -> &HierarchyLeafMesh {
        &self.mesh
    }

    pub fn patch(&self) -> &ElasticPatch {
        &self.patch
    }
}

pub fn embed_geometry_witness(
    source: &GeometryDomainWitness,
    target_patch: &ElasticPatch,
) -> Result<(HierarchyLeafMesh, NestedDomainEmbeddingReport), GeometryEmbeddingError> {
    if target_patch.domain_id.expansion_rings() <= source.domain_id.expansion_rings() {
        return Err(GeometryEmbeddingError::DomainNotNested);
    }
    if !same_connectivity(&source.patch, target_patch) {
        return Err(GeometryEmbeddingError::TopologyChanged);
    }
    let source_movable = source
        .patch
        .movable_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let target_movable = target_patch
        .movable_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if !source_movable.is_subset(&target_movable) {
        return Err(GeometryEmbeddingError::DomainNotNested);
    }

    let mut embedded = source.mesh.clone();
    let newly_released = target_movable
        .difference(&source_movable)
        .copied()
        .collect::<Vec<_>>();
    for &compact in &source_movable {
        let source_slot = embedded.source_vertex_slots[compact]
            .ok_or(GeometryEmbeddingError::MissingSourcePosition(compact))?;
        let point = source
            .source_positions
            .get(&source_slot)
            .copied()
            .ok_or(GeometryEmbeddingError::MissingSourcePosition(source_slot))?;
        embedded.mesh.move_vertex(compact, point);
    }
    for &compact in &newly_released {
        let source_slot = embedded.source_vertex_slots[compact]
            .ok_or(GeometryEmbeddingError::MissingSourcePosition(compact))?;
        let current = embedded.mesh.vertices()[compact];
        let reference = target_patch.reference_positions[compact];
        if !point_bits_equal(current, reference) {
            return Err(GeometryEmbeddingError::NewlyReleasedPositionDrift(
                source_slot,
            ));
        }
        embedded.mesh.move_vertex(compact, reference);
    }

    let common_positions_bitwise_equal = source_movable.iter().all(|&compact| {
        point_bits_equal(
            source.mesh.mesh.vertices()[compact],
            embedded.mesh.vertices()[compact],
        )
    });
    let embedded_angle_range = angle_range(&embedded)?;
    let embedded_signed_margin_deg =
        (embedded_angle_range.0 - 40.2).min(79.8 - embedded_angle_range.1);
    Ok((
        embedded,
        NestedDomainEmbeddingReport {
            source_domain: source.domain_id,
            target_domain: target_patch.domain_id,
            common_vertices: source_movable.len(),
            newly_released_vertices: newly_released.len(),
            common_positions_bitwise_equal,
            topology_equal: true,
            source_signed_margin_deg: source.signed_margin_deg,
            embedded_signed_margin_deg,
            embedded_global_angle_range_deg: embedded_angle_range,
        },
    ))
}

fn validate_angle_range(range: (f64, f64), label: &str) -> Result<(), GeometryEmbeddingError> {
    if !range.0.is_finite() || !range.1.is_finite() || range.0 > range.1 {
        return Err(GeometryEmbeddingError::InvalidWitness(format!(
            "{label} angle range is invalid"
        )));
    }
    Ok(())
}

fn same_connectivity(source: &ElasticPatch, target: &ElasticPatch) -> bool {
    source.topology.component_id == target.topology.component_id
        && source.topology.topology_id == target.topology.topology_id
        && source.topology.core_parents == target.topology.core_parents
        && source.topology.custom_transition_triangles
            == target.topology.custom_transition_triangles
        && source.topology.source_triangles == target.topology.source_triangles
        && source.topology.source_degree_forecast == target.topology.source_degree_forecast
}

fn point_bits_equal(left: CartesianPoint, right: CartesianPoint) -> bool {
    left.x.to_bits() == right.x.to_bits()
        && left.y.to_bits() == right.y.to_bits()
        && left.z.to_bits() == right.z.to_bits()
}

fn angle_range(mesh: &HierarchyLeafMesh) -> Result<(f64, f64), GeometryEmbeddingError> {
    angle_range_faces(mesh, mesh.mesh.active_triangle_slots())
}

fn angle_range_faces(
    mesh: &HierarchyLeafMesh,
    faces: impl IntoIterator<Item = usize>,
) -> Result<(f64, f64), GeometryEmbeddingError> {
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for face in faces {
        let triangle = mesh.mesh.triangles()[face].map(|site| mesh.mesh.vertices()[site]);
        let angles =
            spherical_triangle_angles(triangle).ok_or(GeometryEmbeddingError::AngleScanFailed)?;
        for angle in angles {
            minimum = minimum.min(angle);
            maximum = maximum.max(angle);
        }
    }
    if minimum.is_finite() && maximum.is_finite() {
        Ok((minimum, maximum))
    } else {
        Err(GeometryEmbeddingError::AngleScanFailed)
    }
}

fn range_bits_equal(left: (f64, f64), right: (f64, f64)) -> bool {
    left.0.to_bits() == right.0.to_bits() && left.1.to_bits() == right.1.to_bits()
}

#[cfg(test)]
mod tests {
    use super::super::{ElasticTargetField, ElasticTargetMode, TransitionTopologyCandidate};
    use super::*;
    use crate::mother_grid::VertexAddress;

    fn fixture(
        physical_fixed_sources: &BTreeSet<usize>,
    ) -> (
        MotherGrid,
        GeometryDomainWitness,
        ElasticPatch,
        HierarchyLeafMesh,
    ) {
        let source = MotherGrid::generate(4).unwrap();
        let mesh = HierarchyLeafMesh {
            mesh: source.mesh.clone(),
            triangle_addresses: source.triangle_addresses.clone(),
            source_vertex_slots: (0..source.mesh.vertices().len()).map(Some).collect(),
        };
        let seed = source
            .addresses
            .iter()
            .enumerate()
            .find_map(|(slot, address)| {
                (source.mesh.is_vertex_live(slot)
                    && !matches!(address, Some(VertexAddress::IcosahedronVertex(_))))
                .then_some(slot)
            })
            .unwrap();
        let movable = BTreeSet::from([seed]);
        let guard_faces = source
            .mesh
            .active_triangle_slots()
            .filter(|&face| source.mesh.triangles()[face].contains(&seed))
            .collect::<BTreeSet<_>>();
        let fixed = guard_faces
            .iter()
            .flat_map(|&face| source.mesh.triangles()[face])
            .filter(|site| !movable.contains(site))
            .collect::<BTreeSet<_>>();
        let patch = ElasticPatch {
            domain_id: GeometryDomainId::PlusOneOrdinaryRing,
            topology: TransitionTopologyCandidate {
                component_id: 51,
                topology_id: 1,
                core_parents: Vec::new(),
                custom_transition_triangles: BTreeMap::new(),
                source_triangles: source
                    .mesh
                    .active_triangle_slots()
                    .map(|face| source.mesh.triangles()[face])
                    .collect(),
                source_active_vertices: movable.iter().chain(&fixed).copied().collect(),
                source_degree_forecast: BTreeMap::new(),
            },
            reference_positions: source.mesh.vertices().to_vec(),
            fixed_compact_vertices: fixed.into_iter().collect(),
            movable_compact_vertices: movable.into_iter().collect(),
            guard_faces: guard_faces.iter().copied().collect(),
            target_mode: ElasticTargetMode::TrialReference,
            target_field: ElasticTargetField::default(),
        };
        let failure = GeometryFailureWitness {
            mesh: mesh.clone(),
            patch,
        };
        let global = angle_range(&mesh).unwrap();
        let guard = angle_range_faces(&mesh, guard_faces.iter().copied()).unwrap();
        let witness = GeometryDomainWitness::from_failure(
            51,
            vec![FullPolygonTopologyKey {
                sector_id: 51,
                triangles: vec![source.mesh.triangles()[*guard_faces.iter().next().unwrap()]],
            }],
            GeometryDomainId::PlusOneOrdinaryRing,
            &failure,
            global,
            guard,
        )
        .unwrap();
        let target_patch = witness
            .expanded_patch(
                &source,
                &vec![None; source.mesh.vertices().len()],
                physical_fixed_sources,
                GeometryDomainId::PlusTwoOrdinaryRings,
            )
            .unwrap();
        (source, witness, target_patch, mesh)
    }

    #[test]
    fn plus_one_witness_embeds_in_plus_two() {
        let (_, witness, target, _) = fixture(&BTreeSet::new());
        let (_, report) = embed_geometry_witness(&witness, &target).unwrap();
        assert_eq!(report.source_domain, GeometryDomainId::PlusOneOrdinaryRing);
        assert_eq!(report.target_domain, GeometryDomainId::PlusTwoOrdinaryRings);
        assert!(report.newly_released_vertices > 0);
    }

    #[test]
    fn plus_one_common_coordinates_are_bitwise_equal() {
        let (_, witness, target, _) = fixture(&BTreeSet::new());
        let (embedded, report) = embed_geometry_witness(&witness, &target).unwrap();
        assert!(report.common_positions_bitwise_equal);
        for &compact in &witness.patch().movable_compact_vertices {
            assert!(point_bits_equal(
                witness.mesh().mesh.vertices()[compact],
                embedded.mesh.vertices()[compact]
            ));
        }
    }

    #[test]
    fn embedding_does_not_change_topology() {
        let (_, witness, target, _) = fixture(&BTreeSet::new());
        let (embedded, report) = embed_geometry_witness(&witness, &target).unwrap();
        assert!(report.topology_equal);
        assert_eq!(embedded.mesh.triangles(), witness.mesh().mesh.triangles());
        assert_eq!(
            embedded.source_vertex_slots,
            witness.mesh().source_vertex_slots
        );
        assert_eq!(
            embedded.triangle_addresses,
            witness.mesh().triangle_addresses
        );
    }

    #[test]
    fn embedding_preserves_angle_range() {
        let (_, witness, target, _) = fixture(&BTreeSet::new());
        let (embedded, report) = embed_geometry_witness(&witness, &target).unwrap();
        let embedded_range = angle_range(&embedded).unwrap();
        assert_eq!(
            embedded_range.0.to_bits(),
            witness.global_angle_range_deg.0.to_bits()
        );
        assert_eq!(
            embedded_range.1.to_bits(),
            witness.global_angle_range_deg.1.to_bits()
        );
        assert_eq!(
            report.embedded_signed_margin_deg.to_bits(),
            report.source_signed_margin_deg.to_bits()
        );
    }

    #[test]
    fn anchors_remain_fixed() {
        let (source, witness, target, _) = fixture(&BTreeSet::new());
        let (embedded, _) = embed_geometry_witness(&witness, &target).unwrap();
        for (slot, address) in source.addresses.iter().enumerate() {
            if matches!(address, Some(VertexAddress::IcosahedronVertex(_))) {
                assert!(!target.movable_compact_vertices.contains(&slot));
                assert!(point_bits_equal(
                    embedded.mesh.vertices()[slot],
                    target.reference_positions[slot]
                ));
            }
        }
    }

    #[test]
    fn physical_fixed_sources_remain_fixed() {
        let (source, witness, target_without_fixed, _) = fixture(&BTreeSet::new());
        let old = witness
            .patch()
            .movable_compact_vertices
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let physical = target_without_fixed
            .movable_compact_vertices
            .iter()
            .copied()
            .find(|site| !old.contains(site))
            .unwrap();
        let (_, witness, target, _) = fixture(&BTreeSet::from([physical]));
        let (embedded, _) = embed_geometry_witness(&witness, &target).unwrap();
        assert!(!target.movable_compact_vertices.contains(&physical));
        assert!(point_bits_equal(
            embedded.mesh.vertices()[physical],
            source.mesh.vertices()[physical]
        ));
    }

    #[test]
    fn new_ring_vertices_begin_at_reference_positions() {
        let (_, witness, target, _) = fixture(&BTreeSet::new());
        let (embedded, report) = embed_geometry_witness(&witness, &target).unwrap();
        let old = witness
            .patch()
            .movable_compact_vertices
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let new = target
            .movable_compact_vertices
            .iter()
            .copied()
            .filter(|site| !old.contains(site))
            .collect::<Vec<_>>();
        assert_eq!(new.len(), report.newly_released_vertices);
        for compact in new {
            assert!(point_bits_equal(
                embedded.mesh.vertices()[compact],
                target.reference_positions[compact]
            ));
        }
    }
}
