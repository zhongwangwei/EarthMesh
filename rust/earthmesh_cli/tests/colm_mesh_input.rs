use earthmesh_cli::colm_mesh_input::write_colm_mesh_from_gridfile;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn root(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "earthmesh_colm_input_{name}_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

// Deliberately asymmetric on-disk mesh with one or two sentinel rows.
fn mesh(path: &Path, rings: &[Vec<(f64, f64)>], placeholders: usize) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).unwrap();
    let n = placeholders + rings.iter().map(Vec::len).sum::<usize>();
    let w = placeholders + rings.len();
    let width = rings.iter().map(Vec::len).max().unwrap();
    for (name, length) in [
        ("sjx_points", n),
        ("lbx_points", w),
        ("dimb", 3),
        ("dimc", width),
    ] {
        file.add_dimension(name, length).unwrap();
    }
    let mut mlon = vec![0.; placeholders];
    let mut mlat = vec![0.; placeholders];
    let mut wlon = vec![0.; placeholders];
    let mut wlat = vec![0.; placeholders];
    let mut im = vec![1; w * width];
    let mut counts = vec![1; w];
    for (i, ring) in rings.iter().enumerate() {
        wlon.push(ring.iter().map(|p| p.0).sum::<f64>() / ring.len() as f64);
        wlat.push(ring.iter().map(|p| p.1).sum::<f64>() / ring.len() as f64);
        counts[placeholders + i] = ring.len() as i32;
        for (j, &(lon, lat)) in ring.iter().enumerate() {
            im[(placeholders + i) * width + j] =
                (mlon.len() + usize::from(placeholders < 2)) as i32;
            mlon.push(lon);
            mlat.push(lat);
        }
    }
    for (name, dim, values) in [
        ("GLONM", "sjx_points", mlon),
        ("GLATM", "sjx_points", mlat),
        ("GLONW", "lbx_points", wlon),
        ("GLATW", "lbx_points", wlat),
    ] {
        file.add_variable::<f64>(name, &[dim])
            .unwrap()
            .put_values(&values, ..)
            .unwrap();
    }
    file.add_variable::<i32>("itab_m%iw", &["sjx_points", "dimb"])
        .unwrap()
        .put_values(&vec![1; n * 3], ..)
        .unwrap();
    file.add_variable::<i32>("itab_w%im", &["lbx_points", "dimc"])
        .unwrap()
        .put_values(&im, ..)
        .unwrap();
    file.add_variable::<i32>("n_ngrwm", &["lbx_points"])
        .unwrap()
        .put_values(&counts, ..)
        .unwrap();
    let mut lineage = vec![0_i64; placeholders];
    lineage.extend((0..rings.len()).map(|i| 901 + i as i64));
    file.add_variable::<i64>("earthmesh_w_lineage", &["lbx_points"])
        .unwrap()
        .put_values(&lineage, ..)
        .unwrap();
    file.close().unwrap();
}

fn quad(w: f64, e: f64, s: f64, n: f64) -> Vec<(f64, f64)> {
    vec![(w, s), (e, s), (e, n), (w, n)]
}

