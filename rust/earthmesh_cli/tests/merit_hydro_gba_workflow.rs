//! LOC = MERIT-Hydro "Greater Bay Area" workflow: select the MERIT-Hydro tiles
//! over the Pearl River Delta, classify river/coast masks, and emit the
//! close-mask refinement `.nml` files that drive specified (close) refinement.
//! Runs against the local MERIT-Hydro tile directory; skips otherwise (set
//! EARTHMESH_MERIT_ROOT to override).

use std::path::PathBuf;

fn merit_root() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("EARTHMESH_MERIT_ROOT") {
        let p = PathBuf::from(p);
        return p.exists().then_some(p);
    }
    let d = PathBuf::from("/Volumes/Data01/MERIT_Hydro");
    d.exists().then_some(d)
}

#[test]
fn gba_region_produces_river_close_mask_nmls() {
    let Some(root) = merit_root() else {
        eprintln!("skip: no MERIT-Hydro root (set EARTHMESH_MERIT_ROOT)");
        return;
    };
    // Pearl River Delta / Greater Bay Area.
    let bbox = earthmesh_cli::MeritLonLatBbox {
        west: 111.0,
        east: 115.0,
        south: 21.0,
        north: 24.0,
    };
    let out = std::env::temp_dir().join("gba_workflow_test");
    let _ = std::fs::remove_dir_all(&out);

    let rep = earthmesh_cli::write_merit_hydro_region_close_masks(
        &root,
        bbox,
        30,
        Default::default(),
        &out,
        Default::default(),
    )
    .expect("MERIT-Hydro GBA close-mask workflow");

    assert!(
        rep.window_count >= 1,
        "expected at least one overlapping MERIT window"
    );
    // The delta has both major (R3) and minor (R2) rivers and a long coastline.
    assert!(
        rep.geojson.river_feature_count > 0,
        "expected river features"
    );
    assert!(
        rep.geojson.coast_feature_count > 0,
        "expected coast features"
    );
    assert!(
        rep.geojson.mask_counts.get("R3").copied().unwrap_or(0) > 0,
        "expected R3 (major river) cells"
    );
    // Refinement-ready close-mask namelists were emitted for rivers.
    assert!(
        !rep.river_nml.files.is_empty(),
        "expected river close-mask .nml files"
    );
    for f in &rep.river_nml.files {
        assert!(f.exists());
    }
    let _ = std::fs::remove_dir_all(&out);
}
