//! Dirty-region DQX quality cache with exact integer delta scoring.

use super::{
    domain_quality::{DomainQualityAccumulator, DomainQualityAngle},
    DomainQualityCosts, DomainQualityEvaluation, HierarchyLeafMesh, SpatialFaceContext,
};
use crate::certificate::{spherical_triangle_angles, AngleContract, AngleContractId, AngleWindow};
use earthmesh_mesh::MeshState;
use earthmesh_quality::domain::QualityZone;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalDelaunayStatus {
    Legal,
    Illegal,
    Boundary,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FaceQualityCacheEntry {
    pub triangle_key: [usize; 3],
    pub angles_degrees: [f64; 3],
    pub global_hard_violations: [f64; 3],
    pub preferred_violations: [f64; 3],
    pub zone: QualityZone,
    pub maximum_priority: f64,
    pub mean_priority: f64,
    pub transition_owner: Option<u64>,
    pub generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VertexQualityCacheEntry {
    pub incident_faces: Vec<usize>,
    pub degree: usize,
    pub local_link_key: u64,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeQualityCacheEntry {
    pub incident_faces: [Option<usize>; 2],
    pub delaunay_status: LocalDelaunayStatus,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum QualityCacheItem {
    Face(usize),
    Vertex(usize),
    Edge([usize; 2]),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QualityDirtySet {
    pub faces: BTreeSet<usize>,
    pub vertices: BTreeSet<usize>,
    pub edges: BTreeSet<[usize; 2]>,
}

impl QualityDirtySet {
    fn contains(&self, item: &QualityCacheItem) -> bool {
        match item {
            QualityCacheItem::Face(face) => self.faces.contains(face),
            QualityCacheItem::Vertex(vertex) => self.vertices.contains(vertex),
            QualityCacheItem::Edge(edge) => self.edges.contains(edge),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QualityCacheInstrumentation {
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub stale_requeues: u64,
    pub dirty_faces_recomputed: u64,
    pub dirty_vertices_recomputed: u64,
    pub dirty_edges_recomputed: u64,
    pub full_scans: u64,
    pub work_units: u64,
    pub rollbacks: u64,
}

#[derive(Debug, Clone)]
pub struct DomainQualityCache {
    faces: BTreeMap<usize, FaceQualityCacheEntry>,
    vertices: BTreeMap<usize, VertexQualityCacheEntry>,
    edges: BTreeMap<[usize; 2], EdgeQualityCacheEntry>,
    accumulator: DomainQualityAccumulator,
    transition_faces_in_target: usize,
    transition_faces_in_boundary: usize,
    costs: DomainQualityCosts,
    generation: u64,
    stale_queue: BTreeSet<QualityCacheItem>,
    instrumentation: QualityCacheInstrumentation,
}

#[derive(Debug, Clone)]
pub struct QualityCacheSnapshot {
    faces: Vec<(usize, Option<FaceQualityCacheEntry>)>,
    vertices: Vec<(usize, Option<VertexQualityCacheEntry>)>,
    edges: Vec<([usize; 2], Option<EdgeQualityCacheEntry>)>,
    transition_faces_in_target: usize,
    transition_faces_in_boundary: usize,
    costs: DomainQualityCosts,
    generation: u64,
    stale_queue: BTreeSet<QualityCacheItem>,
    instrumentation: QualityCacheInstrumentation,
}

impl DomainQualityCache {
    pub fn build(
        mesh: &HierarchyLeafMesh,
        face_context: &BTreeMap<usize, SpatialFaceContext>,
        costs: DomainQualityCosts,
    ) -> Result<Self, String> {
        let mut cache = Self {
            faces: BTreeMap::new(),
            vertices: BTreeMap::new(),
            edges: BTreeMap::new(),
            accumulator: DomainQualityAccumulator::default(),
            transition_faces_in_target: 0,
            transition_faces_in_boundary: 0,
            costs,
            generation: 1,
            stale_queue: BTreeSet::new(),
            instrumentation: QualityCacheInstrumentation {
                full_scans: 1,
                ..QualityCacheInstrumentation::default()
            },
        };

        let mut incident_faces = BTreeMap::<usize, Vec<usize>>::new();
        let mut edge_seeds = BTreeMap::<[usize; 2], usize>::new();
        for face in mesh.mesh.active_triangle_slots() {
            let context = face_context
                .get(&face)
                .ok_or_else(|| format!("quality cache is missing context for face {face}"))?;
            let entry = build_face_entry(&mesh.mesh, face, context)?;
            cache.add_face_entry(&entry)?;
            for vertex in entry.triangle_key {
                incident_faces.entry(vertex).or_default().push(face);
            }
            for edge in triangle_edges(entry.triangle_key) {
                edge_seeds.entry(edge).or_insert(face);
            }
            cache.faces.insert(face, entry);
        }
        for (&vertex, faces) in &mut incident_faces {
            faces.sort_unstable();
            cache.vertices.insert(
                vertex,
                build_vertex_entry(&mesh.mesh, vertex, faces.clone())?,
            );
        }
        for (edge, seed) in edge_seeds {
            cache
                .edges
                .insert(edge, build_edge_entry(&mesh.mesh, edge, seed)?);
        }
        cache.instrumentation.cache_misses =
            (cache.faces.len() + cache.vertices.len() + cache.edges.len()) as u64;
        cache.instrumentation.work_units = cache.instrumentation.cache_misses;
        cache.evaluation()?;
        Ok(cache)
    }

    pub fn evaluation(&self) -> Result<DomainQualityEvaluation, String> {
        self.accumulator.evaluation(
            self.costs,
            self.faces.len(),
            self.transition_faces_in_target,
            self.transition_faces_in_boundary,
        )
    }

    pub fn face_entries(&self) -> &BTreeMap<usize, FaceQualityCacheEntry> {
        &self.faces
    }

    pub fn vertex_entries(&self) -> &BTreeMap<usize, VertexQualityCacheEntry> {
        &self.vertices
    }

    pub fn edge_entries(&self) -> &BTreeMap<[usize; 2], EdgeQualityCacheEntry> {
        &self.edges
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn stale_len(&self) -> usize {
        self.stale_queue.len()
    }

    pub fn stale_items(&self) -> &BTreeSet<QualityCacheItem> {
        &self.stale_queue
    }

    pub fn instrumentation(&self) -> QualityCacheInstrumentation {
        self.instrumentation
    }

    pub fn face_entry(&mut self, mesh: &MeshState, face: usize) -> Option<&FaceQualityCacheEntry> {
        let fresh = self
            .faces
            .get(&face)
            .is_some_and(|entry| face_signature(mesh, face) == Some(entry.generation));
        if !fresh {
            self.instrumentation.cache_misses += 1;
            self.instrumentation.stale_requeues += 1;
            self.stale_queue.insert(QualityCacheItem::Face(face));
            return None;
        }
        self.instrumentation.cache_hits += 1;
        self.stale_queue.remove(&QualityCacheItem::Face(face));
        self.faces.get(&face)
    }

    pub fn vertex_entry(
        &mut self,
        mesh: &MeshState,
        vertex: usize,
    ) -> Option<&VertexQualityCacheEntry> {
        let fresh = self.vertices.get(&vertex).is_some_and(|entry| {
            vertex_signature(mesh, vertex, &entry.incident_faces) == Some(entry.generation)
        });
        if !fresh {
            self.instrumentation.cache_misses += 1;
            self.instrumentation.stale_requeues += 1;
            self.stale_queue.insert(QualityCacheItem::Vertex(vertex));
            return None;
        }
        self.instrumentation.cache_hits += 1;
        self.stale_queue.remove(&QualityCacheItem::Vertex(vertex));
        self.vertices.get(&vertex)
    }

    pub fn edge_entry(
        &mut self,
        mesh: &MeshState,
        edge: [usize; 2],
    ) -> Option<&EdgeQualityCacheEntry> {
        let edge = canonical_edge(edge[0], edge[1]);
        let fresh = self.edges.get(&edge).is_some_and(|entry| {
            edge_signature(mesh, edge, entry.incident_faces) == Some(entry.generation)
        });
        if !fresh {
            self.instrumentation.cache_misses += 1;
            self.instrumentation.stale_requeues += 1;
            self.stale_queue.insert(QualityCacheItem::Edge(edge));
            return None;
        }
        self.instrumentation.cache_hits += 1;
        self.stale_queue.remove(&QualityCacheItem::Edge(edge));
        self.edges.get(&edge)
    }

    pub fn dirty_for_vertex_move(&self, vertex: usize) -> Result<QualityDirtySet, String> {
        let entry = self
            .vertices
            .get(&vertex)
            .ok_or_else(|| format!("quality cache has no live vertex {vertex}"))?;
        let mut dirty = QualityDirtySet::default();
        dirty.vertices.insert(vertex);
        dirty.faces.extend(entry.incident_faces.iter().copied());
        for face in &entry.incident_faces {
            let cached = self
                .faces
                .get(face)
                .ok_or_else(|| format!("quality cache has no incident face {face}"))?;
            dirty.edges.extend(triangle_edges(cached.triangle_key));
        }
        Ok(dirty)
    }

    pub fn dirty_for_changed_faces(
        &self,
        mesh_after: &MeshState,
        changed_faces: &BTreeSet<usize>,
    ) -> Result<QualityDirtySet, String> {
        let mut dirty = QualityDirtySet {
            faces: changed_faces.clone(),
            ..QualityDirtySet::default()
        };
        for face in changed_faces {
            if let Some(entry) = self.faces.get(face) {
                dirty.vertices.extend(entry.triangle_key);
            }
            if mesh_after.is_triangle_live(*face) {
                dirty.vertices.extend(mesh_after.triangles()[*face]);
            }
        }
        let changed_vertices = dirty.vertices.clone();
        for vertex in changed_vertices {
            if let Some(entry) = self.vertices.get(&vertex) {
                dirty.faces.extend(entry.incident_faces.iter().copied());
            }
        }
        let seeds = mesh_after.sites_touching(changed_faces);
        for (vertex, seed) in seeds {
            dirty.faces.extend(
                mesh_after
                    .triangle_fan_from(vertex, seed)
                    .map_err(|error| {
                        format!("quality cache cannot walk vertex {vertex}: {error}")
                    })?,
            );
        }
        for face in &dirty.faces {
            if let Some(entry) = self.faces.get(face) {
                dirty.edges.extend(triangle_edges(entry.triangle_key));
            }
            if mesh_after.is_triangle_live(*face) {
                dirty
                    .edges
                    .extend(triangle_edges(mesh_after.triangles()[*face]));
            }
        }
        Ok(dirty)
    }

    pub fn refresh_dirty(
        &mut self,
        mesh: &HierarchyLeafMesh,
        face_context: &BTreeMap<usize, SpatialFaceContext>,
        dirty: &QualityDirtySet,
        costs: DomainQualityCosts,
    ) -> Result<QualityCacheSnapshot, String> {
        let snapshot = self.snapshot(dirty);
        if let Err(reason) = self.apply_dirty(mesh, face_context, dirty, costs) {
            self.rollback(snapshot.clone())?;
            return Err(reason);
        }
        Ok(snapshot)
    }

    pub fn rollback(&mut self, snapshot: QualityCacheSnapshot) -> Result<(), String> {
        for (face, old) in &snapshot.faces {
            if let Some(current) = self.faces.remove(face) {
                self.remove_face_entry(&current)?;
            }
            if let Some(old) = old {
                self.add_face_entry(old)?;
                self.faces.insert(*face, old.clone());
            }
        }
        restore_entries(&mut self.vertices, snapshot.vertices);
        restore_entries(&mut self.edges, snapshot.edges);
        self.transition_faces_in_target = snapshot.transition_faces_in_target;
        self.transition_faces_in_boundary = snapshot.transition_faces_in_boundary;
        self.costs = snapshot.costs;
        self.generation = snapshot.generation;
        self.stale_queue = snapshot.stale_queue;
        self.instrumentation = snapshot.instrumentation;
        self.instrumentation.rollbacks += 1;
        Ok(())
    }

    fn apply_dirty(
        &mut self,
        mesh: &HierarchyLeafMesh,
        face_context: &BTreeMap<usize, SpatialFaceContext>,
        dirty: &QualityDirtySet,
        costs: DomainQualityCosts,
    ) -> Result<(), String> {
        for face in &dirty.faces {
            if let Some(old) = self.faces.remove(face) {
                self.remove_face_entry(&old)?;
            }
            if mesh.mesh.is_triangle_live(*face) {
                let context = face_context
                    .get(face)
                    .ok_or_else(|| format!("quality cache is missing context for face {face}"))?;
                let entry = build_face_entry(&mesh.mesh, *face, context)?;
                self.add_face_entry(&entry)?;
                self.faces.insert(*face, entry);
            }
        }

        let seeds = mesh.mesh.sites_touching(&dirty.faces);
        for vertex in &dirty.vertices {
            self.vertices.remove(vertex);
            if mesh.mesh.is_vertex_live(*vertex) {
                let seed = seeds
                    .get(vertex)
                    .copied()
                    .ok_or_else(|| format!("quality cache cannot seed vertex {vertex}"))?;
                let incident_faces =
                    mesh.mesh
                        .triangle_fan_from(*vertex, seed)
                        .map_err(|error| {
                            format!("quality cache cannot walk vertex {vertex}: {error}")
                        })?;
                self.vertices.insert(
                    *vertex,
                    build_vertex_entry(&mesh.mesh, *vertex, incident_faces)?,
                );
            }
        }
        for edge in &dirty.edges {
            self.edges.remove(edge);
            if let Some(seed) = self.find_edge_seed(*edge) {
                self.edges
                    .insert(*edge, build_edge_entry(&mesh.mesh, *edge, seed)?);
            }
        }

        self.costs = costs;
        self.generation = self.generation.saturating_add(1);
        self.stale_queue.retain(|item| !dirty.contains(item));
        let face_work = dirty.faces.len() as u64;
        let vertex_work = dirty.vertices.len() as u64;
        let edge_work = dirty.edges.len() as u64;
        self.instrumentation.cache_misses += face_work + vertex_work + edge_work;
        self.instrumentation.dirty_faces_recomputed += face_work;
        self.instrumentation.dirty_vertices_recomputed += vertex_work;
        self.instrumentation.dirty_edges_recomputed += edge_work;
        self.instrumentation.work_units += face_work + vertex_work + edge_work;
        self.evaluation()?;
        Ok(())
    }

    fn snapshot(&self, dirty: &QualityDirtySet) -> QualityCacheSnapshot {
        QualityCacheSnapshot {
            faces: dirty
                .faces
                .iter()
                .map(|&key| (key, self.faces.get(&key).cloned()))
                .collect(),
            vertices: dirty
                .vertices
                .iter()
                .map(|&key| (key, self.vertices.get(&key).cloned()))
                .collect(),
            edges: dirty
                .edges
                .iter()
                .map(|&key| (key, self.edges.get(&key).copied()))
                .collect(),
            transition_faces_in_target: self.transition_faces_in_target,
            transition_faces_in_boundary: self.transition_faces_in_boundary,
            costs: self.costs,
            generation: self.generation,
            stale_queue: self.stale_queue.clone(),
            instrumentation: self.instrumentation,
        }
    }

    fn add_face_entry(&mut self, entry: &FaceQualityCacheEntry) -> Result<(), String> {
        for angle in entry.quality_angles() {
            self.accumulator.add_angle(angle)?;
        }
        self.adjust_transition(entry, true)
    }

    fn remove_face_entry(&mut self, entry: &FaceQualityCacheEntry) -> Result<(), String> {
        for angle in entry.quality_angles() {
            self.accumulator.remove_angle(angle)?;
        }
        self.adjust_transition(entry, false)
    }

    fn adjust_transition(
        &mut self,
        entry: &FaceQualityCacheEntry,
        add: bool,
    ) -> Result<(), String> {
        if entry.transition_owner.is_none() {
            return Ok(());
        }
        let target = match entry.zone {
            QualityZone::TargetCore => &mut self.transition_faces_in_target,
            QualityZone::BoundaryProtection => &mut self.transition_faces_in_boundary,
            _ => return Ok(()),
        };
        *target = if add {
            target.checked_add(1)
        } else {
            target.checked_sub(1)
        }
        .ok_or_else(|| "quality cache transition count overflow".to_string())?;
        Ok(())
    }

    fn find_edge_seed(&self, edge: [usize; 2]) -> Option<usize> {
        self.vertices.get(&edge[0]).and_then(|vertex| {
            vertex.incident_faces.iter().copied().find(|face| {
                self.faces
                    .get(face)
                    .is_some_and(|entry| entry.triangle_key.contains(&edge[1]))
            })
        })
    }
}

impl FaceQualityCacheEntry {
    fn quality_angles(&self) -> [DomainQualityAngle; 3] {
        std::array::from_fn(|corner| DomainQualityAngle {
            angle_degrees: self.angles_degrees[corner],
            global_hard_violation: self.global_hard_violations[corner],
            preferred_violation: self.preferred_violations[corner],
            zone: self.zone,
            maximum_priority: self.maximum_priority,
        })
    }
}

fn build_face_entry(
    mesh: &MeshState,
    face: usize,
    context: &SpatialFaceContext,
) -> Result<FaceQualityCacheEntry, String> {
    if !context.quality.maximum_priority.is_finite()
        || !(0.0..=1.0).contains(&context.quality.maximum_priority)
        || !context.quality.mean_priority.is_finite()
        || !(0.0..=1.0).contains(&context.quality.mean_priority)
    {
        return Err(format!("quality cache has invalid context for face {face}"));
    }
    let triangle_key = mesh.triangles()[face];
    let angles_degrees = spherical_triangle_angles(triangle_key.map(|site| mesh.vertices()[site]))
        .ok_or_else(|| format!("quality cache failed geometry for face {face}"))?;
    let contract = AngleContract::for_id(AngleContractId::DomainQuality38To82V1);
    Ok(FaceQualityCacheEntry {
        triangle_key,
        angles_degrees,
        global_hard_violations: angles_degrees
            .map(|angle| window_violation(angle, contract.final_delivery)),
        preferred_violations: angles_degrees.map(|angle| {
            contract
                .preferred
                .map_or(0.0, |window| window_violation(angle, window))
        }),
        zone: context.quality.zone,
        maximum_priority: context.quality.maximum_priority,
        mean_priority: context.quality.mean_priority,
        transition_owner: context.transition_owner,
        generation: face_signature(mesh, face)
            .ok_or_else(|| format!("quality cache face {face} is not live"))?,
    })
}

fn build_vertex_entry(
    mesh: &MeshState,
    vertex: usize,
    mut incident_faces: Vec<usize>,
) -> Result<VertexQualityCacheEntry, String> {
    incident_faces.sort_unstable();
    incident_faces.dedup();
    let generation = vertex_signature(mesh, vertex, &incident_faces)
        .ok_or_else(|| format!("quality cache vertex {vertex} is not live"))?;
    let mut neighbours = BTreeSet::new();
    for face in &incident_faces {
        neighbours.extend(
            mesh.triangles()[*face]
                .into_iter()
                .filter(|&site| site != vertex),
        );
    }
    Ok(VertexQualityCacheEntry {
        degree: incident_faces.len(),
        incident_faces,
        local_link_key: hash_values(neighbours.into_iter().map(|site| site as u64)),
        generation,
    })
}

fn build_edge_entry(
    mesh: &MeshState,
    edge: [usize; 2],
    seed: usize,
) -> Result<EdgeQualityCacheEntry, String> {
    let triangle = mesh.triangles()[seed];
    let corner = triangle
        .iter()
        .position(|site| !edge.contains(site))
        .ok_or_else(|| format!("quality cache face {seed} does not carry edge {edge:?}"))?;
    let neighbour = mesh.neighbours()[seed][corner];
    let incident_faces = if mesh.is_triangle_live(neighbour) {
        let mut faces = [seed, neighbour];
        faces.sort_unstable();
        [Some(faces[0]), Some(faces[1])]
    } else {
        [Some(seed), None]
    };
    let delaunay_status = if incident_faces[1].is_none() {
        LocalDelaunayStatus::Boundary
    } else if mesh
        .edge_is_illegal(seed, corner)
        .map_err(|error| format!("quality cache cannot classify edge {edge:?}: {error}"))?
    {
        LocalDelaunayStatus::Illegal
    } else {
        LocalDelaunayStatus::Legal
    };
    Ok(EdgeQualityCacheEntry {
        incident_faces,
        delaunay_status,
        generation: edge_signature(mesh, edge, incident_faces)
            .ok_or_else(|| format!("quality cache edge {edge:?} is not live"))?,
    })
}

fn window_violation(angle: f64, window: AngleWindow) -> f64 {
    (window.minimum_degrees - angle)
        .max(angle - window.maximum_degrees)
        .max(0.0)
}

fn triangle_edges(triangle: [usize; 3]) -> [[usize; 2]; 3] {
    [
        canonical_edge(triangle[0], triangle[1]),
        canonical_edge(triangle[1], triangle[2]),
        canonical_edge(triangle[2], triangle[0]),
    ]
}

fn canonical_edge(a: usize, b: usize) -> [usize; 2] {
    if a < b {
        [a, b]
    } else {
        [b, a]
    }
}

fn face_signature(mesh: &MeshState, face: usize) -> Option<u64> {
    let id = mesh.face_id(face)?;
    let triangle = mesh.triangles()[face];
    let mut values = vec![id.slot as u64, id.generation];
    for site in triangle {
        let vertex = mesh.vertex_id(site)?;
        let point = mesh.vertices()[site];
        values.extend([
            vertex.slot as u64,
            vertex.generation,
            point.x.to_bits(),
            point.y.to_bits(),
            point.z.to_bits(),
        ]);
    }
    Some(hash_values(values))
}

fn vertex_signature(mesh: &MeshState, vertex: usize, incident_faces: &[usize]) -> Option<u64> {
    let id = mesh.vertex_id(vertex)?;
    let point = mesh.vertices()[vertex];
    let mut values = vec![
        id.slot as u64,
        id.generation,
        point.x.to_bits(),
        point.y.to_bits(),
        point.z.to_bits(),
    ];
    for &face in incident_faces {
        let id = mesh.face_id(face)?;
        values.extend([id.slot as u64, id.generation]);
    }
    Some(hash_values(values))
}

fn edge_signature(
    mesh: &MeshState,
    edge: [usize; 2],
    incident_faces: [Option<usize>; 2],
) -> Option<u64> {
    let id = mesh.edge_id(edge[0], edge[1])?;
    let mut values = Vec::new();
    for vertex in id.vertices {
        let point = mesh.vertices()[vertex.slot];
        values.extend([
            vertex.slot as u64,
            vertex.generation,
            point.x.to_bits(),
            point.y.to_bits(),
            point.z.to_bits(),
        ]);
    }
    for face in incident_faces.into_iter().flatten() {
        let id = mesh.face_id(face)?;
        values.extend([id.slot as u64, id.generation, face_signature(mesh, face)?]);
    }
    Some(hash_values(values))
}

fn hash_values(values: impl IntoIterator<Item = u64>) -> u64 {
    values.into_iter().fold(0xcbf29ce484222325, |hash, value| {
        value.to_le_bytes().into_iter().fold(hash, |hash, byte| {
            (hash ^ byte as u64).wrapping_mul(0x100000001b3)
        })
    })
}

fn restore_entries<K: Ord, V>(map: &mut BTreeMap<K, V>, entries: Vec<(K, Option<V>)>) {
    for (key, value) in entries {
        if let Some(value) = value {
            map.insert(key, value);
        } else {
            map.remove(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coarsen::{
            build_spatial_angle_atlas, domain_quality_evaluation_from_atlas, HierarchyLeafSet,
        },
        mother_grid::MotherGrid,
    };
    use earthmesh_mesh::CartesianPoint;

    fn fixture() -> (HierarchyLeafMesh, BTreeMap<usize, SpatialFaceContext>) {
        let source = MotherGrid::generate(6).unwrap();
        let leaves = HierarchyLeafSet::from_mother_grid(&source).unwrap();
        let mesh =
            super::super::core_condensation::rebuild_from_leaf_set(&source, &leaves).unwrap();
        let mut context = BTreeMap::new();
        for face in mesh.mesh.active_triangle_slots() {
            let zone = match face % 4 {
                0 => QualityZone::TargetCore,
                1 => QualityZone::BoundaryProtection,
                2 => QualityZone::ExportCorridor,
                _ => QualityZone::DeepExterior,
            };
            let priority = match zone {
                QualityZone::TargetCore | QualityZone::BoundaryProtection => 1.0,
                QualityZone::ExportCorridor => 0.25,
                QualityZone::DeepExterior | QualityZone::GlobalNeutral => 0.0,
            };
            let mut face_context = SpatialFaceContext::default();
            face_context.quality.zone = zone;
            face_context.quality.maximum_priority = priority;
            face_context.quality.mean_priority = priority;
            face_context.transition_owner = (face % 11 == 0).then_some(face as u64);
            context.insert(face, face_context);
        }
        (mesh, context)
    }

    fn normalized_between(a: CartesianPoint, b: CartesianPoint) -> CartesianPoint {
        let x = a.x * 0.999 + b.x * 0.001;
        let y = a.y * 0.999 + b.y * 0.001;
        let z = a.z * 0.999 + b.z * 0.001;
        let norm = (x * x + y * y + z * z).sqrt();
        CartesianPoint::new(x / norm, y / norm, z / norm)
    }

    #[test]
    fn vertex_move_delta_equals_full_and_rollback_restores_cache() {
        let (mut mesh, context) = fixture();
        let mut cache =
            DomainQualityCache::build(&mesh, &context, DomainQualityCosts::default()).unwrap();
        assert_eq!(cache.face_entries().len(), mesh.mesh.triangle_count());
        assert_eq!(cache.vertex_entries().len(), mesh.mesh.vertex_count());
        assert!(!cache.edge_entries().is_empty());
        let before = cache.evaluation().unwrap();
        let full_atlas = build_spatial_angle_atlas(
            &mesh,
            &context,
            &BTreeSet::new(),
            AngleContract::for_id(AngleContractId::DomainQuality38To82V1),
        )
        .unwrap();
        assert_eq!(
            before,
            domain_quality_evaluation_from_atlas(&full_atlas, DomainQualityCosts::default())
                .unwrap()
        );
        let full_scans = cache.instrumentation().full_scans;
        let (&vertex, vertex_entry) = cache
            .vertex_entries()
            .iter()
            .find(|(_, entry)| entry.degree == 6)
            .unwrap();
        let incident_face = vertex_entry.incident_faces[0];
        let neighbour = cache.face_entries()[&incident_face]
            .triangle_key
            .into_iter()
            .find(|&site| site != vertex)
            .unwrap();
        let dirty = cache.dirty_for_vertex_move(vertex).unwrap();
        let original = mesh.clone();
        mesh.mesh.move_vertex(
            vertex,
            normalized_between(
                mesh.mesh.vertices()[vertex],
                mesh.mesh.vertices()[neighbour],
            ),
        );
        assert!(cache.face_entry(&mesh.mesh, incident_face).is_none());
        assert!(cache.vertex_entry(&mesh.mesh, vertex).is_none());
        let opposite_edge = triangle_edges(cache.face_entries()[&incident_face].triangle_key)
            .into_iter()
            .find(|edge| !edge.contains(&vertex))
            .unwrap();
        assert!(cache.edge_entry(&mesh.mesh, opposite_edge).is_none());
        assert_eq!(cache.stale_len(), 3);
        let snapshot = cache
            .refresh_dirty(&mesh, &context, &dirty, DomainQualityCosts::default())
            .unwrap();
        assert_eq!(cache.stale_len(), 0);
        let delta = cache.evaluation().unwrap();
        let full = DomainQualityCache::build(&mesh, &context, DomainQualityCosts::default())
            .unwrap()
            .evaluation()
            .unwrap();
        assert_eq!(delta, full);
        assert_eq!(cache.instrumentation().full_scans, full_scans);
        assert!(cache.instrumentation().dirty_faces_recomputed > 0);
        cache.rollback(snapshot).unwrap();
        mesh = original;
        assert_eq!(cache.evaluation().unwrap(), before);
        assert_eq!(cache.stale_len(), 3);
        assert_eq!(cache.instrumentation().rollbacks, 1);
        assert!(cache.face_entry(&mesh.mesh, incident_face).is_some());
        assert!(cache.vertex_entry(&mesh.mesh, vertex).is_some());
        assert!(cache.edge_entry(&mesh.mesh, opposite_edge).is_some());
        assert_eq!(cache.stale_len(), 0);
    }

    #[test]
    fn edge_flip_delta_equals_full_for_face_vertex_and_edge_caches() {
        let (mesh, context) = fixture();
        let mut cache =
            DomainQualityCache::build(&mesh, &context, DomainQualityCosts::default()).unwrap();
        let mut flipped = None;
        for face in mesh.mesh.active_triangle_slots() {
            for corner in 0..3 {
                let neighbour = mesh.mesh.neighbours()[face][corner];
                if !mesh.mesh.is_triangle_live(neighbour) {
                    continue;
                }
                let mut candidate = mesh.clone();
                if candidate.mesh.flip_edge(face, corner).is_ok() {
                    flipped = Some((candidate, BTreeSet::from([face, neighbour])));
                    break;
                }
            }
            if flipped.is_some() {
                break;
            }
        }
        let (candidate, changed) = flipped.expect("N6 has a flippable edge");
        let dirty = cache
            .dirty_for_changed_faces(&candidate.mesh, &changed)
            .unwrap();
        assert!(dirty.faces.len() < mesh.mesh.triangle_count());
        let snapshot = cache
            .refresh_dirty(
                &candidate,
                &context,
                &dirty,
                DomainQualityCosts {
                    topology_change_count: 1,
                    work_units: 1,
                    ..DomainQualityCosts::default()
                },
            )
            .unwrap();
        let delta = cache.evaluation().unwrap();
        let full = DomainQualityCache::build(
            &candidate,
            &context,
            DomainQualityCosts {
                topology_change_count: 1,
                work_units: 1,
                ..DomainQualityCosts::default()
            },
        )
        .unwrap()
        .evaluation()
        .unwrap();
        assert_eq!(delta, full);
        cache.rollback(snapshot).unwrap();
        assert_eq!(cache.evaluation().unwrap().vector.topology_change_count, 0);
    }
}
