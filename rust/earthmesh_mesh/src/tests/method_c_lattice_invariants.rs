//! P-1: is the stride-3 lattice a legal-by-construction mask generator?
//!
//! The nest compiler proposed in
//! `docs/method_c_multilevel_refinement_design_v2_2026-08-06.md` rests on one
//! claim: a union of whole rad3 footprints, seeded on the stride-3 sublattice
//! the thirdm walk enumerates, always closes a perimeter whose M-point count is
//! a multiple of three (G4) and which the walker can traverse (G5). A planar
//! lattice model said 400 of 400. This asks the real mesh.
//!
//! Measurement, not implementation. What it reports decides whether the
//! compiler route exists at all.

use super::*;
use std::collections::BTreeSet;

/// Deterministic sampling, because a sweep nobody can rerun is not evidence.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 11
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

struct LatticeProbe {
    mesh: TriangularMesh,
    m_neighbors: Vec<IcosahedronMPointNeighbors>,
}

impl LatticeProbe {
    fn new(nxp: usize) -> Self {
        let mesh = TriangularMesh::from_icosahedron(nxp, 0, 1.0, 0.25, 0).expect("base mesh");
        let m_neighbors = derive_icosahedron_m_neighbors_canonical_checked(
            mesh.nmd,
            &mesh.u_edges,
            &mesh.w_faces,
        )
        .expect("Method-C neighbors");
        Self { mesh, m_neighbors }
    }

    /// The stride-3 sublattice reachable from `start`, breadth first, capped so
    /// the sweep stays affordable.
    fn lattice_from(&self, start: usize, cap: usize) -> Vec<usize> {
        let mut seen = BTreeSet::new();
        let mut queue = std::collections::VecDeque::new();
        seen.insert(start);
        queue.push_back(start);
        let mut jdone = vec![[false; 6]; self.mesh.nmd + 1];
        while let Some(im) = queue.pop_front() {
            if seen.len() >= cap {
                break;
            }
            let Ok(thirds) = self
                .mesh
                .method_c_thirdm_neighbors_canonical_with_neighbors(
                    im,
                    &mut jdone,
                    &self.m_neighbors,
                )
            else {
                continue;
            };
            for third in thirds {
                if third > 1 && third <= self.mesh.nmd && seen.insert(third) {
                    queue.push_back(third);
                }
            }
        }
        seen.into_iter().collect()
    }

    /// Whether a union of whole footprints closes as the gates require.
    fn union_is_legal(&self, seeds: &[usize]) -> Result<bool, String> {
        let mut selected = vec![false; self.mesh.nwd + 1];
        for &seed in seeds {
            self.mesh
                .mark_fill_rad3_faces_with_neighbors(seed, &mut selected, &self.m_neighbors)
                .map_err(|error| format!("footprint {seed}: {error}"))?;
        }
        match self
            .mesh
            .method_c_perimeters_from_selected_faces(&selected, &self.m_neighbors)
        {
            // G5: the walk closed. G4: every ring is a multiple of three.
            Ok(perimeters) => Ok(TriangularMesh::method_c_perimeters_are_triplets(
                &perimeters,
            )),
            Err(error) => Err(error.to_string()),
        }
    }
}

/// One footprint, so the reading of rad3 can be checked against the model's 54.
#[test]
fn a_single_rad3_footprint_has_the_face_count_the_model_assumed() {
    let probe = LatticeProbe::new(21);
    let interior = (2..=probe.mesh.nmd)
        .find(|im| !probe.mesh.impent.contains(im) && probe.m_neighbors[*im].npoly == 6)
        .expect("a hexagonal M point");
    let faces = probe
        .mesh
        .method_c_rad3_faces_with_neighbors(interior, &probe.m_neighbors)
        .expect("rad3 footprint");
    let unique: BTreeSet<usize> = faces.iter().copied().collect();
    println!("P-1.0 rad3 footprint faces: {} unique", unique.len());
    assert!(
        unique.len() > 1,
        "a footprint that is one face is not a footprint"
    );
}

