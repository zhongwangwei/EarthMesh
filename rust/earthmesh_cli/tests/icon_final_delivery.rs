use earthmesh_cli::{
    coordinate_types::{GridRegion, LonLatPoint},
    unstructured_mesh_io::{
        write_unstructured_mesh_netcdf, write_unstructured_mesh_netcdf_with_method_c_metadata,
    },
    unstructured_mesh_support::{MethodCGridfileMetadataSlices, UnstructuredMesh},
    write_icon_from_final_gridfile, write_icon_from_final_gridfile_with_parent,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

fn root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("icon_final_{name}_{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    root
}

fn fixture(placeholders: usize) -> UnstructuredMesh {
    let state = earthmesh_mesh::gridinit_voronoi_state_canonical(2, 0, 1.0, 0.25, 100).unwrap();
    let mut mesh =
        earthmesh_cli::mesh_conversion_gridfile_state::gridfile_mesh_from_one_based_state(
            &state.grid,
            &state.tabs,
        )
        .unwrap();
    if placeholders == 0 {
        mesh.m_points.remove(0);
        mesh.w_points.remove(0);
        mesh.m_to_w.remove(0);
        mesh.w_to_m.remove(0);
        mesh.n_w_to_m.remove(0);
        for id in mesh
            .m_to_w
            .iter_mut()
            .flatten()
            .chain(mesh.w_to_m.iter_mut().flatten())
        {
            *id -= 1;
        }
    } else if placeholders == 2 {
        let zero = LonLatPoint { lon: 0.0, lat: 0.0 };
        mesh.m_points.insert(0, zero);
        mesh.w_points.insert(0, zero);
        mesh.m_to_w.insert(0, [0; 3]);
        mesh.w_to_m.insert(0, vec![0]);
        mesh.n_w_to_m.insert(0, 0);
    }
    mesh
}

#[test]
fn icon_final_preserves_legacy_variables_for_each_native_layout_without_demand_context() {
    let root = root("layouts");
    for placeholders in 0..=2 {
        let mesh = fixture(placeholders);
        let source = root.join(format!("source_{placeholders}.nc4"));
        let expected = root.join("legacy.nc4");
        let output = root.join("final.nc4");
        write_unstructured_mesh_netcdf(&source, &mesh).unwrap();
        let before = fs::read(&source).unwrap();
        assert!(
            earthmesh_cli::mpas_gridfile_context::read_mpas_gridfile_context(&source)
                .unwrap()
                .is_none()
        );
        earthmesh_cli::mpas_gridfile_writers::write_standard_icon_from_gridfile(
            &source, &expected, 2,
        )
        .unwrap();
        fs::write(&output, b"previous ICON").unwrap();
        let report = write_icon_from_final_gridfile(&source, &output, 2).unwrap();
        assert_eq!(report.output, output);
        assert_eq!(report.cells, mesh.m_points.len() - placeholders);
        assert_eq!(report.vertices, mesh.w_points.len() - placeholders);
        assert!(report.global_grid);
        let expected_file = netcdf::open(&expected).unwrap();
        let actual_file = netcdf::open(&output).unwrap();
        assert_eq!(
            expected_file.variables().count(),
            actual_file.variables().count()
        );
        for expected in expected_file.variables() {
            let actual = actual_file.variable(&expected.name()).unwrap();
            assert_eq!(actual.vartype(), expected.vartype());
            assert_eq!(
                actual
                    .dimensions()
                    .iter()
                    .map(|d| (d.name(), d.len()))
                    .collect::<Vec<_>>(),
                expected
                    .dimensions()
                    .iter()
                    .map(|d| (d.name(), d.len()))
                    .collect::<Vec<_>>()
            );
            // All existing ICON variables are f64/i32; IDs in this fixture are
            // exactly representable as f64. Bit comparison also handles NaN bounds.
            assert_eq!(
                actual
                    .get_values::<f64, _>(..)
                    .unwrap()
                    .into_iter()
                    .map(f64::to_bits)
                    .collect::<Vec<_>>(),
                expected
                    .get_values::<f64, _>(..)
                    .unwrap()
                    .into_iter()
                    .map(f64::to_bits)
                    .collect::<Vec<_>>(),
                "{}",
                expected.name()
            );
        }
        assert!(actual_file.variable("meshDensity").is_none());
        assert!(actual_file.variable("nominalMinDc").is_none());
        actual_file.close().unwrap();
        expected_file.close().unwrap();
        assert_eq!(fs::read(&source).unwrap(), before);
    }
    assert!(!fs::read_dir(&root).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains(".tmp-")));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn icon_final_rejects_incomplete_regional_dual_without_replacing_previous_file() {
    let root = root("regional");
    let source = root.join("source.nc4");
    let region = root.join("region.nc4");
    let output = root.join("ICON_region.nc4");
    let mesh = fixture(1);
    write_unstructured_mesh_netcdf(&source, &mesh).unwrap();
    let kept = earthmesh_cli::regional_gridfile_writers::write_regional_gridfile(
        &source,
        &region,
        &earthmesh_cli::coordinate_types::GridRegion::Bbox {
            west: -180.0,
            east: 0.0,
            south: -90.0,
            north: 90.0,
        },
        "tri",
    )
    .unwrap();
    assert!(kept > 0 && kept < mesh.m_points.len() - 1);
    let before = fs::read(&region).unwrap();
    fs::write(&output, b"previous ICON").unwrap();
    let error = write_icon_from_final_gridfile(&region, &output, 2).unwrap_err();
    assert!(
        error.to_string().contains("MPAS") || error.to_string().contains("ICON"),
        "{error}"
    );
    assert_eq!(fs::read(&output).unwrap(), b"previous ICON");
    assert_eq!(fs::read(&region).unwrap(), before);
    assert!(!fs::read_dir(&root).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains(".tmp-")));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn icon_final_rejects_invalid_inputs_and_aliases_without_overwriting_files() {
    let root = root("safe");
    let source = root.join("source.nc4");
    let output = root.join("final.nc4");
    write_unstructured_mesh_netcdf(&source, &fixture(1)).unwrap();
    let before = fs::read(&source).unwrap();
    fs::write(&output, b"previous ICON").unwrap();
    let error = write_icon_from_final_gridfile(&source, &output, 0).unwrap_err();
    assert!(
        error.to_string().contains("nxp and step must be positive"),
        "{error}"
    );
    assert_eq!(fs::read(&output).unwrap(), b"previous ICON");
    assert!(write_icon_from_final_gridfile(&source, &source, 2).is_err());
    assert_eq!(fs::read(&source).unwrap(), before);
    #[cfg(unix)]
    {
        let symlink = root.join("symlink.nc4");
        let hardlink = root.join("hardlink.nc4");
        std::os::unix::fs::symlink(&source, &symlink).unwrap();
        fs::hard_link(&source, &hardlink).unwrap();
        for alias in [&symlink, &hardlink] {
            assert!(write_icon_from_final_gridfile(&source, alias, 2).is_err());
            assert_eq!(fs::read(&source).unwrap(), before);
        }
    }
    let broken = root.join("broken.nc4");
    fs::copy(&source, &broken).unwrap();
    let mut file = netcdf::append(&broken).unwrap();
    file.variable_mut("itab_m%iw")
        .unwrap()
        .put_value(-9_i32, [1, 0])
        .unwrap();
    file.close().unwrap();
    let error = write_icon_from_final_gridfile(&broken, &output, 2).unwrap_err();
    assert!(error.to_string().contains("invalid W vertex id"), "{error}");
    assert_eq!(fs::read(&output).unwrap(), b"previous ICON");
    assert!(!fs::read_dir(&root).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains(".tmp-")));
    fs::remove_dir_all(root).unwrap();
}