#[test]
fn explicit_export_preserves_ids_counts_footprint_and_fortran_order() {
    for placeholders in [0, 1, 2] {
        let p = root(&format!("order{placeholders}"));
        let input = p.join("native.nc");
        let output = p.join("colm.nc");
        mesh(
            &input,
            &[quad(100., 103.5, 20., 22.), quad(103.5, 107., 20., 22.)],
            placeholders,
        );
        let before = fs::read(&input).unwrap();
        let first_id = if placeholders == 0 { 1 } else { 2 };
        let report = write_colm_mesh_from_gridfile(&input, &output, 1).unwrap();
        assert_eq!(report.cells, 2);
        assert_eq!(report.assigned_pixels, 14);
        assert_eq!(report.boundary_tie_pixels, 2);
        let f = netcdf::open(&output).unwrap();
        let var = f.variable("elmindex").unwrap();
        assert_eq!(
            var.dimensions()
                .iter()
                .map(|d| d.name())
                .collect::<Vec<_>>(),
            ["nlat", "nlon"]
        );
        let ids = var.get_values::<i32, _>(..).unwrap();
        let west = f
            .variable("lon_w")
            .unwrap()
            .get_values::<f64, _>(..)
            .unwrap();
        let north = f
            .variable("lat_n")
            .unwrap()
            .get_values::<f64, _>(..)
            .unwrap();
        for (j, n) in north.iter().enumerate() {
            for (i, w) in west.iter().enumerate() {
                let (x, y) = (w + 0.5, n - 0.5);
                let expected = if (20.0..22.0).contains(&y) && (100.0..107.0).contains(&x) {
                    if x <= 103.5 {
                        first_id
                    } else {
                        first_id + 1
                    }
                } else {
                    0
                };
                assert_eq!(ids[j * west.len() + i], expected, "{x}, {y}");
            }
        }
        assert_eq!(
            f.variable("cell_id")
                .unwrap()
                .get_values::<i32, _>(..)
                .unwrap(),
            [first_id, first_id + 1]
        );
        assert_eq!(
            f.variable("pixel_count")
                .unwrap()
                .get_values::<i64, _>(..)
                .unwrap(),
            [8, 6]
        );
        assert_eq!(
            f.variable("source_lineage")
                .unwrap()
                .get_values::<i64, _>(..)
                .unwrap(),
            [901, 902]
        );
        assert_eq!(before, fs::read(&input).unwrap());
        drop(f);
        fs::remove_dir_all(p).unwrap();
    }
}

#[test]
fn rejected_exports_preserve_old_output_and_leave_no_partials() {
    let p = root("rollback");
    let input = p.join("native.nc");
    let output = p.join("colm.nc");
    for rings in [
        vec![quad(100., 104., 20., 24.), quad(102., 106., 20., 24.)],
        vec![quad(100.01, 100.02, 20.01, 20.02)],
        vec![vec![(100., 20.), (104., 24.), (100., 24.), (104., 20.)]],
    ] {
        mesh(&input, &rings, 1);
        fs::write(&output, b"prior output").unwrap();
        assert!(write_colm_mesh_from_gridfile(&input, &output, 1).is_err());
        assert_eq!(fs::read(&output).unwrap(), b"prior output");
        assert_eq!(fs::read_dir(&p).unwrap().count(), 2);
    }
    mesh(&input, &[quad(100., 104., 20., 24.)], 1);
    let before = fs::read(&input).unwrap();
    assert!(write_colm_mesh_from_gridfile(&input, &input, 1).is_err());
    assert!(write_colm_mesh_from_gridfile(&input, &output, 0).is_err());
    assert!(write_colm_mesh_from_gridfile(&input, &output, usize::MAX).is_err());
    assert_eq!(before, fs::read(&input).unwrap());
    fs::remove_dir_all(p).unwrap();
}

#[test]
fn dateline_and_polar_cells_are_not_lost() {
    for (name, ring) in [
        ("dateline", quad(179., -179., 10., 12.)),
        ("pole", vec![(-120., 80.), (0., 80.), (120., 80.)]),
    ] {
        let p = root(name);
        let input = p.join("native.nc");
        let output = p.join("colm.nc");
        mesh(&input, &[ring], 1);
        let report = write_colm_mesh_from_gridfile(&input, &output, 2).unwrap();
        assert!(report.assigned_pixels > 0);
        assert_eq!(report.cells, 1);
        let f = netcdf::open(&output).unwrap();
        let ids = f
            .variable("elmindex")
            .unwrap()
            .get_values::<i32, _>(..)
            .unwrap();
        assert!(ids.iter().all(|i| *i == 0 || *i == 2));
        if name == "dateline" {
            assert_eq!(report.assigned_pixels, 16);
        }
        drop(f);
        fs::remove_dir_all(p).unwrap();
    }
}

