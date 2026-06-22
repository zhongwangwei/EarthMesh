//! R7 coupling-quality validator (earthmesh_quality::coupling) fed by the mesh +
//! land-type land/ocean signal, exercised on a synthetic coastline — no NetCDF.
//! Proves the validator now consumes the real land/ocean fraction signal: coast cells
//! straddle the boundary (MixedCoast), couple to ocean neighbours, and disconnected
//! cells surface as orphans.

use earthmesh_cli::landtype_coupling_quality;

#[test]
fn synthetic_coastline_classifies_mixed_coast_and_couples_to_ocean() {
    // 3x3 grid, 4-connected:
    //   0 1 2
    //   3 4 5
    //   6 7 8
    // left column land (0,3,6 = 1.0), right column ocean (2,5,8 = 0.0),
    // middle column coast (1,4,7 = 0.5 -> MixedCoast straddling the boundary).
    let land = [1.0, 0.5, 0.0, 1.0, 0.5, 0.0, 1.0, 0.5, 0.0];
    let nb = vec![
        vec![1, 3],
        vec![0, 2, 4],
        vec![1, 5],
        vec![0, 4, 6],
        vec![1, 3, 5, 7],
        vec![2, 4, 8],
        vec![3, 7],
        vec![4, 6, 8],
        vec![5, 7],
    ];
    let r = landtype_coupling_quality(&land, &nb);

    assert_eq!(
        r.total_land_cells, 6,
        "3 pure land + 3 mixed coast count as land"
    );
    assert_eq!(r.total_ocean_cells, 3);
    assert_eq!(r.mixed_coastline_cells, 3);
    assert_eq!(r.coast_overlap_cells, 3);
    assert_eq!(r.orphan_land_cells, 0);
    assert_eq!(r.orphan_ocean_cells, 0);
    // every mixed cell has an ocean neighbour -> coastline fully preserved
    assert_eq!(r.coastline_preservation_score, 1.0);
    // land + ocean = 1 exactly -> mass conserved
    assert!(r.mass_conservation_residual.abs() < 1e-12);
    // coastline couples mixed-coast cells to their ocean neighbours
    assert!(
        r.coupling_row_count > 0,
        "expected coastline coupling maps, got {}",
        r.coupling_row_count
    );
    // mass/orphans clean, but coast overlap present -> Warn (not Pass)
    assert_eq!(r.verdict.as_str(), "warn");
}

#[test]
fn disconnected_cells_are_orphans_and_fail() {
    // one land + one ocean cell, neither has a neighbour -> both orphans -> Fail.
    let land = [1.0, 0.0];
    let nb = vec![vec![], vec![]];
    let r = landtype_coupling_quality(&land, &nb);
    assert_eq!(r.orphan_land_cells, 1);
    assert_eq!(r.orphan_ocean_cells, 1);
    assert_eq!(r.verdict.as_str(), "fail");
}

#[test]
fn island_land_cell_surrounded_by_ocean_stays_land_without_orphan() {
    // a "+" : centre land (0) with four ocean arms (1..4). Centre's neighbours are all
    // ocean -> reclassified Island (still counted as land); nothing is disconnected.
    let land = [1.0, 0.0, 0.0, 0.0, 0.0];
    let nb = vec![vec![1, 2, 3, 4], vec![0], vec![0], vec![0], vec![0]];
    let r = landtype_coupling_quality(&land, &nb);
    assert_eq!(r.total_land_cells, 1, "the island counts as land");
    assert_eq!(r.total_ocean_cells, 4);
    assert_eq!(r.orphan_land_cells, 0);
    assert_eq!(r.orphan_ocean_cells, 0);
    // pure cells, no coast overlap, mass conserved -> Pass
    assert_eq!(r.verdict.as_str(), "pass");
}
