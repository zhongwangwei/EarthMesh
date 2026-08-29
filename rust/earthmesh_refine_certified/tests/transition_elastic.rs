use std::collections::{BTreeMap, BTreeSet};

use earthmesh_mesh::{magnitude, CartesianPoint};
use earthmesh_refine_certified::{
    coarsen::{
        solve_elastic_patch, solve_elastic_transition_block, ElasticBlockLimits,
        ElasticBlockOutcome, ElasticPatch, HierarchyLeafMesh, TransitionBoundary,
        TransitionTopologyCandidate, TransitionTopologyReport, TransitionTopologyTrial,
    },
    Certificate, MotherGrid, VertexAddress,
};

const REPAIR_LIMITS: ElasticBlockLimits = ElasticBlockLimits {
    elastic_iterations: 256,
};

fn blend_on_sphere(left: CartesianPoint, right: CartesianPoint, fraction: f64) -> CartesianPoint {
    let blended = CartesianPoint::new(
        left.x * (1.0 - fraction) + right.x * fraction,
        left.y * (1.0 - fraction) + right.y * fraction,
        left.z * (1.0 - fraction) + right.z * fraction,
    );
    let scale = magnitude(left) / magnitude(blended);
    CartesianPoint::new(blended.x * scale, blended.y * scale, blended.z * scale)
}

fn elastic_fixture() -> (HierarchyLeafMesh, ElasticPatch) {
    let grid = MotherGrid::generate(8).unwrap();
    Certificate::internal().verify_geometry(&grid.mesh).unwrap();
    let seed = grid
        .mesh
        .active_triangle_slots()
        .find(|&face| {
            grid.mesh.triangles()[face].iter().all(|&site| {
                matches!(
                    grid.addresses[site],
                    Some(VertexAddress::IcosahedronFace { .. })
                )
            })
        })
        .unwrap();
    let movable = grid.mesh.triangles()[seed]
        .into_iter()
        .collect::<BTreeSet<_>>();
    let guard = grid
        .mesh
        .active_triangle_slots()
        .filter(|&face| {
            grid.mesh.triangles()[face]
                .iter()
                .any(|site| movable.contains(site))
        })
        .collect::<BTreeSet<_>>();
    let fixed = guard
        .iter()
        .flat_map(|&face| grid.mesh.triangles()[face])
        .filter(|site| !movable.contains(site))
        .collect::<BTreeSet<_>>();
    let patch = ElasticPatch {
        topology: TransitionTopologyCandidate {
            component_id: 29,
            topology_id: 0,
            source_triangles: guard
                .iter()
                .map(|&face| grid.mesh.triangles()[face])
                .collect(),
            source_active_vertices: movable.iter().chain(&fixed).copied().collect(),
            source_degree_forecast: BTreeMap::new(),
        },
        reference_positions: grid.mesh.vertices().to_vec(),
        fixed_compact_vertices: fixed.iter().copied().collect(),
        movable_compact_vertices: movable.iter().copied().collect(),
        guard_faces: guard.iter().copied().collect(),
    };

    let targets = movable
        .iter()
        .map(|&site| {
            let target = guard
                .iter()
                .flat_map(|&face| grid.mesh.triangles()[face])
                .find(|candidate| fixed.contains(candidate) && *candidate != site)
                .unwrap();
            (site, target)
        })
        .collect::<Vec<_>>();
    let mut distorted = None;
    for fraction in [0.05, 0.1, 0.15, 0.2] {
        let mut candidate = grid.mesh.clone();
        for &(site, target) in &targets {
            candidate.move_vertex(
                site,
                blend_on_sphere(
                    grid.mesh.vertices()[site],
                    grid.mesh.vertices()[target],
                    fraction,
                ),
            );
        }
        if Certificate::final_delivery()
            .verify_geometry(&candidate)
            .is_err()
        {
            distorted = Some(candidate);
            break;
        }
    }
    let mesh = distorted.expect("deterministic transition displacement must fail geometry");
    let source_vertex_slots = (0..mesh.vertices().len())
        .map(|site| (site >= 2).then_some(site))
        .collect();
    (
        HierarchyLeafMesh {
            mesh,
            triangle_addresses: grid.triangle_addresses,
            source_vertex_slots,
        },
        patch,
    )
}

#[test]
fn cber_repairs_fixed_topology_with_synchronous_transition_only_motion() {
    let (source, patch) = elastic_fixture();
    let before = source.mesh.clone();

    let first = solve_elastic_patch(&source, patch.clone(), REPAIR_LIMITS);
    let second = solve_elastic_patch(&source, patch, REPAIR_LIMITS);
    assert_eq!(first, second, "CBER must be deterministic");

    let ElasticBlockOutcome::Certified(trial) = first else {
        panic!("CBER must repair the fixed topology fixture: {first:?}");
    };
    trial.geometry.require_geometry_gates().unwrap();
    Certificate::final_delivery()
        .verify_geometry(&trial.mesh.mesh)
        .unwrap();
    assert_eq!(trial.mesh.mesh.triangles(), before.triangles());
    assert_eq!(trial.mesh.mesh.neighbours(), before.neighbours());

    let moved = trial
        .report
        .moved_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let allowed = trial
        .patch
        .movable_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    assert!(!moved.is_empty());
    assert!(moved.is_subset(&allowed));
    for site in before.active_vertex_slots() {
        assert_eq!(
            before.vertices()[site] != trial.mesh.mesh.vertices()[site],
            moved.contains(&site),
            "unexpected motion at compact site {site}"
        );
    }
}

#[test]
fn cber_zero_budget_is_explicit_and_keeps_the_input_unchanged() {
    let (source, patch) = elastic_fixture();
    let before = source.clone();
    assert!(matches!(
        solve_elastic_patch(
            &source,
            patch,
            ElasticBlockLimits {
                elastic_iterations: 0,
            },
        ),
        ElasticBlockOutcome::SearchBudgetExhausted {
            elastic_iterations: 0,
            ..
        }
    ));
    assert_eq!(source, before);
}

#[test]
fn transition_trial_entry_derives_the_same_coordinate_only_block() {
    let (source, patch) = elastic_fixture();
    let transition = TransitionTopologyTrial {
        mesh: source,
        boundary: TransitionBoundary {
            fine_outer_cycles: vec![patch.fixed_compact_vertices.clone()],
            ..TransitionBoundary::default()
        },
        candidate: patch.topology,
        report: TransitionTopologyReport {
            component_id: 29,
            transition_parent_count: 1,
            ..TransitionTopologyReport::default()
        },
    };
    assert!(matches!(
        solve_elastic_transition_block(&transition, REPAIR_LIMITS),
        ElasticBlockOutcome::Certified(_)
    ));
}

#[test]
fn cber_rejects_an_empty_movable_block_before_spending_budget() {
    let (source, mut patch) = elastic_fixture();
    patch.movable_compact_vertices.clear();
    assert!(matches!(
        solve_elastic_patch(
            &source,
            patch,
            ElasticBlockLimits {
                elastic_iterations: 1,
            },
        ),
        ElasticBlockOutcome::InvalidPatch { .. }
    ));
}