fn fixture_lineages(mesh: &UnstructuredMesh) -> (Vec<i64>, Vec<i64>) {
    let m_first = mesh
        .m_to_w
        .iter()
        .position(|row| row.iter().all(|&id| id > 0) && BTreeSet::from_iter(row.iter()).len() == 3)
        .unwrap_or(mesh.m_to_w.len());
    let w_first = mesh
        .n_w_to_m
        .iter()
        .position(|&count| count >= 3)
        .unwrap_or(mesh.n_w_to_m.len());
    let m_lineage = (0..mesh.m_points.len())
        .map(|row| {
            if row < m_first {
                0
            } else {
                10_000 + row as i64
            }
        })
        .collect::<Vec<_>>();
    let w_lineage = (0..mesh.w_points.len())
        .map(|row| {
            if row < w_first {
                0
            } else {
                20_000 + row as i64
            }
        })
        .collect::<Vec<_>>();
    (m_lineage, w_lineage)
}

fn write_gridfile_with_context(path: &Path, mesh: &UnstructuredMesh) {
    let (m_lineage, w_lineage) = fixture_lineages(mesh);
    write_unstructured_mesh_netcdf_with_method_c_metadata(
        path,
        mesh,
        MethodCGridfileMetadataSlices {
            m_lineage: Some(&m_lineage),
            w_lineage: Some(&w_lineage),
            ..Default::default()
        },
    )
    .unwrap();
}

