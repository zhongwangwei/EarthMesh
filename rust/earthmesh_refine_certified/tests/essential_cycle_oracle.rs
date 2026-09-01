use earthmesh_refine_certified::{
    coarsen::{
        build_essential_cycle_problem, enumerate_legacy_face_band_plans,
        essential_cycle_from_face_band_plan, face_band_plan_from_essential_cycle,
        validate_selected_essential_cycle, AnchorBandPolicy, EssentialCycleKey,
        EssentialCycleProblem, FaceBandLimits, FaceBandProblem, RetainedCoreCorridorFamily,
    },
    MotherGrid,
};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn legacy_and_cec_plan_sets_equal_n2() {
    assert_oracle_equal(2, &[]);
}

#[test]
fn legacy_and_cec_plan_sets_equal_n3() {
    assert_oracle_equal(3, &[]);
}

#[test]
fn legacy_and_cec_plan_sets_equal_n4() {
    assert_oracle_equal(4, &[]);
}

#[test]
fn anchor_policy_sets_equal() {
    for policies in [
        vec![(0, 0, AnchorBandPolicy::InteriorOfSingleBand)],
        vec![(1, 0, AnchorBandPolicy::OnSingleInterface)],
        vec![(2, 0, AnchorBandPolicy::FineCapConnectedToExterior)],
    ] {
        assert_oracle_equal(3, &policies);
    }
}

#[test]
fn oracle_detects_seam_parity_mutation() {
    let (source, face_problem, mut cycle_problem) = synthetic_annulus(3, &[]);
    let legacy = legacy_keys(&source, &face_problem, &cycle_problem);
    assert!(!legacy.is_empty());
    for edge in 0..cycle_problem.dual_seam_crossing_edges.len() {
        cycle_problem
            .dual_seam_crossing_edges
            .set(edge, false)
            .unwrap();
    }
    assert!(cec_keys(&source, &face_problem, &cycle_problem).is_empty());
}

fn assert_oracle_equal(scale: usize, policies: &[(usize, usize, AnchorBandPolicy)]) {
    let (source, face_problem, cycle_problem) = synthetic_annulus(scale, policies);
    let legacy = legacy_keys(&source, &face_problem, &cycle_problem);
    let cec = cec_keys(&source, &face_problem, &cycle_problem);
    eprintln!(
        "N{scale}: candidates={} legacy_keys={} cec_keys={}",
        cycle_problem.candidate_edges.len(),
        legacy.len(),
        cec.len()
    );
    assert_eq!(legacy, cec, "N{scale} oracle mismatch");
    assert!(!legacy.is_empty());
}

fn legacy_keys(
    source: &MotherGrid,
    face_problem: &FaceBandProblem,
    cycle_problem: &EssentialCycleProblem,
) -> BTreeSet<EssentialCycleKey> {
    let enumeration = enumerate_legacy_face_band_plans(
        face_problem,
        FaceBandLimits {
            maximum_states: 1_000_000,
        },
    )
    .unwrap();
    assert!(enumeration.complete);
    enumeration
        .plans
        .iter()
        .map(|plan| {
            essential_cycle_from_face_band_plan(source, face_problem, cycle_problem, plan).unwrap()
        })
        .collect()
}

fn cec_keys(
    source: &MotherGrid,
    face_problem: &FaceBandProblem,
    cycle_problem: &EssentialCycleProblem,
) -> BTreeSet<EssentialCycleKey> {
    assert!(cycle_problem.candidate_edges.len() < usize::BITS as usize);
    (0usize..1usize << cycle_problem.candidate_edges.len())
        .filter_map(|mask| {
            let selected = (0..cycle_problem.candidate_edges.len())
                .filter(|edge| mask & (1 << edge) != 0)
                .collect::<Vec<_>>();
            let cycle = validate_selected_essential_cycle(cycle_problem, &selected).ok()?;
            face_band_plan_from_essential_cycle(source, face_problem, cycle_problem, &cycle)
                .ok()
                .map(|_| cycle)
        })
        .collect()
}

