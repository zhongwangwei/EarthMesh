use earthmesh_refine_certified::coarsen::{
    build_essential_cycle_problem, build_face_band_problem, essential_cycle_from_face_band_plan,
    essential_cycle_seam_parity, face_band_plan_from_essential_cycle, n12_lifted_n6_fixture,
    n6_legacy_mixed_fixture, solve_exact_face_bands, validate_selected_essential_cycle,
    EssentialCycleKey, EssentialCycleProblem, FaceBandLimits, FaceBandPlan, FaceBandProblem,
    FaceBandSolveOutcome, HierarchyComponent, RetainedCoreCorridorFamily,
};
use earthmesh_refine_certified::MotherGrid;
use std::collections::{BTreeMap, BTreeSet};

const LEGACY_LIMIT: u64 = 1_000_000;

#[test]
fn simple_essential_cycle_recovers_valid_labels() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    assert_round_trip(&source, &component, LEGACY_LIMIT);
}

#[test]
fn n12_known_w2_plan_round_trips_losslessly() {
    let fixture = n12_lifted_n6_fixture().unwrap();
    assert_round_trip(&fixture.source, &fixture.component, 16_384);
}

#[test]
fn open_path_rejected() {
    let (_, _, problem, cycle, selected) = frozen_cycle();
    assert!(validate_selected_essential_cycle(&problem, &selected[..selected.len() - 1]).is_err());
    assert_eq!(cycle.ordered_vertices.len(), selected.len());
}

#[test]
fn boundary_touch_rejected() {
    let (_, _, mut problem, cycle, selected) = frozen_cycle();
    problem
        .coarse_boundary_vertices
        .insert(cycle.ordered_vertices[0].clone());
    assert!(validate_selected_essential_cycle(&problem, &selected).is_err());
}

#[test]
fn contractible_cycle_rejected() {
    let (_, _, problem, _, _) = frozen_cycle();
    let triangle = contractible_triangle(&problem, &BTreeSet::new());
    assert!(validate_selected_essential_cycle(&problem, &triangle).is_err());
}

#[test]
fn contractible_cycle_has_even_parity() {
    let (_, _, problem, _, _) = frozen_cycle();
    let triangle = contractible_triangle(&problem, &BTreeSet::new());
    assert_eq!(essential_cycle_seam_parity(&problem, triangle), 0);
}

#[test]
fn two_cycles_rejected() {
    let (_, _, mut problem, _, selected) = frozen_cycle();
    let vertices = (0..3)
        .map(
            |offset| earthmesh_refine_certified::coarsen::CanonicalVertexId::FrozenSourceSlot {
                source_n: problem.source_n,
                slot: usize::MAX - offset,
            },
        )
        .collect::<Vec<_>>();
    let mut combined = selected;
    for index in 0..3 {
        let (left, right) =
            ordered_pair(vertices[index].clone(), vertices[(index + 1) % 3].clone());
        combined.push(problem.candidate_edges.len());
        problem
            .candidate_edges
            .push(earthmesh_refine_certified::coarsen::CanonicalEdgeId {
                vertices: [left, right],
            });
    }
    assert!(validate_selected_essential_cycle(&problem, &combined).is_err());
}

#[test]
fn essential_cycle_has_odd_parity() {
    let (_, _, problem, _, selected) = frozen_cycle();
    assert_eq!(essential_cycle_seam_parity(&problem, selected), 1);
}

#[test]
fn parity_is_only_a_prune_not_final_certificate() {
    let (_, _, mut problem, _, _) = frozen_cycle();
    let triangle = contractible_triangle(&problem, &BTreeSet::new());
    for index in 0..problem.dual_seam_crossing_edges.len() {
        problem.dual_seam_crossing_edges.set(index, false).unwrap();
    }
    problem
        .dual_seam_crossing_edges
        .set(triangle[0], true)
        .unwrap();
    assert_eq!(essential_cycle_seam_parity(&problem, triangle.clone()), 1);
    assert!(validate_selected_essential_cycle(&problem, &triangle).is_err());
}