/// P-1.1: the invariant the whole compiler rests on.
#[test]
fn a_union_of_whole_footprints_closes_as_triplets() {
    for nxp in [21usize, 40] {
        let probe = LatticeProbe::new(nxp);
        let start = (2..=probe.mesh.nmd)
            .find(|im| !probe.mesh.impent.contains(im) && probe.m_neighbors[*im].npoly == 6)
            .expect("a hexagonal M point");
        let lattice = probe.lattice_from(start, 400);
        assert!(
            lattice.len() >= 50,
            "nxp {nxp}: the thirdm walk enumerated only {} seeds",
            lattice.len()
        );

        let mut rng = Lcg(0x5EED_1234_ABCD_0001);
        let (mut clean, mut broken, mut walk_failed) = (0usize, 0usize, 0usize);
        let mut first_break: Option<String> = None;
        for _ in 0..400 {
            let count = 2 + rng.below(8);
            let seeds: Vec<usize> = {
                let mut chosen = BTreeSet::new();
                while chosen.len() < count {
                    chosen.insert(lattice[rng.below(lattice.len())]);
                }
                chosen.into_iter().collect()
            };
            match probe.union_is_legal(&seeds) {
                Ok(true) => clean += 1,
                Ok(false) => {
                    broken += 1;
                    first_break.get_or_insert_with(|| format!("mod-3 broken by seeds {seeds:?}"));
                }
                Err(error) => {
                    walk_failed += 1;
                    first_break.get_or_insert_with(|| format!("walk failed on {seeds:?}: {error}"));
                }
            }
        }
        println!(
            "P-1.1 nxp={nxp} lattice={} seeds: clean={clean} mod3_broken={broken} walk_failed={walk_failed}",
            lattice.len()
        );
        if let Some(first) = &first_break {
            println!("P-1.1 nxp={nxp} first counterexample: {first}");
        }
    }
}

/// P-1.2: the pentagons, which the planar model has no way to represent.
#[test]
fn a_union_anchored_on_a_pentagon_closes_as_triplets() {
    let probe = LatticeProbe::new(21);
    let (mut clean, mut broken, mut walk_failed) = (0usize, 0usize, 0usize);
    let mut first_break: Option<String> = None;
    for (index, &pentagon) in probe.mesh.impent.iter().enumerate() {
        let lattice = probe.lattice_from(pentagon, 120);
        let mut rng = Lcg(0xF5EE_D000_u64.wrapping_add(index as u64));
        for _ in 0..20 {
            let count = 2 + rng.below(6);
            let mut chosen = BTreeSet::new();
            chosen.insert(pentagon);
            while chosen.len() < count && chosen.len() < lattice.len() {
                chosen.insert(lattice[rng.below(lattice.len())]);
            }
            let seeds: Vec<usize> = chosen.into_iter().collect();
            match probe.union_is_legal(&seeds) {
                Ok(true) => clean += 1,
                Ok(false) => {
                    broken += 1;
                    first_break.get_or_insert_with(|| format!("mod-3 broken by {seeds:?}"));
                }
                Err(error) => {
                    walk_failed += 1;
                    first_break.get_or_insert_with(|| format!("walk failed on {seeds:?}: {error}"));
                }
            }
        }
    }
    println!(
        "P-1.2 pentagon-anchored: clean={clean} mod3_broken={broken} walk_failed={walk_failed}"
    );
    if let Some(first) = &first_break {
        println!("P-1.2 first counterexample: {first}");
    }
}

