use std::collections::{BTreeSet, HashSet};

use earthmesh_mesh::{orientation_on_sphere, Sign};
use earthmesh_refine_certified::{
    coarsen::{
        solve_transition_topology, ElasticPatch, HierarchyComponent, TransitionTopologyLimits,
        TransitionTopologyOutcome, TransitionTopologyTrial,
    },
    MotherGrid, TriangleAddress, TriangleOrientation, VertexAddress,
};

fn face_slot(grid: &MotherGrid, address: TriangleAddress) -> usize {
    grid.triangle_addresses
        .iter()
        .position(|actual| *actual == Some(address))
        .unwrap()
}

fn parent_sites(grid: &MotherGrid, parent: TriangleAddress) -> ([usize; 3], [usize; 3]) {
    let children = parent.children_2_to_1().unwrap();
    let child_triangles = children.map(|child| grid.mesh.triangles()[face_slot(grid, child)]);
    let corners = match parent.orientation {
        TriangleOrientation::Up => [
            child_triangles[0][0],
            child_triangles[1][1],
            child_triangles[2][2],
        ],
        TriangleOrientation::Down => [
            child_triangles[0][0],
            child_triangles[2][1],
            child_triangles[1][2],
        ],
    };
    let mut edge_set = BTreeSet::new();
    let mut sites = BTreeSet::new();
    for triangle in child_triangles {
        for site in triangle {
            sites.insert(site);
        }
        for [a, b] in [
            [triangle[0], triangle[1]],
            [triangle[1], triangle[2]],
            [triangle[2], triangle[0]],
        ] {
            edge_set.insert((a.min(b), a.max(b)));
        }
    }
    let mut midpoints = [0; 3];
    for side in 0..3 {
        let a = corners[side];
        let b = corners[(side + 1) % 3];
        midpoints[side] = sites
            .iter()
            .copied()
            .find(|&m| {
                m != a
                    && m != b
                    && edge_set.contains(&(a.min(m), a.max(m)))
                    && edge_set.contains(&(m.min(b), m.max(b)))
            })
            .unwrap();
    }
    (corners, midpoints)
}

fn parent_neighbours(grid: &MotherGrid, parent: TriangleAddress) -> Vec<TriangleAddress> {
    let children = parent
        .children_2_to_1()
        .unwrap()
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut neighbours = BTreeSet::new();
    for face in grid.mesh.active_triangle_slots() {
        if !grid.triangle_addresses[face].is_some_and(|address| children.contains(&address)) {
            continue;
        }
        for &neighbour in &grid.mesh.neighbours()[face] {
            let other = grid.triangle_addresses[neighbour]
                .and_then(TriangleAddress::parent_2_to_1)
                .unwrap();
            if other != parent {
                neighbours.insert(other);
            }
        }
    }
    neighbours.into_iter().collect()
}

fn level_three_fixture() -> (MotherGrid, TriangleAddress, Vec<TriangleAddress>) {
    let grid = MotherGrid::generate(8).unwrap();
    let core = TriangleAddress {
        base_face: 0,
        i: 1,
        j: 1,
        n: 4,
        orientation: TriangleOrientation::Down,
    };
    let transition = parent_neighbours(&grid, core);
    assert_eq!(transition.len(), 3);
    (grid, core, transition)
}

fn component(
    core: TriangleAddress,
    transition: Vec<TriangleAddress>,
    all_core: bool,
) -> HierarchyComponent {
    let mut parents = vec![core];
    parents.extend(transition.iter().copied());
    parents.sort_unstable();
    HierarchyComponent {
        id: 7,
        parents: parents.clone(),
        boundary_edges: Vec::new(),
        core_parents: if all_core { parents } else { vec![core] },
        transition_parents: if all_core { Vec::new() } else { transition },
    }
}

