use earthmesh_refine_certified::coarsen::{
    audit_embedding_transfer, build_face_band_problem,
    build_face_band_problem_with_source_face_rings, build_frozen_cldp_gate_evidence,
    build_promotion_patch, build_stratified_annulus, build_violation_support_atlas,
    coarse_core_ears, continue_nested_domain, evaluate_frozen_cldp_gate,
    frozen_n6_geometry_evidence_json_with_solver_domain, max_min_trust_step_evidence,
    n6_legacy_mixed_fixture_with_source_levels, peel_boundary_parent_for_sector,
    restore_fine_compatible_sector, restore_source_patch, search_local_topology_neighbourhood,
    solve_elastic_patch_with_max_min_trust_start, solve_exact_face_bands, solve_expanding_collar,
    solve_full_polygon_merge_free_interface_cber_with_targets_active_trust_starts_and_domain,
    solve_full_polygon_merge_from_face_bands_with_geometry_witness, solve_local_annular_collar,
    violation_support_atlas_json, AnchorBandPolicy, BoundaryParentPeelOutcome,
    DirectSectorRestoreOutcome, DomainContinuationMode, DomainContinuationOutcome,
    DomainContinuationSchedule, ElasticBlockLimits, ElasticBlockOutcome, ElasticTargetMode,
    EmbeddingAuditOutcome, FaceBandLimits, FaceBandSolveOutcome, FrozenCldpGateOutcome,
    FullPolygonCberLimits, FullPolygonMergeEvidence, FullPolygonMergeOutcome,
    FullPolygonTopologyKey, GeometryDomainId, GeometryDomainWitness, GeometryFailureWitness,
    GeometryStartId, GlobalExactSelectedEar, LocalAnnularCollarLevel, LocalAnnularCollarLimits,
    LocalAnnularCollarOutcome, LocalTopologyEvidence, LocalTopologyLimits,
    LocalTopologySearchOutcome, MaxMinTrustOutcomeKind, PromotionBudget, PromotionFailureReason,
    PromotionLevel, PromotionOutcome, PromotionPatchTopology, RecoveryAtom,
};
use std::{collections::BTreeSet, fs, process::Command};

const TASKBOOK_SHA256: &str = "46d5f8d1ab439ce972186ba50798806b520fe9bdac3f675806d4cd18cff38e2b";
const CLDP_TASKBOOK_SHA256: &str =
    "546a7b60ed8ad94e04f337bfb0c870ef9a2a40ea65545ebe2c256be967a8d722";
const SEACR_TASKBOOK_SHA256: &str =
    "5f9e16b4fb8f51935a0aebf9ce313c87ab4dc4a9761aaa53c51c98ab6d8cd6e0";
const LEGACY_MARGIN_DEG: f64 = -1.653_074_281_139_495_8;
const MATERIAL_IMPROVEMENT_DEG: f64 = 0.25;
const DEFAULT_TOPOLOGY_STATES: usize = 500;
const DEFAULT_ELASTIC_ITERATIONS: usize = 64;
const DEFAULT_W3_TOPOLOGY_STATES: usize = 150;
const STARTS: [GeometryStartId; 6] = [
    GeometryStartId::MaterializedSource,
    GeometryStartId::HierarchySpringEquilibrium,
    GeometryStartId::RingScaleInterpolation,
    GeometryStartId::DegreeAngleEquilibrium,
    GeometryStartId::SignedNormalPlus,
    GeometryStartId::SignedNormalMinus,
];

