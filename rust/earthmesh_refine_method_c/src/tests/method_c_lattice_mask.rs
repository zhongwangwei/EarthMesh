//! The mask entry point, and the claim it rests on.
//!
//! `spawn_nest` searches for a legal patch around a shape and refuses when it
//! finds none. `spawn_nest_from_face_masks` does no searching: the mask goes
//! straight to the kernel, and a mask the kernel will not take comes back as
//! the kernel's own error. Nothing here falls back or repairs, which is the
//! point -- these tests measure what the seam actually admits.

use super::*;
use std::collections::BTreeSet;

fn probe(nxp: usize) -> (MethodCMesh, Vec<IcosahedronMPointNeighbors>) {
    let mesh = MethodCMesh::from_icosahedron(nxp, 0, 1.0, 0.25).expect("base mesh");
    let m_neighbors =
        derive_icosahedron_m_neighbors_canonical_checked(mesh.nmd, &mesh.u_edges, &mesh.w_faces)
            .expect("Method-C neighbors");
    (mesh, m_neighbors)
}

fn first_clear_hexagon(mesh: &MethodCMesh, m_neighbors: &[IcosahedronMPointNeighbors]) -> usize {
    let distance = mesh
        .method_c_defect_ring_distance(m_neighbors)
        .expect("defect distance");
    (2..=mesh.nmd)
        .find(|&im| distance[im] > METHOD_C_LATTICE_DEFECT_CLEARANCE_RINGS + 2)
        .expect("an M point clear of every defect")
}

/// What the clearance does buy: a single footprint, at every size and both
/// generations.
///
/// It does not buy the union. That was the nest-compiler premise and it was
/// measured false -- nine clear seeds close 21% of the time. The union sweep
/// lives in `method_c_lattice_invariants`; this pins the part that held.
#[test]
fn a_single_footprint_clear_of_every_defect_closes_as_triplets() {
    for nxp in [21usize, 40] {
        let (mesh, m_neighbors) = probe(nxp);
        let start = first_clear_hexagon(&mesh, &m_neighbors);
        let seeds = mesh
            .method_c_lattice_seeds_with_clearance(
                start,
                400,
                METHOD_C_LATTICE_DEFECT_CLEARANCE_RINGS,
                &m_neighbors,
            )
            .expect("lattice seeds");
        assert!(
            seeds.len() >= 50,
            "nxp {nxp}: the walk found only {} clear seeds",
            seeds.len()
        );

        for &seed in seeds.iter().take(80) {
            let mask = mesh
                .method_c_footprint_mask(&[seed], &m_neighbors)
                .expect("footprint mask");
            assert!(
                mesh.method_c_mask_closes(&mask, &m_neighbors),
                "nxp {nxp}: a footprint more than {METHOD_C_LATTICE_DEFECT_CLEARANCE_RINGS} rings from every defect did not close: seed {seed}"
            );
        }
    }
}

/// Whole footprints, not faces. Dropping one face is what leaves the class.
#[test]
fn dropping_a_single_face_from_a_footprint_breaks_the_count() {
    let (mesh, m_neighbors) = probe(21);
    let start = first_clear_hexagon(&mesh, &m_neighbors);
    let seeds = mesh
        .method_c_lattice_seeds_with_clearance(
            start,
            200,
            METHOD_C_LATTICE_DEFECT_CLEARANCE_RINGS,
            &m_neighbors,
        )
        .expect("lattice seeds");
    let subset: Vec<usize> = seeds.iter().copied().take(3).collect();
    let mask = mesh
        .method_c_footprint_mask(&subset, &m_neighbors)
        .expect("footprint mask");
    assert!(mesh.method_c_mask_closes(&mask, &m_neighbors));

    let mut broken = 0usize;
    let mut tried = 0usize;
    for face in 2..=mesh.nwd {
        if !mask[face] {
            continue;
        }
        let mut dented = mask.clone();
        dented[face] = false;
        tried += 1;
        if !mesh.method_c_mask_closes(&dented, &m_neighbors) {
            broken += 1;
        }
        if tried >= 40 {
            break;
        }
    }
    assert!(
        broken > 0,
        "removing a face never broke the mask, so the class is not what it is claimed to be"
    );
}

/// The kernel takes a mask directly, with no region search between.
///
/// Which mask it takes is a separate question, and a narrower one than the
/// perimeter test predicts: a mask whose every ring is a multiple of three can
/// still be refused during the build, for vertex valence
/// ("exceeds 7-edge Method-C ring"). That is a gate past the five the design
/// enumerated, and it is why this walks candidates instead of asserting the
/// first one lands. The entry point's contract is that an accepted mask refines
/// and a refused one comes back as the kernel's own error, not that every mask
/// is accepted.
#[test]
fn the_kernel_refines_from_a_mask_without_searching_for_a_region() {
    let (mesh, m_neighbors) = probe(21);
    let start = first_clear_hexagon(&mesh, &m_neighbors);
    let seeds = mesh
        .method_c_lattice_seeds_with_clearance(
            start,
            200,
            METHOD_C_LATTICE_DEFECT_CLEARANCE_RINGS,
            &m_neighbors,
        )
        .expect("lattice seeds");
    let faces_before = mesh.nwd;

    let mut accepted = 0usize;
    let mut refused = 0usize;
    let mut refined_once = None;
    for &seed in seeds.iter().take(40) {
        let result = mesh.spawn_nest_from_face_masks(1, 7, |current, _child_level| {
            let neighbors = derive_icosahedron_m_neighbors_canonical_checked(
                current.nmd,
                &current.u_edges,
                &current.w_faces,
            )
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
            current.method_c_footprint_mask(&[seed], &neighbors)
        });
        match result {
            Ok(refined) => {
                accepted += 1;
                refined_once.get_or_insert(refined);
            }
            Err(_) => refused += 1,
        }
    }
    println!("single-footprint masks: accepted={accepted} refused={refused}");

    let refined = refined_once.expect("no single footprint was accepted by the kernel at all");
    assert!(
        refined.nwd > faces_before,
        "the mask asked for faces: {} vs {faces_before}",
        refined.nwd
    );
    refined.validate_topology().expect("refined mesh is valid");
}

/// An empty mask stops, rather than refining nothing and reporting success.
#[test]
fn an_empty_mask_stops_the_pass_loop() {
    let (mesh, _m_neighbors) = probe(21);
    let faces_before = mesh.nwd;
    let unchanged = mesh
        .spawn_nest_from_face_masks(3, 7, |current, _| Ok(vec![false; current.nwd + 1]))
        .expect("an empty mask is not an error");
    assert_eq!(unchanged.nwd, faces_before);
    let _ = BTreeSet::<usize>::new();
}