fn whole_sphere_component(coarse_n: usize) -> HierarchyComponent {
    let parents = MotherGrid::generate(coarse_n)
        .unwrap()
        .triangle_addresses
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    HierarchyComponent {
        id: 9,
        parents: parents.clone(),
        boundary_edges: Vec::new(),
        core_parents: parents,
        transition_parents: Vec::new(),
    }
}

fn flattened_custom_triangles(trial: &TransitionTopologyTrial) -> Vec<[usize; 3]> {
    trial
        .candidate
        .custom_transition_triangles
        .values()
        .flat_map(|triangles| triangles.iter().copied())
        .collect()
}

fn custom_parent_keys(trial: &TransitionTopologyTrial) -> Vec<TriangleAddress> {
    trial
        .candidate
        .custom_transition_triangles
        .keys()
        .copied()
        .collect()
}

fn assert_candidate_delta(
    trial: &TransitionTopologyTrial,
    core: Vec<TriangleAddress>,
    custom: Vec<TriangleAddress>,
) {
    assert_eq!(trial.candidate.core_parents, core);
    assert_eq!(custom_parent_keys(trial), custom);
    assert_eq!(
        trial.candidate.source_triangles,
        flattened_custom_triangles(trial)
    );
}

fn output_source_slots(trial: &TransitionTopologyTrial) -> BTreeSet<usize> {
    trial
        .mesh
        .source_vertex_slots
        .iter()
        .flatten()
        .copied()
        .collect()
}

fn topology(mesh: &earthmesh_mesh::MeshState) -> (usize, usize, usize, isize) {
    let mut edges = HashSet::new();
    for face in mesh.active_triangle_slots() {
        let [a, b, c] = mesh.triangles()[face];
        for [u, v] in [[a, b], [b, c], [c, a]] {
            edges.insert((u.min(v), u.max(v)));
        }
    }
    let vertices = mesh.vertex_count();
    let faces = mesh.triangle_count();
    (
        vertices,
        edges.len(),
        faces,
        vertices as isize - edges.len() as isize + faces as isize,
    )
}

fn assert_hard_topology_gates(mesh: &earthmesh_mesh::MeshState) {
    mesh.validate().unwrap();
    assert_eq!(mesh.open_edge_count(), 0);
    assert_eq!(topology(mesh).3, 2);

    let mut degrees = vec![0usize; mesh.vertices().len()];
    let mut triangles = HashSet::new();
    for face in mesh.active_triangle_slots() {
        let triangle = mesh.triangles()[face];
        for vertex in triangle {
            degrees[vertex] += 1;
        }
        let mut canonical = triangle;
        canonical.sort_unstable();
        assert!(
            triangles.insert(canonical),
            "duplicate triangle {canonical:?}"
        );
        assert_eq!(
            orientation_on_sphere(
                mesh.vertices()[triangle[0]],
                mesh.vertices()[triangle[1]],
                mesh.vertices()[triangle[2]],
            )
            .unwrap(),
            Sign::Positive
        );
    }
    assert!(degrees
        .into_iter()
        .all(|degree| degree == 0 || (5..=7).contains(&degree)));
}

fn assert_source_slot_forecast(trial: &TransitionTopologyTrial, source: &MotherGrid) {
    let mut source_to_output = vec![None; source.mesh.vertices().len()];
    for (output, source_slot) in trial.mesh.source_vertex_slots.iter().copied().enumerate() {
        if let Some(source_slot) = source_slot {
            source_to_output[source_slot] = Some(output);
        }
    }
    let mut output_degrees = vec![0usize; trial.mesh.mesh.vertices().len()];
    for face in trial.mesh.mesh.active_triangle_slots() {
        for vertex in trial.mesh.mesh.triangles()[face] {
            output_degrees[vertex] += 1;
        }
    }
    for (&source_slot, &forecast) in &trial.candidate.source_degree_forecast {
        match source_to_output[source_slot] {
            Some(output) => assert_eq!(output_degrees[output], forecast),
            None => assert_eq!(forecast, 0),
        }
    }
    for triangle in &trial.candidate.source_triangles {
        assert_eq!(
            orientation_on_sphere(
                source.mesh.vertices()[triangle[0]],
                source.mesh.vertices()[triangle[1]],
                source.mesh.vertices()[triangle[2]],
            )
            .unwrap(),
            Sign::Positive
        );
    }
}

