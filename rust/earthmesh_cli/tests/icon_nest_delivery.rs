//! ICON nest sets: written from a planned global grid plus nests, read back
//! and checked the way ICON checks the domains it loads (guide 11.86).

use earthmesh_cli::{validate_icon_nest_set, write_icon_nest_set};
use earthmesh_mesh::{plan_icon_nests, IconNestOptions};
use std::{collections::BTreeMap, fs, path::PathBuf};

type P = [f64; 3];

fn root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("icon_nest_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn unit(a: P) -> P {
    let n = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    [a[0] / n, a[1] / n, a[2] / n]
}

/// An icosahedron bisected `levels` times, triangles counter-clockwise.
fn icosphere(levels: u32) -> (Vec<P>, Vec<[usize; 3]>) {
    let t = (1.0 + 5f64.sqrt()) / 2.0;
    let mut points = [
        [-1.0, t, 0.0],
        [1.0, t, 0.0],
        [-1.0, -t, 0.0],
        [1.0, -t, 0.0],
        [0.0, -1.0, t],
        [0.0, 1.0, t],
        [0.0, -1.0, -t],
        [0.0, 1.0, -t],
        [t, 0.0, -1.0],
        [t, 0.0, 1.0],
        [-t, 0.0, -1.0],
        [-t, 0.0, 1.0],
    ]
    .map(unit)
    .to_vec();
    let mut triangles = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    for _ in 0..levels {
        let mut mids = BTreeMap::new();
        let mut mid = |a: usize, b: usize, points: &mut Vec<P>| {
            *mids.entry((a.min(b), a.max(b))).or_insert_with(|| {
                let (p, q) = (points[a], points[b]);
                points.push(unit([p[0] + q[0], p[1] + q[1], p[2] + q[2]]));
                points.len() - 1
            })
        };
        let mut next = Vec::new();
        for [a, b, c] in triangles {
            let ab = mid(a, b, &mut points);
            let bc = mid(b, c, &mut points);
            let ca = mid(c, a, &mut points);
            next.extend([[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]);
        }
        triangles = next;
    }
    (points, triangles)
}

fn lonlat(lon: f64, lat: f64) -> P {
    let (lon, lat) = (lon.to_radians(), lat.to_radians());
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}

fn within(p: P, centre: P, degrees: f64) -> bool {
    (p[0] * centre[0] + p[1] * centre[1] + p[2] * centre[2])
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees()
        <= degrees
}

/// Two levels over one spot and a single level over a distant one.
fn demand(p: P) -> u32 {
    let asia = lonlat(115.0, 23.0);
    if within(p, asia, 2.0) {
        2
    } else if within(p, asia, 5.0) || within(p, lonlat(-60.0, -20.0), 2.0) {
        1
    } else {
        0
    }
}

fn write_set(name: &str) -> (PathBuf, Vec<earthmesh_cli::IconNestFileReport>) {
    let (points, triangles) = icosphere(4);
    let domains = plan_icon_nests(points, triangles, demand, &IconNestOptions::default()).unwrap();
    let dir = root(name);
    let reports = write_icon_nest_set(&domains, 16, &dir, "earthmesh").unwrap();
    (dir, reports)
}

#[test]
fn a_nest_set_is_written_linked_and_passes_icon_load_checks() {
    let (dir, reports) = write_set("linked");
    // Global, two siblings at level 1, one nest at level 2 inside the first.
    let shape = reports
        .iter()
        .map(|r| (r.domain, r.parent, r.grid_level))
        .collect::<Vec<_>>();
    assert_eq!(shape, vec![(1, 0, 0), (2, 1, 1), (3, 1, 1), (4, 2, 2)]);
    assert!(reports[0].output.ends_with("earthmesh_DOM01.nc"));
    assert_eq!(reports[0].cells, 20 * 4usize.pow(4));
    for r in &reports[1..] {
        assert_eq!(r.cells % 4, 0);
    }
    // The writer validated already; do it again from the files alone.
    let files = reports
        .iter()
        .map(|r| (r.output.clone(), r.parent))
        .collect::<Vec<_>>();
    validate_icon_nest_set(&files).unwrap();
    // UUIDs link each nest to its parent.
    let uuid = |path: &PathBuf, name: &str| {
        let file = netcdf::open(path).unwrap();
        match file.attribute(name).unwrap().value().unwrap() {
            netcdf::AttributeValue::Str(s) => s,
            other => panic!("{other:?}"),
        }
    };
    for r in &reports[1..] {
        assert_eq!(
            uuid(&r.output, "uuidOfParHGrid"),
            uuid(&reports[r.parent - 1].output, "uuidOfHGrid")
        );
    }
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn the_validator_catches_what_icon_would_stop_on() {
    let (dir, reports) = write_set("broken");
    let files = reports
        .iter()
        .map(|r| (r.output.clone(), r.parent))
        .collect::<Vec<_>>();
    let corrupt = |variable: &str, change: &dyn Fn(&mut Vec<i32>)| {
        let path = &reports[1].output;
        let backup = path.with_extension("orig");
        fs::copy(path, &backup).unwrap();
        {
            let mut file = netcdf::append(path).unwrap();
            let mut var = file.variable_mut(variable).unwrap();
            let mut values = var.get_values::<i32, _>(..).unwrap();
            change(&mut values);
            var.put_values(&values, ..).unwrap();
        }
        let error = validate_icon_nest_set(&files).unwrap_err().to_string();
        fs::rename(&backup, path).unwrap();
        error
    };
    // A parent cell with a child moved elsewhere: no longer 0 or 4.
    let error = corrupt("parent_cell_index", &|v| {
        v[0] = if v[0] == 1 { 2 } else { 1 }
    });
    assert!(
        error.contains("children")
            || error.contains("inner edges")
            || error.contains("parent edge"),
        "{error}"
    );
    // Boundary rows out of order.
    let error = corrupt("refin_c_ctrl", &|v| {
        let last = v.len() - 1;
        v.swap(0, last)
    });
    assert!(error.contains("sorted"), "{error}");
    // Too shallow a boundary zone.
    let error = corrupt("refin_c_ctrl", &|v| {
        v.iter_mut().for_each(|x| *x = (*x).min(11))
    });
    assert!(error.contains("12"), "{error}");
    // Restored, the set passes again.
    validate_icon_nest_set(&files).unwrap();
    let _ = fs::remove_dir_all(dir);
}
