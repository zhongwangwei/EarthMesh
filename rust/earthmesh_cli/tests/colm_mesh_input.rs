mod support;

use earthmesh_cli::{
    colm_mesh_input::{write_colm_mesh_from_gridfile, write_colm_mesh_from_gridfile_with_kind},
    unstructured_mesh_support::GridfileCellKind,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

// Keep complete fixture lifetimes and child CLI launches mutually exclusive.
// Per-call NetCDF locks do not prevent this overlap from causing HDF5 file-lock errors.
static NETCDF_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

fn tri_mesh(path: &Path, placeholders: usize, reversed_second: bool) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).unwrap();
    let m_rows = placeholders + 2;
    let w_rows = placeholders + 4;
    for (name, length) in [
        ("sjx_points", m_rows),
        ("lbx_points", w_rows),
        ("dimb", 3),
        ("dimc", 2),
    ] {
        file.add_dimension(name, length).unwrap();
    }
    let vertices = [(100., 20.), (104., 20.), (104., 22.), (100., 22.)];
    let mut mlon = vec![0.; placeholders];
    let mut mlat = vec![0.; placeholders];
    mlon.extend([102.7, 101.3]);
    mlat.extend([20.7, 21.3]);
    let mut wlon = vec![0.; placeholders];
    let mut wlat = vec![0.; placeholders];
    for (lon, lat) in vertices {
        wlon.push(lon);
        wlat.push(lat);
    }
    let id = |physical: i32| -> i32 { physical + if placeholders == 2 { 2 } else { 1 } };
    let mut m_to_w = vec![1; m_rows * 3];
    let tri1 = [id(0), id(1), id(2)];
    let tri2 = if reversed_second {
        [id(3), id(2), id(0)]
    } else {
        [id(0), id(2), id(3)]
    };
    m_to_w[placeholders * 3..placeholders * 3 + 3].copy_from_slice(&tri1);
    m_to_w[(placeholders + 1) * 3..(placeholders + 1) * 3 + 3].copy_from_slice(&tri2);
    let mut w_to_m = vec![1; w_rows * 2];
    let mut counts = vec![1; w_rows];
    let m1 = id(0);
    let m2 = id(1);
    for (physical_w, owners) in [
        (0usize, vec![m1, m2]),
        (1, vec![m1]),
        (2, vec![m1, m2]),
        (3, vec![m2]),
    ] {
        let row = placeholders + physical_w;
        counts[row] = owners.len() as i32;
        for (k, owner) in owners.into_iter().enumerate() {
            w_to_m[row * 2 + k] = owner;
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
        .put_values(&m_to_w, ..)
        .unwrap();
    file.add_variable::<i32>("itab_w%im", &["lbx_points", "dimc"])
        .unwrap()
        .put_values(&w_to_m, ..)
        .unwrap();
    file.add_variable::<i32>("n_ngrwm", &["lbx_points"])
        .unwrap()
        .put_values(&counts, ..)
        .unwrap();
    let mut m_lineage = vec![0_i64; placeholders];
    m_lineage.extend([701, 702]);
    file.add_variable::<i64>("earthmesh_m_lineage", &["sjx_points"])
        .unwrap()
        .put_values(&m_lineage, ..)
        .unwrap();
    let mut w_lineage = vec![0_i64; placeholders];
    w_lineage.extend([901, 902, 903, 904]);
    file.add_variable::<i64>("earthmesh_w_lineage", &["lbx_points"])
        .unwrap()
        .put_values(&w_lineage, ..)
        .unwrap();
    file.close().unwrap();
}

#[test]
fn explicit_export_preserves_ids_counts_footprint_and_fortran_order() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
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
        let center_lon_var = f.variable("longitude").expect("longitude centers");
        assert_eq!(
            center_lon_var
                .dimensions()
                .iter()
                .map(|d| d.name())
                .collect::<Vec<_>>(),
            ["nlon"]
        );
        let center_lat_var = f.variable("latitude").expect("latitude centers");
        assert_eq!(
            center_lat_var
                .dimensions()
                .iter()
                .map(|d| d.name())
                .collect::<Vec<_>>(),
            ["nlat"]
        );
        assert_eq!(
            center_lon_var.vartype(),
            netcdf::types::NcVariableType::Float(netcdf::types::FloatType::F64)
        );
        assert_eq!(
            center_lat_var.vartype(),
            netcdf::types::NcVariableType::Float(netcdf::types::FloatType::F64)
        );
        let center_lon = center_lon_var.get_values::<f64, _>(..).unwrap();
        let center_lat = center_lat_var.get_values::<f64, _>(..).unwrap();
        assert_eq!(center_lon.len(), west.len());
        assert_eq!(center_lat.len(), north.len());
        for i in 0..west.len() {
            assert_eq!(center_lon[i], west[i] + 0.5);
        }
        for j in 0..north.len() {
            assert_eq!(center_lat[j], north[j] - 0.5);
        }
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
fn triangle_export_uses_m_cells_ids_lineage_and_exact_ownership() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    for placeholders in [0, 2] {
        for reversed_second in [false, true] {
            let p = root(&format!("tri{placeholders}_{reversed_second}"));
            let input = p.join("native_tri.nc");
            let output = p.join("colm_tri.nc");
            tri_mesh(&input, placeholders, reversed_second);
            let before = fs::read(&input).unwrap();
            let first_id = if placeholders == 2 { 2 } else { 1 };
            let report =
                write_colm_mesh_from_gridfile_with_kind(&input, &output, 1, GridfileCellKind::Tri)
                    .unwrap();
            assert_eq!(report.cells, 2);
            assert_eq!(report.assigned_pixels, 8);
            assert_eq!(report.boundary_tie_pixels, 0);
            let f = netcdf::open(&output).unwrap();
            let semantics: String = f
                .attribute("earthmesh_semantics")
                .unwrap()
                .value()
                .unwrap()
                .try_into()
                .unwrap();
            assert_eq!(
                semantics,
                "pixel_center_rasterized_native_m_cell_ids; outside=0; boundary_tie=smallest_id"
            );
            assert_eq!(
                f.variable("cell_id")
                    .unwrap()
                    .get_values::<i32, _>(..)
                    .unwrap(),
                [first_id, first_id + 1]
            );
            assert_eq!(
                f.variable("source_lineage")
                    .unwrap()
                    .get_values::<i64, _>(..)
                    .unwrap(),
                [701, 702]
            );
            assert_eq!(
                f.variable("pixel_count")
                    .unwrap()
                    .get_values::<i64, _>(..)
                    .unwrap(),
                [4, 4]
            );
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
            let ids = f
                .variable("elmindex")
                .unwrap()
                .get_values::<i32, _>(..)
                .unwrap();
            for (j, n) in north.iter().enumerate() {
                for (i, w) in west.iter().enumerate() {
                    let (x, y) = (w + 0.5, n - 0.5);
                    let expected = if (100.0..104.0).contains(&x) && (20.0..22.0).contains(&y) {
                        let split_y = 20.0 + (x - 100.0) * (2.0 / 4.0);
                        if y <= split_y {
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
            assert_eq!(before, fs::read(&input).unwrap());
            drop(f);
            fs::remove_dir_all(p).unwrap();
        }
    }
}

#[test]
fn default_hex_export_does_not_accept_true_triangle_grid() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let p = root("tri_default_hex");
    let input = p.join("native_tri.nc");
    let output = p.join("colm.nc");
    tri_mesh(&input, 2, false);
    let err = write_colm_mesh_from_gridfile(&input, &output, 1).unwrap_err();
    assert!(err.to_string().contains("hex") || err.to_string().contains("W"));
    assert!(!output.exists());
    fs::remove_dir_all(p).unwrap();
}

#[test]
fn triangle_export_rejects_invalid_m_triangle_connectivity() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let p = root("bad_tri");
    let input = p.join("native_tri.nc");
    let output = p.join("colm.nc");
    tri_mesh(&input, 2, false);
    {
        let mut f = netcdf::append(&input).unwrap();
        f.variable_mut("itab_m%iw")
            .unwrap()
            .put_values(&[2_i32, 2, 4], (2, ..))
            .unwrap();
        f.close().unwrap();
    }
    let err = write_colm_mesh_from_gridfile_with_kind(&input, &output, 1, GridfileCellKind::Tri)
        .unwrap_err();
    assert!(err.to_string().contains("duplicate W vertex"));
    assert!(!output.exists());
    fs::remove_dir_all(p).unwrap();
}

#[test]
fn rejected_exports_preserve_old_output_and_leave_no_partials() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
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
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
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
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let p = root("cli");
    let input = p.join("native.nc");
    let output = p.join("colm.nc");
    mesh(&input, &[quad(100., 104., 20., 22.)], 1);
    let run = |tail: &[&str]| {
        support::output(
            Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
                .arg("--colm-mesh-from-gridfile")
                .arg(&input)
                .arg(&output)
                .args(tail),
        )
        .unwrap()
    };
    assert!(!run(&[]).status.success());
    assert!(!run(&["--pixels-per-degree", "0"]).status.success());
    assert!(!run(&["--pixels-per-degree", "1", "extra"]).status.success());
    assert!(!run(&["--pixels-per-degree", "1", "--kind"])
        .status
        .success());
    assert!(!run(&["--pixels-per-degree", "1", "--kind", "quad"])
        .status
        .success());
    assert!(
        !run(&["--pixels-per-degree", "1", "--kind", "hex", "--kind", "hex"])
            .status
            .success()
    );
    assert!(
        !run(&["--pixels-per-degree", "1", "--pixels-per-degree", "2",])
            .status
            .success()
    );
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
fn cli_triangle_kind_exports_m_cell_ids() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let p = root("cli_tri");
    let input = p.join("native_tri.nc");
    let output = p.join("colm_tri.nc");
    tri_mesh(&input, 2, false);
    let result = support::output(
        Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
            .arg("--colm-mesh-from-gridfile")
            .arg(&input)
            .arg(&output)
            .args(["--pixels-per-degree", "1", "--kind", "tri"]),
    )
    .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["cells"], 2);
    assert_eq!(value["assigned_pixels"], 8);
    let f = netcdf::open(&output).unwrap();
    assert_eq!(
        f.variable("cell_id")
            .unwrap()
            .get_values::<i32, _>(..)
            .unwrap(),
        [2, 3]
    );
    assert_eq!(
        f.variable("source_lineage")
            .unwrap()
            .get_values::<i64, _>(..)
            .unwrap(),
        [701, 702]
    );
    drop(f);
    fs::remove_dir_all(p).unwrap();
}

#[test]
fn regional_240_per_degree_export_does_not_require_a_global_raster() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
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
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
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
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
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
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
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
