use earthmesh_mesh::{normalize_cartesian_to_radius, CartesianPoint, MeshState};
use earthmesh_refine_certified::{
    coarsen::{
        coarsening_patch_candidates, run_certified_coarsening_epochs,
        solve_certified_coarsening_patch, solve_coarsening_patch, CavitySolveOutcome,
        CertifiedCavitySolveOutcome, CertifiedEpochLimits, CertifiedEpochOutcome, CoarseningPatch,
        EpochCandidateStatus,
    },
    Certificate, MotherGrid, SourceLevelField, VertexAddress,
};

fn mother_with_redundant_site() -> (MeshState, CoarseningPatch) {
    let mother = MotherGrid::generate(1).unwrap();
    let mut mesh = mother.mesh;
    let face = mesh.active_triangle_slots().next().unwrap();
    let [a, b, c] = mesh.triangles()[face];
    let point = normalize_cartesian_to_radius(
        CartesianPoint::new(
            mesh.vertices()[a].x + mesh.vertices()[b].x + mesh.vertices()[c].x,
            mesh.vertices()[a].y + mesh.vertices()[b].y + mesh.vertices()[c].y,
            mesh.vertices()[a].z + mesh.vertices()[b].z + mesh.vertices()[c].z,
        ),
        1.0,
    )
    .unwrap();
    let insertion = mesh
        .insert_site_transactionally(point, |candidate, _| candidate.validate().is_ok())
        .unwrap();
    let mut retained_vertices = mesh
        .triangle_fan(insertion.site)
        .unwrap()
        .into_iter()
        .flat_map(|face| mesh.triangles()[face])
        .filter(|&site| site != insertion.site)
        .collect::<Vec<_>>();
    retained_vertices.sort_unstable();
    retained_vertices.dedup();
    let patch = CoarseningPatch {
        vertex: insertion.site,
        address: VertexAddress::IcosahedronFace {
            face: 0,
            i: 1,
            j: 1,
            k: 1,
            n: 3,
        },
        level: 1,
        parent_faces: Vec::new(),
        boundary_cycle: retained_vertices.clone(),
        retained_vertices,
        removable_vertices: vec![insertion.site],
        transition_halo: Vec::new(),
        requirement_margin: 1,
    };
    (mesh, patch)
}

#[test]
fn hierarchy_site_candidates_are_stable_complete_and_requirement_ranked() {
    let grid = MotherGrid::generate(2).unwrap();
    let open = vec![0; grid.mesh.vertices().len()];
    let candidates = coarsening_patch_candidates(&grid, &open, 0);
    assert!(!candidates.is_empty());
    assert!(candidates.windows(2).all(|pair| {
        pair[0].requirement_margin > pair[1].requirement_margin
            || (pair[0].requirement_margin == pair[1].requirement_margin
                && pair[0].address <= pair[1].address)
    }));
    assert!(candidates.iter().all(|patch| {
        patch.removable_vertices == vec![patch.vertex]
            && patch.boundary_cycle == patch.retained_vertices
            && (5..=6).contains(&patch.boundary_cycle.len())
            && !patch.parent_faces.is_empty()
            && !patch.transition_halo.is_empty()
            && patch.requirement_margin == 0
    }));

    let protected = vec![1; grid.mesh.vertices().len()];
    let protected_candidates = coarsening_patch_candidates(&grid, &protected, 0);
    assert_eq!(protected_candidates.len(), candidates.len());
    assert!(protected_candidates
        .iter()
        .all(|patch| patch.requirement_margin < 0));
}

#[test]
fn bounded_certified_cavity_exhaustion_and_infeasibility_are_atomic() {
    let grid = MotherGrid::generate(2).unwrap();
    let required = vec![0; grid.mesh.vertices().len()];
    let patch = coarsening_patch_candidates(&grid, &required, 0)
        .into_iter()
        .next()
        .unwrap();
    let mut mesh = grid.mesh.clone();
    let before = mesh.clone();

    assert!(matches!(
        solve_coarsening_patch(&mut mesh, &patch, 0),
        CavitySolveOutcome::SearchBudgetExhausted { states_examined: 0 }
    ));
    assert_eq!(mesh, before);

    let outcome = solve_coarsening_patch(&mut mesh, &patch, usize::MAX);
    let CavitySolveOutcome::ProvenInfeasible {
        states_examined, ..
    } = outcome
    else {
        panic!("the complete strict ring search must be infeasible: {outcome:?}");
    };
    assert!(states_examined > 0);
    assert_eq!(mesh, before);
}