fn parent_and_region(root: &Path, placeholders: usize) -> (PathBuf, PathBuf, UnstructuredMesh) {
    let mesh = fixture(placeholders);
    let parent = root.join(format!("parent_{placeholders}.nc4"));
    let region = root.join(format!("region_{placeholders}.nc4"));
    write_gridfile_with_context(&parent, &mesh);
    let kept = earthmesh_cli::regional_gridfile_writers::write_regional_gridfile(
        &parent,
        &region,
        &GridRegion::Bbox {
            west: -180.0,
            east: 0.0,
            south: -90.0,
            north: 90.0,
        },
        "tri",
    )
    .unwrap();
    assert!(kept > 0 && kept < mesh.m_points.len().saturating_sub(placeholders));
    (parent, region, mesh)
}

fn coord_key(lon_rad: f64, lat_rad: f64) -> (i64, i64) {
    let lon = lon_rad.rem_euclid(std::f64::consts::TAU);
    (
        (lon * 1.0e12).round() as i64,
        (lat_rad * 1.0e12).round() as i64,
    )
}

fn icon_coord_values(path: &Path, lon: &str, lat: &str) -> BTreeSet<(i64, i64)> {
    let file = netcdf::open(path).unwrap();
    file.variable(lon)
        .unwrap()
        .get_values::<f64, _>(..)
        .unwrap()
        .into_iter()
        .zip(
            file.variable(lat)
                .unwrap()
                .get_values::<f64, _>(..)
                .unwrap(),
        )
        .map(|(x, y)| coord_key(x, y))
        .collect()
}

fn selected_cell_coords(path: &Path) -> BTreeSet<(i64, i64)> {
    let mesh = earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(path).unwrap();
    mesh.m_to_w
        .iter()
        .zip(mesh.m_points.iter())
        .filter(|(row, _)| {
            row.iter().all(|&id| id > 0) && BTreeSet::from_iter(row.iter()).len() == 3
        })
        .map(|(_, point)| coord_key(point.lon.to_radians(), point.lat.to_radians()))
        .collect()
}

fn selected_vertex_coords(path: &Path) -> BTreeSet<(i64, i64)> {
    let points = earthmesh_cli::grid_quality_pipeline::read_gridfile_mesh_points(path).unwrap();
    let input = earthmesh_cli::grid_quality_pipeline::quality_input_from_gridfile(&points).unwrap();
    input
        .cells
        .iter()
        .flat_map(|cell| cell.vertices.iter().copied())
        .map(|row| {
            coord_key(
                points.w_lon[row].to_radians(),
                points.w_lat[row].to_radians(),
            )
        })
        .collect()
}

fn f64_map(path: &Path, lon: &str, lat: &str, value: &str) -> BTreeMap<(i64, i64), f64> {
    let file = netcdf::open(path).unwrap();
    let lon = file
        .variable(lon)
        .unwrap()
        .get_values::<f64, _>(..)
        .unwrap();
    let lat = file
        .variable(lat)
        .unwrap()
        .get_values::<f64, _>(..)
        .unwrap();
    let values = file
        .variable(value)
        .unwrap()
        .get_values::<f64, _>(..)
        .unwrap();
    lon.into_iter()
        .zip(lat)
        .zip(values)
        .map(|((x, y), value)| (coord_key(x, y), value))
        .collect()
}