fn assert_custom_faces_have_elastic_guard_coverage(trial: &TransitionTopologyTrial) {
    let fixed_sources = trial
        .boundary
        .fine_outer_cycles
        .iter()
        .chain(&trial.boundary.coarse_inner_cycles)
        .flat_map(|cycle| cycle.iter().copied())
        .chain(trial.boundary.seam.iter().copied())
        .chain(trial.boundary.pentagon.iter().copied())
        .collect::<BTreeSet<_>>();
    let source_active = trial
        .candidate
        .source_active_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let has_nonboundary_custom_vertex = trial.mesh.mesh.active_triangle_slots().any(|face| {
        trial.mesh.triangle_addresses[face].is_none()
            && trial.mesh.mesh.triangles()[face].iter().any(|&compact| {
                trial.mesh.source_vertex_slots[compact]
                    .is_some_and(|source| !fixed_sources.contains(&source))
            })
    });
    if !has_nonboundary_custom_vertex {
        return;
    }
    let patch = ElasticPatch::from_transition(trial).unwrap();
    let movable = patch
        .movable_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let guard = patch.guard_faces.iter().copied().collect::<BTreeSet<_>>();

    for face in trial.mesh.mesh.active_triangle_slots() {
        if trial.mesh.triangle_addresses[face].is_some() {
            continue;
        }
        for compact in trial.mesh.mesh.triangles()[face] {
            let source = trial.mesh.source_vertex_slots[compact].unwrap();
            if fixed_sources.contains(&source) {
                continue;
            }
            assert!(source_active.contains(&source));
            assert!(movable.contains(&compact));
            assert!(guard
                .iter()
                .any(|&guard_face| trial.mesh.mesh.triangles()[guard_face].contains(&compact)));
        }
    }
}

#[test]
fn level_three_to_two_mixed_transition_closes_without_hanging_nodes() {
    let (fine, core, transition) = level_three_fixture();
    let source_vertices = fine.mesh.vertex_count();
    let source_faces = fine.mesh.triangle_count();
    let component = component(core, transition.clone(), false);

    let TransitionTopologyOutcome::Closed(trial) = solve_transition_topology(
        &fine,
        &component,
        TransitionTopologyLimits {
            topology_states: 1_000,
            maximum_halo_expansions: 0,
        },
    ) else {
        panic!("the nontrivial Level 3/2 fixture must close");
    };

    assert_hard_topology_gates(&trial.mesh.mesh);
    assert_source_slot_forecast(&trial, &fine);
    assert_custom_faces_have_elastic_guard_coverage(&trial);
    assert_candidate_delta(&trial, vec![core], transition.clone());
    assert!(trial
        .mesh
        .triangle_addresses
        .iter()
        .flatten()
        .any(|address| address.n == 4));
    assert!(trial
        .mesh
        .triangle_addresses
        .iter()
        .flatten()
        .any(|address| address.n == 8));
    assert!(trial.mesh.mesh.vertex_count() < source_vertices);
    assert!(trial.mesh.mesh.triangle_count() < source_faces);
    assert_eq!(trial.boundary.coarse_inner_cycles.len(), 1);
    assert_eq!(trial.boundary.coarse_inner_cycles[0].len(), 3);
    assert_eq!(trial.boundary.fine_outer_cycles.len(), 1);
    assert_eq!(trial.report.core_parent_count, 1);
    assert_eq!(trial.report.transition_parent_count, 3);
    assert_eq!(trial.report.halo_expansions, 0);
    assert!(trial.report.topology_states > 0);
}

