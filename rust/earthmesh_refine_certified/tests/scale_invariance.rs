//! Reverse coarsening of one lattice pattern at finer and finer bases (design
//! N1). The requirement is a disk drawn in the finest lattice's own indices
//! around the same base lattice vertex, so every base sees the same pattern,
//! cell for cell, only smaller; and near one point the lattice has the same
//! shape at every scale. A coarsening whose search depends on how large the
//! cells are -- not on their shapes -- shows here as counts that change with
//! the base.

use std::collections::BTreeSet;

use earthmesh_refine_certified::{
    coarsen::{
        run_region_component_epochs, ElasticCmrcConfig, ElasticCmrcOutcome, RegionEpochs,
        RegionScope, SettledRegion,
    },
    mother_grid::{lattice, region::descendant_faces, vertex_origin},
    requirement::{graded_envelope, one_ring_adjacency},
    AngleContractId, MotherGrid, SourceLevelField, TriangleAddress,
};

fn grow(faces: &mut BTreeSet<TriangleAddress>, rings: usize) {
    for _ in 0..rings {
        for face in faces.clone() {
            faces.extend(lattice::faces_around(face).unwrap());
        }
    }
}

/// The requirement: a disk of `radius` finest-lattice steps around the
/// centre vertex, or every base face within `rings` vertex rings of it.
#[derive(Clone, Copy, Debug)]
enum Pattern {
    Disk { radius: f64 },
    BaseRings { rings: usize },
}

/// One component's account: levels, topology states, elastic iterations,
/// outcome.
type Account = (usize, usize, usize, usize, String);

/// Coarsens `pattern` at level `levels` around base face 0's lattice vertex
/// nearest its centre; returns each component's account. `retry_at_failure`
/// is `ElasticCmrcConfig::retry_at_failure`.
fn coarsen(
    base_n: usize,
    levels: usize,
    pattern: Pattern,
    rings: usize,
    retry_at_failure: bool,
) -> Vec<Account> {
    let fine_n = base_n << levels;
    let centre = (base_n / 3, base_n / 3);
    let (ci, cj) = ((centre.0 << levels) as f64, (centre.1 << levels) as f64);
    let origin = vertex_origin(base_n, 0, centre.0, centre.1).unwrap();
    let mut demand_faces = lattice::faces_at_vertex(base_n, origin)
        .into_iter()
        .collect::<BTreeSet<_>>();
    if let Pattern::BaseRings { rings } = pattern {
        grow(&mut demand_faces, rings);
    }
    let mut certified = lattice::faces_at_vertex(base_n, origin)
        .into_iter()
        .collect::<BTreeSet<_>>();
    grow(&mut certified, rings);
    let mut built = certified.clone();
    grow(&mut built, 2);
    let settled = SettledRegion::by_address(base_n, &built).unwrap();
    let region = MotherGrid::generate_faces(
        fine_n,
        descendant_faces(built.iter().copied(), fine_n).unwrap(),
    )
    .unwrap();
    let index = region.region.as_ref().unwrap();
    // Sites by the faces they corner, for the base-ring pattern.
    let mut demand_sites = BTreeSet::new();
    if let Pattern::BaseRings { .. } = pattern {
        for face in region.mesh.active_triangle_slots() {
            let mut base = region.triangle_addresses[face].unwrap();
            while base.n > base_n {
                base = base.parent_2_to_1().unwrap();
            }
            if demand_faces.contains(&base) {
                demand_sites.extend(region.mesh.triangles()[face]);
            }
        }
    }
    let level_at = |slot: usize| match pattern {
        Pattern::Disk { radius } => {
            let origin = index.origin(slot).unwrap();
            let (di, dj) = (origin.i as f64 - ci, origin.j as f64 - cj);
            if origin.face == 0 && (di * di + dj * dj + di * dj).sqrt() <= radius {
                levels
            } else {
                0
            }
        }
        Pattern::BaseRings { .. } => {
            if demand_sites.contains(&slot) {
                levels
            } else {
                0
            }
        }
    };
    let sites = region.mesh.active_vertex_slots().collect::<Vec<_>>();
    let mut compact = vec![usize::MAX; region.mesh.vertices().len()];
    for (row, &site) in sites.iter().enumerate() {
        compact[site] = row;
    }
    let projected = sites.iter().map(|&site| level_at(site)).collect::<Vec<_>>();
    let triangles = region
        .mesh
        .active_triangle_slots()
        .map(|face| region.mesh.triangles()[face].map(|site| compact[site]))
        .collect::<Vec<_>>();
    let adjacency = one_ring_adjacency(&[vec![[0; 3]; 2], triangles].concat(), sites.len());
    let graded = graded_envelope(&adjacency, &projected, 3);
    let mut graded_by_slot = vec![0; region.mesh.vertices().len()];
    for (&site, &level) in sites.iter().zip(&graded) {
        graded_by_slot[site] = level;
    }
    let scope = RegionScope::new(&region, &certified, base_n).unwrap();
    let config = ElasticCmrcConfig {
        angle_contract: AngleContractId::DomainQuality38To82V1,
        max_level: levels,
        max_adjacent_level_delta: 1,
        initial_transition_rings: 1,
        maximum_transition_rings: 4,
        topology_states_per_component: 10_000,
        elastic_iterations_per_topology: 256,
        interval_boxes_per_component: 1_000_000,
        total_transition_states: 100_000,
        allow_safe_fallback: false,
        retry_at_failure,
    };
    let outcome = run_region_component_epochs(
        &region,
        &region.mesh,
        &SourceLevelField::from_active_voronoi_cells(&region.mesh, projected).unwrap(),
        &graded_by_slot,
        &config,
        &RegionEpochs {
            built_bases: built,
            settled,
            scope,
        },
    );
    let ElasticCmrcOutcome::Completed(result) = outcome else {
        panic!("base {base_n}: the region must complete: {outcome:?}");
    };
    result
        .report
        .components
        .iter()
        .map(|component| {
            (
                component.source_level,
                component.target_level,
                component.topology_states,
                component.elastic_iterations,
                format!("{:?}", component.outcome),
            )
        })
        .collect()
}