fn assert_same_f64_bits(a: f64, b: f64) {
    assert_eq!(a.to_bits(), b.to_bits(), "{a} != {b}");
}

fn assert_regional_icon_matches_parent_and_selection(
    parent_icon: &Path,
    region: &Path,
    output: &Path,
) {
    assert_eq!(
        icon_coord_values(output, "clon", "clat"),
        selected_cell_coords(region)
    );
    assert_eq!(
        icon_coord_values(output, "vlon", "vlat"),
        selected_vertex_coords(region)
    );

    let parent_cells = f64_map(parent_icon, "clon", "clat", "cell_area");
    for (coord, area) in f64_map(output, "clon", "clat", "cell_area") {
        assert_same_f64_bits(area, parent_cells[&coord]);
    }
    let parent_vertices = f64_map(parent_icon, "vlon", "vlat", "dual_area");
    for (coord, area) in f64_map(output, "vlon", "vlat", "dual_area") {
        assert_same_f64_bits(area, parent_vertices[&coord]);
    }

    let file = netcdf::open(output).unwrap();
    assert_eq!(
        file.attribute("earthmesh_dual_area_scope")
            .unwrap()
            .value()
            .unwrap(),
        netcdf::AttributeValue::Str("full_parent_dual".into())
    );
    assert!(file.attribute("earthmesh_geometry_parent").is_some());
    let edges = file.dimension("edge").unwrap().len();
    let adjacent = file
        .variable("adjacent_cell_of_edge")
        .unwrap()
        .get_values::<i32, _>(..)
        .unwrap();
    let distances = file
        .variable("edge_cell_distance")
        .unwrap()
        .get_values::<f64, _>(..)
        .unwrap();
    let mut boundary = 0;
    for edge in 0..edges {
        if adjacent[edges + edge] == -1 {
            boundary += 1;
            assert_eq!(distances[edges + edge], 0.0);
        }
    }
    assert!(
        boundary > 0,
        "regional ICON fixture must have open boundary edges"
    );
}

#[test]
fn icon_final_with_explicit_parent_delivers_regional_triangles_for_each_native_layout() {
    let root = root("regional_parent_layouts");
    for placeholders in 0..=2 {
        let (parent, region, _) = parent_and_region(&root, placeholders);
        let parent_icon = root.join(format!("parent_{placeholders}_ICON.nc4"));
        let output = root.join(format!("region_{placeholders}_ICON.nc4"));
        let parent_before = fs::read(&parent).unwrap();
        let region_before = fs::read(&region).unwrap();
        write_icon_from_final_gridfile(&parent, &parent_icon, 2).unwrap();

        let report =
            write_icon_from_final_gridfile_with_parent(&region, &parent, &output, 2).unwrap();

        assert_eq!(report.output, output);
        assert!(!report.global_grid);
        assert_eq!(fs::read(&parent).unwrap(), parent_before);
        assert_eq!(fs::read(&region).unwrap(), region_before);
        assert_regional_icon_matches_parent_and_selection(&parent_icon, &region, &output);
    }
    assert!(!fs::read_dir(&root).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains(".tmp-")));
    fs::remove_dir_all(root).unwrap();
}

fn rewrite_without_lineage(path: &Path, omit: &str) {
    let tmp = path.with_extension("rewrite.nc4");
    let mesh = earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(path).unwrap();
    let lineages = earthmesh_cli::grid_quality_pipeline::read_gridfile_cell_lineages(path).unwrap();
    write_unstructured_mesh_netcdf_with_method_c_metadata(
        &tmp,
        &mesh,
        MethodCGridfileMetadataSlices {
            m_lineage: (omit != "earthmesh_m_lineage").then_some(lineages.m.as_slice()),
            w_lineage: (omit != "earthmesh_w_lineage").then_some(lineages.w.as_slice()),
            ..Default::default()
        },
    )
    .unwrap();
    fs::rename(tmp, path).unwrap();
}

