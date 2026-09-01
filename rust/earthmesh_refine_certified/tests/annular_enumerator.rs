use earthmesh_refine_certified::coarsen::{
    annular_small_exact_oracle_json, brute_force_flip_annulus_keys, cut_annulus_polygon,
    enumerate_canonical_seam_annulus,
};
use std::collections::BTreeSet;

fn cycles(m: usize, n: usize) -> (Vec<usize>, Vec<usize>) {
    ((0..m).collect(), (100..100 + n).collect())
}

#[test]
fn cut_polygon_duplicates_only_root_endpoints() {
    let (lower, upper) = cycles(4, 5);
    let cut = cut_annulus_polygon(&lower, &upper, 1, 2).unwrap();
    assert_eq!(cut.occurrences.len(), lower.len() + upper.len() + 2);
    let mut counts = std::collections::BTreeMap::<usize, usize>::new();
    for occurrence in &cut.occurrences {
        *counts.entry(occurrence.global_source_slot).or_default() += 1;
    }
    assert_eq!(counts[&lower[1]], 2);
    assert_eq!(counts[&upper[2]], 2);
    assert!(counts
        .iter()
        .all(|(&vertex, &count)| count == 1 || [lower[1], upper[2]].contains(&vertex)));
}

#[test]
fn csae_matches_flip_closure_small_exact_oracles() {
    for (m, n) in [(3, 3), (3, 4), (4, 4), (4, 5)] {
        let (lower, upper) = cycles(m, n);
        let csae = enumerate_canonical_seam_annulus(&lower, &upper, &BTreeSet::new()).unwrap();
        let csae_keys = csae
            .topologies
            .iter()
            .map(|topology| topology.topology_key.clone())
            .collect::<BTreeSet<_>>();
        let brute_force = brute_force_flip_annulus_keys(&lower, &upper).unwrap();
        assert_eq!(csae_keys, brute_force, "{m}+{n} annulus family");
        assert!(csae.topologies.iter().all(|topology| {
            topology.triangles.len() == m + n
                && topology
                    .topology_key
                    .triangles
                    .windows(2)
                    .all(|pair| pair[0] != pair[1])
        }));
    }
}

#[test]
fn canonical_root_removes_cross_seam_duplicates() {
    let (lower, upper) = cycles(3, 4);
    let family = enumerate_canonical_seam_annulus(&lower, &upper, &BTreeSet::new()).unwrap();
    let keys = family
        .topologies
        .iter()
        .map(|topology| &topology.topology_key)
        .collect::<BTreeSet<_>>();
    assert_eq!(keys.len(), family.topologies.len());
    assert!(family
        .evidence
        .glue_rejects
        .get("NonCanonicalRootBridge")
        .is_some_and(|count| *count > 0));
}

#[test]
fn frozen_small_exact_oracle_passes() {
    let evidence = include_str!("fixtures/annular_small_exact_oracle.json");
    assert!(evidence.contains("\"lower\":3,\"upper\":3,\"csae_topologies\":21"));
    assert!(evidence.contains("\"lower\":4,\"upper\":5,\"csae_topologies\":4180"));
    assert!(evidence.contains("\"all_families_equal\":true"));
}

#[test]
#[ignore = "write the PR105 small exact oracle artifact"]
fn write_small_exact_oracle() {
    let json = annular_small_exact_oracle_json().unwrap();
    if let Ok(path) = std::env::var("EARTHMESH_ANNULAR_ORACLE_JSON") {
        std::fs::write(path, &json).unwrap();
    }
    println!("{json}");
}