/// Where the invariant actually breaks: at the union, or already at one seed?
///
/// The planar model reported 18 M points for a single footprint and 54 faces.
/// The real footprint is a different size, so this measures the ring itself
/// before blaming the union.
#[test]
fn a_single_footprint_and_a_pair_are_measured_before_the_union_is_blamed() {
    let probe = LatticeProbe::new(21);
    let start = (2..=probe.mesh.nmd)
        .find(|im| !probe.mesh.impent.contains(im) && probe.m_neighbors[*im].npoly == 6)
        .expect("a hexagonal M point");
    let lattice = probe.lattice_from(start, 400);

    let mut faces_seen = BTreeSet::new();
    let mut ring_lengths = BTreeSet::new();
    let (mut single_clean, mut single_broken, mut single_failed) = (0usize, 0usize, 0usize);
    for &seed in lattice.iter().take(120) {
        let faces = probe
            .mesh
            .method_c_rad3_faces_with_neighbors(seed, &probe.m_neighbors)
            .expect("footprint");
        faces_seen.insert(faces.iter().copied().collect::<BTreeSet<_>>().len());
        let mut selected = vec![false; probe.mesh.nwd + 1];
        probe
            .mesh
            .mark_fill_rad3_faces_with_neighbors(seed, &mut selected, &probe.m_neighbors)
            .expect("mark");
        match probe
            .mesh
            .method_c_perimeters_from_selected_faces(&selected, &probe.m_neighbors)
        {
            Ok(perimeters) => {
                for perimeter in &perimeters {
                    ring_lengths.insert(perimeter.len());
                }
                if TriangularMesh::method_c_perimeters_are_triplets(&perimeters) {
                    single_clean += 1;
                } else {
                    single_broken += 1;
                }
            }
            Err(_) => single_failed += 1,
        }
    }
    println!("P-1.x single footprint: face_counts={faces_seen:?} ring_lengths={ring_lengths:?}");
    println!(
        "P-1.x single footprint: clean={single_clean} mod3_broken={single_broken} walk_failed={single_failed}"
    );

    let mut rng = Lcg(0x2222_3333_4444_5555);
    let (mut pair_clean, mut pair_broken, mut pair_failed) = (0usize, 0usize, 0usize);
    for _ in 0..200 {
        let a = lattice[rng.below(lattice.len())];
        let b = lattice[rng.below(lattice.len())];
        if a == b {
            continue;
        }
        match probe.union_is_legal(&[a.min(b), a.max(b)]) {
            Ok(true) => pair_clean += 1,
            Ok(false) => pair_broken += 1,
            Err(_) => pair_failed += 1,
        }
    }
    println!(
        "P-1.x two-seed unions: clean={pair_clean} mod3_broken={pair_broken} walk_failed={pair_failed}"
    );
}

/// Is the breakage local to the twelve icosahedral defects, or everywhere?
///
/// This decides whether a compiler could exist with a pentagon rule, or not at
/// all. Distance is counted in M-point rings, so "0" means the footprint holds
/// a pentagon outright.
#[test]
fn the_footprint_irregularity_is_located_against_the_pentagons() {
    let probe = LatticeProbe::new(21);
    let pentagons: BTreeSet<usize> = probe.mesh.impent.iter().copied().collect();

    // Ring distance from every M point to the nearest pentagon, breadth first.
    let mut distance = vec![usize::MAX; probe.mesh.nmd + 1];
    let mut queue = std::collections::VecDeque::new();
    for &pentagon in &pentagons {
        distance[pentagon] = 0;
        queue.push_back(pentagon);
    }
    while let Some(im) = queue.pop_front() {
        let neighbors = probe.m_neighbors[im];
        for j in 0..neighbors.npoly.min(6) {
            let Ok(next) = probe.mesh.other_m_endpoint(neighbors.iu[j], im) else {
                continue;
            };
            if next > 1 && next <= probe.mesh.nmd && distance[next] == usize::MAX {
                distance[next] = distance[im] + 1;
                queue.push_back(next);
            }
        }
    }

    let start = (2..=probe.mesh.nmd)
        .find(|im| !pentagons.contains(im) && probe.m_neighbors[*im].npoly == 6)
        .expect("a hexagonal M point");
    let lattice = probe.lattice_from(start, 400);

    // Rows: near a pentagon (<= 3 rings) and far from one.
    let mut near = (0usize, 0usize);
    let mut far = (0usize, 0usize);
    let mut far_break_distances = BTreeSet::new();
    for &seed in &lattice {
        let mut selected = vec![false; probe.mesh.nwd + 1];
        probe
            .mesh
            .mark_fill_rad3_faces_with_neighbors(seed, &mut selected, &probe.m_neighbors)
            .expect("mark");
        let clean = probe
            .mesh
            .method_c_perimeters_from_selected_faces(&selected, &probe.m_neighbors)
            .map(|perimeters| TriangularMesh::method_c_perimeters_are_triplets(&perimeters))
            .unwrap_or(false);
        let bucket = if distance[seed] <= 3 {
            &mut near
        } else {
            &mut far
        };
        if clean {
            bucket.0 += 1;
        } else {
            bucket.1 += 1;
            if distance[seed] > 3 {
                far_break_distances.insert(distance[seed]);
            }
        }
    }
    println!(
        "P-1.y single footprint by pentagon distance: near(<=3) clean={} broken={} | far(>3) clean={} broken={}",
        near.0, near.1, far.0, far.1
    );
    println!("P-1.y far-side breakage at ring distances {far_break_distances:?}");
}