fn synthetic_annulus(
    scale: usize,
    policies: &[(usize, usize, AnchorBandPolicy)],
) -> (MotherGrid, FaceBandProblem, EssentialCycleProblem) {
    let sectors = scale + 2;
    let source = MotherGrid::generate(scale).unwrap();
    let vertices = source
        .addresses
        .iter()
        .enumerate()
        .filter_map(|(slot, address)| address.is_some().then_some(slot))
        .take(3 * sectors)
        .collect::<Vec<_>>();
    let vertex = |ring: usize, sector: usize| vertices[ring * sectors + sector % sectors];
    let mut triangles = Vec::<[usize; 3]>::new();
    let mut coarse_boundary_faces = BTreeSet::new();
    let mut fine_boundary_faces = BTreeSet::new();
    for sector in 0..sectors {
        let next = (sector + 1) % sectors;
        coarse_boundary_faces.insert(triangles.len());
        triangles.push([vertex(0, sector), vertex(0, next), vertex(1, next)]);
        triangles.push([vertex(0, sector), vertex(1, next), vertex(1, sector)]);
        triangles.push([vertex(1, sector), vertex(2, next), vertex(1, next)]);
        fine_boundary_faces.insert(triangles.len());
        triangles.push([vertex(1, sector), vertex(2, sector), vertex(2, next)]);
    }
    let transition_faces = (0..triangles.len()).collect::<Vec<_>>();
    let mut vertex_incident_faces = BTreeMap::<usize, Vec<usize>>::new();
    let mut edge_faces = BTreeMap::<(usize, usize), Vec<usize>>::new();
    for (face, triangle) in triangles.iter().enumerate() {
        for &slot in triangle {
            vertex_incident_faces.entry(slot).or_default().push(face);
        }
        for side in 0..3 {
            edge_faces
                .entry(edge(triangle[(side + 1) % 3], triangle[(side + 2) % 3]))
                .or_default()
                .push(face);
        }
    }
    let mut face_adjacency = BTreeMap::<usize, Vec<usize>>::new();
    let mut face_shared_edges = BTreeMap::new();
    for (shared, faces) in edge_faces {
        if let [left, right] = faces[..] {
            face_adjacency.entry(left).or_default().push(right);
            face_adjacency.entry(right).or_default().push(left);
            face_shared_edges.insert((left.min(right), left.max(right)), shared);
        }
    }
    for neighbours in face_adjacency.values_mut() {
        neighbours.sort_unstable();
        neighbours.dedup();
    }
    let mut face_vertex_neighbours = BTreeMap::<usize, Vec<usize>>::new();
    for faces in vertex_incident_faces.values() {
        for &face in faces {
            face_vertex_neighbours
                .entry(face)
                .or_default()
                .extend(faces);
        }
    }
    for faces in face_vertex_neighbours.values_mut() {
        faces.sort_unstable();
        faces.dedup();
    }
    let addresses = source
        .triangle_addresses
        .iter()
        .flatten()
        .copied()
        .take(triangles.len())
        .collect::<Vec<_>>();
    assert_eq!(addresses.len(), triangles.len());
    let face_problem = FaceBandProblem {
        transition_faces: transition_faces.clone(),
        coarse_boundary_faces,
        fine_boundary_faces,
        face_adjacency,
        vertex_incident_faces,
        face_vertex_neighbours,
        band_count: 2,
        anchor_policies: policies
            .iter()
            .map(|&(ring, sector, policy)| (vertex(ring, sector), policy))
            .collect(),
        face_shared_edges,
        coarse_boundary_vertices: (0..sectors).map(|sector| vertex(0, sector)).collect(),
        fine_boundary_vertices: (0..sectors).map(|sector| vertex(2, sector)).collect(),
        face_addresses: transition_faces
            .iter()
            .map(|face| (*face, addresses[*face]))
            .collect(),
        core_nonempty: true,
        source_face_rings: 0,
    };
    let cycle_problem = build_essential_cycle_problem(
        &source,
        &face_problem,
        [],
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor,
    )
    .unwrap();
    (source, face_problem, cycle_problem)
}

fn edge(left: usize, right: usize) -> (usize, usize) {
    (left.min(right), left.max(right))
}