fn mutate_i64(path: &Path, var: &str, mutate: impl FnOnce(&mut Vec<i64>)) {
    let mut file = netcdf::append(path).unwrap();
    let mut values = file
        .variable(var)
        .unwrap()
        .get_values::<i64, _>(..)
        .unwrap();
    mutate(&mut values);
    file.variable_mut(var)
        .unwrap()
        .put_values(&values, ..)
        .unwrap();
}

fn first_positive_lineage(path: &Path) -> (usize, i64) {
    earthmesh_cli::grid_quality_pipeline::read_gridfile_cell_lineages(path)
        .unwrap()
        .m
        .into_iter()
        .enumerate()
        .find(|(_, id)| *id > 0)
        .unwrap()
}

fn parent_row_for_m_lineage(parent: &Path, lineage: i64) -> usize {
    earthmesh_cli::grid_quality_pipeline::read_gridfile_cell_lineages(parent)
        .unwrap()
        .m
        .into_iter()
        .position(|id| id == lineage)
        .unwrap()
}

fn mutate_parent_coordinate(parent: &Path) {
    let mut file = netcdf::append(parent).unwrap();
    let mut lon = file
        .variable("GLONW")
        .unwrap()
        .get_values::<f64, _>(..)
        .unwrap();
    let row = earthmesh_cli::grid_quality_pipeline::read_gridfile_cell_lineages(parent)
        .unwrap()
        .w
        .into_iter()
        .position(|id| id > 0)
        .unwrap();
    lon[row] += 0.125;
    file.variable_mut("GLONW")
        .unwrap()
        .put_values(&lon, ..)
        .unwrap();
}

fn mutate_parent_triangle_corner(parent: &Path, region: &Path) {
    let (_, lineage) = first_positive_lineage(region);
    let parent_row = parent_row_for_m_lineage(parent, lineage);
    let lineage = earthmesh_cli::grid_quality_pipeline::read_gridfile_cell_lineages(parent)
        .unwrap()
        .w;
    let offset = i32::from(!lineage.starts_with(&[0, 0]));
    let mut file = netcdf::append(parent).unwrap();
    let mut corners = file
        .variable("itab_m%iw")
        .unwrap()
        .get_values::<i32, _>(..)
        .unwrap();
    let idx = parent_row * 3;
    corners[idx] = lineage
        .iter()
        .enumerate()
        .filter(|(_, lineage)| **lineage > 0)
        .map(|(row, _)| row as i32 + offset)
        .find(|id| !corners[idx..idx + 3].contains(id))
        .expect("distinct physical replacement corner");
    file.variable_mut("itab_m%iw")
        .unwrap()
        .put_values(&corners, (.., ..))
        .unwrap();
}

