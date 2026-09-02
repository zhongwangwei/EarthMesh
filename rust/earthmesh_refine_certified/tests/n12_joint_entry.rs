use earthmesh_refine_certified::coarsen::{
    build_face_band_problem, build_global_incidence_contract,
    build_stratified_transition_domain_v3, certify_annular_topology,
    enumerate_balanced_annular_strips, n12_lifted_n6_fixture, solve_exact_face_bands,
    solve_joint_concrete_extraction, AnnularCellDomain, AnnularIncidenceTarget, AnnularTopology,
    EssentialCycleKey, FaceBandLimits, FaceBandSolveOutcome, GlobalIncidenceContract,
    GlobalIncidencePlan, GlobalIncidencePlanKey, JointConcreteExtractionOutcome,
    JointConcreteExtractionPlan, JointConcreteLimits, TransitionCellDomain,
};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn frozen_lifted_plan_enters_joint_extraction() {
    let evidence = include_str!("fixtures/n12_joint_entry.json");
    assert!(evidence.contains("\"incidence_plan_found\":true"));
    assert!(evidence.contains("\"entered_joint_extraction\":true"));
    assert!(evidence.contains("\"candidate_pairs\":1"));
    assert!(evidence.contains("\"outcome\":\"SearchIncomplete\""));
    assert!(evidence.contains("\"gate_passed\":true"));
    assert!(evidence.contains("\"cec_shards_resumed\":false"));
}

