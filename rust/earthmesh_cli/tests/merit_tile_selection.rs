//! Regression for the antimeridian MERIT-Hydro tile-selection bug (audit H-B1):
//! a query bbox that crosses ±180° (west > east) must still select the tiles on
//! both sides of the dateline. `select_merit_hydro_tiles` only reads file *names*
//! (5° tile bounds), so this needs no NetCDF data — empty files suffice.

use earthmesh_cli::{select_merit_hydro_tiles, MeritLonLatBbox};
use std::path::Path;

fn touch(dir: &Path, name: &str) {
    std::fs::write(dir.join(name), b"").expect("touch tile file");
}

fn names(paths: &[std::path::PathBuf]) -> Vec<String> {
    paths
        .iter()
        .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(String::from))
        .collect()
}

#[test]
fn dateline_crossing_query_selects_both_sides() {
    let tmp = std::env::temp_dir().join(format!("em3_merit_tiles_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("mkdir tmp");
    // Tiles straddling the antimeridian + two far-away tiles.
    touch(&tmp, "n10e175.nc"); // 175..180 E
    touch(&tmp, "n10w180.nc"); // -180..-175
    touch(&tmp, "n10e000.nc"); // 0..5 E   (far)
    touch(&tmp, "n10e090.nc"); // 90..95 E (far)

    // Query 178E .. -178 (wraps the dateline), lat 11..14.
    let q = MeritLonLatBbox {
        west: 178.0,
        east: -178.0,
        south: 11.0,
        north: 14.0,
    };
    let selected = names(&select_merit_hydro_tiles(&tmp, q).expect("select"));
    assert!(
        selected.contains(&"n10e175.nc".to_string()),
        "missing east-of-dateline tile; got {selected:?}"
    );
    assert!(
        selected.contains(&"n10w180.nc".to_string()),
        "missing west-of-dateline tile (the H-B1 bug); got {selected:?}"
    );
    assert!(
        !selected.contains(&"n10e000.nc".to_string()),
        "false positive far tile"
    );
    assert!(
        !selected.contains(&"n10e090.nc".to_string()),
        "false positive far tile"
    );

    // Regression: an ordinary (non-wrapping) query is unaffected.
    let normal = MeritLonLatBbox {
        west: 1.0,
        east: 4.0,
        south: 11.0,
        north: 14.0,
    };
    let selected = names(&select_merit_hydro_tiles(&tmp, normal).expect("select normal"));
    assert_eq!(selected, vec!["n10e000.nc".to_string()]);

    let _ = std::fs::remove_dir_all(&tmp);
}