#[test]
fn topology_cursor_reports_strictly_increasing_ordinals() {
    let fine = MotherGrid::generate(64).unwrap();
    let coarse = MotherGrid::generate(32).unwrap();
    let core = coarse
        .triangle_addresses
        .iter()
        .flatten()
        .copied()
        .filter(|parent| {
            parent.base_face == 0 && parent.i >= 8 && parent.j >= 8 && parent.i + parent.j < 24
        })
        .collect::<Vec<_>>();
    let core_set = core.iter().copied().collect::<BTreeSet<_>>();
    let transition = core
        .iter()
        .flat_map(|parent| parent_neighbours(&fine, *parent))
        .filter(|parent| !core_set.contains(parent))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut parents = core.clone();
    parents.extend(transition.iter().copied());
    parents.sort_unstable();
    let component = HierarchyComponent {
        id: 12,
        parents,
        boundary_edges: Vec::new(),
        core_parents: core,
        transition_parents: transition,
    };
    let limits = TransitionTopologyLimits {
        topology_states: 2,
        maximum_halo_expansions: 0,
    };

    let TransitionTopologyOutcome::Closed(trial) = limits.solve_from_cursor(&fine, &component, 1)
    else {
        panic!("cursor should resume at the next stable topology ordinal");
    };

    assert_eq!(trial.candidate.topology_id, 1);
    assert_eq!(trial.report.topology_states, 2);
    assert_hard_topology_gates(&trial.mesh.mesh);
}

#[test]
fn topology_search_can_resume_after_a_checked_candidate_without_replay() {
    let (fine, core, transition) = level_three_fixture();
    let component = component(core, transition, false);
    let limits = TransitionTopologyLimits {
        topology_states: 1_000,
        maximum_halo_expansions: 0,
    };

    let TransitionTopologyOutcome::Closed(first) =
        solve_transition_topology(&fine, &component, limits)
    else {
        panic!("first topology candidate must close");
    };
    let cursor = first.report.topology_states;

    let TransitionTopologyOutcome::Closed(resumed) =
        limits.solve_from_cursor(&fine, &component, cursor)
    else {
        panic!("resumed topology search must find a later candidate");
    };

    assert_ne!(first.candidate.topology_id, resumed.candidate.topology_id);
    assert_eq!(first.candidate.topology_id + 1, cursor);
    assert_eq!(
        resumed.candidate.topology_id + 1,
        resumed.report.topology_states
    );
    assert!(resumed.report.topology_states > cursor);
    assert_hard_topology_gates(&resumed.mesh.mesh);
}

#[test]
fn zero_topology_budget_is_exhaustion_not_infeasibility() {
    let (fine, core, transition) = level_three_fixture();
    let component = component(core, transition, false);

    assert!(matches!(
        solve_transition_topology(
            &fine,
            &component,
            TransitionTopologyLimits {
                topology_states: 0,
                maximum_halo_expansions: 0,
            },
        ),
        TransitionTopologyOutcome::SearchBudgetExhausted {
            states_examined: 0,
            halo_expansions: 0,
        }
    ));
}

#[test]
fn exposed_core_requires_and_can_use_one_wider_halo() {
    let (fine, core, transition) = level_three_fixture();
    let component = component(core, transition.clone(), true);

    assert!(matches!(
        solve_transition_topology(
            &fine,
            &component,
            TransitionTopologyLimits {
                topology_states: 1_000,
                maximum_halo_expansions: 0,
            },
        ),
        TransitionTopologyOutcome::RequiresWiderHalo {
            states_examined: 0,
            halo_expansions: 0,
        }
    ));

    let TransitionTopologyOutcome::Closed(trial) = solve_transition_topology(
        &fine,
        &component,
        TransitionTopologyLimits {
            topology_states: 1_000,
            maximum_halo_expansions: 1,
        },
    ) else {
        panic!("one halo expansion must expose the same closable mixed fixture");
    };
    assert_hard_topology_gates(&trial.mesh.mesh);
    assert_candidate_delta(&trial, vec![core], transition.clone());
    assert_eq!(trial.report.halo_expansions, 1);
    assert_eq!(trial.report.core_parent_count, 1);
    assert_eq!(trial.report.transition_parent_count, 3);
}