/// P-1.3: does the stride-3 lattice survive into the child generation?
///
/// The design calls this its largest uncertainty. The second level's seeds live
/// on the mesh the first level produced, and that mesh carries 5/7-degree
/// defects through its transition band. If the lattice is only regular in the
/// interior, the compiler can still chain by staying clear of the band -- which
/// is what G2's three-row clearance would then be for. If it is irregular
/// throughout, the lattice cannot be reused across generations.
#[test]
fn the_lattice_is_measured_again_on_the_child_generation() {
    let probe = LatticeProbe::new(21);
    let regions = [RefinementRegion::Circle {
        center: LonLatDegrees::new(0.0, 0.0),
        radius_meters: 3_000_000.0,
        level: 1,
    }];
    let Ok(child) = probe.mesh.spawn_nest(&regions, 1) else {
        println!("P-1.3 the single level did not build; nothing to measure");
        return;
    };
    let child_neighbors =
        derive_icosahedron_m_neighbors_canonical_checked(child.nmd, &child.u_edges, &child.w_faces)
            .expect("child neighbors");
    let child_probe = LatticeProbe {
        mesh: child,
        m_neighbors: child_neighbors,
    };

    // Distance in M rings to the nearest defect: a pentagon, or any point whose
    // ring is not six, which is what the transition band leaves behind.
    let defects: BTreeSet<usize> = (2..=child_probe.mesh.nmd)
        .filter(|&im| {
            child_probe.mesh.impent.contains(&im) || child_probe.m_neighbors[im].npoly != 6
        })
        .collect();
    let mut distance = vec![usize::MAX; child_probe.mesh.nmd + 1];
    let mut queue = std::collections::VecDeque::new();
    for &defect in &defects {
        distance[defect] = 0;
        queue.push_back(defect);
    }
    while let Some(im) = queue.pop_front() {
        let neighbors = child_probe.m_neighbors[im];
        for j in 0..neighbors.npoly.min(6) {
            let Ok(next) = child_probe.mesh.other_m_endpoint(neighbors.iu[j], im) else {
                continue;
            };
            if next > 1 && next <= child_probe.mesh.nmd && distance[next] == usize::MAX {
                distance[next] = distance[im] + 1;
                queue.push_back(next);
            }
        }
    }

    // Start deep inside the refined generation.
    let deepest = (2..=child_probe.mesh.nmd)
        .map(|im| child_probe.mesh.m_metadata[im].mrlm)
        .max()
        .unwrap_or(0);
    let Some(start) = (2..=child_probe.mesh.nmd)
        .find(|&im| child_probe.mesh.m_metadata[im].mrlm == deepest && distance[im] > 4)
    else {
        println!("P-1.3 no interior point at generation {deepest} more than 4 rings from a defect");
        return;
    };
    let lattice = child_probe.lattice_from(start, 300);

    let mut near = (0usize, 0usize);
    let mut far = (0usize, 0usize);
    for &seed in &lattice {
        let mut selected = vec![false; child_probe.mesh.nwd + 1];
        if child_probe
            .mesh
            .mark_fill_rad3_faces_with_neighbors(seed, &mut selected, &child_probe.m_neighbors)
            .is_err()
        {
            continue;
        }
        let clean = child_probe
            .mesh
            .method_c_perimeters_from_selected_faces(&selected, &child_probe.m_neighbors)
            .map(|perimeters| TriangularMesh::method_c_perimeters_are_triplets(&perimeters))
            .unwrap_or(false);
        let bucket = if distance[seed] <= 3 {
            &mut near
        } else {
            &mut far
        };
        if clean {
            bucket.0 += 1;
        } else {
            bucket.1 += 1;
        }
    }
    println!(
        "P-1.3 child generation {deepest}: near-defect(<=3) clean={} broken={} | interior(>3) clean={} broken={}",
        near.0, near.1, far.0, far.1
    );
}