#[test]
#[ignore = "PR116 deterministic Lifted-N12 joint-entry fixture writer"]
fn write_lifted_joint_entry() {
    let fixture = n12_lifted_n6_fixture().unwrap();
    let problem = build_face_band_problem(&fixture.source, &fixture.component, 2).unwrap();
    let FaceBandSolveOutcome::Closed(face_plan, _) = solve_exact_face_bands(
        &problem,
        FaceBandLimits {
            maximum_states: 16_384,
        },
    ) else {
        panic!("Lifted N12 W2 plan must close")
    };
    let domain =
        build_stratified_transition_domain_v3(&fixture.source, &fixture.component, &face_plan)
            .unwrap();
    let contract =
        build_global_incidence_contract(&fixture.source, &fixture.component, &domain).unwrap();
    let cells = domain
        .cells
        .iter()
        .map(|cell| match cell {
            TransitionCellDomain::Annulus(cell) => cell,
            TransitionCellDomain::Disk(_) => panic!("Lifted W2 cells must be annular"),
        })
        .collect::<Vec<_>>();
    let families = cells
        .iter()
        .map(|cell| {
            enumerate_balanced_annular_strips(
                &cell.lower_cycle,
                &cell.upper_cycle,
                &cell.forbidden_global_edges,
                256,
            )
            .unwrap()
            .family
            .topologies
        })
        .collect::<Vec<_>>();
    let mut beam = families[0]
        .iter()
        .flat_map(|lower| {
            families[1]
                .iter()
                .map(move |upper| [lower.clone(), upper.clone()])
        })
        .collect::<Vec<_>>();
    rank_and_truncate(&mut beam, &contract, &cells, 32);
    let mut selected = None;
    for depth in 0..32 {
        if pair_score(&beam[0], &contract, &cells) == 0 {
            selected = Some((depth, beam[0].clone()));
            break;
        }
        let mut next = BTreeMap::new();
        for pair in &beam {
            for side in 0..2 {
                for neighbor in flip_neighbors(cells[side], &pair[side]) {
                    let mut candidate = pair.clone();
                    candidate[side] = neighbor;
                    next.insert(
                        (
                            candidate[0].topology_key.clone(),
                            candidate[1].topology_key.clone(),
                        ),
                        candidate,
                    );
                }
            }
        }
        beam = next.into_values().collect();
        rank_and_truncate(&mut beam, &contract, &cells, 32);
    }
    let (flip_depth, pair) = selected.expect("bounded flip beam must recover a legal plan");
    let incidence_plan = incidence_plan_for_pair(&pair, &contract, &cells);
    let plan = JointConcreteExtractionPlan::new(
        incidence_plan.clone(),
        AnnularIncidenceTarget::new(
            cells[0],
            pair[0].root_bridge,
            incidence_plan.cell_incidences[&cells[0].cell_id].clone(),
        ),
        AnnularIncidenceTarget::new(
            cells[1],
            pair[1].root_bridge,
            incidence_plan.cell_incidences[&cells[1].cell_id].clone(),
        ),
    );
    let JointConcreteExtractionOutcome::SearchIncomplete { evidence, .. } =
        solve_joint_concrete_extraction(
            &fixture.source,
            &fixture.component,
            &domain,
            &plan,
            JointConcreteLimits { maximum_pairs: 0 },
            None,
        )
    else {
        panic!("zero pair budget must checkpoint after concrete candidates are built")
    };
    assert!(evidence.entered_joint_extraction && evidence.candidate_pairs > 0);
    let json = format!(
        "{{\"schema_version\":1,\"taskbook_sha256\":\"65f26b64c78dd7dfadaaf2a1099f52d11c6a67461afb0a9558edbbf5941ef473\",\"fixture\":\"N12-Lifted-N6\",\"declared_topology_family\":\"TransitionCellV3+GIPC+PIER+JointZeroEar\",\"plan_source\":\"deterministic_bounded_flip_feasibility_probe\",\"incidence_plan_found\":true,\"balanced_family_counts\":[{},{}],\"beam_width\":32,\"flip_depth\":{},\"selected_plan_key\":\"{}\",\"selected_roots\":[[{},{}],[{},{}]],\"lower_witnesses\":{},\"upper_witnesses\":{},\"dynamic_secondary_targets\":{},\"dynamic_forbidden_edges\":{},\"candidate_pairs\":{},\"pairs_examined\":{},\"entered_joint_extraction\":{},\"outcome\":\"SearchIncomplete\",\"gate_passed\":true,\"cec_shards_resumed\":false,\"geometry_attempted\":false,\"product_gate_changed\":false}}",
        families[0].len(),
        families[1].len(),
        flip_depth,
        incidence_plan.plan_key.0,
        pair[0].root_bridge.0,
        pair[0].root_bridge.1,
        pair[1].root_bridge.0,
        pair[1].root_bridge.1,
        evidence.lower_witnesses,
        evidence.upper_witnesses,
        evidence.dynamic_secondary_targets,
        evidence.dynamic_forbidden_edges,
        evidence.candidate_pairs,
        evidence.pairs_examined,
        evidence.entered_joint_extraction,
    );
    if let Ok(path) = std::env::var("EARTHMESH_N12_JOINT_ENTRY_JSON") {
        std::fs::write(path, &json).unwrap();
    }
    println!("{json}");
}

fn incidence_plan_for_pair(
    pair: &[AnnularTopology; 2],
    contract: &GlobalIncidenceContract,
    cells: &[&AnnularCellDomain],
) -> GlobalIncidencePlan {
    let cell_incidences = cells
        .iter()
        .enumerate()
        .map(|(side, cell)| (cell.cell_id, vertex_incidences(&pair[side])))
        .collect::<BTreeMap<_, _>>();
    let final_degrees = contract
        .vertex_domains
        .iter()
        .map(|(&vertex, domain)| {
            let tuple = domain
                .allowed_owner_tuples
                .iter()
                .find(|tuple| {
                    tuple
                        .owner_counts
                        .iter()
                        .all(|&(cell_id, count)| cell_incidences[&cell_id][&vertex] == count)
                })
                .expect("zero-score pair must satisfy every incidence tuple");
            (vertex, tuple.final_degree)
        })
        .collect();
    GlobalIncidencePlan {
        cycle_key: EssentialCycleKey {
            ordered_vertices: Vec::new(),
        },
        final_degrees,
        cell_incidences,
        ordinary_curvature_score: 0,
        incidence_roughness_score: 0,
        plan_key: GlobalIncidencePlanKey("lifted-bounded-flip-plan".into()),
    }
}