#[test]
fn isolated_exposed_core_requires_wider_halo_without_misreporting_budget_exhaustion() {
    let (fine, core, _) = level_three_fixture();
    let component = component(core, Vec::new(), true);

    assert!(matches!(
        solve_transition_topology(
            &fine,
            &component,
            TransitionTopologyLimits {
                topology_states: 0,
                maximum_halo_expansions: 8,
            },
        ),
        TransitionTopologyOutcome::RequiresWiderHalo {
            states_examined: 0,
            halo_expansions: 0,
        }
    ));
}

#[test]
fn pure_whole_sphere_core_closes_at_zero_topology_budget_as_exact_coarse_grid() {
    let fine = MotherGrid::generate(8).unwrap();
    let expected = MotherGrid::generate(4).unwrap();
    let component = whole_sphere_component(4);

    let TransitionTopologyOutcome::Closed(trial) = solve_transition_topology(
        &fine,
        &component,
        TransitionTopologyLimits {
            topology_states: 0,
            maximum_halo_expansions: 0,
        },
    ) else {
        panic!("pure whole-sphere core must not spend topology search budget");
    };

    assert_eq!(trial.mesh.mesh, expected.mesh);
    assert_eq!(trial.mesh.triangle_addresses, expected.triangle_addresses);
    assert_eq!(trial.report.core_parent_count, component.parents.len());
    assert_eq!(trial.report.transition_parent_count, 0);
    assert_eq!(trial.report.topology_states, 0);
    assert!(trial.boundary.fine_outer_cycles.is_empty());
    assert!(trial.boundary.coarse_inner_cycles.is_empty());
    assert_candidate_delta(&trial, component.parents.clone(), Vec::new());
}

#[test]
fn multi_ring_transition_only_retriangulates_core_adjacent_parents_and_closes() {
    let (fine, core, first_ring) = level_three_fixture();
    let mut transition = first_ring.clone();
    for parent in &first_ring {
        transition.extend(
            parent_neighbours(&fine, *parent)
                .into_iter()
                .filter(|parent| *parent != core && !first_ring.contains(parent)),
        );
    }
    transition.sort_unstable();
    transition.dedup();
    assert!(transition.len() > first_ring.len());
    let component = component(core, transition.clone(), false);

    let TransitionTopologyOutcome::Closed(trial) = solve_transition_topology(
        &fine,
        &component,
        TransitionTopologyLimits {
            topology_states: 10_000,
            maximum_halo_expansions: 0,
        },
    ) else {
        panic!("two-ring transition fixture must close");
    };

    assert_hard_topology_gates(&trial.mesh.mesh);
    assert_eq!(trial.report.core_parent_count, 1);
    assert_eq!(trial.report.transition_parent_count, transition.len());
    assert_eq!(trial.boundary.halo_parents, transition);
    assert_candidate_delta(&trial, vec![core], first_ring.clone());
    assert_eq!(trial.candidate.source_triangles.len(), first_ring.len() * 3);
    assert_eq!(
        trial
            .mesh
            .triangle_addresses
            .iter()
            .filter(|address| address.is_none())
            .count(),
        2 + trial.candidate.source_triangles.len()
    );
    let elastic = ElasticPatch::from_transition(&trial).unwrap();
    let fixed_sources = trial
        .boundary
        .fine_outer_cycles
        .iter()
        .chain(&trial.boundary.coarse_inner_cycles)
        .flat_map(|cycle| cycle.iter().copied())
        .collect::<BTreeSet<_>>();
    assert!(!elastic.movable_compact_vertices.is_empty());
    assert!(elastic.movable_compact_vertices.iter().all(|&compact| {
        trial.mesh.source_vertex_slots[compact]
            .is_some_and(|source| !fixed_sources.contains(&source))
    }));
}