/// The number the whole design turns on: unions of whole footprints, with every
/// seed held clear of a defect.
///
/// The single-footprint sweep said the clearance is what the irregularity is
/// about. This asks whether that carries to unions, which is what a compiler
/// would actually emit.
#[test]
fn unions_of_clear_footprints_are_measured_at_several_sizes() {
    for nxp in [21usize, 40] {
        let probe = LatticeProbe::new(nxp);
        let distance = probe
            .mesh
            .method_c_defect_ring_distance(&probe.m_neighbors)
            .expect("defect distance");
        let start = (2..=probe.mesh.nmd)
            .find(|&im| distance[im] > 5)
            .expect("a clear M point");
        let lattice: Vec<usize> = probe
            .lattice_from(start, 600)
            .into_iter()
            .filter(|&im| distance[im] > 3)
            .collect();
        println!("P-1.z nxp={nxp} clear lattice seeds: {}", lattice.len());
        if lattice.len() < 12 {
            continue;
        }
        for size in [2usize, 3, 5, 9] {
            let mut rng = Lcg(0xC1EA_0000_u64 + size as u64 * 7 + nxp as u64);
            let (mut clean, mut broken, mut failed) = (0usize, 0usize, 0usize);
            for _ in 0..200 {
                let mut chosen = BTreeSet::new();
                while chosen.len() < size {
                    chosen.insert(lattice[rng.below(lattice.len())]);
                }
                let seeds: Vec<usize> = chosen.into_iter().collect();
                match probe.union_is_legal(&seeds) {
                    Ok(true) => clean += 1,
                    Ok(false) => broken += 1,
                    Err(_) => failed += 1,
                }
            }
            println!(
                "P-1.z nxp={nxp} union size {size}: clean={clean} mod3_broken={broken} walk_failed={failed}"
            );
        }
        // Adjacency matters: neighbouring seeds share footprint faces.
        let mut rng = Lcg(0xADAC_0000_u64 + nxp as u64);
        let (mut clean, mut broken, mut failed) = (0usize, 0usize, 0usize);
        for _ in 0..200 {
            let anchor = rng.below(lattice.len().saturating_sub(6).max(1));
            let seeds: Vec<usize> = lattice[anchor..(anchor + 4).min(lattice.len())].to_vec();
            if seeds.len() < 2 {
                continue;
            }
            match probe.union_is_legal(&seeds) {
                Ok(true) => clean += 1,
                Ok(false) => broken += 1,
                Err(_) => failed += 1,
            }
        }
        println!(
            "P-1.z nxp={nxp} contiguous runs of 4: clean={clean} mod3_broken={broken} walk_failed={failed}"
        );
    }
}
