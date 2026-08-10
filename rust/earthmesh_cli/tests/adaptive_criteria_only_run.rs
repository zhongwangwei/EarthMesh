//! A criteria-driven point+radius run, all the way through the CLI.
//!
//! This is the shape that only appears in a whole run: `refine_cal` on because
//! a criterion is on, no calculated mask files anywhere, and the point+radius
//! route expected to read the criteria itself. The legacy calculated-region
//! reader used to stand down only for the h-field, so this configuration sent
//! it looking for mask files a criteria-driven run never has, and the run died
//! with "unsupported mask source extension for /tmp".
//!
//! Ocean runs did not catch it: the ocean example drives refinement through
//! `refine_spc`, which is a different branch. It takes a land or atmosphere
//! mesh, where `refine_num_landtypes` is the criterion, to reach this path.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn temp_root(name: &str) -> PathBuf {
    let sequence = ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "earthmesh_adaptive_criteria_{name}_{}_{sequence}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
}

/// Production land-type raster; skipped when the data is not mounted.
fn landtype_path() -> Option<PathBuf> {
    let path = std::env::var("EARTHMESH_LANDTYPE")
        .map(PathBuf::from)
        .ok()?;
    path.is_file().then_some(path)
}

/// The shipped example for `mesh_type`, rewired so a criterion is the only
/// refinement source.
///
/// Derived from the example rather than hand-written: a namelist assembled from
/// scratch needs every field the engine validates, and chasing them one error at
/// a time produces a file that resembles no real configuration. This keeps the
/// example's own settings and changes exactly what the case is about.
fn criteria_only_namelist(mesh_type: &str, landtype: &str, depth: usize, backend: &str) -> String {
    let example = match mesh_type {
        "atmos" | "atmosmesh" => "atmosphere_hex_global.nml",
        _ => "land_hex_global.nml",
    };
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/default")
        .join(example);
    let source = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));

    let replace_quoted = |text: String, field: &str, value: &str| -> String {
        let mut out = String::with_capacity(text.len());
        for line in text.lines() {
            if line.trim_start().starts_with(field) {
                out.push_str(&format!("  {field} = '{value}'\n"));
            } else {
                out.push_str(line);
                out.push('\n');
            }
        }
        out
    };

    let mut text = source
        .replace(
            "NL%nxp                   = 64",
            "NL%nxp                   = 21",
        )
        .replace(
            "NL%mask_domain_global    = .TRUE.",
            "NL%mask_domain_global    = .FALSE.",
        )
        // A criterion is the only source: no specified regions, and the
        // calculated-region prefix left at the engine's "unconfigured" sentinel.
        .replace(
            "RL%refine_spc              = .TRUE.",
            "RL%refine_spc              = .FALSE.",
        )
        .replace(
            "RL%refine_cal              = .FALSE.",
            "RL%refine_cal              = .TRUE.",
        );
    text = replace_quoted(text, "NL%landtype_file", landtype);
    // The example ships 120 cells per degree; the mounted raster decides.
    let per_degree = landtype_cells_per_degree(landtype);
    text = text
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("NL%gridnum_perdegree") {
                format!("  NL%gridnum_perdegree     = {per_degree}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    // Inserted, not replaced: the examples do not carry this field, and
    // `replace_quoted` on a line that is not there is a silent no-op -- the
    // case would run the other backend and only say so much later.
    text = text.replacen(
        "&mkgrd",
        &format!("&mkgrd\n  NL%refine_backend = '{backend}'"),
        1,
    );
    assert!(
        text.contains(&format!("NL%refine_backend = '{backend}'")),
        "the fixture must actually select the backend it was asked for"
    );
    text = replace_quoted(text, "NL%mask_domain_type", "bbox");
    text = replace_quoted(
        text,
        "NL%mask_domain_fprefix",
        "inline:bbox:w=108,e=120,s=18,n=26",
    );
    text = replace_quoted(text, "RL%mask_refine_cal_fprefix", "/tmp");
    text.push_str(&format!(
        "\n&adaptive\n adaptive_on = .true.\n adaptive_max_level = {depth}\n \
         adaptive_coastline = .false.\n/\n"
    ));
    text
}

/// Cells per degree the mounted land-type raster actually carries.
fn landtype_cells_per_degree(landtype: &str) -> usize {
    let file = netcdf::open(Path::new(landtype)).expect("open landtype");
    let lon = ["lon", "longitude"]
        .iter()
        .find_map(|name| file.dimension(name))
        .expect("landtype longitude dimension")
        .len();
    lon / 360
}

fn run(mesh_type: &str, depth: usize) -> (bool, String) {
    run_with_backend(mesh_type, depth, "method_c")
}

fn run_with_backend(mesh_type: &str, depth: usize, backend: &str) -> (bool, String) {
    let Some(landtype) = landtype_path() else {
        return (true, "EARTHMESH_LANDTYPE not set; skipped".to_string());
    };
    let root = temp_root(&format!("{mesh_type}_{backend}"));
    let namelist = root.join("case.nml");
    fs::write(
        &namelist,
        criteria_only_namelist(mesh_type, &landtype.to_string_lossy(), depth, backend),
    )
    .expect("write namelist");

    let output = Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
        .current_dir(&root)
        .arg(&namelist)
        .output()
        .expect("run earthmesh_cli");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(root);
    (output.status.success(), combined)
}

#[test]
fn a_land_mesh_refines_from_its_criterion_alone() {
    let (ok, log) = run("landmesh", 2);
    if log.contains("skipped") {
        eprintln!("{log}");
        return;
    }
    assert!(ok, "criteria-only land run failed:\n{log}");
    assert!(
        log.contains("adaptive refine level 1"),
        "the point+radius route must have run:\n{log}"
    );
    assert!(
        !log.contains("unsupported mask source extension"),
        "the legacy calculated-region reader must stand down:\n{log}"
    );
    assert!(
        log.contains("refine_realized_max_level=2"),
        "both levels must materialize:\n{log}"
    );
}

#[test]
fn an_atmosphere_mesh_refines_from_its_criterion_alone() {
    let (ok, log) = run("atmosmesh", 2);
    if log.contains("skipped") {
        eprintln!("{log}");
        return;
    }
    assert!(ok, "criteria-only atmosphere run failed:\n{log}");
    assert!(log.contains("adaptive refine level 1"), "{log}");
    assert!(log.contains("refine_realized_max_level=2"), "{log}");
}

#[test]
fn a_resolution_dependent_criterion_narrows_as_the_levels_deepen() {
    // The reason for re-planning per pass: fewer land types crowd into a cell
    // once the cell is smaller, so each level has less left to ask for. A route
    // that settled the demand once would report the same count every level.
    let (ok, log) = run("landmesh", 3);
    if log.contains("skipped") {
        eprintln!("{log}");
        return;
    }
    assert!(ok, "criteria-only land run failed:\n{log}");
    let demanded: Vec<usize> = log
        .lines()
        .filter_map(|line| {
            let tail = line.split("circles over ").nth(1)?;
            tail.split(' ').next()?.parse().ok()
        })
        .collect();
    assert_eq!(demanded.len(), 3, "expected three passes:\n{log}");
    for pair in demanded.windows(2) {
        assert!(
            pair[1] < pair[0],
            "demand must narrow as cells shrink, got {demanded:?}:\n{log}"
        );
    }
}

/// The same criteria-only run on red-green, where the criteria are served
/// rather than refused.
///
/// Method-C refuses this configuration outright the moment a criterion demands
/// anything (`METHOD_C_ADAPTIVE_SUSPENDED`): its seed lattice steps three cells
/// at a time and its perimeter must be a multiple of three, so a region shaped
/// by the data is refused rather than approximated. Red-green grows a marking
/// it cannot take as given, which is the whole reason the backend exists -- and
/// this is the run that says so end to end.
#[test]
fn red_green_serves_the_criterion_method_c_suspends() {
    let (ok, log) = run_with_backend("landmesh", 2, "red_green");
    if log.contains("skipped") {
        eprintln!("{log}");
        return;
    }
    assert!(ok, "criteria-only red-green run failed:\n{log}");
    assert!(
        log.contains("red-green refine level 1"),
        "the red-green route must have run:\n{log}"
    );
    assert!(
        log.contains("triangles split"),
        "and must have split triangles the criterion asked for:\n{log}"
    );
    assert!(
        log.contains("refine_realized_max_level=2"),
        "completed Red-Green passes must not be reported as level zero:\n{log}"
    );

    // The same configuration on Method-C is refused, which is what makes the
    // run above worth having rather than a second way of doing the same thing.
    let (method_c_ok, method_c_log) = run_with_backend("landmesh", 2, "method_c");
    assert!(
        !method_c_ok && method_c_log.contains("suspended on the Method-C backend"),
        "Method-C must still refuse the shape it cannot build:\n{method_c_log}"
    );
}