#[test]
fn cli_requires_explicit_valid_resolution_and_exports() {
    let p = root("cli");
    let input = p.join("native.nc");
    let output = p.join("colm.nc");
    mesh(&input, &[quad(100., 104., 20., 22.)], 1);
    let run = |tail: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
            .arg("--colm-mesh-from-gridfile")
            .arg(&input)
            .arg(&output)
            .args(tail)
            .output()
            .unwrap()
    };
    assert!(!run(&[]).status.success());
    assert!(!run(&["--pixels-per-degree", "0"]).status.success());
    assert!(!run(&["--pixels-per-degree", "1", "extra"]).status.success());
    assert!(!output.exists());
    let result = run(&["--pixels-per-degree", "1"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["assigned_pixels"], 8);
    assert!(output.is_file());
    fs::remove_dir_all(p).unwrap();
}

#[test]
fn regional_240_per_degree_export_does_not_require_a_global_raster() {
    let p = root("regional240");
    let input = p.join("native.nc");
    let output = p.join("colm.nc");
    mesh(&input, &[quad(100., 101., 20., 21.)], 1);
    let report = write_colm_mesh_from_gridfile(&input, &output, 240).unwrap();
    assert!(report.nlon < 500 && report.nlat < 500);
    assert!(report.assigned_pixels > 50_000 && report.assigned_pixels < 65_000);
    fs::remove_dir_all(p).unwrap();
}

#[cfg(unix)]
#[test]
fn aliases_and_directory_outputs_are_rejected_without_side_effects() {
    let p = root("aliases");
    let input = p.join("native.nc");
    mesh(&input, &[quad(100., 101., 20., 21.)], 1);
    let before = fs::read(&input).unwrap();
    let hard = p.join("hard.nc");
    fs::hard_link(&input, &hard).unwrap();
    let link = p.join("link.nc");
    std::os::unix::fs::symlink(&input, &link).unwrap();
    for output in [&hard, &link, &p] {
        assert!(write_colm_mesh_from_gridfile(&input, output, 1).is_err());
    }
    assert_eq!(before, fs::read(&input).unwrap());
    assert_eq!(before, fs::read(&hard).unwrap());
    assert_eq!(fs::read_dir(&p).unwrap().count(), 3);
    fs::remove_dir_all(p).unwrap();
}

#[test]
fn subpixel_interior_overlap_is_rejected_before_publication() {
    let p = root("sliver");
    let input = p.join("native.nc");
    let output = p.join("colm.nc");
    mesh(
        &input,
        &[quad(100., 102.1, 20., 22.), quad(102., 104., 20., 22.)],
        1,
    );
    fs::write(&output, b"old").unwrap();
    let err = write_colm_mesh_from_gridfile(&input, &output, 1).unwrap_err();
    assert!(err.to_string().contains("overlap"));
    assert_eq!(fs::read(&output).unwrap(), b"old");
    fs::remove_dir_all(p).unwrap();
}

#[test]
fn great_circle_edge_bulge_is_included_above_vertex_latitudes() {
    let p = root("bulge");
    let input = p.join("native.nc");
    let output = p.join("colm.nc");
    mesh(&input, &[quad(-50., 50., 60., 70.)], 1);
    write_colm_mesh_from_gridfile(&input, &output, 1).unwrap();
    let f = netcdf::open(&output).unwrap();
    let north = f
        .variable("lat_n")
        .unwrap()
        .get_values::<f64, _>(..)
        .unwrap();
    let west = f
        .variable("lon_w")
        .unwrap()
        .get_values::<f64, _>(..)
        .unwrap();
    let j = north.iter().position(|v| *v == 76.).unwrap();
    let i = west.iter().position(|v| *v == 0.).unwrap();
    assert_eq!(
        f.variable("elmindex")
            .unwrap()
            .get_value::<i32, _>((j, i))
            .unwrap(),
        2
    );
    drop(f);
    fs::remove_dir_all(p).unwrap();
}
