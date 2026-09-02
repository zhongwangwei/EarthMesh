//! Deterministic localization of the worst Frozen N6 angle constraints.

use super::elastic_block::{face_graph_distance_to_any, graph_distances, local_triangle_edges};
use super::{
    ElasticPatch, FullPolygonTopologyKey, GlobalExactSelectedEar, HierarchyLeafMesh,
    RingAnchorKind, StratifiedAnnulus, TraceRole,
};
use crate::{
    certificate::{spherical_triangle_angles, AngleContract, AngleContractId, AngleWindow},
    mother_grid::{MotherGrid, TriangleAddress, TriangleOrientation},
};
use earthmesh_mesh::arc_length_unit_sphere;
use earthmesh_quality::domain::{QualityPrioritySample, QualityZone};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt::Write as _,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EdgeClass {
    MotherGridEdge,
    CoarseInterface,
    FineInterface,
    SectorBoundary,
    CrossChainDiagonal,
    SameChainDiagonal,
    AnchorEarChord,
    OtherFullPolygonDiagonal,
}

impl EdgeClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MotherGridEdge => "MotherGridEdge",
            Self::CoarseInterface => "CoarseInterface",
            Self::FineInterface => "FineInterface",
            Self::SectorBoundary => "SectorBoundary",
            Self::CrossChainDiagonal => "CrossChainDiagonal",
            Self::SameChainDiagonal => "SameChainDiagonal",
            Self::AnchorEarChord => "AnchorEarChord",
            Self::OtherFullPolygonDiagonal => "OtherFullPolygonDiagonal",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AngleWitness {
    pub face: usize,
    pub corner: usize,
    pub corner_source_slot: Option<usize>,
    pub angle_deg: f64,
    pub signed_margin_deg: f64,
    pub topology_key: FullPolygonTopologyKey,
    pub sector_id: Option<u64>,
    pub band_id: Option<usize>,
    pub distance_to_shared_junction: Option<usize>,
    pub distance_to_pentagon_anchor: Option<usize>,
    pub distance_to_fixed_guard_face: Option<usize>,
    pub fixed_vertex_count: usize,
    pub edge_classes: [EdgeClass; 3],
    pub target_edge_log_errors: [f64; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AngleBlockerClassification {
    WidthDominated,
    TopologyDiagonalDominated,
    BoundaryDominated,
    DistributedSolverDominated,
}

impl AngleBlockerClassification {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WidthDominated => "WidthDominated",
            Self::TopologyDiagonalDominated => "TopologyDiagonalDominated",
            Self::BoundaryDominated => "BoundaryDominated",
            Self::DistributedSolverDominated => "DistributedSolverDominated",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorstAngleAtlas {
    pub total_angles: usize,
    pub worst_angles: Vec<AngleWitness>,
    pub adjacent_pentagon_or_junction_fraction: f64,
    pub long_full_polygon_diagonal_fraction: f64,
    pub fixed_guard_neighbourhood_fraction: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialFaceContext {
    pub quality: QualityPrioritySample,
    pub transition_owner: Option<u64>,
    pub distance_to_pentagon_anchor: Option<usize>,
    pub distance_to_seam: Option<usize>,
    pub edge_classes: [EdgeClass; 3],
}

impl Default for SpatialFaceContext {
    fn default() -> Self {
        Self {
            quality: QualityPrioritySample {
                zone: QualityZone::GlobalNeutral,
                maximum_priority: 1.0,
                mean_priority: 1.0,
                minimum_distance_to_target: f64::INFINITY,
                minimum_distance_to_boundary: f64::INFINITY,
            },
            transition_owner: None,
            distance_to_pentagon_anchor: None,
            distance_to_seam: None,
            edge_classes: [EdgeClass::MotherGridEdge; 3],
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SpatialZoneAngleMetrics {
    pub angle_count: usize,
    pub minimum_angle_degrees: Option<f64>,
    pub maximum_angle_degrees: Option<f64>,
    pub global_hard_violation_count: usize,
    pub preferred_violation_count: usize,
    pub transition_face_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialAtlasConclusion {
    NoGlobalTopologySearchRequired,
    DomainRepairRequired,
}

impl SpatialAtlasConclusion {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoGlobalTopologySearchRequired => "NoGlobalTopologySearchRequired",
            Self::DomainRepairRequired => "DomainRepairRequired",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpatialAngleWitness {
    pub face: usize,
    pub corner: usize,
    pub angle_degrees: f64,
    pub global_hard_violation: f64,
    pub preferred_violation: f64,
    pub zone: QualityZone,
    pub maximum_priority: f64,
    pub distance_to_target: f64,
    pub distance_to_boundary: f64,
    pub is_transition_face: bool,
    pub transition_owner: Option<u64>,
    pub component_id: Option<u64>,
    pub hierarchy_address: Option<TriangleAddress>,
    pub movable_vertex_count: usize,
    pub fixed_vertex_count: usize,
    pub distance_to_pentagon_anchor: Option<usize>,
    pub distance_to_seam: Option<usize>,
    pub edge_classes: [EdgeClass; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpatialAngleAtlas {
    pub contract_id: AngleContractId,
    pub global: SpatialZoneAngleMetrics,
    pub target: SpatialZoneAngleMetrics,
    pub boundary: SpatialZoneAngleMetrics,
    pub export: SpatialZoneAngleMetrics,
    pub deep_exterior: SpatialZoneAngleMetrics,
    pub global_neutral: SpatialZoneAngleMetrics,
    pub worst_angle_distance_to_target: Option<f64>,
    pub bad_angle_component_count: usize,
    pub conclusion: SpatialAtlasConclusion,
    pub witnesses: Vec<SpatialAngleWitness>,
}

pub fn build_spatial_angle_atlas(
    mesh: &HierarchyLeafMesh,
    face_context: &BTreeMap<usize, SpatialFaceContext>,
    fixed_vertices: &BTreeSet<usize>,
    contract: AngleContract,
) -> Result<SpatialAngleAtlas, String> {
    let active_faces = mesh.mesh.active_triangle_slots().collect::<Vec<_>>();
    let mut face_angles = BTreeMap::new();
    let mut bad_faces = BTreeSet::new();
    for &face in &active_faces {
        let context = face_context
            .get(&face)
            .ok_or_else(|| format!("spatial angle atlas is missing context for face {face}"))?;
        validate_spatial_context(face, context)?;
        let triangle = mesh.mesh.triangles()[face];
        let angles = spherical_triangle_angles(triangle.map(|site| mesh.mesh.vertices()[site]))
            .ok_or_else(|| format!("spatial angle atlas failed on face {face}"))?;
        if angles.into_iter().any(|angle| {
            window_violation(angle, contract.final_delivery) > 0.0
                || preferred_violation(angle, contract.preferred) > 0.0
        }) {
            bad_faces.insert(face);
        }
        face_angles.insert(face, angles);
    }
    let components = bad_face_components(&mesh.mesh, &bad_faces);
    let mut global = SpatialZoneAngleMetrics::default();
    let mut target = SpatialZoneAngleMetrics::default();
    let mut boundary = SpatialZoneAngleMetrics::default();
    let mut export = SpatialZoneAngleMetrics::default();
    let mut deep_exterior = SpatialZoneAngleMetrics::default();
    let mut global_neutral = SpatialZoneAngleMetrics::default();
    let mut witnesses = Vec::with_capacity(active_faces.len().saturating_mul(3));

    for face in active_faces {
        let context = &face_context[&face];
        if context.transition_owner.is_some() {
            global.transition_face_count += 1;
            zone_metrics_mut(
                context.quality.zone,
                &mut target,
                &mut boundary,
                &mut export,
                &mut deep_exterior,
                &mut global_neutral,
            )
            .transition_face_count += 1;
        }
        let triangle = mesh.mesh.triangles()[face];
        let fixed_vertex_count = triangle
            .into_iter()
            .filter(|site| fixed_vertices.contains(site))
            .count();
        for (corner, angle_degrees) in face_angles[&face].into_iter().enumerate() {
            let global_hard_violation = window_violation(angle_degrees, contract.final_delivery);
            let preferred_violation = preferred_violation(angle_degrees, contract.preferred);
            update_spatial_metrics(
                &mut global,
                angle_degrees,
                global_hard_violation,
                preferred_violation,
            );
            update_spatial_metrics(
                zone_metrics_mut(
                    context.quality.zone,
                    &mut target,
                    &mut boundary,
                    &mut export,
                    &mut deep_exterior,
                    &mut global_neutral,
                ),
                angle_degrees,
                global_hard_violation,
                preferred_violation,
            );
            witnesses.push(SpatialAngleWitness {
                face,
                corner,
                angle_degrees,
                global_hard_violation,
                preferred_violation,
                zone: context.quality.zone,
                maximum_priority: context.quality.maximum_priority,
                distance_to_target: context.quality.minimum_distance_to_target,
                distance_to_boundary: context.quality.minimum_distance_to_boundary,
                is_transition_face: context.transition_owner.is_some(),
                transition_owner: context.transition_owner,
                component_id: components.get(&face).copied(),
                hierarchy_address: mesh.triangle_addresses.get(face).copied().flatten(),
                movable_vertex_count: 3 - fixed_vertex_count,
                fixed_vertex_count,
                distance_to_pentagon_anchor: context.distance_to_pentagon_anchor,
                distance_to_seam: context.distance_to_seam,
                edge_classes: context.edge_classes,
            });
        }
    }

    let worst_angle_distance_to_target = witnesses
        .iter()
        .filter(|witness| witness.global_hard_violation > 0.0 || witness.preferred_violation > 0.0)
        .max_by(|left, right| {
            left.global_hard_violation
                .total_cmp(&right.global_hard_violation)
                .then_with(|| {
                    left.preferred_violation
                        .total_cmp(&right.preferred_violation)
                })
                .then_with(|| right.face.cmp(&left.face))
                .then_with(|| right.corner.cmp(&left.corner))
        })
        .map(|witness| witness.distance_to_target);
    let conclusion = if witnesses
        .iter()
        .all(|witness| witness.global_hard_violation == 0.0)
        && witnesses
            .iter()
            .filter(|witness| witness.preferred_violation > 0.0)
            .all(|witness| witness.zone == QualityZone::DeepExterior)
    {
        SpatialAtlasConclusion::NoGlobalTopologySearchRequired
    } else {
        SpatialAtlasConclusion::DomainRepairRequired
    };

    Ok(SpatialAngleAtlas {
        contract_id: contract.id,
        global,
        target,
        boundary,
        export,
        deep_exterior,
        global_neutral,
        worst_angle_distance_to_target,
        bad_angle_component_count: components.values().copied().collect::<BTreeSet<_>>().len(),
        conclusion,
        witnesses,
    })
}

pub fn spatial_angle_atlas_json(atlas: &SpatialAngleAtlas) -> String {
    let witnesses = atlas
        .witnesses
        .iter()
        .map(spatial_angle_witness_json)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema_version\":1,\"contract_id\":\"{}\",\"conclusion\":\"{}\",\"global\":{},\"target\":{},\"boundary\":{},\"export\":{},\"deep_exterior\":{},\"global_neutral\":{},\"worst_angle_distance_to_target\":{},\"bad_angle_component_count\":{},\"witnesses\":[{}]}}",
        atlas.contract_id.as_str(),
        atlas.conclusion.as_str(),
        spatial_metrics_json(&atlas.global),
        spatial_metrics_json(&atlas.target),
        spatial_metrics_json(&atlas.boundary),
        spatial_metrics_json(&atlas.export),
        spatial_metrics_json(&atlas.deep_exterior),
        spatial_metrics_json(&atlas.global_neutral),
        option_f64(atlas.worst_angle_distance_to_target),
        atlas.bad_angle_component_count,
        witnesses,
    )
}

pub(super) fn validate_spatial_context(
    face: usize,
    context: &SpatialFaceContext,
) -> Result<(), String> {
    let quality = context.quality;
    if !quality.maximum_priority.is_finite()
        || !(0.0..=1.0).contains(&quality.maximum_priority)
        || !quality.mean_priority.is_finite()
        || !(0.0..=1.0).contains(&quality.mean_priority)
        || quality.minimum_distance_to_target.is_nan()
        || quality.minimum_distance_to_target < 0.0
        || quality.minimum_distance_to_boundary.is_nan()
        || quality.minimum_distance_to_boundary < 0.0
    {
        return Err(format!(
            "spatial angle atlas has invalid quality context for face {face}"
        ));
    }
    Ok(())
}

fn window_violation(angle: f64, window: AngleWindow) -> f64 {
    (window.minimum_degrees - angle)
        .max(angle - window.maximum_degrees)
        .max(0.0)
}

fn preferred_violation(angle: f64, preferred: Option<AngleWindow>) -> f64 {
    preferred.map_or(0.0, |window| window_violation(angle, window))
}

fn update_spatial_metrics(
    metrics: &mut SpatialZoneAngleMetrics,
    angle: f64,
    global_hard_violation: f64,
    preferred_violation: f64,
) {
    metrics.angle_count += 1;
    metrics.minimum_angle_degrees = Some(
        metrics
            .minimum_angle_degrees
            .map_or(angle, |current| current.min(angle)),
    );
    metrics.maximum_angle_degrees = Some(
        metrics
            .maximum_angle_degrees
            .map_or(angle, |current| current.max(angle)),
    );
    metrics.global_hard_violation_count += usize::from(global_hard_violation > 0.0);
    metrics.preferred_violation_count += usize::from(preferred_violation > 0.0);
}

fn zone_metrics_mut<'a>(
    zone: QualityZone,
    target: &'a mut SpatialZoneAngleMetrics,
    boundary: &'a mut SpatialZoneAngleMetrics,
    export: &'a mut SpatialZoneAngleMetrics,
    deep_exterior: &'a mut SpatialZoneAngleMetrics,
    global_neutral: &'a mut SpatialZoneAngleMetrics,
) -> &'a mut SpatialZoneAngleMetrics {
    match zone {
        QualityZone::TargetCore => target,
        QualityZone::BoundaryProtection => boundary,
        QualityZone::ExportCorridor => export,
        QualityZone::DeepExterior => deep_exterior,
        QualityZone::GlobalNeutral => global_neutral,
    }
}

fn bad_face_components(
    mesh: &earthmesh_mesh::MeshState,
    bad_faces: &BTreeSet<usize>,
) -> BTreeMap<usize, u64> {
    let mut components = BTreeMap::new();
    let mut next_component = 0_u64;
    for &start in bad_faces {
        if components.contains_key(&start) {
            continue;
        }
        let mut queue = VecDeque::from([start]);
        components.insert(start, next_component);
        while let Some(face) = queue.pop_front() {
            for &neighbor in &mesh.neighbours()[face] {
                if bad_faces.contains(&neighbor) && !components.contains_key(&neighbor) {
                    components.insert(neighbor, next_component);
                    queue.push_back(neighbor);
                }
            }
        }
        next_component = next_component.saturating_add(1);
    }
    components
}

fn spatial_metrics_json(metrics: &SpatialZoneAngleMetrics) -> String {
    format!(
        "{{\"angle_count\":{},\"minimum_angle_degrees\":{},\"maximum_angle_degrees\":{},\"global_hard_violation_count\":{},\"preferred_violation_count\":{},\"transition_face_count\":{}}}",
        metrics.angle_count,
        option_f64(metrics.minimum_angle_degrees),
        option_f64(metrics.maximum_angle_degrees),
        metrics.global_hard_violation_count,
        metrics.preferred_violation_count,
        metrics.transition_face_count,
    )
}

fn spatial_angle_witness_json(witness: &SpatialAngleWitness) -> String {
    let edge_classes = witness
        .edge_classes
        .iter()
        .map(|class| format!("\"{}\"", class.as_str()))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"face\":{},\"corner\":{},\"angle_degrees\":{:.12},\"global_hard_violation\":{:.12},\"preferred_violation\":{:.12},\"zone\":\"{}\",\"maximum_priority\":{:.12},\"distance_to_target\":{},\"distance_to_boundary\":{},\"is_transition_face\":{},\"transition_owner\":{},\"component_id\":{},\"hierarchy_address\":{},\"movable_vertex_count\":{},\"fixed_vertex_count\":{},\"distance_to_pentagon_anchor\":{},\"distance_to_seam\":{},\"edge_classes\":[{}]}}",
        witness.face,
        witness.corner,
        witness.angle_degrees,
        witness.global_hard_violation,
        witness.preferred_violation,
        witness.zone.as_str(),
        witness.maximum_priority,
        finite_f64(witness.distance_to_target),
        finite_f64(witness.distance_to_boundary),
        witness.is_transition_face,
        option_u64(witness.transition_owner),
        option_u64(witness.component_id),
        triangle_address_json(witness.hierarchy_address),
        witness.movable_vertex_count,
        witness.fixed_vertex_count,
        option_usize(witness.distance_to_pentagon_anchor),
        option_usize(witness.distance_to_seam),
        edge_classes,
    )
}

fn option_f64(value: Option<f64>) -> String {
    value.map_or_else(|| "null".into(), finite_f64)
}

fn finite_f64(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.12}")
    } else {
        "null".into()
    }
}

fn triangle_address_json(address: Option<TriangleAddress>) -> String {
    address.map_or_else(
        || "null".into(),
        |address| {
            format!(
                "{{\"base_face\":{},\"i\":{},\"j\":{},\"n\":{},\"orientation\":\"{}\"}}",
                address.base_face,
                address.i,
                address.j,
                address.n,
                match address.orientation {
                    TriangleOrientation::Up => "up",
                    TriangleOrientation::Down => "down",
                }
            )
        },
    )
}

pub fn build_worst_angle_atlas(
    source: &MotherGrid,
    mesh: &HierarchyLeafMesh,
    patch: &ElasticPatch,
    stratified: &StratifiedAnnulus,
    topology_keys: &[FullPolygonTopologyKey],
    selected_ears: &[GlobalExactSelectedEar],
    limit: usize,
) -> Result<WorstAngleAtlas, String> {
    if topology_keys.is_empty() {
        return Err("angle atlas requires at least one topology key".into());
    }
    if mesh.source_vertex_slots.len() != mesh.mesh.vertices().len() {
        return Err("angle atlas source-slot map does not match mesh vertices".into());
    }

    let source_to_compact = mesh
        .source_vertex_slots
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(compact, source)| source.map(|source| (source, compact)))
        .collect::<BTreeMap<_, _>>();
    let guard_edges = mesh
        .mesh
        .active_triangle_slots()
        .flat_map(|face| local_triangle_edges(mesh.mesh.triangles()[face]))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let shared_sources = stratified
        .shared_junctions
        .iter()
        .map(|junction| junction.source_slot)
        .collect::<BTreeSet<_>>();
    let pentagon_sources = stratified
        .link_contracts
        .iter()
        .filter_map(|(&source, contract)| {
            matches!(
                contract.anchor_kind,
                RingAnchorKind::IcosahedronPentagon { .. }
            )
            .then_some(source)
        })
        .collect::<BTreeSet<_>>();
    let shared_compact = compact_seeds(&shared_sources, &source_to_compact);
    let pentagon_compact = compact_seeds(&pentagon_sources, &source_to_compact);
    let shared_distances = graph_distances(&guard_edges, &shared_compact);
    let pentagon_distances = graph_distances(&guard_edges, &pentagon_compact);
    let fixed = patch
        .fixed_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let fixed_guard_faces = patch
        .guard_faces
        .iter()
        .copied()
        .filter(|&face| {
            mesh.mesh.triangles()[face]
                .iter()
                .any(|site| fixed.contains(site))
        })
        .collect::<BTreeSet<_>>();
    let topology = TopologyIndex::new(stratified, topology_keys, selected_ears);
    let band_vertices = band_vertex_sets(source, stratified);
    let mut witnesses = Vec::new();

    for face in mesh.mesh.active_triangle_slots() {
        let triangle = mesh.mesh.triangles()[face];
        let points = triangle.map(|site| mesh.mesh.vertices()[site]);
        let angles = spherical_triangle_angles(points)
            .ok_or_else(|| format!("angle atlas failed on face {face}"))?;
        let source_triangle = triangle.map(|site| mesh.source_vertex_slots[site]);
        let owner = topology.owner(source_triangle);
        let edge_classes = topology.edge_classes(source_triangle);
        let target_edge_log_errors = target_edge_log_errors(mesh, patch, triangle);
        let band_id = best_band(source_triangle, &band_vertices);
        let fixed_vertex_count = triangle.iter().filter(|site| fixed.contains(site)).count();
        let distance_to_shared_junction = minimum_triangle_distance(triangle, &shared_distances);
        let distance_to_pentagon_anchor = minimum_triangle_distance(triangle, &pentagon_distances);
        let distance_to_fixed_guard_face =
            face_graph_distance_to_any(&mesh.mesh, face, &fixed_guard_faces);
        for (corner, angle_deg) in angles.into_iter().enumerate() {
            witnesses.push(AngleWitness {
                face,
                corner,
                corner_source_slot: source_triangle[corner],
                angle_deg,
                signed_margin_deg: signed_margin(angle_deg),
                topology_key: topology_keys[owner].clone(),
                sector_id: topology.exact_sector(source_triangle),
                band_id,
                distance_to_shared_junction,
                distance_to_pentagon_anchor,
                distance_to_fixed_guard_face,
                fixed_vertex_count,
                edge_classes,
                target_edge_log_errors,
            });
        }
    }
    witnesses.sort_by(|left, right| {
        left.signed_margin_deg
            .total_cmp(&right.signed_margin_deg)
            .then_with(|| left.face.cmp(&right.face))
            .then_with(|| left.corner.cmp(&right.corner))
    });
    let total_angles = witnesses.len();
    witnesses.truncate(limit.min(total_angles));
    let denominator = witnesses.len();
    let fraction = |count| {
        if denominator == 0 {
            0.0
        } else {
            count as f64 / denominator as f64
        }
    };
    let adjacent_pentagon_or_junction_fraction = fraction(
        witnesses
            .iter()
            .filter(|angle| {
                angle
                    .distance_to_shared_junction
                    .is_some_and(|distance| distance <= 1)
                    || angle
                        .distance_to_pentagon_anchor
                        .is_some_and(|distance| distance <= 1)
            })
            .count(),
    );
    let long_threshold = 1.5248_f64.ln();
    let long_full_polygon_diagonal_fraction = fraction(
        witnesses
            .iter()
            .filter(|angle| {
                angle
                    .edge_classes
                    .iter()
                    .zip(angle.target_edge_log_errors)
                    .any(|(class, error)| {
                        matches!(
                            class,
                            EdgeClass::SameChainDiagonal | EdgeClass::OtherFullPolygonDiagonal
                        ) && error > long_threshold
                    })
            })
            .count(),
    );
    let fixed_guard_neighbourhood_fraction = fraction(
        witnesses
            .iter()
            .filter(|angle| {
                angle.fixed_vertex_count > 0
                    || angle
                        .distance_to_fixed_guard_face
                        .is_some_and(|distance| distance <= 1)
            })
            .count(),
    );
    Ok(WorstAngleAtlas {
        total_angles,
        worst_angles: witnesses,
        adjacent_pentagon_or_junction_fraction,
        long_full_polygon_diagonal_fraction,
        fixed_guard_neighbourhood_fraction,
    })
}

pub fn classify_angle_blockers(
    atlas: &WorstAngleAtlas,
    worst_angle_near_pinch_fraction: f64,
) -> AngleBlockerClassification {
    if worst_angle_near_pinch_fraction >= 0.6 || atlas.adjacent_pentagon_or_junction_fraction >= 0.6
    {
        AngleBlockerClassification::WidthDominated
    } else if atlas.long_full_polygon_diagonal_fraction >= 0.6 {
        AngleBlockerClassification::TopologyDiagonalDominated
    } else if atlas.fixed_guard_neighbourhood_fraction >= 0.6 {
        AngleBlockerClassification::BoundaryDominated
    } else {
        AngleBlockerClassification::DistributedSolverDominated
    }
}

pub fn worst_angle_atlas_json(atlas: &WorstAngleAtlas) -> String {
    let worst = atlas
        .worst_angles
        .iter()
        .map(angle_witness_json)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"total_angles\":{},\"reported_angles\":{},\"adjacent_pentagon_or_junction_fraction\":{:.12},\"long_full_polygon_diagonal_fraction\":{:.12},\"fixed_guard_neighbourhood_fraction\":{:.12},\"worst_angles\":[{}]}}",
        atlas.total_angles,
        atlas.worst_angles.len(),
        atlas.adjacent_pentagon_or_junction_fraction,
        atlas.long_full_polygon_diagonal_fraction,
        atlas.fixed_guard_neighbourhood_fraction,
        worst,
    )
}

fn angle_witness_json(angle: &AngleWitness) -> String {
    let edge_classes = angle
        .edge_classes
        .iter()
        .map(|class| format!("\"{}\"", class.as_str()))
        .collect::<Vec<_>>()
        .join(",");
    let mut json = String::new();
    write!(
        json,
        "{{\"face\":{},\"corner\":{},\"corner_source_slot\":{},\"angle_deg\":{:.12},\"signed_margin_deg\":{:.12},\"topology_key\":{},\"sector_id\":{},\"band_id\":{},\"distance_to_shared_junction\":{},\"distance_to_pentagon_anchor\":{},\"distance_to_fixed_guard_face\":{},\"fixed_vertex_count\":{},\"edge_classes\":[{}],\"target_edge_log_errors\":[{:.12},{:.12},{:.12}]}}",
        angle.face,
        angle.corner,
        option_usize(angle.corner_source_slot),
        angle.angle_deg,
        angle.signed_margin_deg,
        topology_key_json(&angle.topology_key),
        option_u64(angle.sector_id),
        option_usize(angle.band_id),
        option_usize(angle.distance_to_shared_junction),
        option_usize(angle.distance_to_pentagon_anchor),
        option_usize(angle.distance_to_fixed_guard_face),
        angle.fixed_vertex_count,
        edge_classes,
        angle.target_edge_log_errors[0],
        angle.target_edge_log_errors[1],
        angle.target_edge_log_errors[2],
    )
    .unwrap();
    json
}

struct TopologyIndex {
    vertex_sets: Vec<BTreeSet<usize>>,
    exact_triangles: BTreeMap<[usize; 3], usize>,
    boundary_edges: BTreeSet<(usize, usize)>,
    topology_edges: BTreeSet<(usize, usize)>,
    coarse_edges: BTreeSet<(usize, usize)>,
    fine_edges: BTreeSet<(usize, usize)>,
    trace_memberships: BTreeMap<usize, BTreeSet<usize>>,
    ear_chords: BTreeSet<(usize, usize)>,
    sector_ids: Vec<u64>,
}

impl TopologyIndex {
    fn new(
        stratified: &StratifiedAnnulus,
        topology_keys: &[FullPolygonTopologyKey],
        selected_ears: &[GlobalExactSelectedEar],
    ) -> Self {
        let mut exact_triangles = BTreeMap::new();
        let mut boundary_edges = BTreeSet::new();
        let mut topology_edges = BTreeSet::new();
        let mut vertex_sets = Vec::new();
        let mut sector_ids = Vec::new();
        for (index, key) in topology_keys.iter().enumerate() {
            let mut counts = BTreeMap::<(usize, usize), usize>::new();
            let mut vertices = BTreeSet::new();
            for triangle in &key.triangles {
                let triangle = canonical_triangle(*triangle);
                exact_triangles.insert(triangle, index);
                vertices.extend(triangle);
                for edge in source_edges(triangle) {
                    *counts.entry(edge).or_default() += 1;
                    topology_edges.insert(edge);
                }
            }
            boundary_edges.extend(
                counts
                    .into_iter()
                    .filter_map(|(edge, count)| (count == 1).then_some(edge)),
            );
            vertex_sets.push(vertices);
            sector_ids.push(key.sector_id);
        }
        let mut coarse_edges = BTreeSet::new();
        let mut fine_edges = BTreeSet::new();
        let mut trace_memberships = BTreeMap::<usize, BTreeSet<usize>>::new();
        for trace in &stratified.traces {
            for occurrence in &trace.occurrences {
                trace_memberships
                    .entry(occurrence.source_slot)
                    .or_default()
                    .insert(trace.trace_id);
            }
            let out = match trace.role {
                TraceRole::CoarseInterface => &mut coarse_edges,
                TraceRole::FineInterface => &mut fine_edges,
                TraceRole::Intermediate => continue,
            };
            out.extend(
                trace
                    .directed_edges
                    .iter()
                    .map(|edge| source_edge(edge.from, edge.to)),
            );
        }
        Self {
            vertex_sets,
            exact_triangles,
            boundary_edges,
            topology_edges,
            coarse_edges,
            fine_edges,
            trace_memberships,
            ear_chords: selected_ears
                .iter()
                .map(|ear| source_edge(ear.inserted_chord.0, ear.inserted_chord.1))
                .collect(),
            sector_ids,
        }
    }

    fn owner(&self, triangle: [Option<usize>; 3]) -> usize {
        if let Some(triangle) = complete_triangle(triangle) {
            if let Some(&owner) = self.exact_triangles.get(&canonical_triangle(triangle)) {
                return owner;
            }
            return self
                .vertex_sets
                .iter()
                .enumerate()
                .max_by_key(|(index, vertices)| {
                    (
                        triangle
                            .iter()
                            .filter(|site| vertices.contains(site))
                            .count(),
                        usize::MAX - index,
                    )
                })
                .map(|(index, _)| index)
                .unwrap_or(0);
        }
        0
    }

    fn exact_sector(&self, triangle: [Option<usize>; 3]) -> Option<u64> {
        complete_triangle(triangle)
            .and_then(|triangle| self.exact_triangles.get(&canonical_triangle(triangle)))
            .map(|&owner| self.sector_ids[owner])
    }

    fn edge_classes(&self, triangle: [Option<usize>; 3]) -> [EdgeClass; 3] {
        let Some(triangle) = complete_triangle(triangle) else {
            return [EdgeClass::MotherGridEdge; 3];
        };
        source_edges(triangle).map(|edge| self.edge_class(edge))
    }

    fn edge_class(&self, edge: (usize, usize)) -> EdgeClass {
        if self.coarse_edges.contains(&edge) {
            EdgeClass::CoarseInterface
        } else if self.fine_edges.contains(&edge) {
            EdgeClass::FineInterface
        } else if self.ear_chords.contains(&edge) {
            EdgeClass::AnchorEarChord
        } else if self.boundary_edges.contains(&edge) {
            EdgeClass::SectorBoundary
        } else if self.topology_edges.contains(&edge) {
            let left = self.trace_memberships.get(&edge.0);
            let right = self.trace_memberships.get(&edge.1);
            if left.is_some_and(|left| right.is_some_and(|right| !left.is_disjoint(right))) {
                EdgeClass::SameChainDiagonal
            } else if left.is_some() && right.is_some() {
                EdgeClass::CrossChainDiagonal
            } else {
                EdgeClass::OtherFullPolygonDiagonal
            }
        } else {
            EdgeClass::MotherGridEdge
        }
    }
}

fn target_edge_log_errors(
    mesh: &HierarchyLeafMesh,
    patch: &ElasticPatch,
    triangle: [usize; 3],
) -> [f64; 3] {
    local_triangle_edges(triangle).map(|edge| {
        let actual =
            arc_length_unit_sphere(mesh.mesh.vertices()[edge.0], mesh.mesh.vertices()[edge.1]);
        let target = patch
            .target_field
            .target_edge_lengths
            .get(&edge)
            .copied()
            .unwrap_or_else(|| {
                arc_length_unit_sphere(
                    patch.reference_positions[edge.0],
                    patch.reference_positions[edge.1],
                )
            });
        if actual > 0.0 && target > 0.0 {
            (actual / target).ln()
        } else {
            0.0
        }
    })
}

fn band_vertex_sets(
    source: &MotherGrid,
    stratified: &StratifiedAnnulus,
) -> BTreeMap<usize, BTreeSet<usize>> {
    let mut sets = BTreeMap::<usize, BTreeSet<usize>>::new();
    for label in &stratified.band_face_labels {
        sets.entry(label.band_id)
            .or_default()
            .extend(source.mesh.triangles()[label.face_slot]);
    }
    sets
}

fn best_band(
    triangle: [Option<usize>; 3],
    band_vertices: &BTreeMap<usize, BTreeSet<usize>>,
) -> Option<usize> {
    let triangle = complete_triangle(triangle)?;
    band_vertices
        .iter()
        .max_by_key(|(band, vertices)| {
            (
                triangle
                    .iter()
                    .filter(|site| vertices.contains(site))
                    .count(),
                usize::MAX - **band,
            )
        })
        .and_then(|(&band, vertices)| {
            triangle
                .iter()
                .any(|site| vertices.contains(site))
                .then_some(band)
        })
}

fn compact_seeds(
    sources: &BTreeSet<usize>,
    source_to_compact: &BTreeMap<usize, usize>,
) -> BTreeSet<usize> {
    sources
        .iter()
        .filter_map(|source| source_to_compact.get(source).copied())
        .collect()
}

fn minimum_triangle_distance(
    triangle: [usize; 3],
    distances: &BTreeMap<usize, usize>,
) -> Option<usize> {
    triangle
        .into_iter()
        .filter_map(|site| distances.get(&site).copied())
        .min()
}

fn signed_margin(angle: f64) -> f64 {
    (angle - 40.2).min(79.8 - angle)
}

fn complete_triangle(triangle: [Option<usize>; 3]) -> Option<[usize; 3]> {
    Some([triangle[0]?, triangle[1]?, triangle[2]?])
}

fn canonical_triangle(mut triangle: [usize; 3]) -> [usize; 3] {
    triangle.sort_unstable();
    triangle
}

fn source_edges([a, b, c]: [usize; 3]) -> [(usize, usize); 3] {
    [source_edge(a, b), source_edge(b, c), source_edge(c, a)]
}

fn source_edge(left: usize, right: usize) -> (usize, usize) {
    (left.min(right), left.max(right))
}

fn option_usize(value: Option<usize>) -> String {
    value.map_or_else(|| "null".into(), |value| value.to_string())
}

fn option_u64(value: Option<u64>) -> String {
    value.map_or_else(|| "null".into(), |value| value.to_string())
}

fn topology_key_json(key: &FullPolygonTopologyKey) -> String {
    let triangles = key
        .triangles
        .iter()
        .map(|triangle| format!("[{},{},{}]", triangle[0], triangle[1], triangle[2]))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"sector_id\":{},\"triangles\":[{}]}}",
        key.sector_id, triangles
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf_grid(n: usize) -> HierarchyLeafMesh {
        let grid = MotherGrid::generate(n).unwrap();
        let vertex_slots = grid.mesh.vertices().len();
        HierarchyLeafMesh {
            mesh: grid.mesh,
            triangle_addresses: grid.triangle_addresses,
            source_vertex_slots: vec![None; vertex_slots],
        }
    }

    fn spatial_context(
        mesh: &HierarchyLeafMesh,
        zone: QualityZone,
    ) -> BTreeMap<usize, SpatialFaceContext> {
        mesh.mesh
            .active_triangle_slots()
            .map(|face| {
                (
                    face,
                    SpatialFaceContext {
                        quality: QualityPrioritySample {
                            zone,
                            maximum_priority: if zone == QualityZone::DeepExterior {
                                0.0
                            } else {
                                1.0
                            },
                            mean_priority: if zone == QualityZone::DeepExterior {
                                0.0
                            } else {
                                1.0
                            },
                            minimum_distance_to_target: 4.0,
                            minimum_distance_to_boundary: 3.0,
                        },
                        ..SpatialFaceContext::default()
                    },
                )
            })
            .collect()
    }

    fn atlas(width: f64, diagonal: f64, boundary: f64) -> WorstAngleAtlas {
        WorstAngleAtlas {
            total_angles: 0,
            worst_angles: Vec::new(),
            adjacent_pentagon_or_junction_fraction: width,
            long_full_polygon_diagonal_fraction: diagonal,
            fixed_guard_neighbourhood_fraction: boundary,
        }
    }

    #[test]
    fn blocker_gate_uses_frozen_priority_and_threshold() {
        assert_eq!(
            classify_angle_blockers(&atlas(0.6, 1.0, 1.0), 0.0),
            AngleBlockerClassification::WidthDominated
        );
        assert_eq!(
            classify_angle_blockers(&atlas(0.0, 0.6, 1.0), 0.0),
            AngleBlockerClassification::TopologyDiagonalDominated
        );
        assert_eq!(
            classify_angle_blockers(&atlas(0.0, 0.0, 0.6), 0.0),
            AngleBlockerClassification::BoundaryDominated
        );
        assert_eq!(
            classify_angle_blockers(&atlas(0.59, 0.59, 0.59), 0.59),
            AngleBlockerClassification::DistributedSolverDominated
        );
    }

    #[test]
    fn empty_atlas_json_is_stable() {
        let atlas = atlas(0.0, 0.0, 0.0);
        assert_eq!(
            worst_angle_atlas_json(&atlas),
            worst_angle_atlas_json(&atlas)
        );
    }

    #[test]
    fn spatial_atlas_localizes_n6_without_global_search() {
        let mesh = leaf_grid(6);
        let fixed = BTreeSet::new();
        let mut context = spatial_context(&mesh, QualityZone::DeepExterior);
        let contract = AngleContract::for_id(AngleContractId::DomainQuality38To82V1);
        let atlas = build_spatial_angle_atlas(&mesh, &context, &fixed, contract).unwrap();

        assert!(
            (atlas.global.minimum_angle_degrees.unwrap() - 54.361673298250).abs() < 1.0e-9,
            "actual minimum {:?}",
            atlas.global.minimum_angle_degrees
        );
        assert!(
            (atlas.global.maximum_angle_degrees.unwrap() - 72.0).abs() < 1.0e-9,
            "actual maximum {:?}",
            atlas.global.maximum_angle_degrees
        );
        assert_eq!(
            atlas.conclusion,
            SpatialAtlasConclusion::NoGlobalTopologySearchRequired
        );
        assert_eq!(atlas.target.angle_count, 0);
        assert_eq!(atlas.deep_exterior.preferred_violation_count, 0);
        assert_eq!(atlas.bad_angle_component_count, 0);

        let diagnostic_contract = AngleContract {
            preferred: Some(AngleWindow {
                minimum_degrees: 60.0,
                maximum_degrees: 60.0,
            }),
            ..contract
        };
        let atlas =
            build_spatial_angle_atlas(&mesh, &context, &fixed, diagnostic_contract).unwrap();
        assert_eq!(
            atlas.conclusion,
            SpatialAtlasConclusion::NoGlobalTopologySearchRequired
        );
        assert!(atlas.deep_exterior.preferred_violation_count > 0);
        assert!(atlas.bad_angle_component_count > 0);

        let control_face = atlas
            .witnesses
            .iter()
            .find(|witness| witness.preferred_violation > 0.0)
            .unwrap()
            .face;
        let control = context.get_mut(&control_face).unwrap();
        control.quality.zone = QualityZone::ExportCorridor;
        control.quality.maximum_priority = 1.0;
        control.quality.mean_priority = 1.0;
        control.quality.minimum_distance_to_boundary = 0.0;
        let atlas =
            build_spatial_angle_atlas(&mesh, &context, &fixed, diagnostic_contract).unwrap();
        assert_eq!(
            atlas.conclusion,
            SpatialAtlasConclusion::DomainRepairRequired
        );

        let control = context.get_mut(&control_face).unwrap();
        control.quality.zone = QualityZone::TargetCore;
        control.quality.maximum_priority = 1.0;
        control.quality.mean_priority = 1.0;
        control.quality.minimum_distance_to_target = 0.0;
        control.transition_owner = Some(7);
        let atlas =
            build_spatial_angle_atlas(&mesh, &context, &fixed, diagnostic_contract).unwrap();
        assert_eq!(
            atlas.conclusion,
            SpatialAtlasConclusion::DomainRepairRequired
        );
        assert!(atlas.target.preferred_violation_count > 0);
        assert_eq!(atlas.target.transition_face_count, 1);
        assert!(atlas
            .witnesses
            .iter()
            .filter(|witness| witness.face == control_face)
            .all(|witness| witness.component_id.is_some()));

        let json = spatial_angle_atlas_json(&atlas);
        assert!(json.starts_with("{\"schema_version\":1,"));
        assert!(json.contains("\"conclusion\":\"DomainRepairRequired\""));
        assert!(json.contains("\"contract_id\":\"domain_quality_38_to_82_v1\""));
        assert!(!json.contains("NaN"));
        assert!(!json.contains("inf"));
    }
}
