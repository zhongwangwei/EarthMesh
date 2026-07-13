//! R7 coupling-quality validator (earthmesh_quality::coupling) fed by the mesh +
//! land-type land/ocean signal, exercised on a synthetic coastline — no NetCDF.
//! Proves the validator now consumes the real land/ocean fraction signal: coast cells
//! straddle the boundary (MixedCoast), couple to ocean neighbours, and disconnected
//! cells surface as orphans.

use earthmesh_cli::{
    hydro_delivery_coupling_quality::landtype_coupling_quality,
    hydro_delivery_coupling_quality::write_landtype_cell_mask_geojson,
};

fn write_landtype_file_with_points(path: &std::path::Path, land_points: &[(usize, usize)]) {
    let (nlons, nlats) = (360, 180);
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create landtype file");
    file.add_dimension("longitude", nlons)
        .expect("longitude dim");
    file.add_dimension("latitude", nlats).expect("latitude dim");
    let mut values = vec![0_i8; nlons * nlats];
    let idx = |lon: usize, lat: usize| lon * nlats + lat;
    for &(lon, lat) in land_points {
        values[idx(lon, lat)] = 1;
    }
    let mut var = file
        .add_variable::<i8>("landtype", &["longitude", "latitude"])
        .expect("landtype var");
    var.put_values(&values, (.., ..)).expect("write landtype");
}

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
    // Every fractional coast cell has an explicit coupling path, so no area is
    // unresolved merely because a conservative fraction is non-zero.
    assert_eq!(r.unresolved_fractional_area, 0.0);
    assert_eq!(r.verdict.as_str(), "pass");
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

#[test]
fn landtype_cell_mask_marks_fractional_cell_as_coast() {
    let dir = std::env::temp_dir().join(format!("em3_landtype_cell_mask_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let landtype = dir.join("landtype.nc");
    write_landtype_file_with_points(&landtype, &[(1, 0)]);
    let cells = dir.join("cells.geojson");
    std::fs::write(
        &cells,
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"cell_id":"c1","center_lon":-178.5,"center_lat":89.5},
         "geometry":{"type":"Polygon","coordinates":[[[-178.5,89.5],[-177.5,89.5],[-176.5,89.5],[-178.5,89.5]]]}}
        ]}"#,
    )
    .unwrap();

    let out = dir.join("mask.geojson");
    let count = write_landtype_cell_mask_geojson(&cells, &landtype, 1, &out).expect("cell mask");
    assert_eq!(count, 1);
    let json = std::fs::read_to_string(&out).unwrap();
    assert!(json.contains("\"mask_class\": \"COAST\""), "{json}");
    assert!(json.contains("\"land_fraction\""), "{json}");
    assert!(json.contains("\"ocean_fraction\""), "{json}");
    let _ = std::fs::remove_dir_all(dir);
}