#[test]
fn many_custom_transition_parents_respect_budget_without_recursive_stack_growth() {
    let fine = MotherGrid::generate(64).unwrap();
    let coarse = MotherGrid::generate(32).unwrap();
    let core = coarse
        .triangle_addresses
        .iter()
        .flatten()
        .copied()
        .filter(|parent| {
            parent.base_face == 0 && parent.i >= 4 && parent.j >= 4 && parent.i + parent.j < 28
        })
        .collect::<Vec<_>>();
    let core_set = core.iter().copied().collect::<BTreeSet<_>>();
    let transition = core
        .iter()
        .flat_map(|parent| parent_neighbours(&fine, *parent))
        .filter(|parent| !core_set.contains(parent))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    assert!(
        transition.len() > 50,
        "fixture must exercise many custom-transition parents"
    );

    let mut parents = core.clone();
    parents.extend(transition.iter().copied());
    parents.sort_unstable();
    let component = HierarchyComponent {
        id: 10,
        parents,
        boundary_edges: Vec::new(),
        core_parents: core.clone(),
        transition_parents: transition,
    };

    let TransitionTopologyOutcome::Closed(trial) = solve_transition_topology(
        &fine,
        &component,
        TransitionTopologyLimits {
            topology_states: 1,
            maximum_halo_expansions: 0,
        },
    ) else {
        panic!("wide transition fixture should close within one topology state");
    };
    assert_eq!(trial.report.core_parent_count, 420);
    assert_eq!(trial.report.transition_parent_count, 61);
    assert_eq!(trial.report.topology_states, 1);
    assert_eq!(trial.candidate.core_parents, core);
    assert_eq!(custom_parent_keys(&trial).len(), 61);
    assert_eq!(
        trial.candidate.source_triangles,
        flattened_custom_triangles(&trial)
    );
}

#[test]
fn degree_pruned_topology_search_resumes_across_halo_layouts() {
    let (fine, center, first_ring) = level_three_fixture();
    let mut core = vec![center];
    core.extend(first_ring.iter().copied());
    core.sort_unstable();
    let core_set = core.iter().copied().collect::<BTreeSet<_>>();
    let transition = core
        .iter()
        .flat_map(|parent| parent_neighbours(&fine, *parent))
        .filter(|parent| !core_set.contains(parent))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut parents = core.clone();
    parents.extend(transition.iter().copied());
    parents.sort_unstable();
    let component = HierarchyComponent {
        id: 11,
        parents,
        boundary_edges: Vec::new(),
        core_parents: core,
        transition_parents: transition,
    };
    let limits = TransitionTopologyLimits {
        topology_states: 2,
        maximum_halo_expansions: 1,
    };

    let TransitionTopologyOutcome::Closed(first) =
        solve_transition_topology(&fine, &component, limits)
    else {
        panic!("first degree-feasible topology candidate must close");
    };
    assert_eq!(first.report.topology_states, 1);
    assert_eq!(first.report.halo_expansions, 0);

    let TransitionTopologyOutcome::Closed(resumed) = limits.solve_from_cursor(&fine, &component, 1)
    else {
        panic!("resumed search must find the next degree-feasible candidate after halo expansion");
    };
    assert_eq!(resumed.report.topology_states, 2);
    assert_eq!(resumed.report.halo_expansions, 1);
    assert_ne!(first.candidate.topology_id, resumed.candidate.topology_id);

    assert!(matches!(
        limits.solve_from_cursor(&fine, &component, 2),
        TransitionTopologyOutcome::SearchBudgetExhausted {
            states_examined: 2,
            halo_expansions: 0,
        }
    ));
}

#[test]
fn insufficient_external_halo_requires_wider_halo_before_proving_infeasible() {
    let (fine, core, _) = level_three_fixture();
    let component = component(core, Vec::new(), true);

    assert!(matches!(
        solve_transition_topology(
            &fine,
            &component,
            TransitionTopologyLimits {
                topology_states: 1_000,
                maximum_halo_expansions: 0,
            },
        ),
        TransitionTopologyOutcome::RequiresWiderHalo {
            states_examined: 0,
            halo_expansions: 0,
        }
    ));
}