#[test]
fn certified_hierarchy_cavity_relocates_its_transition_block_and_commits() {
    let grid = MotherGrid::generate(2).unwrap();
    let source_mesh = grid.mesh.clone();
    let source_levels = SourceLevelField::from_active_voronoi_cells(
        &source_mesh,
        vec![0; source_mesh.vertex_count()],
    )
    .unwrap();
    let patch =
        coarsening_patch_candidates(&grid, &vec![0; grid.mesh.vertices().len()], 0).remove(0);
    let mut mesh = grid.mesh;
    let initial_vertices = mesh.vertex_count();
    let mut delivered = vec![Some(1); mesh.vertices().len()];

    let outcome = solve_certified_coarsening_patch(
        &mut mesh,
        &patch,
        &source_mesh,
        &source_levels,
        &mut delivered,
        0,
        1,
        usize::MAX,
    );
    let CertifiedCavitySolveOutcome::Feasible {
        geometry,
        requirements,
        ..
    } = outcome
    else {
        panic!("the hierarchy transition block must relocate and certify: {outcome:?}");
    };
    assert_eq!(mesh.vertex_count(), initial_vertices - 1);
    geometry.require_geometry_gates().unwrap();
    Certificate::final_delivery()
        .verify_geometry(&mesh)
        .unwrap();
    assert_eq!(requirements.physical_residuals(), 0);
    assert_eq!(requirements.balance_residuals(), 0);
    assert_eq!(delivered[patch.vertex], None);
    assert!(patch
        .retained_vertices
        .iter()
        .all(|&site| delivered[site] == Some(0)));
    assert!(mesh
        .active_vertex_slots()
        .any(|site| delivered[site] == Some(1)));
}

#[test]
fn relocation_trials_are_charged_to_the_search_budget() {
    let grid = MotherGrid::generate(6).unwrap();
    let source_mesh = grid.mesh.clone();
    let source_levels = SourceLevelField::from_active_voronoi_cells(
        &source_mesh,
        vec![0; source_mesh.vertex_count()],
    )
    .unwrap();
    let patch = coarsening_patch_candidates(&grid, &vec![0; grid.mesh.vertices().len()], 0)
        .into_iter()
        .find(|patch| {
            patch.address
                == VertexAddress::IcosahedronEdge {
                    a: 0,
                    b: 10,
                    step: 3,
                    n: 6,
                }
        })
        .unwrap();
    let mut mesh = grid.mesh;
    let before = mesh.clone();
    let mut delivered = vec![Some(1); mesh.vertices().len()];

    let outcome = solve_certified_coarsening_patch(
        &mut mesh,
        &patch,
        &source_mesh,
        &source_levels,
        &mut delivered,
        0,
        1,
        1,
    );
    assert!(matches!(
        outcome,
        CertifiedCavitySolveOutcome::SearchBudgetExhausted { states_examined: 1 }
    ));
    assert_eq!(mesh, before);
}

#[test]
fn finite_cavity_commits_when_it_restores_a_strict_mother_triangle() {
    let (mut mesh, patch) = mother_with_redundant_site();
    let before_vertices = mesh.vertex_count();

    let outcome = solve_coarsening_patch(&mut mesh, &patch, usize::MAX);
    let CavitySolveOutcome::Feasible { certificate, .. } = outcome else {
        panic!("the inserted site must retire back to a strict mother mesh: {outcome:?}");
    };
    assert_eq!(mesh.vertex_count(), before_vertices - 1);
    certificate.require_geometry_gates().unwrap();
    Certificate::final_delivery()
        .verify_geometry(&mesh)
        .unwrap();
}

#[test]
fn certified_cavity_commits_only_with_final_cell_requirement_evidence() {
    let (mut mesh, patch) = mother_with_redundant_site();
    let source_mesh = mesh.clone();
    let source_levels = SourceLevelField::from_active_voronoi_cells(
        &source_mesh,
        vec![0; source_mesh.vertex_count()],
    )
    .unwrap();
    let mut delivered = mesh
        .vertices()
        .iter()
        .enumerate()
        .map(|(site, _)| mesh.is_vertex_live(site).then_some(1))
        .collect::<Vec<_>>();

    let outcome = solve_certified_coarsening_patch(
        &mut mesh,
        &patch,
        &source_mesh,
        &source_levels,
        &mut delivered,
        0,
        1,
        usize::MAX,
    );
    let CertifiedCavitySolveOutcome::Feasible {
        geometry,
        requirements,
        ..
    } = outcome
    else {
        panic!("the certified redundant site must retire: {outcome:?}");
    };
    geometry.require_geometry_gates().unwrap();
    assert_eq!(requirements.physical_residuals(), 0);
    assert_eq!(requirements.balance_residuals(), 0);
    assert_eq!(delivered[patch.vertex], None);
    assert!(patch
        .retained_vertices
        .iter()
        .all(|&site| delivered[site] == Some(0)));
}

