//! Snapshot (golden) tests for small, deterministic outputs. No external data — a
//! fixed tiny synthetic mesh produces byte-stable CSV / JSON, so a regression in a
//! writer fails loudly with a diff.

use earthmesh_quality::coupling::{
    classify_all, to_coupling_csv, to_coupling_manifest_json, CoupledCellFractions,
    CoupledCellInput, CoupledThresholds,
};
use earthmesh_quality::QualityLevel;

fn cell(land: f64, ocean: f64, neighbors: Vec<usize>) -> CoupledCellInput {
    CoupledCellInput {
        fractions: CoupledCellFractions {
            land_fraction: land,
            ocean_fraction: ocean,
            ..Default::default()
        },
        neighbors,
        ..Default::default()
    }
}

#[test]
fn coupling_csv_snapshot() {
    let cells = vec![cell(0.5, 0.5, vec![1]), cell(0.0, 1.0, vec![0])];
    let classes = classify_all(&cells, &CoupledThresholds::default());
    let csv = to_coupling_csv(&cells, &classes);
    let expected = "cell_id,class,land_fraction,ocean_fraction,river_fraction,wetland_fraction,estuary_fraction\n\
0,mixed_coast,0.5,0.5,0,0,0\n\
1,ocean,0,1,0,0,0\n";
    assert_eq!(csv, expected, "coupling CSV snapshot drifted");
}

#[test]
fn coupling_manifest_snapshot() {
    let manifest =
        to_coupling_manifest_json(&[("coupling_csv", "out/coupling.csv")], QualityLevel::Warn);
    let expected = "{\n  \"kind\": \"earthmesh_coupling_manifest\",\n  \"verdict\": \"warn\",\n  \"products\": {\n    \"coupling_csv\": \"out/coupling.csv\"\n  }\n}\n";
    assert_eq!(
        manifest, expected,
        "coupling manifest JSON snapshot drifted"
    );
}