fn rank_and_truncate(
    pairs: &mut Vec<[AnnularTopology; 2]>,
    contract: &GlobalIncidenceContract,
    cells: &[&AnnularCellDomain],
    maximum: usize,
) {
    pairs.sort_by_key(|pair| {
        (
            pair_score(pair, contract, cells),
            pair[0].topology_key.clone(),
            pair[1].topology_key.clone(),
        )
    });
    pairs.truncate(maximum);
}

fn pair_score(
    pair: &[AnnularTopology; 2],
    contract: &GlobalIncidenceContract,
    cells: &[&AnnularCellDomain],
) -> usize {
    let counts = [vertex_incidences(&pair[0]), vertex_incidences(&pair[1])];
    contract
        .vertex_domains
        .iter()
        .map(|(&vertex, domain)| {
            domain
                .allowed_owner_tuples
                .iter()
                .map(|tuple| {
                    tuple
                        .owner_counts
                        .iter()
                        .map(|&(cell_id, expected)| {
                            let side = usize::from(cell_id != cells[0].cell_id);
                            counts[side][&vertex].abs_diff(expected) as usize
                        })
                        .sum::<usize>()
                })
                .min()
                .unwrap()
        })
        .sum()
}

fn vertex_incidences(topology: &AnnularTopology) -> BTreeMap<usize, u8> {
    topology
        .triangles
        .iter()
        .flatten()
        .fold(BTreeMap::<usize, u8>::new(), |mut counts, &vertex| {
            *counts.entry(vertex).or_default() += 1;
            counts
        })
}

fn flip_neighbors(cell: &AnnularCellDomain, topology: &AnnularTopology) -> Vec<AnnularTopology> {
    let boundary = boundary_edges(&cell.lower_cycle)
        .into_iter()
        .chain(boundary_edges(&cell.upper_cycle))
        .collect::<BTreeSet<_>>();
    let mut incidence = BTreeMap::<(usize, usize), Vec<usize>>::new();
    for (index, triangle) in topology.triangles.iter().enumerate() {
        for candidate in triangle_edges(*triangle) {
            incidence.entry(candidate).or_default().push(index);
        }
    }
    let mut out = BTreeMap::new();
    for (shared, owners) in incidence
        .iter()
        .filter(|(edge, owners)| owners.len() == 2 && !boundary.contains(edge))
    {
        let first = owners[0];
        let second = owners[1];
        let opposite = |triangle: [usize; 3]| {
            triangle
                .into_iter()
                .find(|vertex| ![shared.0, shared.1].contains(vertex))
                .unwrap()
        };
        let a = opposite(topology.triangles[first]);
        let b = opposite(topology.triangles[second]);
        if a == b || incidence.contains_key(&edge(a, b)) {
            continue;
        }
        let mut triangles = topology.triangles.clone();
        triangles[first] = triangle(a, b, shared.0);
        triangles[second] = triangle(a, b, shared.1);
        if let Ok(neighbor) = certify_annular_topology(
            &cell.lower_cycle,
            &cell.upper_cycle,
            &cell.forbidden_global_edges,
            &triangles,
        ) {
            out.insert(neighbor.topology_key.clone(), neighbor);
        }
    }
    out.into_values().collect()
}

fn boundary_edges(cycle: &[usize]) -> Vec<(usize, usize)> {
    cycle
        .iter()
        .copied()
        .zip(cycle.iter().copied().cycle().skip(1))
        .take(cycle.len())
        .map(|(a, b)| edge(a, b))
        .collect()
}

fn edge(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

fn triangle(a: usize, b: usize, c: usize) -> [usize; 3] {
    let mut triangle = [a, b, c];
    triangle.sort_unstable();
    triangle
}

fn triangle_edges(triangle: [usize; 3]) -> [(usize, usize); 3] {
    [
        edge(triangle[0], triangle[1]),
        edge(triangle[1], triangle[2]),
        edge(triangle[2], triangle[0]),
    ]
}