#[test]
fn certified_cavity_rolls_back_when_a_source_cell_requires_the_fine_level() {
    let (mut mesh, patch) = mother_with_redundant_site();
    let source_mesh = mesh.clone();
    let source_levels = SourceLevelField::from_active_voronoi_cells(
        &source_mesh,
        source_mesh
            .active_vertex_slots()
            .map(|site| usize::from(site == patch.vertex))
            .collect(),
    )
    .unwrap();
    let mut delivered = mesh
        .vertices()
        .iter()
        .enumerate()
        .map(|(site, _)| mesh.is_vertex_live(site).then_some(1))
        .collect::<Vec<_>>();
    let before_mesh = mesh.clone();
    let before_levels = delivered.clone();

    let outcome = solve_certified_coarsening_patch(
        &mut mesh,
        &patch,
        &source_mesh,
        &source_levels,
        &mut delivered,
        0,
        1,
        usize::MAX,
    );
    assert!(matches!(
        outcome,
        CertifiedCavitySolveOutcome::ProvenInfeasible {
            states_examined: 1,
            ..
        }
    ));
    assert_eq!(mesh, before_mesh);
    assert_eq!(delivered, before_levels);
}

#[test]
fn certified_epochs_are_finite_stable_and_end_after_a_zero_commit_epoch() {
    let (mut mesh, patch) = mother_with_redundant_site();
    let source_mesh = mesh.clone();
    let source_levels = SourceLevelField::from_active_voronoi_cells(
        &source_mesh,
        vec![0; source_mesh.vertex_count()],
    )
    .unwrap();
    let mut delivered = vec![Some(1); mesh.vertices().len()];

    let outcome = run_certified_coarsening_epochs(
        &mut mesh,
        vec![patch.clone(), patch],
        &source_mesh,
        &source_levels,
        &mut delivered,
        0,
        CertifiedEpochLimits {
            max_adjacent_level_delta: 1,
            search_state_budget: usize::MAX,
        },
    );
    let CertifiedEpochOutcome::Certified {
        report,
        requirements,
    } = outcome
    else {
        panic!("the redundant-site epoch must certify: {outcome:?}");
    };
    assert_eq!(report.epoch_commits, vec![1, 0]);
    assert_eq!(report.epochs(), 2);
    assert_eq!(report.candidates_attempted(), 1);
    assert_eq!(report.candidates_accepted(), 1);
    assert_eq!(report.vertices_removed(), 1);
    assert_eq!(report.attempts[0].status, EpochCandidateStatus::Committed);
    assert_eq!(requirements.physical_residuals(), 0);
    assert_eq!(requirements.balance_residuals(), 0);
}

#[test]
fn certified_epoch_budget_exhaustion_is_explicit_and_atomic() {
    let (mut mesh, patch) = mother_with_redundant_site();
    let source_mesh = mesh.clone();
    let source_levels = SourceLevelField::from_active_voronoi_cells(
        &source_mesh,
        vec![0; source_mesh.vertex_count()],
    )
    .unwrap();
    let mut delivered = vec![Some(1); mesh.vertices().len()];
    let before_mesh = mesh.clone();
    let before_levels = delivered.clone();

    let outcome = run_certified_coarsening_epochs(
        &mut mesh,
        vec![patch],
        &source_mesh,
        &source_levels,
        &mut delivered,
        0,
        CertifiedEpochLimits {
            max_adjacent_level_delta: 1,
            search_state_budget: 0,
        },
    );
    let CertifiedEpochOutcome::SearchBudgetExhausted { report } = outcome else {
        panic!("zero budget must stop explicitly: {outcome:?}");
    };
    assert_eq!(report.epochs(), 0);
    assert_eq!(report.candidates_attempted(), 0);
    assert_eq!(mesh, before_mesh);
    assert_eq!(delivered, before_levels);
}

#[test]
fn infeasible_epoch_promotes_the_blocked_transition_and_keeps_the_fine_mesh() {
    let (mut mesh, patch) = mother_with_redundant_site();
    let source_mesh = mesh.clone();
    let source_levels = SourceLevelField::from_active_voronoi_cells(
        &source_mesh,
        vec![1; source_mesh.vertex_count()],
    )
    .unwrap();
    let mut delivered = vec![Some(1); mesh.vertices().len()];
    let before = mesh.clone();

    let outcome = run_certified_coarsening_epochs(
        &mut mesh,
        vec![patch],
        &source_mesh,
        &source_levels,
        &mut delivered,
        0,
        CertifiedEpochLimits {
            max_adjacent_level_delta: 1,
            search_state_budget: usize::MAX,
        },
    );
    let CertifiedEpochOutcome::Certified { report, .. } = outcome else {
        panic!("keeping the fine blocked region must certify: {outcome:?}");
    };
    assert_eq!(report.epoch_commits, vec![0]);
    assert_eq!(
        report.attempts[0].status,
        EpochCandidateStatus::ProvenInfeasible
    );
    assert_eq!(report.transition_promotion.blocked_regions.len(), 1);
    assert_eq!(report.transition_promotion.directly_promoted_sites, 0);
    assert_eq!(mesh, before);
    assert!(delivered.iter().all(|level| *level == Some(1)));
}