fn assert_round_trip(source: &MotherGrid, component: &HierarchyComponent, limit: u64) {
    let face_problem = build_face_band_problem(source, component, 2).unwrap();
    let plan = closed_plan(&face_problem, limit);
    let cycle_problem = build_essential_cycle_problem(
        source,
        &face_problem,
        component.core_parents.iter().copied(),
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor,
    )
    .unwrap();
    let cycle =
        essential_cycle_from_face_band_plan(source, &face_problem, &cycle_problem, &plan).unwrap();
    let recovered =
        face_band_plan_from_essential_cycle(source, &face_problem, &cycle_problem, &cycle).unwrap();
    assert_eq!(recovered, plan);
}

fn frozen_cycle() -> (
    MotherGrid,
    FaceBandProblem,
    EssentialCycleProblem,
    EssentialCycleKey,
    Vec<usize>,
) {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let face_problem = build_face_band_problem(&source, &component, 2).unwrap();
    let plan = closed_plan(&face_problem, LEGACY_LIMIT);
    let problem = build_essential_cycle_problem(
        &source,
        &face_problem,
        component.core_parents.iter().copied(),
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor,
    )
    .unwrap();
    let cycle =
        essential_cycle_from_face_band_plan(&source, &face_problem, &problem, &plan).unwrap();
    let edge_index = problem
        .candidate_edges
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, edge)| (edge, index))
        .collect::<BTreeMap<_, _>>();
    let selected = (0..cycle.ordered_vertices.len())
        .map(|index| {
            let left = cycle.ordered_vertices[index].clone();
            let right = cycle.ordered_vertices[(index + 1) % cycle.ordered_vertices.len()].clone();
            let edge = if left <= right {
                earthmesh_refine_certified::coarsen::CanonicalEdgeId {
                    vertices: [left, right],
                }
            } else {
                earthmesh_refine_certified::coarsen::CanonicalEdgeId {
                    vertices: [right, left],
                }
            };
            edge_index[&edge]
        })
        .collect();
    (source, face_problem, problem, cycle, selected)
}

fn contractible_triangle(
    problem: &EssentialCycleProblem,
    excluded_vertices: &BTreeSet<earthmesh_refine_certified::coarsen::CanonicalVertexId>,
) -> Vec<usize> {
    let index = problem
        .candidate_edges
        .iter()
        .enumerate()
        .map(|(index, edge)| ((edge.vertices[0].clone(), edge.vertices[1].clone()), index))
        .collect::<BTreeMap<_, _>>();
    for first in 0..problem.candidate_edges.len() {
        let [a, b] = problem.candidate_edges[first].vertices.clone();
        if excluded_vertices.contains(&a) || excluded_vertices.contains(&b) {
            continue;
        }
        for c in &problem.candidate_vertices {
            if c == &a || c == &b || excluded_vertices.contains(c) {
                continue;
            }
            let ac = ordered_pair(a.clone(), c.clone());
            let bc = ordered_pair(b.clone(), c.clone());
            if let (Some(&second), Some(&third)) = (index.get(&ac), index.get(&bc)) {
                let triangle = vec![first, second, third];
                if essential_cycle_seam_parity(problem, triangle.clone()) == 0 {
                    return triangle;
                }
            }
        }
    }
    panic!("fixture must contain a boundary-free contractible candidate triangle")
}

fn ordered_pair(
    left: earthmesh_refine_certified::coarsen::CanonicalVertexId,
    right: earthmesh_refine_certified::coarsen::CanonicalVertexId,
) -> (
    earthmesh_refine_certified::coarsen::CanonicalVertexId,
    earthmesh_refine_certified::coarsen::CanonicalVertexId,
) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}

fn closed_plan(problem: &FaceBandProblem, limit: u64) -> FaceBandPlan {
    let FaceBandSolveOutcome::Closed(plan, _) = solve_exact_face_bands(
        problem,
        FaceBandLimits {
            maximum_states: limit,
        },
    ) else {
        panic!("known W2 fixture must close")
    };
    *plan
}