#[test]
fn seam_and_pentagon_boundary_report_source_slots_for_icosahedron_vertex_parent() {
    let fine = MotherGrid::generate(8).unwrap();
    let coarse_n = 4;
    let core = TriangleAddress {
        base_face: 0,
        i: 0,
        j: 0,
        n: coarse_n,
        orientation: TriangleOrientation::Down,
    };
    let transition = parent_neighbours(&fine, core);
    let component = component(core, transition, false);

    let TransitionTopologyOutcome::Closed(trial) = solve_transition_topology(
        &fine,
        &component,
        TransitionTopologyLimits {
            topology_states: 1_000,
            maximum_halo_expansions: 0,
        },
    ) else {
        panic!("icosahedron-vertex transition fixture must close");
    };

    assert_hard_topology_gates(&trial.mesh.mesh);
    assert!(!trial.boundary.pentagon.is_empty());
    assert!(trial.boundary.pentagon.iter().all(|&slot| {
        matches!(
            fine.addresses[slot],
            Some(VertexAddress::IcosahedronVertex(_))
        )
    }));
    assert!(
        trial
            .boundary
            .pentagon
            .iter()
            .all(|slot| trial.boundary.seam.contains(slot)),
        "icosahedron vertices are also seam source slots"
    );
}

#[test]
fn invalid_component_boundary_is_reported_before_zero_budget_exhaustion() {
    let (fine, core, transition) = level_three_fixture();
    let wrong_level = core.children_2_to_1().unwrap()[0];
    let cases = [
        HierarchyComponent {
            id: 1,
            parents: vec![core],
            boundary_edges: Vec::new(),
            core_parents: vec![core],
            transition_parents: vec![core],
        },
        HierarchyComponent {
            id: 2,
            parents: vec![core, core],
            boundary_edges: Vec::new(),
            core_parents: vec![core, core],
            transition_parents: Vec::new(),
        },
        HierarchyComponent {
            id: 3,
            parents: vec![wrong_level],
            boundary_edges: Vec::new(),
            core_parents: vec![wrong_level],
            transition_parents: Vec::new(),
        },
        HierarchyComponent {
            id: 4,
            parents: vec![core],
            boundary_edges: Vec::new(),
            core_parents: vec![core],
            transition_parents: transition,
        },
    ];

    for component in cases {
        assert!(
            matches!(
                solve_transition_topology(
                    &fine,
                    &component,
                    TransitionTopologyLimits {
                        topology_states: 0,
                        maximum_halo_expansions: 0,
                    },
                ),
                TransitionTopologyOutcome::InvalidBoundary {
                    states_examined: 0,
                    halo_expansions: 0,
                    ..
                }
            ),
            "component {} should fail preflight before budget checks",
            component.id
        );
    }
}

#[test]
fn core_coarse_edge_contract_retains_only_corners_not_fine_edge_midpoints() {
    let (fine, core, transition) = level_three_fixture();
    let (corners, midpoints) = parent_sites(&fine, core);
    let component = component(core, transition, false);

    let TransitionTopologyOutcome::Closed(trial) = solve_transition_topology(
        &fine,
        &component,
        TransitionTopologyLimits {
            topology_states: 1_000,
            maximum_halo_expansions: 0,
        },
    ) else {
        panic!("Level 3/2 fixture must close");
    };

    let output_slots = output_source_slots(&trial);
    assert_eq!(trial.boundary.coarse_inner_cycles.len(), 1);
    assert_eq!(
        trial.boundary.coarse_inner_cycles[0]
            .iter()
            .copied()
            .collect::<BTreeSet<_>>(),
        corners.into_iter().collect()
    );
    assert!(corners.iter().all(|slot| output_slots.contains(slot)));
    assert!(midpoints.iter().all(|slot| !output_slots.contains(slot)));
}
