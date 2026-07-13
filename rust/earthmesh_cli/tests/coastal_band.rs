//! Rust port of util/hydro_mesh/coastal_band.py (pure grid core): land mask from
//! elevation + Chebyshev coastal-band cell selection. Pure (no NetCDF/CaMa data).

use earthmesh_cli::{
    coastal_band_io::coastal_band_cells, coastal_band_io::coastal_land_mask_from_elevation,
};

#[test]
fn land_mask_drops_undef_and_nonfinite() {
    let elev = vec![vec![10.0, -9999.0, 5.0], vec![f64::NAN, 0.0, -9999.0]];
    let mask = coastal_land_mask_from_elevation(&elev, -9999.0);
    assert_eq!(mask[0], vec![true, false, true]);
    assert_eq!(mask[1], vec![false, true, false]);
}

#[test]
fn band_selects_cells_adjacent_to_transition() {
    // Left two columns land, right two ocean; the coastline runs between col 1 and 2.
    let land_mask = vec![
        vec![true, true, false, false],
        vec![true, true, false, false],
        vec![true, true, false, false],
    ];
    let band = coastal_band_cells(&land_mask, 1, true, true).expect("band");
    for row in &band {
        // only the two columns straddling the transition are in the band.
        assert_eq!(*row, vec![false, true, true, false]);
    }
}

#[test]
fn ocean_side_only_excludes_land_cells() {
    let land_mask = vec![vec![true, true, false, false]];
    let band = coastal_band_cells(&land_mask, 1, false, true).expect("band");
    // land-side excluded -> col 1 (land) is false even though adjacent to ocean.
    assert_eq!(band[0], vec![false, false, true, false]);
}

#[test]
fn rejects_radius_below_one() {
    assert!(coastal_band_cells(&[vec![true]], 0, true, true).is_err());
}