#[test]
#[ignore = "explicit finite Frozen N6 PR58 inherited PF-W2 geometry gate"]
fn frozen_n6_pr58_pfw2_geometry_probe() {
    let topology_states = usize_env("EARTHMESH_FULL_POLYGON_STATES", DEFAULT_TOPOLOGY_STATES);
    let elastic_iterations = usize_env("EARTHMESH_CBER_ITERATIONS", DEFAULT_ELASTIC_ITERATIONS);
    let (source, component, source_levels) = n6_legacy_mixed_fixture_with_source_levels().unwrap();
    let (incumbent, incumbent_range) = pr52_incumbent(&source, &component, &source_levels);
    assert_close(incumbent_range.0, 38.551_143_486_745_16);
    assert_close(incumbent_range.1, 81.453_074_281_139_5);

    let problem = build_face_band_problem(&source, &component, 2).unwrap();
    let FaceBandSolveOutcome::Closed(plan, _) = solve_exact_face_bands(
        &problem,
        FaceBandLimits {
            maximum_states: 1_000_000,
        },
    ) else {
        panic!("Frozen N6 PF-W2 face-band plan must close")
    };
    let outcome = solve_full_polygon_merge_from_face_bands_with_geometry_witness(
        &source,
        &component,
        &plan,
        &incumbent,
        &BTreeSet::new(),
        FullPolygonCberLimits {
            topology_states,
            elastic_iterations,
        },
        ElasticTargetMode::HierarchyEdgeAreaDegree,
        Some(&source_levels),
        &STARTS,
        GeometryDomainId::PlusTwoOrdinaryRings,
    );
    assert!(!matches!(
        outcome,
        FullPolygonMergeOutcome::InvalidInput { .. }
    ));
    let evidence = outcome_evidence(&outcome);
    assert!(evidence.geometry_candidates_attempted > 0);
    let best = evidence.best_geometry_failure.as_ref();
    let best_margin = best.and_then(|failure| failure.signed_margin_degrees());
    let gate = if matches!(outcome, FullPolygonMergeOutcome::Closed(_)) {
        "Certified"
    } else if best_margin
        .is_some_and(|margin| margin >= LEGACY_MARGIN_DEG + MATERIAL_IMPROVEMENT_DEG)
    {
        "MaterialImprovement"
    } else {
        "NoImprovementEnterW3"
    };
    let (common_vertices, fallback_vertices, witness) = best
        .and_then(|failure| failure.witness.as_deref())
        .map(|witness| {
            let inherited = incumbent
                .mesh
                .source_vertex_slots
                .iter()
                .flatten()
                .copied()
                .collect::<BTreeSet<_>>();
            let common = witness
                .mesh
                .source_vertex_slots
                .iter()
                .filter(|slot| slot.is_some_and(|slot| inherited.contains(&slot)))
                .count();
            (
                common,
                witness.mesh.mesh.vertices().len() - common,
                Some(witness),
            )
        })
        .unwrap_or((0, 0, None));
    if let Some(witness) = witness {
        for anchor in [2usize, 29, 77, 155] {
            let compact = witness
                .mesh
                .source_vertex_slots
                .iter()
                .position(|slot| *slot == Some(anchor))
                .expect("original anchor must remain materialized");
            assert!(witness.patch.fixed_compact_vertices.contains(&compact));
        }
    }

    let start_names = STARTS.map(GeometryStartId::as_str);
    let run = frozen_n6_geometry_evidence_json_with_solver_domain(
        &outcome,
        earthmesh_refine_certified::mesh_fingerprint(&source.mesh),
        topology_states,
        elastic_iterations,
        git_head().as_deref(),
        ElasticTargetMode::HierarchyEdgeAreaDegree,
        &start_names,
        "ActiveTangentTrust",
        GeometryDomainId::PlusTwoOrdinaryRings,
    );
    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr58PfW2Geometry\",\"taskbook_sha256\":\"{TASKBOOK_SHA256}\",\"incumbent_angle_degrees\":[{:.12},{:.12}],\"incumbent_signed_margin_deg\":{LEGACY_MARGIN_DEG:.12},\"material_improvement_threshold_deg\":{MATERIAL_IMPROVEMENT_DEG:.12},\"transferred_common_vertices\":{common_vertices},\"safe_fallback_vertices\":{fallback_vertices},\"gate\":\"{gate}\",\"run\":{run}}}",
        incumbent_range.0, incumbent_range.1,
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

#[test]
#[ignore = "explicit finite Frozen N6 PR60 W3 geometry gate"]
fn frozen_n6_pr60_w3_geometry_probe() {
    let topology_states = usize_env("EARTHMESH_FULL_POLYGON_STATES", DEFAULT_W3_TOPOLOGY_STATES);
    let elastic_iterations = usize_env("EARTHMESH_CBER_ITERATIONS", DEFAULT_ELASTIC_ITERATIONS);
    let (source, component, source_levels) = n6_legacy_mixed_fixture_with_source_levels().unwrap();
    let (plus_one, plus_two, incumbent_range) =
        pr49_and_pr52_witnesses(&source, &component, &source_levels);
    let mut problem =
        build_face_band_problem_with_source_face_rings(&source, &component, 3, 2).unwrap();
    for anchor in [2usize, 155] {
        problem
            .anchor_policies
            .insert(anchor, AnchorBandPolicy::OnSingleInterface);
    }
    let FaceBandSolveOutcome::Closed(plan, _) = solve_exact_face_bands(
        &problem,
        FaceBandLimits {
            maximum_states: 1_000_000,
        },
    ) else {
        panic!("Frozen N6 W3 face-band plan must close")
    };

    let solve = |domain_id, witness: &GeometryFailureWitness| {
        solve_full_polygon_merge_from_face_bands_with_geometry_witness(
            &source,
            &component,
            &plan,
            witness,
            &BTreeSet::new(),
            FullPolygonCberLimits {
                topology_states,
                elastic_iterations,
            },
            ElasticTargetMode::HierarchyEdgeAreaDegree,
            Some(&source_levels),
            &STARTS,
            domain_id,
        )
    };
    let plus_one_outcome = solve(GeometryDomainId::PlusOneOrdinaryRing, &plus_one);
    let plus_two_outcome = solve(GeometryDomainId::PlusTwoOrdinaryRings, &plus_two);
    for outcome in [&plus_one_outcome, &plus_two_outcome] {
        assert!(!matches!(
            outcome,
            FullPolygonMergeOutcome::InvalidInput { .. }
        ));
        assert!(outcome_evidence(outcome).geometry_candidates_attempted > 0);
    }
    let start_names = STARTS.map(GeometryStartId::as_str);
    let plus_one_run = frozen_n6_geometry_evidence_json_with_solver_domain(
        &plus_one_outcome,
        earthmesh_refine_certified::mesh_fingerprint(&source.mesh),
        topology_states,
        elastic_iterations,
        None,
        ElasticTargetMode::HierarchyEdgeAreaDegree,
        &start_names,
        "ActiveTangentTrust",
        GeometryDomainId::PlusOneOrdinaryRing,
    );
    let plus_two_run = frozen_n6_geometry_evidence_json_with_solver_domain(
        &plus_two_outcome,
        earthmesh_refine_certified::mesh_fingerprint(&source.mesh),
        topology_states,
        elastic_iterations,
        None,
        ElasticTargetMode::HierarchyEdgeAreaDegree,
        &start_names,
        "ActiveTangentTrust",
        GeometryDomainId::PlusTwoOrdinaryRings,
    );
    let gate = if matches!(plus_one_outcome, FullPolygonMergeOutcome::Closed(_))
        || matches!(plus_two_outcome, FullPolygonMergeOutcome::Closed(_))
    {
        "Certified"
    } else {
        "W3GeometryNotCertified"
    };
    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr60W3Geometry\",\"taskbook_sha256\":\"{TASKBOOK_SHA256}\",\"target_trace_scales\":\"H*2^(-k/3), k=0..3\",\"incumbent_angle_degrees\":[{:.12},{:.12}],\"gate\":\"{gate}\",\"plus_one\":{plus_one_run},\"plus_two\":{plus_two_run}}}",
        incumbent_range.0, incumbent_range.1,
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

#[test]
#[ignore = "explicit finite Frozen N6 PR61 W3 embedding audit"]
fn frozen_n6_pr61_w3_embedding_audit_probe() {
    let topology_states = usize_env("EARTHMESH_FULL_POLYGON_STATES", DEFAULT_W3_TOPOLOGY_STATES);
    let (source, component, source_levels) = n6_legacy_mixed_fixture_with_source_levels().unwrap();
    let (plus_one, plus_two, _) = pr49_and_pr52_witnesses(&source, &component, &source_levels);
    let mut problem =
        build_face_band_problem_with_source_face_rings(&source, &component, 3, 2).unwrap();
    for anchor in [2usize, 155] {
        problem
            .anchor_policies
            .insert(anchor, AnchorBandPolicy::OnSingleInterface);
    }
    let FaceBandSolveOutcome::Closed(plan, _) = solve_exact_face_bands(
        &problem,
        FaceBandLimits {
            maximum_states: 1_000_000,
        },
    ) else {
        panic!("Frozen N6 W3 face-band plan must close")
    };

    let solve = |domain_id, witness: &GeometryFailureWitness| {
        solve_full_polygon_merge_from_face_bands_with_geometry_witness(
            &source,
            &component,
            &plan,
            witness,
            &BTreeSet::new(),
            FullPolygonCberLimits {
                topology_states,
                elastic_iterations: 0,
            },
            ElasticTargetMode::HierarchyEdgeAreaDegree,
            Some(&source_levels),
            &STARTS,
            domain_id,
        )
    };
    let plus_one_outcome = solve(GeometryDomainId::PlusOneOrdinaryRing, &plus_one);
    let plus_two_outcome = solve(GeometryDomainId::PlusTwoOrdinaryRings, &plus_two);
    let plus_one_audit = embedding_audit(&plus_one, &plus_one_outcome);
    let plus_two_audit = embedding_audit(&plus_two, &plus_two_outcome);
    for audit in [&plus_one_audit, &plus_two_audit] {
        assert_eq!(
            audit.outcome(),
            EmbeddingAuditOutcome::TopologyClosedNoUsableEmbedding
        );
        assert!(
            audit.non_positive_triangles > 0
                || audit.crossing_pairs > 0
                || audit.near_degenerate_triangles > 0
                || !audit.rotation_mismatch_vertices.is_empty(),
            "W3 failure must be attributed to its transferred embedding"
        );
    }

    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr61W3EmbeddingAudit\",\"taskbook_sha256\":\"{CLDP_TASKBOOK_SHA256}\",\"outcome\":\"TopologyClosedNoUsableEmbedding\",\"w4_started\":false,\"plus_one\":{},\"plus_two\":{}}}",
        embedding_audit_json(&plus_one_audit),
        embedding_audit_json(&plus_two_audit),
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

#[test]
#[ignore = "explicit finite Frozen N6 PR62 W2 max-min gate"]
fn frozen_n6_pr62_w2_max_min_probe() {
    let iterations = usize_env("EARTHMESH_CBER_ITERATIONS", 128);
    let (source, component, source_levels) = n6_legacy_mixed_fixture_with_source_levels().unwrap();
    let (_, incumbent, incumbent_range) =
        pr49_and_pr52_witnesses(&source, &component, &source_levels);
    assert_close(incumbent_range.0, 38.551_143_486_745_16);
    assert_close(incumbent_range.1, 81.453_074_281_139_5);

    let outcome = solve_elastic_patch_with_max_min_trust_start(
        &incumbent.mesh,
        incumbent.patch.clone(),
        ElasticBlockLimits {
            elastic_iterations: iterations,
        },
        GeometryStartId::MaterializedSource,
    );
    let (range, mesh, patch, certified) = elastic_outcome_geometry(&outcome);
    let margin = (range.0 - 40.2).min(79.8 - range.1);
    assert!(
        margin + 1.0e-10 >= LEGACY_MARGIN_DEG,
        "max-min must preserve the W2 incumbent"
    );
    let kkt = max_min_trust_step_evidence(&mesh.mesh, patch, 0.01).unwrap();
    let gate = if certified {
        "StrictPass"
    } else if margin >= LEGACY_MARGIN_DEG + 0.1 {
        "Improved"
    } else if kkt.outcome == MaxMinTrustOutcomeKind::FirstOrderStationary {
        "FirstOrderStationary"
    } else {
        "ContinuousIncomplete"
    };
    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr62W2MaxMin\",\"taskbook_sha256\":\"{CLDP_TASKBOOK_SHA256}\",\"solver\":\"LinearizedMaxMinTrust\",\"iterations\":{iterations},\"incumbent_angle_degrees\":[{:.12},{:.12}],\"final_angle_degrees\":[{:.12},{:.12}],\"final_signed_margin_deg\":{margin:.12},\"gate\":\"{gate}\",\"kkt\":{{\"outcome\":\"{:?}\",\"active_constraints\":{},\"orientation_guards\":{},\"projection_sweeps\":{},\"current_slack_deg\":{:.12},\"achieved_slack_deg\":{:.12},\"linearized_upper_bound_deg\":{:.12},\"projected_stationarity_norm\":{:.12e}}}}}",
        incumbent_range.0,
        incumbent_range.1,
        range.0,
        range.1,
        kkt.outcome,
        kkt.active_constraints,
        kkt.orientation_guards,
        kkt.projection_sweeps,
        kkt.current_slack_deg,
        kkt.achieved_slack_deg,
        kkt.linearized_upper_bound_deg,
        kkt.projected_stationarity_norm,
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

#[test]
#[ignore = "explicit finite Frozen N6 PR63 violation-support gate"]
fn frozen_n6_pr63_violation_support_probe() {
    let (source, component, source_levels) = n6_legacy_mixed_fixture_with_source_levels().unwrap();
    let (_, incumbent, _, topology_keys, selected_ears) =
        pr49_and_pr52_witnesses_with_topology(&source, &component, &source_levels);
    let outcome = solve_elastic_patch_with_max_min_trust_start(
        &incumbent.mesh,
        incumbent.patch.clone(),
        ElasticBlockLimits {
            elastic_iterations: 128,
        },
        GeometryStartId::MaterializedSource,
    );
    let (_, mesh, patch, _) = elastic_outcome_geometry(&outcome);
    let stratified = build_stratified_annulus(&source, &component).unwrap();
    let atlas = build_violation_support_atlas(
        &source,
        mesh,
        patch,
        &stratified,
        &topology_keys,
        &selected_ears,
    )
    .unwrap();
    assert!(!atlas.evidence_sets.optimization_active.is_empty());
    assert!(atlas
        .evidence_sets
        .optimization_active
        .iter()
        .all(|angle| !angle.source_support_faces.is_empty() && !angle.parent_support.is_empty()));
    assert!(atlas
        .evidence_sets
        .strict_violations
        .iter()
        .all(|angle| angle.signed_margin_deg < 0.0));
    assert!(atlas
        .evidence_sets
        .near_boundary_guards
        .iter()
        .all(|angle| angle.signed_margin_deg >= 0.0));
    assert_eq!(atlas.support_inflation.guard_angles, 64);
    assert_eq!(
        atlas.support_inflation.promotion_seed_angles,
        atlas.support_inflation.actual_violation_angles
    );
    assert!(atlas.custom_triangle_provenance.iter().all(|item| {
        item.precision
            == earthmesh_refine_certified::coarsen::ProvenancePrecision::ConservativeSectorUpperBound
            && !item.covered_source_faces.is_empty()
            && !item.source_parents.is_empty()
    }));
    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr63ViolationSupport\",\"taskbook_sha256\":\"{CLDP_TASKBOOK_SHA256}\",\"gate\":\"AllActiveAnglesHaveFiniteSourceSupport\",\"atlas\":{}}}",
        violation_support_atlas_json(&atlas),
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

#[test]
#[ignore = "explicit finite Frozen N6 PR73 exact-sector recovery gate"]
fn frozen_n6_pr73_exact_sector_recovery_atlas_probe() {
    let (source, component, source_levels) = n6_legacy_mixed_fixture_with_source_levels().unwrap();
    let (_, incumbent, _, topology_keys, selected_ears) =
        pr49_and_pr52_witnesses_with_topology(&source, &component, &source_levels);
    let outcome = solve_elastic_patch_with_max_min_trust_start(
        &incumbent.mesh,
        incumbent.patch.clone(),
        ElasticBlockLimits {
            elastic_iterations: 128,
        },
        GeometryStartId::MaterializedSource,
    );
    let (_, mesh, patch, _) = elastic_outcome_geometry(&outcome);
    let mesh_before = earthmesh_refine_certified::mesh_fingerprint(&mesh.mesh);
    let stratified = build_stratified_annulus(&source, &component).unwrap();
    let atlas = build_violation_support_atlas(
        &source,
        mesh,
        patch,
        &stratified,
        &topology_keys,
        &selected_ears,
    )
    .unwrap();
    assert_eq!(
        mesh_before,
        earthmesh_refine_certified::mesh_fingerprint(&mesh.mesh)
    );
    assert_eq!(atlas.sector_recovery_atlas.sectors.len(), 14);
    assert_eq!(
        atlas
            .sector_recovery_atlas
            .source_face_owner
            .keys()
            .copied()
            .collect::<BTreeSet<_>>(),
        stratified
            .coupled
            .annulus_face_slots
            .iter()
            .copied()
            .collect()
    );
    let strict_faces = atlas
        .evidence_sets
        .strict_violations
        .iter()
        .map(|angle| angle.face)
        .collect::<BTreeSet<_>>();
    let atom_faces = atlas
        .recovery_atoms
        .iter()
        .flat_map(|atom| match atom {
            RecoveryAtom::HierarchyLeaf { mixed_face, .. } => BTreeSet::from([*mixed_face]),
            RecoveryAtom::Sector { mixed_faces, .. } => mixed_faces.clone(),
        })
        .collect::<BTreeSet<_>>();
    assert!(strict_faces.is_subset(&atom_faces));
    let actual_custom_faces = mesh
        .mesh
        .active_triangle_slots()
        .filter(|&face| mesh.triangle_addresses[face].is_none())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual_custom_faces.len(),
        atlas.sector_recovery_atlas.custom_face_owner.len()
    );
    let violating_custom_faces = strict_faces
        .iter()
        .filter(|&&face| mesh.triangle_addresses[face].is_none())
        .count();
    let violating_hierarchy_faces = strict_faces.len() - violating_custom_faces;
    assert!(atlas.sector_recovery_atlas.sectors.values().all(|sector| {
        sector.boundary_cycles.len() == 1
            && sector
                .source_area_interval
                .sub_out(sector.custom_area_interval)
                .contains(0.0)
    }));
    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr73ExactSectorRecoveryAtlas\",\"taskbook_sha256\":\"{SEACR_TASKBOOK_SHA256}\",\"gate\":\"ExactSectorPartitionUnambiguous\",\"mesh_unchanged\":true,\"strict_violation_angles\":{},\"strict_violation_faces\":{},\"violating_custom_faces\":{violating_custom_faces},\"violating_hierarchy_faces\":{violating_hierarchy_faces},\"exact_sectors\":{},\"annulus_source_faces\":{},\"custom_transition_faces\":{},\"recovery_atoms\":{},\"recovery_atom_mixed_faces\":{},\"atlas\":{}}}",
        atlas.evidence_sets.strict_violations.len(),
        strict_faces.len(),
        atlas.sector_recovery_atlas.sectors.len(),
        atlas.sector_recovery_atlas.source_face_owner.len(),
        actual_custom_faces.len(),
        atlas.recovery_atoms.len(),
        atom_faces.len(),
        violation_support_atlas_json(&atlas),
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

#[test]
#[ignore = "explicit finite Frozen N6 PR74 incumbent-local component gate"]
fn frozen_n6_pr74_incumbent_local_components_probe() {
    let (source, component, source_levels) = n6_legacy_mixed_fixture_with_source_levels().unwrap();
    let (_, incumbent, _, topology_keys, selected_ears) =
        pr49_and_pr52_witnesses_with_topology(&source, &component, &source_levels);
    let outcome = solve_elastic_patch_with_max_min_trust_start(
        &incumbent.mesh,
        incumbent.patch.clone(),
        ElasticBlockLimits {
            elastic_iterations: 128,
        },
        GeometryStartId::MaterializedSource,
    );
    let (_, mesh, patch, _) = elastic_outcome_geometry(&outcome);
    let mesh_before = earthmesh_refine_certified::mesh_fingerprint(&mesh.mesh);
    let stratified = build_stratified_annulus(&source, &component).unwrap();
    let atlas = build_violation_support_atlas(
        &source,
        mesh,
        patch,
        &stratified,
        &topology_keys,
        &selected_ears,
    )
    .unwrap();
    assert_eq!(
        mesh_before,
        earthmesh_refine_certified::mesh_fingerprint(&mesh.mesh)
    );
    let strict_faces = atlas
        .evidence_sets
        .strict_violations
        .iter()
        .map(|angle| angle.face)
        .collect::<BTreeSet<_>>();
    let component_mixed_faces = atlas
        .local_recovery_components
        .iter()
        .flat_map(|component| component.mixed_faces.iter().copied())
        .collect::<BTreeSet<_>>();
    assert!(strict_faces.is_subset(&component_mixed_faces));
    assert_eq!(
        atlas
            .local_recovery_components
            .iter()
            .map(|component| component.atoms.len())
            .sum::<usize>(),
        atlas.recovery_atoms.len()
    );
    assert!(atlas.local_recovery_components.iter().all(|component| {
        match &component.topology {
            PromotionPatchTopology::WholeSphere => component.boundary_cycles.is_empty(),
            PromotionPatchTopology::Disk => component.boundary_cycles.len() == 1,
            PromotionPatchTopology::Annulus { .. } => component.boundary_cycles.len() == 2,
            PromotionPatchTopology::MultiHole { protected_holes } => {
                component.boundary_cycles.len() == protected_holes.len() + 1
            }
        }
    }));
    let violating_sectors = atlas
        .recovery_atoms
        .iter()
        .filter(|atom| matches!(atom, RecoveryAtom::Sector { .. }))
        .count();
    let violating_hierarchy_leaves = atlas.recovery_atoms.len() - violating_sectors;
    let source_faces_per_component = atlas
        .local_recovery_components
        .iter()
        .map(|component| component.source_faces.len().to_string())
        .collect::<Vec<_>>()
        .join(",");
    let topologies = atlas
        .local_recovery_components
        .iter()
        .map(|component| match component.topology {
            PromotionPatchTopology::WholeSphere => "\"WholeSphere\"",
            PromotionPatchTopology::Disk => "\"Disk\"",
            PromotionPatchTopology::Annulus { .. } => "\"Annulus\"",
            PromotionPatchTopology::MultiHole { .. } => "\"MultiHole\"",
        })
        .collect::<Vec<_>>()
        .join(",");
    let surrounds_core = atlas
        .local_recovery_components
        .iter()
        .filter(|component| !component.protected_coarse_regions.is_empty())
        .count();
    let largest_component_atoms = atlas
        .local_recovery_components
        .iter()
        .map(|component| component.atoms.len())
        .max()
        .unwrap_or(0);
    let largest_component_source_faces = atlas
        .local_recovery_components
        .iter()
        .map(|component| component.source_faces.len())
        .max()
        .unwrap_or(0);
    assert!(largest_component_source_faces < atlas.support_inflation.strict_seed_source_faces);
    assert!(surrounds_core > 0);
    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr74IncumbentLocalComponents\",\"taskbook_sha256\":\"{SEACR_TASKBOOK_SHA256}\",\"gate\":\"LocalComponentsClassified\",\"mesh_unchanged\":true,\"strict_violation_angles\":{},\"strict_violation_faces\":{},\"violating_sectors\":{violating_sectors},\"violating_hierarchy_leaves\":{violating_hierarchy_leaves},\"recovery_atoms\":{},\"local_component_count\":{},\"largest_component_atoms\":{largest_component_atoms},\"largest_component_source_faces\":{largest_component_source_faces},\"source_faces_per_component\":[{source_faces_per_component}],\"topologies\":[{topologies}],\"components_surrounding_coarse_core\":{surrounds_core},\"legacy_strict_seed_source_faces\":{},\"atlas\":{}}}",
        atlas.evidence_sets.strict_violations.len(),
        strict_faces.len(),
        atlas.recovery_atoms.len(),
        atlas.local_recovery_components.len(),
        atlas.support_inflation.strict_seed_source_faces,
        violation_support_atlas_json(&atlas),
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

#[test]
#[ignore = "explicit finite Frozen N6 PR75 direct-sector restore gate"]
fn frozen_n6_pr75_direct_sector_restore_probe() {
    let local_iterations = usize_env("EARTHMESH_LOCAL_RESTORE_ITERATIONS", 8);
    let (source, component, source_levels) = n6_legacy_mixed_fixture_with_source_levels().unwrap();
    let (_, incumbent, _, topology_keys, selected_ears) =
        pr49_and_pr52_witnesses_with_topology(&source, &component, &source_levels);
    let outcome = solve_elastic_patch_with_max_min_trust_start(
        &incumbent.mesh,
        incumbent.patch.clone(),
        ElasticBlockLimits {
            elastic_iterations: 128,
        },
        GeometryStartId::MaterializedSource,
    );
    let (_, mesh, patch, _) = elastic_outcome_geometry(&outcome);
    let mesh_before = earthmesh_refine_certified::mesh_fingerprint(&mesh.mesh);
    let stratified = build_stratified_annulus(&source, &component).unwrap();
    let atlas = build_violation_support_atlas(
        &source,
        mesh,
        patch,
        &stratified,
        &topology_keys,
        &selected_ears,
    )
    .unwrap();
    let sectors = atlas
        .recovery_atoms
        .iter()
        .filter_map(|atom| match atom {
            RecoveryAtom::Sector { sector_id, .. } => Some(*sector_id),
            RecoveryAtom::HierarchyLeaf { .. } => None,
        })
        .collect::<BTreeSet<_>>();
    let mut fine_compatible = 0usize;
    let mut certified = 0usize;
    let mut geometry_attempted = 0usize;
    let mut parent_peel = 0usize;
    let mut local_collar = 0usize;
    let mut invalid = 0usize;
    let mut exterior_topology_equal = true;
    let mut exterior_coordinates_equal = true;
    let mut edge_incidence_two = true;
    let mut best_range = None::<(f64, f64)>;
    for sector_id in sectors.iter().copied() {
        match restore_fine_compatible_sector(
            &source,
            mesh,
            patch,
            &atlas.sector_recovery_atlas,
            sector_id,
            local_iterations,
        ) {
            DirectSectorRestoreOutcome::Certified(trial) => {
                fine_compatible += 1;
                certified += 1;
                geometry_attempted += usize::from(trial.local_geometry_attempted);
                exterior_topology_equal &= trial.outside_topology_bitwise_equal;
                exterior_coordinates_equal &= trial.outside_coordinates_bitwise_equal;
                edge_incidence_two &= trial.edge_incidence_at_most_two;
                if let Some(range) = trial.angle_range_deg {
                    if best_range.is_none_or(|best| {
                        (range.0 - 40.2).min(79.8 - range.1) > (best.0 - 40.2).min(79.8 - best.1)
                    }) {
                        best_range = Some(range);
                    }
                }
            }
            DirectSectorRestoreOutcome::GeometryNotCertified { trial, .. } => {
                fine_compatible += 1;
                geometry_attempted += usize::from(trial.local_geometry_attempted);
                exterior_topology_equal &= trial.outside_topology_bitwise_equal;
                exterior_coordinates_equal &= trial.outside_coordinates_bitwise_equal;
                edge_incidence_two &= trial.edge_incidence_at_most_two;
                if let Some(range) = trial.angle_range_deg {
                    if best_range.is_none_or(|best| {
                        (range.0 - 40.2).min(79.8 - range.1) > (best.0 - 40.2).min(79.8 - best.1)
                    }) {
                        best_range = Some(range);
                    }
                }
            }
            DirectSectorRestoreOutcome::RequiresBoundaryParentPeel { .. } => parent_peel += 1,
            DirectSectorRestoreOutcome::RequiresLocalCollar { .. } => local_collar += 1,
            DirectSectorRestoreOutcome::InvalidInput { .. } => invalid += 1,
        }
    }
    assert!(fine_compatible > 0);
    assert!(parent_peel + local_collar > 0);
    assert_eq!(invalid, 0);
    assert!(exterior_topology_equal);
    assert!(exterior_coordinates_equal);
    assert!(edge_incidence_two);
    assert_eq!(
        mesh_before,
        earthmesh_refine_certified::mesh_fingerprint(&mesh.mesh)
    );
    let best_range_json = best_range.map_or_else(
        || "null".into(),
        |range| format!("[{:.12},{:.12}]", range.0, range.1),
    );
    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr75DirectSectorRestore\",\"taskbook_sha256\":\"{SEACR_TASKBOOK_SHA256}\",\"gate\":\"DirectRestoreMaterialized\",\"mesh_unchanged\":true,\"violating_sectors\":{},\"fine_compatible_restores\":{fine_compatible},\"boundary_parent_peel_blockers\":{parent_peel},\"local_collar_blockers\":{local_collar},\"invalid_candidates\":{invalid},\"local_geometry_iterations\":{local_iterations},\"local_geometry_attempted\":{geometry_attempted},\"strict_certified\":{certified},\"outside_topology_bitwise_equal\":{exterior_topology_equal},\"outside_coordinates_bitwise_equal\":{exterior_coordinates_equal},\"edge_incidence_at_most_two\":{edge_incidence_two},\"best_direct_angle_range\":{best_range_json}}}",
        sectors.len(),
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

#[test]
#[ignore = "explicit finite Frozen N6 PR76 boundary-parent peel gate"]
fn frozen_n6_pr76_boundary_parent_peel_probe() {
    let local_iterations = usize_env("EARTHMESH_PARENT_PEEL_ITERATIONS", 8);
    let (source, component, source_levels) = n6_legacy_mixed_fixture_with_source_levels().unwrap();
    let (_, incumbent, _, topology_keys, selected_ears) =
        pr49_and_pr52_witnesses_with_topology(&source, &component, &source_levels);
    let outcome = solve_elastic_patch_with_max_min_trust_start(
        &incumbent.mesh,
        incumbent.patch.clone(),
        ElasticBlockLimits {
            elastic_iterations: 128,
        },
        GeometryStartId::MaterializedSource,
    );
    let (_, mesh, patch, _) = elastic_outcome_geometry(&outcome);
    let mesh_before = earthmesh_refine_certified::mesh_fingerprint(&mesh.mesh);
    let stratified = build_stratified_annulus(&source, &component).unwrap();
    let atlas = build_violation_support_atlas(
        &source,
        mesh,
        patch,
        &stratified,
        &topology_keys,
        &selected_ears,
    )
    .unwrap();
    let retained_parents = component
        .core_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let ears = coarse_core_ears(&source, &retained_parents).unwrap();
    let ear_parents = ears.iter().map(|ear| ear.parent).collect::<BTreeSet<_>>();
    let sectors = atlas
        .recovery_atoms
        .iter()
        .filter_map(|atom| match atom {
            RecoveryAtom::Sector { sector_id, .. } => Some(*sector_id),
            RecoveryAtom::HierarchyLeaf { .. } => None,
        })
        .collect::<BTreeSet<_>>();
    let mut candidates = 0usize;
    let mut closed = 0usize;
    let mut certified = 0usize;
    let mut geometry_attempted = 0usize;
    let mut topology_failures = 0usize;
    let mut invalid = 0usize;
    let mut not_ear = 0usize;
    let mut max_restored_sectors = 0usize;
    let mut max_split_interfaces = 0usize;
    let mut exterior_topology_equal = true;
    let mut exterior_coordinates_equal = true;
    let mut edge_incidence_two = true;
    let mut best_range = None::<(f64, f64)>;
    for sector_id in sectors {
        let DirectSectorRestoreOutcome::RequiresBoundaryParentPeel {
            adjacent_parents, ..
        } = restore_fine_compatible_sector(
            &source,
            mesh,
            patch,
            &atlas.sector_recovery_atlas,
            sector_id,
            0,
        )
        else {
            continue;
        };
        for parent in adjacent_parents.intersection(&ear_parents).copied() {
            candidates += 1;
            let peel = peel_boundary_parent_for_sector(
                &source,
                mesh,
                patch,
                &atlas.sector_recovery_atlas,
                &retained_parents,
                sector_id,
                parent,
                local_iterations,
            );
            let (trial, topology_closed) = match peel {
                BoundaryParentPeelOutcome::Certified(trial) => {
                    certified += 1;
                    closed += 1;
                    (trial, true)
                }
                BoundaryParentPeelOutcome::GeometryNotCertified { trial, .. } => {
                    closed += usize::from(trial.topology_closed);
                    (trial, true)
                }
                BoundaryParentPeelOutcome::TopologyNotClosed { trial, .. } => {
                    topology_failures += 1;
                    (trial, false)
                }
                BoundaryParentPeelOutcome::NotCoarseCoreEar { .. } => {
                    not_ear += 1;
                    continue;
                }
                BoundaryParentPeelOutcome::InvalidInput { .. } => {
                    invalid += 1;
                    continue;
                }
            };
            if !topology_closed {
                continue;
            }
            geometry_attempted += usize::from(trial.local_geometry_attempted);
            max_restored_sectors = max_restored_sectors.max(trial.restored_sector_ids.len());
            max_split_interfaces = max_split_interfaces.max(trial.split_interface_parents.len());
            exterior_topology_equal &= trial.outside_topology_bitwise_equal;
            exterior_coordinates_equal &= trial.outside_coordinates_bitwise_equal;
            edge_incidence_two &= trial.edge_incidence_at_most_two;
            if let Some(range) = trial.angle_range_deg {
                if best_range.is_none_or(|best| {
                    (range.0 - 40.2).min(79.8 - range.1) > (best.0 - 40.2).min(79.8 - best.1)
                }) {
                    best_range = Some(range);
                }
            }
        }
    }
    assert!(candidates > 0);
    assert!(closed > 0);
    assert_eq!(invalid, 0);
    assert_eq!(not_ear, 0);
    assert!(exterior_topology_equal);
    assert!(exterior_coordinates_equal);
    assert!(edge_incidence_two);
    assert_eq!(
        mesh_before,
        earthmesh_refine_certified::mesh_fingerprint(&mesh.mesh)
    );
    let best_range_json = best_range.map_or_else(
        || "null".into(),
        |range| format!("[{:.12},{:.12}]", range.0, range.1),
    );
    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr76BoundaryParentPeel\",\"taskbook_sha256\":\"{SEACR_TASKBOOK_SHA256}\",\"gate\":\"BoundaryParentPeelTopologyClosed\",\"mesh_unchanged\":true,\"retained_parents\":{},\"coarse_core_ears\":{},\"candidate_sector_parent_pairs\":{candidates},\"closed_topologies\":{closed},\"topology_failures\":{topology_failures},\"invalid_candidates\":{invalid},\"not_ear\":{not_ear},\"local_geometry_iterations\":{local_iterations},\"local_geometry_attempted\":{geometry_attempted},\"strict_certified\":{certified},\"max_restored_sector_cluster\":{max_restored_sectors},\"max_split_interface_parents\":{max_split_interfaces},\"outside_topology_bitwise_equal\":{exterior_topology_equal},\"outside_coordinates_bitwise_equal\":{exterior_coordinates_equal},\"edge_incidence_at_most_two\":{edge_incidence_two},\"best_peel_angle_range\":{best_range_json}}}",
        retained_parents.len(),
        ears.len(),
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

#[test]
#[ignore = "explicit finite Frozen N6 PR77 local-annular-collar gate"]
fn frozen_n6_pr77_local_annular_collar_probe() {
    let local_iterations = usize_env("EARTHMESH_LOCAL_COLLAR_ITERATIONS", 8);
    let (source, hierarchy, source_levels) = n6_legacy_mixed_fixture_with_source_levels().unwrap();
    let (_, incumbent, _, topology_keys, selected_ears) =
        pr49_and_pr52_witnesses_with_topology(&source, &hierarchy, &source_levels);
    let outcome = solve_elastic_patch_with_max_min_trust_start(
        &incumbent.mesh,
        incumbent.patch.clone(),
        ElasticBlockLimits {
            elastic_iterations: 128,
        },
        GeometryStartId::MaterializedSource,
    );
    let (_, mesh, patch, _) = elastic_outcome_geometry(&outcome);
    let mesh_before = earthmesh_refine_certified::mesh_fingerprint(&mesh.mesh);
    let stratified = build_stratified_annulus(&source, &hierarchy).unwrap();
    let atlas = build_violation_support_atlas(
        &source,
        mesh,
        patch,
        &stratified,
        &topology_keys,
        &selected_ears,
    )
    .unwrap();
    let retained_parents = hierarchy
        .core_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let sectors = atlas
        .recovery_atoms
        .iter()
        .filter_map(|atom| match atom {
            RecoveryAtom::Sector { sector_id, .. } => Some(*sector_id),
            RecoveryAtom::HierarchyLeaf { .. } => None,
        })
        .collect::<BTreeSet<_>>();
    let mut two_parent_blockers = 0usize;
    let mut materialized = 0usize;
    let mut two_parent_materialized = 0usize;
    let mut topology_trials = 0usize;
    let mut closed_trials = 0usize;
    let mut certified = 0usize;
    let mut invalid = 0usize;
    let mut min_retained_parents = usize::MAX;
    let mut max_promoted_source_faces = 0usize;
    let mut protected_core_preserved = true;
    let mut fixed_outside_links = true;
    let mut outside_coordinates_equal = true;
    let mut edge_incidence_two = true;
    let mut best_range = None::<(f64, f64)>;
    for sector_id in sectors {
        let DirectSectorRestoreOutcome::RequiresBoundaryParentPeel {
            adjacent_parents, ..
        } = restore_fine_compatible_sector(
            &source,
            mesh,
            patch,
            &atlas.sector_recovery_atlas,
            sector_id,
            0,
        )
        else {
            continue;
        };
        if adjacent_parents.len() != 2 {
            continue;
        }
        two_parent_blockers += 1;
        let component = atlas
            .local_recovery_components
            .iter()
            .find(|component| {
                component.atoms.iter().any(|atom| {
                    matches!(atom, RecoveryAtom::Sector { sector_id: candidate, .. } if *candidate == sector_id)
                })
            })
            .expect("two-parent sector must belong to a local recovery component");
        let collar = solve_local_annular_collar(
            &source,
            mesh,
            patch,
            &atlas.sector_recovery_atlas,
            component,
            &retained_parents,
            sector_id,
            &adjacent_parents,
            LocalAnnularCollarLimits {
                topology_states: 3,
                geometry_iterations: local_iterations,
                maximum_parent_peels: 2,
            },
        );
        let (best, trials) = match collar {
            LocalAnnularCollarOutcome::Certified(trial) => {
                certified += 1;
                materialized += 1;
                (Some(trial), Vec::new())
            }
            LocalAnnularCollarOutcome::MaterializedNotCertified { best, trials } => {
                materialized += 1;
                (Some(best), trials)
            }
            LocalAnnularCollarOutcome::TopologyFamilyExhausted { trials }
            | LocalAnnularCollarOutcome::SearchBudgetExhausted { trials } => (None, trials),
            LocalAnnularCollarOutcome::InvalidInput(_) => {
                invalid += 1;
                continue;
            }
        };
        topology_trials += trials.len();
        closed_trials += trials.iter().filter(|trial| trial.topology_closed).count();
        let Some(best) = best else {
            continue;
        };
        two_parent_materialized +=
            usize::from(best.evidence.level == LocalAnnularCollarLevel::TwoParentPeel);
        min_retained_parents = min_retained_parents.min(best.evidence.retained_parents.len());
        max_promoted_source_faces =
            max_promoted_source_faces.max(best.evidence.promoted_source_faces);
        protected_core_preserved &= best.evidence.protected_core_preserved;
        fixed_outside_links &= best.evidence.fixed_outside_link_contracts;
        outside_coordinates_equal &= best.evidence.outside_coordinates_bitwise_equal;
        edge_incidence_two &= best.evidence.edge_incidence_at_most_two;
        if let Some(range) = best.evidence.angle_range_deg {
            if best_range.is_none_or(|current| {
                (range.0 - 40.2).min(79.8 - range.1) > (current.0 - 40.2).min(79.8 - current.1)
            }) {
                best_range = Some(range);
            }
        }
    }
    assert!(two_parent_blockers > 0);
    assert!(materialized > 0);
    assert!(two_parent_materialized > 0);
    assert_eq!(invalid, 0);
    assert!(protected_core_preserved);
    assert!(fixed_outside_links);
    assert!(outside_coordinates_equal);
    assert!(edge_incidence_two);
    assert_eq!(
        mesh_before,
        earthmesh_refine_certified::mesh_fingerprint(&mesh.mesh)
    );
    let best_range_json = best_range.map_or_else(
        || "null".into(),
        |range| format!("[{:.12},{:.12}]", range.0, range.1),
    );
    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr77LocalAnnularCollar\",\"taskbook_sha256\":\"{SEACR_TASKBOOK_SHA256}\",\"gate\":\"LocalAnnularCollarMaterialized\",\"mesh_unchanged\":true,\"two_parent_blockers\":{two_parent_blockers},\"topology_trials\":{topology_trials},\"closed_topology_trials\":{closed_trials},\"materialized_collars\":{materialized},\"two_parent_materialized\":{two_parent_materialized},\"invalid_candidates\":{invalid},\"local_geometry_iterations\":{local_iterations},\"strict_certified\":{certified},\"minimum_retained_coarse_parents\":{min_retained_parents},\"maximum_promoted_source_faces\":{max_promoted_source_faces},\"protected_core_preserved\":{protected_core_preserved},\"fixed_outside_link_contracts\":{fixed_outside_links},\"outside_coordinates_bitwise_equal\":{outside_coordinates_equal},\"edge_incidence_at_most_two\":{edge_incidence_two},\"best_collar_angle_range\":{best_range_json}}}"
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

#[test]
#[ignore = "explicit Frozen N6 PR78 final local-recovery gate"]
fn frozen_n6_pr78_final_local_recovery_gate_probe() {
    let local_iterations = usize_env("EARTHMESH_FINAL_LOCAL_ITERATIONS", 8);
    let (source, hierarchy, source_levels) = n6_legacy_mixed_fixture_with_source_levels().unwrap();
    let (_, incumbent, _, topology_keys, selected_ears) =
        pr49_and_pr52_witnesses_with_topology(&source, &hierarchy, &source_levels);
    let outcome = solve_elastic_patch_with_max_min_trust_start(
        &incumbent.mesh,
        incumbent.patch.clone(),
        ElasticBlockLimits {
            elastic_iterations: 128,
        },
        GeometryStartId::MaterializedSource,
    );
    let (_, mesh, patch, _) = elastic_outcome_geometry(&outcome);
    let mesh_before = earthmesh_refine_certified::mesh_fingerprint(&mesh.mesh);
    let stratified = build_stratified_annulus(&source, &hierarchy).unwrap();
    let atlas = build_violation_support_atlas(
        &source,
        mesh,
        patch,
        &stratified,
        &topology_keys,
        &selected_ears,
    )
    .unwrap();
    let retained_parents = hierarchy
        .core_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let ear_parents = coarse_core_ears(&source, &retained_parents)
        .unwrap()
        .into_iter()
        .map(|ear| ear.parent)
        .collect::<BTreeSet<_>>();
    let sectors = atlas
        .recovery_atoms
        .iter()
        .filter_map(|atom| match atom {
            RecoveryAtom::Sector { sector_id, .. } => Some(*sector_id),
            RecoveryAtom::HierarchyLeaf { .. } => None,
        })
        .collect::<BTreeSet<_>>();
    let sector_count = sectors.len();
    let mut materialized = 0usize;
    let mut strict_candidates = 0usize;
    let mut direct_trials = 0usize;
    let mut one_parent_trials = 0usize;
    let mut two_parent_trials = 0usize;
    let mut invalid = 0usize;
    let mut best_local_range = None::<(f64, f64)>;
    for sector_id in sectors {
        match restore_fine_compatible_sector(
            &source,
            mesh,
            patch,
            &atlas.sector_recovery_atlas,
            sector_id,
            local_iterations,
        ) {
            DirectSectorRestoreOutcome::Certified(trial) => {
                direct_trials += 1;
                materialized += 1;
                strict_candidates += 1;
                update_best_range(&mut best_local_range, trial.angle_range_deg);
            }
            DirectSectorRestoreOutcome::GeometryNotCertified { trial, .. } => {
                direct_trials += 1;
                materialized += 1;
                update_best_range(&mut best_local_range, trial.angle_range_deg);
            }
            DirectSectorRestoreOutcome::RequiresBoundaryParentPeel {
                adjacent_parents, ..
            } if adjacent_parents.len() == 1 => {
                let Some(parent) = adjacent_parents.intersection(&ear_parents).copied().next()
                else {
                    invalid += 1;
                    continue;
                };
                match peel_boundary_parent_for_sector(
                    &source,
                    mesh,
                    patch,
                    &atlas.sector_recovery_atlas,
                    &retained_parents,
                    sector_id,
                    parent,
                    local_iterations,
                ) {
                    BoundaryParentPeelOutcome::Certified(trial) => {
                        one_parent_trials += 1;
                        materialized += 1;
                        strict_candidates += 1;
                        update_best_range(&mut best_local_range, trial.angle_range_deg);
                    }
                    BoundaryParentPeelOutcome::GeometryNotCertified { trial, .. } => {
                        one_parent_trials += 1;
                        materialized += 1;
                        update_best_range(&mut best_local_range, trial.angle_range_deg);
                    }
                    _ => invalid += 1,
                }
            }
            DirectSectorRestoreOutcome::RequiresBoundaryParentPeel {
                adjacent_parents, ..
            } if adjacent_parents.len() == 2 => {
                let component = atlas
                    .local_recovery_components
                    .iter()
                    .find(|component| {
                        component.atoms.iter().any(|atom| {
                            matches!(atom, RecoveryAtom::Sector { sector_id: candidate, .. } if *candidate == sector_id)
                        })
                    })
                    .expect("two-parent sector must belong to a local recovery component");
                match solve_local_annular_collar(
                    &source,
                    mesh,
                    patch,
                    &atlas.sector_recovery_atlas,
                    component,
                    &retained_parents,
                    sector_id,
                    &adjacent_parents,
                    LocalAnnularCollarLimits {
                        topology_states: 3,
                        geometry_iterations: local_iterations,
                        maximum_parent_peels: 2,
                    },
                ) {
                    LocalAnnularCollarOutcome::Certified(trial) => {
                        two_parent_trials += 1;
                        materialized += 1;
                        strict_candidates += 1;
                        update_best_range(&mut best_local_range, trial.evidence.angle_range_deg);
                    }
                    LocalAnnularCollarOutcome::MaterializedNotCertified { best, .. } => {
                        two_parent_trials += 1;
                        materialized += 1;
                        update_best_range(&mut best_local_range, best.evidence.angle_range_deg);
                    }
                    _ => invalid += 1,
                }
            }
            _ => invalid += 1,
        }
    }
    assert_eq!(materialized, sector_count);
    assert_eq!(strict_candidates, 0);
    assert_eq!(invalid, 0);
    assert_eq!(
        mesh_before,
        earthmesh_refine_certified::mesh_fingerprint(&mesh.mesh)
    );

    let budget = PromotionBudget {
        local_topology_states: 128,
        local_geometry_iterations: local_iterations,
        maximum_patch_rings: 2,
        maximum_helper_vertices: 512,
    };
    let (fallback_source, fallback_levels, fallback_result) = frozen_n6_cldp_result(budget);
    let fallback = match &fallback_result.outcome {
        PromotionOutcome::SafeMotherFallback(trial) => trial,
        other => panic!("PR78 expected a certified safe fallback, got {other:?}"),
    };
    let required_levels = fallback_levels
        .into_iter()
        .map(|level| level.unwrap_or(0))
        .collect::<Vec<_>>();
    let geometry = match earthmesh_refine_certified::certify_geometry(fallback.mesh.mesh.clone()) {
        earthmesh_refine_certified::CertifiedMeshOutcome::GeometryCertified(geometry) => geometry,
        other => panic!("PR78 fallback geometry rejected: {other:?}"),
    };
    let final_evidence = earthmesh_refine_certified::safe_mother_final_evidence(
        &required_levels,
        1,
        geometry.primal(),
    )
    .unwrap();
    let final_mesh =
        earthmesh_refine_certified::finalize_geometry_certified_mother(*geometry, final_evidence)
            .unwrap();
    let gate_evidence =
        build_frozen_cldp_gate_evidence(&fallback_source, fallback, final_mesh.certificate())
            .unwrap();
    let gate = evaluate_frozen_cldp_gate(&gate_evidence);
    assert_eq!(gate, FrozenCldpGateOutcome::CertifiedSafeFallback);
    assert_eq!(gate_evidence.retained_coarse_parents, 0);
    assert_eq!(gate_evidence.compression_ratio, 1.0);
    let best_range_json = best_local_range.map_or_else(
        || "null".into(),
        |range| format!("[{:.12},{:.12}]", range.0, range.1),
    );
    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr78FinalLocalRecoveryGate\",\"taskbook_sha256\":\"{SEACR_TASKBOOK_SHA256}\",\"gate\":\"CertifiedSafeFallback\",\"certified_adaptive\":false,\"mesh_unchanged\":true,\"violating_sectors\":{sector_count},\"materialized_local_candidates\":{materialized},\"direct_trials\":{direct_trials},\"one_parent_trials\":{one_parent_trials},\"two_parent_trials\":{two_parent_trials},\"invalid_candidates\":{invalid},\"strict_local_candidates\":{strict_candidates},\"local_geometry_iterations\":{local_iterations},\"best_local_angle_range\":{best_range_json},\"internal_fallback_angle_range\":[{:.12},{:.12}],\"final_fallback_angle_range\":[{:.12},{:.12}],\"retained_coarse_parents\":{},\"compression_ratio\":{:.12},\"mixed_levels_delivered\":{},\"euler\":{},\"charge\":{},\"delaunay_violations\":{},\"voronoi_invalid_cells\":{},\"voronoi_reciprocal_errors\":{},\"physical_residuals\":{},\"balance_residuals\":{},\"remap_closure_errors\":{},\"adaptive_failures\":[\"mixed_levels_delivered\",\"retained_coarse_parents\",\"compression_ratio\"],\"pr79_required\":false,\"pr80_pr81_gated\":true}}",
        gate_evidence.internal_angle_range_deg.0,
        gate_evidence.internal_angle_range_deg.1,
        gate_evidence.final_angle_range_deg.0,
        gate_evidence.final_angle_range_deg.1,
        gate_evidence.retained_coarse_parents,
        gate_evidence.compression_ratio,
        gate_evidence.mixed_levels_delivered,
        gate_evidence.euler,
        gate_evidence.charge,
        gate_evidence.delaunay_violations,
        gate_evidence.voronoi_invalid_cells,
        gate_evidence.voronoi_reciprocal_errors,
        gate_evidence.physical_residuals,
        gate_evidence.balance_residuals,
        gate_evidence.remap_closure_errors,
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

#[test]
#[ignore = "explicit finite Frozen N6 PR64 local-topology gate"]
fn frozen_n6_pr64_local_topology_probe() {
    let (source, component, source_levels) = n6_legacy_mixed_fixture_with_source_levels().unwrap();
    let (_, incumbent, _, topology_keys, selected_ears) =
        pr49_and_pr52_witnesses_with_topology(&source, &component, &source_levels);
    let outcome = solve_elastic_patch_with_max_min_trust_start(
        &incumbent.mesh,
        incumbent.patch.clone(),
        ElasticBlockLimits {
            elastic_iterations: 128,
        },
        GeometryStartId::MaterializedSource,
    );
    let (_, mesh, patch, _) = elastic_outcome_geometry(&outcome);
    let improved = GeometryFailureWitness {
        mesh: mesh.clone(),
        patch: patch.clone(),
    };
    let stratified = build_stratified_annulus(&source, &component).unwrap();
    let atlas = build_violation_support_atlas(
        &source,
        &improved.mesh,
        &improved.patch,
        &stratified,
        &topology_keys,
        &selected_ears,
    )
    .unwrap();
    let limits = LocalTopologyLimits {
        maximum_states: usize_env("EARTHMESH_LOCAL_TOPOLOGY_STATES", 128),
        maximum_flips: 3,
        local_geometry_iterations: usize_env("EARTHMESH_LOCAL_GEOMETRY_ITERATIONS", 32),
    };
    let anchors = [2usize, 29, 77, 155].into_iter().collect();
    let result = search_local_topology_neighbourhood(&improved, &atlas, &anchors, limits);
    let (gate, evidence) = match &result {
        LocalTopologySearchOutcome::StrictCertified { evidence, .. } => {
            ("StrictCandidateStopPromotion", evidence)
        }
        LocalTopologySearchOutcome::NoStrictCandidate(evidence) => {
            ("NoStrictCandidateEnterPromotion", evidence)
        }
        LocalTopologySearchOutcome::SearchBudgetExhausted(evidence) => {
            ("LocalTopologyBudgetExhausted", evidence)
        }
        LocalTopologySearchOutcome::InvalidInput(reason) => {
            panic!("PR64 local-topology input rejected: {reason}")
        }
    };
    assert!(evidence.incumbent_preserved);
    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr64LocalTopology\",\"taskbook_sha256\":\"{CLDP_TASKBOOK_SHA256}\",\"limits\":{{\"maximum_states\":{},\"maximum_flips\":{},\"local_geometry_iterations\":{}}},\"gate\":\"{gate}\",\"evidence\":{}}}",
        limits.maximum_states,
        limits.maximum_flips,
        limits.local_geometry_iterations,
        local_topology_evidence_json(evidence),
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

#[test]
#[ignore = "explicit finite Frozen N6 PR65 source-face promotion gate"]
fn frozen_n6_pr65_source_face_promotion_probe() {
    let (source, component, source_levels) = n6_legacy_mixed_fixture_with_source_levels().unwrap();
    let (_, incumbent, _, topology_keys, selected_ears) =
        pr49_and_pr52_witnesses_with_topology(&source, &component, &source_levels);
    let outcome = solve_elastic_patch_with_max_min_trust_start(
        &incumbent.mesh,
        incumbent.patch.clone(),
        ElasticBlockLimits {
            elastic_iterations: 128,
        },
        GeometryStartId::MaterializedSource,
    );
    let (_, mesh, patch, _) = elastic_outcome_geometry(&outcome);
    let stratified = build_stratified_annulus(&source, &component).unwrap();
    let atlas = build_violation_support_atlas(
        &source,
        mesh,
        patch,
        &stratified,
        &topology_keys,
        &selected_ears,
    )
    .unwrap();
    assert_eq!(atlas.components.len(), 1);
    let violation = &atlas.components[0];
    let p1 =
        build_promotion_patch(&source, violation, PromotionLevel::P1RestoreSourceFaces).unwrap();
    let p2 =
        build_promotion_patch(&source, violation, PromotionLevel::P2RestoreOneParentRing).unwrap();
    assert!(p1.source_faces.is_subset(&p2.source_faces));
    let p1 = restore_source_patch(&source, p1).unwrap();
    let p2 = restore_source_patch(&source, p2).unwrap();
    for restored in [&p1, &p2] {
        assert!(restored.faces.iter().all(|face| {
            face.triangle == source.mesh.triangles()[face.source_face]
                && Some(face.hierarchy_address) == source.triangle_addresses[face.source_face]
                && face.coordinates == face.triangle.map(|site| source.mesh.vertices()[site])
        }));
    }
    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr65SourceFacePromotion\",\"taskbook_sha256\":\"{CLDP_TASKBOOK_SHA256}\",\"gate\":\"P1P2ExactSafeRestore\",\"violation_source_faces\":{},\"p1\":{},\"p2\":{}}}",
        violation.source_faces.len(),
        restored_patch_json(&p1),
        restored_patch_json(&p2),
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

#[test]
#[ignore = "explicit finite Frozen N6 PR66 expanding-collar gate"]
fn frozen_n6_pr66_expanding_collar_probe() {
    let budget = PromotionBudget {
        local_topology_states: 128,
        local_geometry_iterations: usize_env("EARTHMESH_LOCAL_GEOMETRY_ITERATIONS", 32),
        maximum_patch_rings: 2,
        maximum_helper_vertices: 512,
    };
    let (_, _, result) = frozen_n6_cldp_result(budget);
    assert!(result
        .trials
        .windows(2)
        .all(|pair| pair[0].level < pair[1].level));
    let (gate, adaptive, final_range, final_vertices, final_faces) = match &result.outcome {
        PromotionOutcome::Certified(trial) => {
            assert!(trial.adaptive);
            assert!(!trial.promotion_patch.protected_exterior_faces.is_empty());
            (
                "StrictLocalPromotion",
                true,
                Some((
                    trial.geometry.min_angle_degrees,
                    trial.geometry.max_angle_degrees,
                )),
                trial.geometry.vertices,
                trial.geometry.faces,
            )
        }
        PromotionOutcome::SafeMotherFallback(trial) => {
            assert!(!trial.adaptive);
            (
                "CertifiedSafeFallback",
                false,
                Some((
                    trial.geometry.min_angle_degrees,
                    trial.geometry.max_angle_degrees,
                )),
                trial.geometry.vertices,
                trial.geometry.faces,
            )
        }
        PromotionOutcome::SearchBudgetExhausted { .. } => {
            ("PromotionBudgetExhausted", false, None, 0, 0)
        }
        PromotionOutcome::NeedsLargerPatch { .. } => {
            panic!("the expanding solver must consume its finite promotion ladder")
        }
        PromotionOutcome::InvalidInput(reason) => panic!("PR66 rejected its input: {reason}"),
    };
    let trials = result
        .trials
        .iter()
        .map(promotion_trial_json)
        .collect::<Vec<_>>()
        .join(",");
    let final_range = final_range
        .map(|range| format!("[{:.12},{:.12}]", range.0, range.1))
        .unwrap_or_else(|| "null".into());
    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr66ExpandingCollar\",\"taskbook_sha256\":\"{CLDP_TASKBOOK_SHA256}\",\"budget\":{{\"local_topology_states\":{},\"local_geometry_iterations\":{},\"maximum_patch_rings\":{},\"maximum_helper_vertices\":{}}},\"gate\":\"{gate}\",\"adaptive\":{adaptive},\"final_angle_degrees\":{final_range},\"final_vertices\":{final_vertices},\"final_faces\":{final_faces},\"trials\":[{trials}]}}",
        budget.local_topology_states,
        budget.local_geometry_iterations,
        budget.maximum_patch_rings,
        budget.maximum_helper_vertices,
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

#[test]
#[ignore = "explicit Frozen N6 PR67 strict CLDP product gate"]
fn frozen_n6_pr67_strict_cldp_gate_probe() {
    let budget = PromotionBudget {
        local_topology_states: 128,
        local_geometry_iterations: usize_env("EARTHMESH_LOCAL_GEOMETRY_ITERATIONS", 32),
        maximum_patch_rings: 2,
        maximum_helper_vertices: 512,
    };
    let (source, source_levels, result) = frozen_n6_cldp_result(budget);
    let trial = match &result.outcome {
        PromotionOutcome::Certified(trial) | PromotionOutcome::SafeMotherFallback(trial) => trial,
        PromotionOutcome::NeedsLargerPatch { .. } => {
            panic!("PR67 received an unfinished promotion ladder")
        }
        PromotionOutcome::SearchBudgetExhausted { .. } => {
            panic!("PR67 promotion budget exhausted")
        }
        PromotionOutcome::InvalidInput(reason) => panic!("PR67 rejected its input: {reason}"),
    };
    let required_levels = source_levels
        .into_iter()
        .map(|level| level.unwrap_or(0))
        .collect::<Vec<_>>();
    let geometry = match earthmesh_refine_certified::certify_geometry(trial.mesh.mesh.clone()) {
        earthmesh_refine_certified::CertifiedMeshOutcome::GeometryCertified(geometry) => geometry,
        other => panic!("PR67 final geometry rejected: {other:?}"),
    };
    let final_evidence = earthmesh_refine_certified::safe_mother_final_evidence(
        &required_levels,
        1,
        geometry.primal(),
    )
    .unwrap();
    let final_mesh =
        earthmesh_refine_certified::finalize_geometry_certified_mother(*geometry, final_evidence)
            .unwrap();
    let evidence =
        build_frozen_cldp_gate_evidence(&source, trial, final_mesh.certificate()).unwrap();
    let outcome = evaluate_frozen_cldp_gate(&evidence);
    let (gate, strict_mixed, failures) = match &outcome {
        FrozenCldpGateOutcome::CertifiedAdaptive => {
            ("FrozenN6StrictMixedPass", true, "[]".to_string())
        }
        FrozenCldpGateOutcome::CertifiedSafeFallback => (
            "FrozenN6StrictMixedFailedSafeFallback",
            false,
            "[\"mixed_levels_delivered\"]".to_string(),
        ),
        FrozenCldpGateOutcome::Failed(failures) => (
            "FrozenN6HardGateFailed",
            false,
            format!(
                "[{}]",
                failures
                    .iter()
                    .map(|failure| format!("\"{failure}\""))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        ),
    };
    assert_eq!(outcome, FrozenCldpGateOutcome::CertifiedSafeFallback);
    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr67StrictCldpGate\",\"taskbook_sha256\":\"{CLDP_TASKBOOK_SHA256}\",\"gate\":\"{gate}\",\"strict_mixed\":{strict_mixed},\"hard_gate_failures\":{failures},\"internal_angle_degrees\":[{:.12},{:.12}],\"final_angle_degrees\":[{:.12},{:.12}],\"anchors_degree_five\":{},\"ordinary_degree_window\":{},\"links_are_cycles\":{},\"edge_incidence_two\":{},\"vertices\":{},\"edges\":{},\"faces\":{},\"euler\":{},\"charge\":{},\"delaunay_violations\":{},\"voronoi_invalid_cells\":{},\"voronoi_reciprocal_errors\":{},\"physical_residuals\":{},\"balance_residuals\":{},\"remap_closure_errors\":{},\"mixed_levels_delivered\":{},\"promoted_source_faces\":{},\"retained_coarse_parents\":{},\"compression_ratio\":{:.12},\"promotion_level\":\"{:?}\",\"pr68_started\":false,\"pr69_started\":false}}",
        evidence.internal_angle_range_deg.0,
        evidence.internal_angle_range_deg.1,
        evidence.final_angle_range_deg.0,
        evidence.final_angle_range_deg.1,
        evidence.anchors_degree_five,
        evidence.ordinary_degree_window,
        evidence.links_are_cycles,
        evidence.edge_incidence_two,
        evidence.vertices,
        evidence.edges,
        evidence.faces,
        evidence.euler,
        evidence.charge,
        evidence.delaunay_violations,
        evidence.voronoi_invalid_cells,
        evidence.voronoi_reciprocal_errors,
        evidence.physical_residuals,
        evidence.balance_residuals,
        evidence.remap_closure_errors,
        evidence.mixed_levels_delivered,
        evidence.promoted_source_faces,
        evidence.retained_coarse_parents,
        evidence.compression_ratio,
        evidence.promotion_level,
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

fn frozen_n6_cldp_result(
    budget: PromotionBudget,
) -> (
    earthmesh_refine_certified::MotherGrid,
    Vec<Option<usize>>,
    earthmesh_refine_certified::coarsen::ExpandingCollarResult,
) {
    let (source, component, source_levels) = n6_legacy_mixed_fixture_with_source_levels().unwrap();
    let (_, incumbent, _, topology_keys, selected_ears) =
        pr49_and_pr52_witnesses_with_topology(&source, &component, &source_levels);
    let outcome = solve_elastic_patch_with_max_min_trust_start(
        &incumbent.mesh,
        incumbent.patch.clone(),
        ElasticBlockLimits {
            elastic_iterations: 128,
        },
        GeometryStartId::MaterializedSource,
    );
    let (_, mesh, patch, _) = elastic_outcome_geometry(&outcome);
    let improved = GeometryFailureWitness {
        mesh: mesh.clone(),
        patch: patch.clone(),
    };
    let stratified = build_stratified_annulus(&source, &component).unwrap();
    let atlas = build_violation_support_atlas(
        &source,
        &improved.mesh,
        &improved.patch,
        &stratified,
        &topology_keys,
        &selected_ears,
    )
    .unwrap();
    assert_eq!(atlas.components.len(), 1);
    let result = solve_expanding_collar(
        &source,
        &component,
        &improved,
        &atlas,
        &atlas.components[0],
        budget,
    );
    (source, source_levels, result)
}

fn elastic_outcome_geometry(
    outcome: &ElasticBlockOutcome,
) -> (
    (f64, f64),
    &earthmesh_refine_certified::coarsen::HierarchyLeafMesh,
    &earthmesh_refine_certified::coarsen::ElasticPatch,
    bool,
) {
    match outcome {
        ElasticBlockOutcome::Certified(trial) => (
            (
                trial.geometry.min_angle_degrees,
                trial.geometry.max_angle_degrees,
            ),
            &trial.mesh,
            &trial.patch,
            true,
        ),
        ElasticBlockOutcome::ElasticNoImprovement {
            global_angle_degrees,
            witness,
            ..
        }
        | ElasticBlockOutcome::SearchBudgetExhausted {
            global_angle_degrees,
            witness,
            ..
        }
        | ElasticBlockOutcome::RequiresDifferentTopology {
            global_angle_degrees,
            witness,
            ..
        } => (
            global_angle_degrees.expect("max-min outcome must report its angle range"),
            &witness.mesh,
            &witness.patch,
            false,
        ),
        ElasticBlockOutcome::InvalidPatch { reason } => panic!("invalid max-min patch: {reason}"),
    }
}

fn local_topology_evidence_json(evidence: &LocalTopologyEvidence) -> String {
    let flips = evidence
        .best_flips
        .iter()
        .map(|flip| {
            format!(
                "{{\"face\":{},\"neighbour\":{}}}",
                flip.face, flip.neighbour
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"states_examined\":{},\"rejected_flips\":{},\"topology_gate_rejections\":{},\"geometry_candidates\":{},\"incumbent_angle_degrees\":[{:.12},{:.12}],\"best_angle_degrees\":[{:.12},{:.12}],\"best_signed_margin_deg\":{:.12},\"best_flips\":[{flips}],\"incumbent_preserved\":{}}}",
        evidence.states_examined,
        evidence.rejected_flip_count,
        evidence.topology_gate_rejections,
        evidence.geometry_candidates,
        evidence.incumbent_angle_range_deg.0,
        evidence.incumbent_angle_range_deg.1,
        evidence.best_angle_range_deg.0,
        evidence.best_angle_range_deg.1,
        evidence.best_signed_margin_deg,
        evidence.incumbent_preserved,
    )
}

fn restored_patch_json(
    restored: &earthmesh_refine_certified::coarsen::RestoredSourcePatch,
) -> String {
    let patch = &restored.patch;
    format!(
        "{{\"level\":\"{:?}\",\"source_faces\":{},\"interior_faces\":{},\"collar_faces\":{},\"hierarchy_parents\":{},\"boundary_cycles\":{},\"protected_exterior_faces\":{},\"source_mesh_fingerprint\":{},\"patch_fingerprint\":{},\"restored_fingerprint\":{}}}",
        patch.level,
        patch.source_faces.len(),
        patch.interior_faces.len(),
        patch.collar_faces.len(),
        patch.hierarchy_parents.len(),
        patch.boundary_cycles.len(),
        patch.protected_exterior_faces.len(),
        patch.source_mesh_fingerprint,
        patch.patch_fingerprint,
        restored.restored_fingerprint,
    )
}

fn promotion_trial_json(
    trial: &earthmesh_refine_certified::coarsen::PromotionTrialEvidence,
) -> String {
    let range = trial
        .angle_range_deg
        .map(|range| format!("[{:.12},{:.12}]", range.0, range.1))
        .unwrap_or_else(|| "null".into());
    let lambda = trial
        .homotopy_lambda
        .map(|lambda| format!("{lambda:.2}"))
        .unwrap_or_else(|| "null".into());
    let reason = match trial.reason.as_ref() {
        None => "None",
        Some(PromotionFailureReason::PatchBoundaryMismatch(_)) => "PatchBoundaryMismatch",
        Some(PromotionFailureReason::HelperVertexBudget { .. }) => "HelperVertexBudget",
        Some(PromotionFailureReason::OrientationGuard) => "OrientationGuard",
        Some(PromotionFailureReason::GeometryNotCertified) => "GeometryNotCertified",
        Some(PromotionFailureReason::NoCompressedExterior) => "NoCompressedExterior",
    };
    format!(
        "{{\"level\":\"{:?}\",\"promoted_source_faces\":{},\"collar_source_faces\":{},\"helper_source_vertices\":{},\"homotopy_lambda\":{lambda},\"angle_degrees\":{range},\"strict\":{},\"protected_exterior_preserved\":{},\"reason\":\"{reason}\"}}",
        trial.level,
        trial.promoted_source_faces,
        trial.collar_source_faces,
        trial.helper_source_vertices,
        trial.strict,
        trial.protected_exterior_preserved,
    )
}

fn embedding_audit(
    reference: &GeometryFailureWitness,
    outcome: &FullPolygonMergeOutcome,
) -> earthmesh_refine_certified::coarsen::EmbeddingAudit {
    let failure = outcome_evidence(outcome)
        .best_geometry_failure
        .as_ref()
        .expect("W3 embedding audit requires a geometry candidate");
    let candidate = failure
        .witness
        .as_deref()
        .expect("W3 embedding audit requires candidate coordinates");
    let topology_key = failure
        .topology_keys
        .first()
        .cloned()
        .expect("W3 geometry candidate must identify its topology");
    audit_embedding_transfer(
        topology_key,
        &reference.mesh,
        &candidate.mesh,
        &candidate.patch.fixed_compact_vertices,
    )
    .unwrap()
}

fn embedding_audit_json(audit: &earthmesh_refine_certified::coarsen::EmbeddingAudit) -> String {
    format!(
        "{{\"common_source_vertices\":{},\"added_source_vertices\":{:?},\"removed_source_vertices\":{:?},\"common_edges\":{},\"added_edges\":{},\"removed_edges\":{},\"common_triangles\":{},\"added_triangles\":{},\"removed_triangles\":{},\"non_positive_triangles\":{},\"crossing_pairs\":{},\"near_degenerate_triangles\":{},\"fixed_only_degenerate_triangles\":{},\"rotation_mismatch_vertices\":{}}}",
        audit.common_source_vertices,
        audit.added_source_vertices,
        audit.removed_source_vertices,
        audit.common_edges,
        audit.added_edges,
        audit.removed_edges,
        audit.common_triangles,
        audit.added_triangles,
        audit.removed_triangles,
        audit.non_positive_triangles,
        audit.crossing_pairs,
        audit.near_degenerate_triangles,
        audit.fixed_only_degenerate_triangles,
        audit.rotation_mismatch_vertices.len(),
    )
}

fn pr52_incumbent(
    source: &earthmesh_refine_certified::MotherGrid,
    component: &earthmesh_refine_certified::coarsen::HierarchyComponent,
    source_levels: &[Option<usize>],
) -> (GeometryFailureWitness, (f64, f64)) {
    let (_, plus_two, range) = pr49_and_pr52_witnesses(source, component, source_levels);
    (plus_two, range)
}

fn pr49_and_pr52_witnesses(
    source: &earthmesh_refine_certified::MotherGrid,
    component: &earthmesh_refine_certified::coarsen::HierarchyComponent,
    source_levels: &[Option<usize>],
) -> (GeometryFailureWitness, GeometryFailureWitness, (f64, f64)) {
    let (plus_one, plus_two, range, _, _) =
        pr49_and_pr52_witnesses_with_topology(source, component, source_levels);
    (plus_one, plus_two, range)
}

fn pr49_and_pr52_witnesses_with_topology(
    source: &earthmesh_refine_certified::MotherGrid,
    component: &earthmesh_refine_certified::coarsen::HierarchyComponent,
    source_levels: &[Option<usize>],
) -> (
    GeometryFailureWitness,
    GeometryFailureWitness,
    (f64, f64),
    Vec<FullPolygonTopologyKey>,
    Vec<GlobalExactSelectedEar>,
) {
    let outcome =
        solve_full_polygon_merge_free_interface_cber_with_targets_active_trust_starts_and_domain(
            source,
            component,
            &BTreeSet::new(),
            FullPolygonCberLimits {
                topology_states: DEFAULT_TOPOLOGY_STATES,
                elastic_iterations: DEFAULT_ELASTIC_ITERATIONS,
            },
            ElasticTargetMode::HierarchyEdgeAreaDegree,
            Some(source_levels),
            &[GeometryStartId::MaterializedSource],
            GeometryDomainId::PlusOneOrdinaryRing,
        );
    let evidence = outcome_evidence(&outcome);
    let selected_ears = evidence.selected_ears.clone();
    let failure = evidence
        .best_geometry_failure
        .as_ref()
        .expect("PR52 reconstruction requires the PR49 +1 witness");
    let witness = failure
        .witness
        .as_deref()
        .expect("PR52 reconstruction requires coordinates");
    let plus_one = witness.clone();
    let inherited = GeometryDomainWitness::from_failure(
        earthmesh_refine_certified::mesh_fingerprint(&source.mesh),
        failure.topology_keys.clone(),
        GeometryDomainId::PlusOneOrdinaryRing,
        witness,
        failure.global_angle_degrees.unwrap(),
        failure.guard_angle_degrees.unwrap(),
    )
    .unwrap();
    let topology_keys = inherited.topology_keys.clone();
    let target_patch = inherited
        .expanded_patch(
            source,
            source_levels,
            &BTreeSet::new(),
            GeometryDomainId::PlusTwoOrdinaryRings,
        )
        .unwrap();
    let DomainContinuationOutcome::Completed(continuation) = continue_nested_domain(
        &inherited,
        target_patch,
        DomainContinuationSchedule::frozen_n6(),
        DomainContinuationMode::InheritedBestMonotone,
    ) else {
        panic!("PR52 continuation must complete")
    };
    (
        plus_one,
        continuation.best_witness.as_ref().clone(),
        continuation.best_angle_range_deg,
        topology_keys,
        selected_ears,
    )
}

fn outcome_evidence(outcome: &FullPolygonMergeOutcome) -> &FullPolygonMergeEvidence {
    match outcome {
        FullPolygonMergeOutcome::Closed(trial) => &trial.evidence,
        FullPolygonMergeOutcome::TopologyFamilyExhaustedNoSolution(evidence)
        | FullPolygonMergeOutcome::SearchBudgetExhausted(evidence)
        | FullPolygonMergeOutcome::InvalidInput { evidence, .. } => evidence,
    }
}

fn update_best_range(best: &mut Option<(f64, f64)>, candidate: Option<(f64, f64)>) {
    let Some(candidate) = candidate else {
        return;
    };
    if best.is_none_or(|current| {
        (candidate.0 - 40.2).min(79.8 - candidate.1) > (current.0 - 40.2).min(79.8 - current.1)
    }) {
        *best = Some(candidate);
    }
}

fn usize_env(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn git_head() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1.0e-10,
        "{actual} != {expected}"
    );
}