/// From 3.7 km cells to 29 m (bases 480 and 61440) the lattice is flat
/// enough that one pattern is one problem: in the search's own order the
/// coarsening takes the same topology states, and elastic iterations within
/// a few. A fixed finite-difference floor once made it 8 states at 3.7 km and
/// 5 at 29 m (guide 11.110). With retries drawn to the failed face (guide
/// 11.122) the next candidate depends on which face failed, and the elastic
/// solutions at the two scales differ by a tenth of a degree, enough to fail
/// first at different faces; there both scales must still certify.
#[test]
fn one_pattern_coarsens_alike_from_kilometres_to_thirty_metres() {
    let pattern = Pattern::BaseRings { rings: 1 };
    let coarse = coarsen(480, 2, pattern, 14, false);
    let fine = coarsen(61440, 2, pattern, 14, false);
    for base_n in [480, 61440] {
        let drawn = coarsen(base_n, 2, pattern, 14, true);
        assert_eq!(drawn.len(), coarse.len(), "base {base_n}");
        for (drawn, alike) in drawn.iter().zip(&coarse) {
            assert_eq!(
                (drawn.0, drawn.1, &drawn.4),
                (alike.0, alike.1, &alike.4),
                "base {base_n}: {drawn:?}"
            );
        }
    }
    assert_eq!(coarse.len(), fine.len());
    for (coarse, fine) in coarse.iter().zip(&fine) {
        assert_eq!(
            (coarse.0, coarse.1, coarse.2, &coarse.4),
            (fine.0, fine.1, fine.2, &fine.4),
            "{coarse:?} against {fine:?}"
        );
        assert!(
            coarse.3.abs_diff(fine.3) * 20 <= coarse.3.max(fine.3),
            "{coarse:?} against {fine:?}"
        );
    }
}

/// Probe: `cargo test --release --test scale_invariance -- --ignored --nocapture`.
#[test]
#[ignore]
fn probe_one_pattern_at_finer_bases() {
    let bases = std::env::var("SCALE_PROBE_BASES")
        .map(|bases| {
            bases
                .split(',')
                .map(|base| base.trim().parse::<usize>().unwrap())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|_| vec![30, 120, 480, 1920]);
    for base_n in bases {
        let started = std::time::Instant::now();
        let pattern = match std::env::var("SCALE_PROBE_DISK") {
            Ok(radius) => Pattern::Disk {
                radius: radius.parse().unwrap(),
            },
            Err(_) => Pattern::BaseRings { rings: 1 },
        };
        let components = coarsen(base_n, 2, pattern, 14, true);
        println!(
            "base {base_n:5} ({:.1} s): {}",
            started.elapsed().as_secs_f64(),
            components
                .iter()
                .map(|(from, to, states, iterations, outcome)| {
                    format!("{from}->{to} states {states} iterations {iterations} {outcome}")
                })
                .collect::<Vec<_>>()
                .join(" | ")
        );
    }
}