#[test]
fn icon_final_with_explicit_parent_rejects_bad_mapping_without_replacing_output() {
    for case in [
        "missing_final_lineage",
        "missing_parent_lineage",
        "wrong_final_lineage",
        "changed_parent_coordinate",
        "changed_parent_corner",
        "changed_final_corner",
        "regional_parent",
    ] {
        let root = root(case);
        let (parent, region, _) = parent_and_region(&root, 1);
        match case {
            "missing_final_lineage" => rewrite_without_lineage(&region, "earthmesh_w_lineage"),
            "missing_parent_lineage" => rewrite_without_lineage(&parent, "earthmesh_m_lineage"),
            "wrong_final_lineage" => mutate_i64(&region, "earthmesh_w_lineage", |values| {
                let (row, _) = values.iter().enumerate().find(|(_, id)| **id > 0).unwrap();
                values[row] = 999_999;
            }),
            "changed_parent_coordinate" => mutate_parent_coordinate(&parent),
            "changed_parent_corner" => mutate_parent_triangle_corner(&parent, &region),
            "changed_final_corner" => mutate_parent_triangle_corner(&region, &region),
            "regional_parent" => {}
            _ => unreachable!(),
        }
        let parent_arg = if case == "regional_parent" {
            &region
        } else {
            &parent
        };
        let output = root.join("ICON.nc4");
        fs::write(&output, b"previous ICON").unwrap();
        let parent_before = fs::read(parent_arg).unwrap();
        let region_before = fs::read(&region).unwrap();

        let err = write_icon_from_final_gridfile_with_parent(&region, parent_arg, &output, 2)
            .unwrap_err();

        let diagnostic = err.to_string();
        let expected = match case {
            "missing_final_lineage" | "missing_parent_lineage" => {
                "requires complete M/W parent lineage"
            }
            "wrong_final_lineage" => "outside explicit parent",
            "changed_parent_coordinate" => "outside explicit parent",
            "changed_final_corner" => "triangle corners",
            "changed_parent_corner" | "regional_parent" => "closed triangular sphere",
            _ => unreachable!(),
        };
        assert!(
            diagnostic.contains(expected),
            "{case}: expected {expected:?} in {diagnostic:?}"
        );
        assert_eq!(fs::read(&output).unwrap(), b"previous ICON");
        assert_eq!(fs::read(parent_arg).unwrap(), parent_before);
        assert_eq!(fs::read(&region).unwrap(), region_before);
        assert!(!fs::read_dir(&root).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".tmp-")));
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn icon_final_with_explicit_parent_rejects_parent_output_aliases_without_touching_sources() {
    use std::os::unix::fs::symlink;

    for case in ["parent_same_path", "parent_symlink", "parent_hardlink"] {
        let root = root(case);
        let (parent, region, _) = parent_and_region(&root, 1);
        let output = if case == "parent_same_path" {
            parent.clone()
        } else {
            root.join("ICON_alias.nc4")
        };
        match case {
            "parent_same_path" => {}
            "parent_symlink" => symlink(&parent, &output).unwrap(),
            "parent_hardlink" => fs::hard_link(&parent, &output).unwrap(),
            _ => unreachable!(),
        }
        let parent_before = fs::read(&parent).unwrap();
        let region_before = fs::read(&region).unwrap();

        let err =
            write_icon_from_final_gridfile_with_parent(&region, &parent, &output, 2).unwrap_err();

        assert!(
            matches!(
                err.kind(),
                std::io::ErrorKind::AlreadyExists | std::io::ErrorKind::InvalidInput
            ),
            "{err}"
        );
        assert_eq!(fs::read(&parent).unwrap(), parent_before);
        assert_eq!(fs::read(&region).unwrap(), region_before);
        assert!(!fs::read_dir(&root).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".tmp-")));
        fs::remove_dir_all(root).unwrap();
    }
}

fn bipyramid(sides: usize) -> UnstructuredMesh {
    let mut w_points = vec![
        LonLatPoint { lon: 0.0, lat: 0.0 },
        LonLatPoint {
            lon: 0.0,
            lat: 90.0,
        },
    ];
    w_points.extend((0..sides).map(|i| LonLatPoint {
        lon: (360.0 * i as f64 / sides as f64 + 180.0).rem_euclid(360.0) - 180.0,
        lat: 0.0,
    }));
    w_points.push(LonLatPoint {
        lon: 0.0,
        lat: -90.0,
    });
    let mut m_to_w = vec![[1; 3]];
    for pole in [2, sides as i32 + 3] {
        for i in 0..sides {
            let (a, b) = (i as i32 + 3, ((i + 1) % sides) as i32 + 3);
            m_to_w.push(if pole == 2 {
                [pole, a, b]
            } else {
                [pole, b, a]
            });
        }
    }
    let mut m_points = vec![w_points[0]];
    for corners in &m_to_w[1..] {
        let (mut x, mut y, mut z) = (0.0_f64, 0.0_f64, 0.0_f64);
        for &id in corners {
            let point = w_points[id as usize - 1];
            let (lon, lat) = (point.lon.to_radians(), point.lat.to_radians());
            x += lat.cos() * lon.cos();
            y += lat.cos() * lon.sin();
            z += lat.sin();
        }
        m_points.push(LonLatPoint {
            lon: y.atan2(x).to_degrees(),
            lat: z.atan2(x.hypot(y)).to_degrees(),
        });
    }
    let mut w_to_m = vec![vec![1; 7]];
    for id in 2..=sides as i32 + 3 {
        w_to_m.push(
            m_to_w
                .iter()
                .enumerate()
                .skip(1)
                .filter(|(_, corners)| corners.contains(&id))
                .map(|(row, _)| row as i32 + 1)
                .collect(),
        );
    }
    let n_w_to_m = w_to_m
        .iter()
        .enumerate()
        .map(|(i, ring)| if i == 0 { 1 } else { ring.len() as i32 })
        .collect();
    UnstructuredMesh {
        m_points,
        w_points,
        m_to_w,
        w_to_m,
        n_w_to_m,
    }
}

#[test]
fn icon_regional_parent_uses_triangle_topology_not_hex_degree_admission() {
    let mesh = bipyramid(4); // Octahedron: six degree-four fans, eight physical triangles.
    assert!(mesh.n_w_to_m[1..].iter().all(|&n| n == 4));
    let root = root("triangle_parent_degree_four");
    let parent = root.join("parent.nc4");
    let region = root.join("north.nc4");
    let parent_icon = root.join("parent_icon.nc4");
    let output = root.join("selected_icon.nc4");
    write_gridfile_with_context(&parent, &mesh);
    let before = fs::read(&parent).unwrap();
    write_icon_from_final_gridfile(&parent, &parent_icon, 1).unwrap();
    let kept = earthmesh_cli::regional_gridfile_writers::write_regional_gridfile(
        &parent,
        &region,
        &GridRegion::Bbox {
            west: -180.0,
            east: 180.0,
            south: 0.0,
            north: 90.0,
        },
        "tri",
    )
    .unwrap();
    assert_eq!(kept, 4);
    let selected_before = fs::read(&region).unwrap();
    let report = write_icon_from_final_gridfile_with_parent(&region, &parent, &output, 1).unwrap();
    assert_eq!(
        (
            report.cells,
            report.vertices,
            report.edges,
            report.global_grid
        ),
        (4, 5, 8, false)
    );
    assert_regional_icon_matches_parent_and_selection(&parent_icon, &region, &output);
    assert_eq!(fs::read(&parent).unwrap(), before);
    assert_eq!(fs::read(&region).unwrap(), selected_before);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn icon_regional_fan_limit_applies_to_selection_not_discarded_parent_triangles() {
    let root = root("selected_fan_limit");
    let mesh = bipyramid(7); // Pole degree seven; equatorial degree four.
    let parent = root.join("parent.nc4");
    let region = root.join("selection.nc4");
    let output = root.join("ICON.nc4");
    write_gridfile_with_context(&parent, &mesh);
    let before = fs::read(&parent).unwrap();
    fs::write(&output, b"previous ICON").unwrap();
    let error = write_icon_from_final_gridfile(&parent, &output, 1).unwrap_err();
    assert!(error.to_string().contains("degree 7"), "{error}");
    assert_eq!(fs::read(&output).unwrap(), b"previous ICON");
    for (east, expected_cells) in [(180.0, 7), (120.0, 2)] {
        let west = if east == 180.0 { -180.0 } else { 0.0 };
        let kept = earthmesh_cli::regional_gridfile_writers::write_regional_gridfile(
            &parent,
            &region,
            &GridRegion::Bbox {
                west,
                east,
                south: 0.0,
                north: 90.0,
            },
            "tri",
        )
        .unwrap();
        assert_eq!(kept, expected_cells);
        let result = write_icon_from_final_gridfile_with_parent(&region, &parent, &output, 1);
        if expected_cells == 7 {
            let error = result.unwrap_err();
            assert!(error.to_string().contains("degree 7"), "{error}");
            assert_eq!(fs::read(&output).unwrap(), b"previous ICON");
        } else {
            let report = result.unwrap();
            assert_eq!((report.cells, report.vertices, report.edges), (2, 4, 5));
            assert_eq!(
                icon_coord_values(&output, "clon", "clat"),
                selected_cell_coords(&region)
            );
            assert_eq!(
                icon_coord_values(&output, "vlon", "vlat"),
                selected_vertex_coords(&region)
            );
        }
    }
    assert_eq!(fs::read(&parent).unwrap(), before);
    fs::remove_dir_all(root).unwrap();
}
