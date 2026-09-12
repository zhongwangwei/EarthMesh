use std::{
    fs, io,
    path::{Path, PathBuf},
};

use earthmesh_cli::{
    coordinate_types::LonLatPoint,
    mpas_gridfile_context::MpasGridfileContext,
    unstructured_mesh_support::{MethodCGridfileMetadataSlices, UnstructuredMesh},
};
use earthmesh_project::ModelFormat;

const MPAS_OCEAN_SPHERE_RADIUS_METERS: f64 = 6_371_220.0;

fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_mpas_final_{name}_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn canonical_voronoi_fixture_mesh() -> UnstructuredMesh {
    let state = earthmesh_mesh::gridinit_voronoi_state_canonical(1, 0, 1.0, 0.25, 100)
        .expect("gridinit voronoi state");
    earthmesh_cli::mesh_conversion_gridfile_state::gridfile_mesh_from_one_based_state(
        &state.grid,
        &state.tabs,
    )
    .expect("one-based gridinit fixture to unstructured mesh")
}

fn explicit_two_placeholder_mesh(mesh: &UnstructuredMesh) -> UnstructuredMesh {
    let point = LonLatPoint { lon: 0.0, lat: 0.0 };
    let mut explicit = mesh.clone();
    explicit.m_points.insert(0, point);
    explicit.w_points.insert(0, point);
    explicit.m_to_w.insert(0, [0, 0, 0]);
    explicit.w_to_m.insert(0, vec![0]);
    explicit.n_w_to_m.insert(0, 0);
    explicit
}

fn mesh_with_placeholders(count: usize) -> UnstructuredMesh {
    let mut mesh = canonical_voronoi_fixture_mesh();
    match count {
        0 => {
            mesh.m_points.remove(0);
            mesh.w_points.remove(0);
            mesh.m_to_w.remove(0);
            mesh.w_to_m.remove(0);
            mesh.n_w_to_m.remove(0);
            for row in &mut mesh.m_to_w {
                for id in row {
                    *id -= 1;
                }
            }
            for ring in &mut mesh.w_to_m {
                for id in ring {
                    *id -= 1;
                }
            }
            mesh
        }
        1 => mesh,
        2 => explicit_two_placeholder_mesh(&mesh),
        _ => unreachable!(),
    }
}

fn bad_degree_mesh() -> UnstructuredMesh {
    UnstructuredMesh {
        m_points: vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 0.2, lat: 0.2 },
        ],
        w_points: vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 1.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 1.0 },
        ],
        m_to_w: vec![[1, 1, 1], [2, 3, 4]],
        w_to_m: vec![vec![1], vec![2], vec![2], vec![2]],
        n_w_to_m: vec![1, 1, 1, 1],
    }
}

fn open_pentagon_mesh() -> UnstructuredMesh {
    UnstructuredMesh {
        m_points: vec![
            LonLatPoint { lon: 0.0, lat: 1.0 },
            LonLatPoint {
                lon: 0.95,
                lat: 0.31,
            },
            LonLatPoint {
                lon: 0.59,
                lat: -0.81,
            },
            LonLatPoint {
                lon: -0.59,
                lat: -0.81,
            },
            LonLatPoint {
                lon: -0.95,
                lat: 0.31,
            },
        ],
        w_points: vec![LonLatPoint { lon: 0.0, lat: 0.0 }],
        m_to_w: vec![[1, 1, 1]; 5],
        w_to_m: vec![vec![1, 2, 3, 4, 5]],
        n_w_to_m: vec![5],
    }
}

fn context_for(mesh: &UnstructuredMesh) -> MpasGridfileContext {
    let mut widths = (0..mesh.w_points.len())
        .map(|idx| 80.0 + idx as f64 * 7.5)
        .collect::<Vec<_>>();
    widths[0] = if widths.len() == 1 { 80.0 } else { 999.0 };
    if widths.len() > 1 {
        widths[1] = 888.0;
    }
    MpasGridfileContext {
        cellwidth_km: widths,
        base_nxp: 80,
        step: 2,
        density_reference_width_km: 25.0,
        source: "cmrc-test-final".to_string(),
    }
}

fn write_gridfile(path: &Path, mesh: &UnstructuredMesh, context: Option<&MpasGridfileContext>) {
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_method_c_metadata(
        path,
        mesh,
        MethodCGridfileMetadataSlices {
            mpas: context,
            ..Default::default()
        },
    )
    .unwrap();
}

fn read_f64(path: &Path, name: &str) -> Vec<f64> {
    netcdf::open(path)
        .unwrap()
        .variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<f64, _>(..)
        .unwrap()
}

fn read_i32(path: &Path, name: &str) -> Vec<i32> {
    netcdf::open(path)
        .unwrap()
        .variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<i32, _>(..)
        .unwrap()
}

fn scalar_attr_f64(path: &Path, name: &str) -> f64 {
    match netcdf::open(path)
        .unwrap()
        .attribute(name)
        .unwrap()
        .value()
        .unwrap()
    {
        netcdf::AttributeValue::Double(value) => value,
        other => panic!("{name} has unexpected value {other:?}"),
    }
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1.0e-12,
        "expected {expected}, got {actual}"
    );
}

fn assert_vec_close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (&actual, &expected) in actual.iter().zip(expected) {
        assert_close(actual, expected);
    }
}

fn unit_xyz(lon: f64, lat: f64) -> (f64, f64, f64) {
    let (lon, lat) = (lon.to_radians(), lat.to_radians());
    (lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin())
}

fn assert_density_from_context(output: &Path, context: &MpasGridfileContext) {
    let density = read_f64(output, "meshDensity");
    let widths = &context.cellwidth_km[context.cellwidth_km.len() - density.len()..];
    for (&actual, &width) in density.iter().zip(widths) {
        assert_close(actual, (context.density_reference_width_km / width).powi(4));
    }
}

fn expected_full_mesh_file(
    path: &Path,
    mesh: &UnstructuredMesh,
    context: &MpasGridfileContext,
    format: ModelFormat,
) {
    let mut full =
        earthmesh_cli::mpas_unstructured_mesh_builders::build_mpas_mesh_from_unstructured_one_based(
            mesh,
            &context.cellwidth_km,
            context.base_nxp,
            context.step,
        )
        .unwrap();
    let first_physical_width = context.cellwidth_km.len() - (full.mesh_density.len() - 1);
    let density = full
        .mesh_density
        .iter()
        .enumerate()
        .map(|(idx, _)| {
            if idx == 0 {
                1.0
            } else {
                (context.density_reference_width_km
                    / context.cellwidth_km[first_physical_width + idx - 1])
                    .powi(4)
            }
        })
        .collect::<Vec<_>>();
    full.mesh_density = density;
    if format == ModelFormat::MpasOcean {
        earthmesh_cli::write_mpas_ocean_mesh_netcdf(path, &full).unwrap();
    } else {
        earthmesh_cli::write_mpas_mesh_netcdf(path, &full).unwrap();
    }
}

fn assert_full_matches_existing_builder(delivered: &Path, expected: &Path) {
    for name in ["cellsOnCell", "cellsOnVertex"] {
        assert_eq!(read_i32(delivered, name), read_i32(expected, name));
    }
    for name in ["dcEdge", "areaCell", "weightsOnEdge"] {
        assert_vec_close(&read_f64(delivered, name), &read_f64(expected, name));
    }
}

fn seed_bundle(out: &Path) {
    fs::create_dir_all(out).unwrap();
    fs::write(out.join("mesh.nc4"), "old mesh").unwrap();
    fs::write(out.join("graph.info"), "old graph").unwrap();
}

fn assert_bundle_preserved(out: &Path) {
    assert_eq!(fs::read(out.join("mesh.nc4")).unwrap(), b"old mesh");
    assert_eq!(fs::read(out.join("graph.info")).unwrap(), b"old graph");
}

fn assert_no_staged_artifacts(out: &Path) {
    if !out.exists() {
        return;
    }
    for entry in fs::read_dir(out).unwrap() {
        let name = entry.unwrap().file_name();
        assert!(!name.to_string_lossy().starts_with(".mpas-tmp-"));
    }
}

#[test]
fn final_mpas_delivery_uses_native_context_density_for_all_layouts_and_full_formats() {
    for placeholders in 0..=2 {
        for format in [ModelFormat::Mpas, ModelFormat::MpasOcean] {
            let root = temp_root(&format!("density_explicit_{placeholders}"));
            let mesh = mesh_with_placeholders(placeholders);
            let context = context_for(&mesh);
            let gridfile = root.join("final_grid.nc4");
            let out = root.join("mpas");
            let expected = root.join("expected.nc4");
            write_gridfile(&gridfile, &mesh, Some(&context));
            expected_full_mesh_file(&expected, &mesh, &context, format);
            let input_before = fs::read(&gridfile).unwrap();

            let (mesh_out, graph_out) =
                earthmesh_cli::mpas_gridfile_writers::write_mpas_from_final_gridfile(
                    &gridfile, &out, format,
                )
                .unwrap();

            assert_eq!(mesh_out, out.join("mesh.nc4"));
            assert_eq!(graph_out, Some(out.join("graph.info")));
            assert!(graph_out.unwrap().exists());
            assert_density_from_context(&mesh_out, &context);
            assert_full_matches_existing_builder(&mesh_out, &expected);
            assert_eq!(fs::read(&gridfile).unwrap(), input_before);
            let nominal = read_f64(&mesh_out, "nominalMinDc")[0];
            let expected_nominal = read_f64(&expected, "nominalMinDc")[0];
            assert_close(nominal, expected_nominal);
            if format == ModelFormat::MpasOcean {
                assert_close(
                    scalar_attr_f64(&mesh_out, "sphere_radius"),
                    MPAS_OCEAN_SPHERE_RADIUS_METERS,
                );
            } else {
                assert_close(scalar_attr_f64(&mesh_out, "sphere_radius"), 1.0);
            }
            let _ = fs::remove_dir_all(&root);
        }
    }
}

#[test]
fn final_mpas_simple_delivery_writes_mesh_only_for_all_layouts_and_removes_stale_graph() {
    for placeholders in 0..=2 {
        let root = temp_root(&format!("simple_explicit_{placeholders}"));
        let mesh = mesh_with_placeholders(placeholders);
        let context = context_for(&mesh);
        let gridfile = root.join("final_grid.nc4");
        let out = root.join("mpas");
        fs::create_dir_all(&out).unwrap();
        fs::write(out.join("graph.info"), "stale").unwrap();
        write_gridfile(&gridfile, &mesh, Some(&context));

        let (mesh_out, graph_out) =
            earthmesh_cli::mpas_gridfile_writers::write_mpas_from_final_gridfile(
                &gridfile,
                &out,
                ModelFormat::MpasSimple,
            )
            .unwrap();

        assert_eq!(mesh_out, out.join("mesh.nc4"));
        assert_eq!(graph_out, None);
        assert!(!out.join("graph.info").exists());
        assert_density_from_context(&mesh_out, &context);
        let _ = fs::remove_dir_all(&root);
    }
}

#[test]
fn final_mpas_delivery_rejects_missing_malformed_bad_degree_and_open_global_inputs() {
    for case in [
        "missing_context",
        "malformed_context",
        "bad_degree",
        "open_global_parent_required",
        "density_underflow",
        "zero_nominal",
        "bad_format",
    ] {
        let root = temp_root(case);
        let gridfile = root.join("final_grid.nc4");
        let out = root.join("mpas");
        match case {
            "missing_context" => write_gridfile(&gridfile, &canonical_voronoi_fixture_mesh(), None),
            "malformed_context" => {
                write_gridfile(&gridfile, &canonical_voronoi_fixture_mesh(), None);
                netcdf::append(&gridfile)
                    .unwrap()
                    .add_attribute("earthmesh_mpas_base_nxp", 80_i32)
                    .unwrap();
            }
            "bad_degree" => {
                let mesh = bad_degree_mesh();
                write_gridfile(&gridfile, &mesh, Some(&context_for(&mesh)));
            }
            "open_global_parent_required" => {
                let mesh = open_pentagon_mesh();
                write_gridfile(&gridfile, &mesh, Some(&context_for(&mesh)));
            }
            "density_underflow" => {
                let mesh = canonical_voronoi_fixture_mesh();
                let mut context = context_for(&mesh);
                context.density_reference_width_km = 1.0e-100;
                write_gridfile(&gridfile, &mesh, Some(&context));
            }
            "zero_nominal" => {
                let mesh = canonical_voronoi_fixture_mesh();
                let mut context = context_for(&mesh);
                context.step = usize::BITS as usize;
                write_gridfile(&gridfile, &mesh, Some(&context));
            }
            "bad_format" => write_gridfile(
                &gridfile,
                &canonical_voronoi_fixture_mesh(),
                Some(&context_for(&canonical_voronoi_fixture_mesh())),
            ),
            _ => unreachable!(),
        }
        seed_bundle(&out);

        let format = if case == "bad_format" {
            ModelFormat::CoLM
        } else {
            ModelFormat::Mpas
        };
        let err = earthmesh_cli::mpas_gridfile_writers::write_mpas_from_final_gridfile(
            &gridfile, &out, format,
        )
        .unwrap_err();
        if case == "open_global_parent_required" {
            assert_eq!(err.kind(), io::ErrorKind::Unsupported);
            assert!(err.to_string().contains("global-parent"));
        } else {
            assert!(matches!(
                err.kind(),
                io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData
            ));
        }
        assert_bundle_preserved(&out);
        assert_no_staged_artifacts(&out);
        let _ = fs::remove_dir_all(&root);
    }
}

#[test]
fn final_mpas_delivery_rejects_unsafe_output_without_overwriting_previous_bundle() {
    let root = temp_root("transaction");
    let mesh = canonical_voronoi_fixture_mesh();
    let context = context_for(&mesh);
    let gridfile = root.join("final_grid.nc4");
    let out = root.join("mpas");
    write_gridfile(&gridfile, &mesh, Some(&context));
    seed_bundle(&out);
    fs::remove_file(out.join("graph.info")).unwrap();
    fs::create_dir(out.join("graph.info")).unwrap();

    let err = earthmesh_cli::mpas_gridfile_writers::write_mpas_from_final_gridfile(
        &gridfile,
        &out,
        ModelFormat::Mpas,
    )
    .unwrap_err();

    assert!(matches!(
        err.kind(),
        io::ErrorKind::AlreadyExists | io::ErrorKind::InvalidInput
    ));
    assert_eq!(fs::read(out.join("mesh.nc4")).unwrap(), b"old mesh");
    assert!(out.join("graph.info").is_dir());
    assert_no_staged_artifacts(&out);

    let same_input_dir = root.join("same_input");
    fs::create_dir_all(&same_input_dir).unwrap();
    let same_input = same_input_dir.join("mesh.nc4");
    write_gridfile(&same_input, &mesh, Some(&context));
    let before = fs::read(&same_input).unwrap();
    assert!(
        earthmesh_cli::mpas_gridfile_writers::write_mpas_from_final_gridfile(
            &same_input,
            &same_input_dir,
            ModelFormat::MpasSimple,
        )
        .is_err()
    );
    assert_eq!(fs::read(&same_input).unwrap(), before);
    assert_no_staged_artifacts(&same_input_dir);
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn final_mpas_delivery_rejects_symlink_and_hardlink_output_aliases() {
    use std::os::unix::fs::symlink;

    for case in ["symlink", "hardlink"] {
        let root = temp_root(case);
        let mesh = canonical_voronoi_fixture_mesh();
        let context = context_for(&mesh);
        let gridfile = root.join("final_grid.nc4");
        let out = root.join("mpas");
        fs::create_dir_all(&out).unwrap();
        write_gridfile(&gridfile, &mesh, Some(&context));
        let input_before = fs::read(&gridfile).unwrap();
        match case {
            "symlink" => symlink(&gridfile, out.join("mesh.nc4")).unwrap(),
            "hardlink" => fs::hard_link(&gridfile, out.join("mesh.nc4")).unwrap(),
            _ => unreachable!(),
        }
        fs::write(out.join("graph.info"), "old graph").unwrap();

        let format = if case == "bad_format" {
            ModelFormat::CoLM
        } else {
            ModelFormat::Mpas
        };
        let err = earthmesh_cli::mpas_gridfile_writers::write_mpas_from_final_gridfile(
            &gridfile, &out, format,
        )
        .unwrap_err();

        assert!(matches!(
            err.kind(),
            io::ErrorKind::AlreadyExists | io::ErrorKind::InvalidInput
        ));
        assert_eq!(fs::read(&gridfile).unwrap(), input_before);
        assert_eq!(fs::read(out.join("graph.info")).unwrap(), b"old graph");
        assert_no_staged_artifacts(&out);
        let _ = fs::remove_dir_all(&root);
    }
}

fn parent_lineage_context(mesh: &UnstructuredMesh) -> (MpasGridfileContext, Vec<i64>, Vec<i64>) {
    let mut context = context_for(mesh);
    context.density_reference_width_km = 31.25;
    for (idx, width) in context.cellwidth_km.iter_mut().enumerate() {
        *width = 300.0 + idx as f64 * 11.0;
    }
    let sentinel_rows = |points: &[LonLatPoint], counts: &[i32], rings: &[Vec<i32>]| {
        (0..2)
            .take_while(|&idx| {
                points
                    .get(idx)
                    .is_some_and(|p| p.lon == 0.0 && p.lat == 0.0)
                    && counts
                        .get(idx)
                        .is_some_and(|&count| (0..=1).contains(&count))
                    && rings
                        .get(idx)
                        .is_some_and(|ring| ring.windows(2).all(|pair| pair[0] == pair[1]))
            })
            .count()
    };
    let w_first = sentinel_rows(&mesh.w_points, &mesh.n_w_to_m, &mesh.w_to_m);
    let m_first = if mesh
        .m_to_w
        .first()
        .is_some_and(|row| row.windows(2).all(|pair| pair[0] == pair[1]))
    {
        1 + usize::from(
            mesh.m_to_w
                .get(1)
                .is_some_and(|row| row.windows(2).all(|pair| pair[0] == pair[1])),
        )
    } else {
        0
    };
    let m_lineage = (0..mesh.m_points.len())
        .map(|idx| {
            if idx < m_first {
                0
            } else {
                10_000 + idx as i64 * 17
            }
        })
        .collect::<Vec<_>>();
    let w_lineage = (0..mesh.w_points.len())
        .map(|idx| {
            if idx < w_first {
                0
            } else {
                20_000 + idx as i64 * 19
            }
        })
        .collect::<Vec<_>>();
    (context, m_lineage, w_lineage)
}

fn write_gridfile_with_lineage(
    path: &Path,
    mesh: &UnstructuredMesh,
    context: Option<&MpasGridfileContext>,
    m_lineage: Option<&[i64]>,
    w_lineage: Option<&[i64]>,
) {
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_method_c_metadata(
        path,
        mesh,
        MethodCGridfileMetadataSlices {
            mpas: context,
            m_lineage,
            w_lineage,
            ..Default::default()
        },
    )
    .unwrap();
}

fn write_parent_and_region(
    root: &Path,
) -> (PathBuf, PathBuf, UnstructuredMesh, MpasGridfileContext) {
    let parent = root.join("parent.nc4");
    let regional = root.join("regional.nc4");
    let mesh = canonical_voronoi_fixture_mesh();
    let (context, m_lineage, w_lineage) = parent_lineage_context(&mesh);
    write_gridfile_with_lineage(
        &parent,
        &mesh,
        Some(&context),
        Some(&m_lineage),
        Some(&w_lineage),
    );
    let kept = earthmesh_cli::regional_gridfile_writers::write_regional_gridfile(
        &parent,
        &regional,
        &earthmesh_cli::coordinate_types::GridRegion::Bbox {
            west: -181.0,
            east: 0.0,
            south: -91.0,
            north: 91.0,
        },
        "hex",
    )
    .unwrap();
    assert!(kept > 0, "regional fixture must keep at least one W cell");
    assert!(
        kept < mesh.w_points.len().saturating_sub(1),
        "regional fixture must be an open proper subset"
    );
    (parent, regional, mesh, context)
}

fn lineage_keep_mask(parent: &Path, regional: &Path, parent_cell_count: usize) -> Vec<bool> {
    let parent_lineage = earthmesh_cli::grid_quality_pipeline::read_gridfile_cell_lineages(parent)
        .unwrap()
        .w;
    let regional_lineage =
        earthmesh_cli::grid_quality_pipeline::read_gridfile_cell_lineages(regional)
            .unwrap()
            .w;
    let mut keep = vec![false; parent_cell_count];
    for lineage in regional_lineage.into_iter().filter(|lineage| *lineage > 0) {
        let parent_row = parent_lineage
            .iter()
            .position(|candidate| *candidate == lineage)
            .expect("regional W lineage maps to parent");
        keep[parent_row] = true;
    }
    keep
}

fn expected_parent_subset_file(
    path: &Path,
    parent_gridfile: &Path,
    regional_gridfile: &Path,
    context: &MpasGridfileContext,
    format: ModelFormat,
) {
    let parent_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(parent_gridfile)
            .unwrap();
    let mut global =
        earthmesh_cli::mpas_unstructured_mesh_builders::build_mpas_mesh_from_unstructured_one_based(
            &parent_mesh,
            &context.cellwidth_km,
            context.base_nxp,
            context.step,
        )
        .unwrap();
    for idx in 1..global.mesh_density.len() {
        global.mesh_density[idx] =
            (context.density_reference_width_km / context.cellwidth_km[idx]).powi(4);
    }
    let keep = lineage_keep_mask(parent_gridfile, regional_gridfile, global.lat_cell.len());
    let subset = earthmesh_cli::mpas_topology::subset_mpas_mesh(&global, &keep).unwrap();
    if format == ModelFormat::MpasSimple {
        let simple = earthmesh_cli::mpas_simple_writer::MpasSimpleMesh {
            x_cell: subset.x_cell,
            y_cell: subset.y_cell,
            z_cell: subset.z_cell,
            x_vertex: subset.x_vertex,
            y_vertex: subset.y_vertex,
            z_vertex: subset.z_vertex,
            cells_on_vertex: subset.cells_on_vertex,
            mesh_density: subset.mesh_density,
        };
        earthmesh_cli::mpas_simple_writer::write_mpas_simple_mesh_netcdf(path, &simple).unwrap();
    } else if format == ModelFormat::MpasOcean {
        earthmesh_cli::write_mpas_ocean_mesh_netcdf(path, &subset).unwrap();
    } else {
        earthmesh_cli::write_mpas_mesh_netcdf(path, &subset).unwrap();
    }
}

#[test]
fn regional_mpas_final_delivery_uses_parent_lineage_subset_for_all_formats() {
    for format in [
        ModelFormat::Mpas,
        ModelFormat::MpasOcean,
        ModelFormat::MpasSimple,
    ] {
        let root = temp_root("regional_parent_positive");
        let (parent, regional, _mesh, context) = write_parent_and_region(&root);
        let out = root.join("mpas");
        let expected = root.join("expected.nc4");
        expected_parent_subset_file(&expected, &parent, &regional, &context, format);
        let final_before = fs::read(&regional).unwrap();
        let parent_before = fs::read(&parent).unwrap();

        let (mesh_out, graph_out) =
            earthmesh_cli::mpas_gridfile_writers::write_mpas_from_final_gridfile_with_parent(
                &regional, &parent, &out, format,
            )
            .unwrap();

        assert_eq!(mesh_out, out.join("mesh.nc4"));
        assert_eq!(graph_out.is_some(), format != ModelFormat::MpasSimple);
        assert_vec_close(
            &read_f64(&mesh_out, "meshDensity"),
            &read_f64(&expected, "meshDensity"),
        );
        if format == ModelFormat::MpasSimple {
            assert_eq!(
                read_i32(&mesh_out, "cellsOnVertex"),
                read_i32(&expected, "cellsOnVertex")
            );
        } else {
            assert_full_matches_existing_builder(&mesh_out, &expected);
            assert_boundary_edges_have_zero_weight_sentinels(&mesh_out);
        }
        assert_eq!(fs::read(&regional).unwrap(), final_before);
        assert_eq!(fs::read(&parent).unwrap(), parent_before);
        let _ = fs::remove_dir_all(&root);
    }
}

#[test]
fn regional_mpas_final_delivery_preserves_nonmonotonic_final_w_order() {
    for format in [ModelFormat::Mpas, ModelFormat::MpasSimple] {
        let root = temp_root("regional_parent_reordered");
        let (parent, regional, _mesh, _context) = write_parent_and_region(&root);
        let final_context = swap_first_two_final_w_rows(&regional);
        let out = root.join("mpas");

        let (mesh_out, graph_out) =
            earthmesh_cli::mpas_gridfile_writers::write_mpas_from_final_gridfile_with_parent(
                &regional, &parent, &out, format,
            )
            .unwrap();

        assert_eq!(graph_out.is_some(), format != ModelFormat::MpasSimple);
        assert_density_from_context(&mesh_out, &final_context);
        let delivered_x = read_f64(&mesh_out, "xCell");
        let delivered_y = read_f64(&mesh_out, "yCell");
        let delivered_z = read_f64(&mesh_out, "zCell");
        let regional_mesh =
            earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&regional).unwrap();
        let first = final_context.cellwidth_km.len() - delivered_x.len();
        for (idx, point) in regional_mesh.w_points[first..].iter().enumerate() {
            let (x, y, z) = unit_xyz(point.lon, point.lat);
            assert_close(delivered_x[idx], x);
            assert_close(delivered_y[idx], y);
            assert_close(delivered_z[idx], z);
        }
        let density = read_f64(&mesh_out, "meshDensity");
        assert!(
            (density[0]
                - (final_context.density_reference_width_km / final_context.cellwidth_km[2])
                    .powi(4))
            .abs()
                < 1.0e-12
        );
        assert_eq!(mesh_out, out.join("mesh.nc4"));
        let _ = fs::remove_dir_all(&root);
    }
}

fn remove_var(path: &Path, name: &str) {
    let tmp = path.with_extension("rewrite.nc4");
    let mesh = earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(path).unwrap();
    let context = earthmesh_cli::mpas_gridfile_context::read_mpas_gridfile_context(path).unwrap();
    let lineages = earthmesh_cli::grid_quality_pipeline::read_gridfile_cell_lineages(path).unwrap();
    write_gridfile_with_lineage(
        &tmp,
        &mesh,
        context.as_ref(),
        (name != "earthmesh_m_lineage").then_some(lineages.m.as_slice()),
        (name != "earthmesh_w_lineage").then_some(lineages.w.as_slice()),
    );
    fs::rename(tmp, path).unwrap();
}

fn rewrite_gridfile_preserving_metadata(
    path: &Path,
    mesh: &UnstructuredMesh,
    context: &MpasGridfileContext,
    m_lineage: &[i64],
    w_lineage: &[i64],
) {
    let tmp = path.with_extension("rewrite.nc4");
    write_gridfile_with_lineage(&tmp, mesh, Some(context), Some(m_lineage), Some(w_lineage));
    fs::rename(tmp, path).unwrap();
}

fn mutate_context(path: &Path, mutate: impl FnOnce(&mut MpasGridfileContext)) {
    let mesh = earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(path).unwrap();
    let mut context = earthmesh_cli::mpas_gridfile_context::read_mpas_gridfile_context(path)
        .unwrap()
        .unwrap();
    let lineages = earthmesh_cli::grid_quality_pipeline::read_gridfile_cell_lineages(path).unwrap();
    mutate(&mut context);
    rewrite_gridfile_preserving_metadata(path, &mesh, &context, &lineages.m, &lineages.w);
}

fn mutate_mesh(path: &Path, mutate: impl FnOnce(&mut UnstructuredMesh)) {
    let mut mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(path).unwrap();
    let context = earthmesh_cli::mpas_gridfile_context::read_mpas_gridfile_context(path)
        .unwrap()
        .unwrap();
    let lineages = earthmesh_cli::grid_quality_pipeline::read_gridfile_cell_lineages(path).unwrap();
    mutate(&mut mesh);
    rewrite_gridfile_preserving_metadata(path, &mesh, &context, &lineages.m, &lineages.w);
}

fn swap_first_two_final_w_rows(path: &Path) -> MpasGridfileContext {
    let mut mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(path).unwrap();
    let mut context = earthmesh_cli::mpas_gridfile_context::read_mpas_gridfile_context(path)
        .unwrap()
        .unwrap();
    let mut lineages =
        earthmesh_cli::grid_quality_pipeline::read_gridfile_cell_lineages(path).unwrap();
    assert!(
        mesh.w_points.len() > 3,
        "regional fixture needs two physical W rows to swap"
    );
    mesh.w_points.swap(2, 3);
    mesh.w_to_m.swap(2, 3);
    mesh.n_w_to_m.swap(2, 3);
    for tri in &mut mesh.m_to_w {
        for id in tri {
            *id = match *id {
                2 => 3,
                3 => 2,
                other => other,
            };
        }
    }
    context.cellwidth_km.swap(2, 3);
    lineages.w.swap(2, 3);
    rewrite_gridfile_preserving_metadata(path, &mesh, &context, &lineages.m, &lineages.w);
    context
}

fn mutate_lineage(path: &Path, target: &str, mutate: impl FnOnce(&mut Vec<i64>)) {
    let mut file = netcdf::append(path).unwrap();
    let mut values = file
        .variable(target)
        .unwrap_or_else(|| panic!("missing {target}"))
        .get_values::<i64, _>(..)
        .unwrap();
    mutate(&mut values);
    file.variable_mut(target)
        .unwrap()
        .put_values(&values, ..)
        .unwrap();
}

fn assert_boundary_edges_have_zero_weight_sentinels(path: &Path) {
    let file = netcdf::open(path).unwrap();
    let cells = file
        .variable("cellsOnEdge")
        .unwrap()
        .get_values::<i32, _>(..)
        .unwrap();
    let edges = file
        .variable("edgesOnEdge")
        .unwrap()
        .get_values::<i32, _>(..)
        .unwrap();
    let weights = file
        .variable("weightsOnEdge")
        .unwrap()
        .get_values::<f64, _>(..)
        .unwrap();
    let n_edges = file.dimension("nEdges").unwrap().len();
    let max_edges2 = file.dimension("maxEdges2").unwrap().len();
    assert_eq!(cells.len(), n_edges * 2);
    assert_eq!(edges.len(), n_edges * max_edges2);
    assert_eq!(weights.len(), n_edges * max_edges2);
    let mut boundary_edges = 0;
    for edge in 0..n_edges {
        if cells[edge * 2] == 0 || cells[edge * 2 + 1] == 0 {
            boundary_edges += 1;
            for slot in 0..max_edges2 {
                let idx = edge * max_edges2 + slot;
                if edges[idx] == 0 {
                    assert_eq!(
                        weights[idx], 0.0,
                        "edge {edge} slot {slot} has zero edge id but nonzero weight"
                    );
                }
            }
        }
    }
    assert!(
        boundary_edges > 0,
        "regional fixture must expose open boundary edges"
    );
}

fn corrupt_first_physical_width(path: &Path) {
    let context = earthmesh_cli::mpas_gridfile_context::read_mpas_gridfile_context(path)
        .unwrap()
        .unwrap();
    let mut mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(path).unwrap();
    let lineages = earthmesh_cli::grid_quality_pipeline::read_gridfile_cell_lineages(path).unwrap();
    mesh.w_points[2].lon += 0.125;
    write_gridfile_with_lineage(
        path,
        &mesh,
        Some(&context),
        Some(&lineages.m),
        Some(&lineages.w),
    );
}

#[test]
fn regional_mpas_final_delivery_rejects_bad_parent_mapping_without_touching_bundle() {
    for case in [
        "missing_parent_context",
        "missing_parent_lineage",
        "missing_final_context",
        "missing_final_lineage",
        "duplicate_final_lineage",
        "stale_final_lineage",
        "changed_final_ring",
        "changed_final_width",
        "changed_final_reference",
        "duplicate_parent_lineage",
        "changed_parent_coordinate",
        "wrong_parent",
        "open_parent",
    ] {
        let root = temp_root(case);
        let (parent, regional, mesh, context) = write_parent_and_region(&root);
        let bad_parent = root.join("bad_parent.nc4");
        match case {
            "missing_parent_context" => {
                let (_context, m_lineage, w_lineage) = parent_lineage_context(&mesh);
                write_gridfile_with_lineage(
                    &bad_parent,
                    &mesh,
                    None,
                    Some(&m_lineage),
                    Some(&w_lineage),
                );
            }
            "missing_parent_lineage" => remove_var(&parent, "earthmesh_w_lineage"),
            "missing_final_context" => {
                let regional_mesh =
                    earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&regional)
                        .unwrap();
                let lineages =
                    earthmesh_cli::grid_quality_pipeline::read_gridfile_cell_lineages(&regional)
                        .unwrap();
                write_gridfile_with_lineage(
                    &regional,
                    &regional_mesh,
                    None,
                    Some(&lineages.m),
                    Some(&lineages.w),
                );
            }
            "missing_final_lineage" => remove_var(&regional, "earthmesh_w_lineage"),
            "duplicate_final_lineage" => {
                mutate_lineage(&regional, "earthmesh_w_lineage", |values| {
                    values[2] = values[3]
                })
            }
            "stale_final_lineage" => mutate_lineage(&regional, "earthmesh_w_lineage", |values| {
                values[2] = 999_999
            }),
            "changed_final_ring" => mutate_mesh(&regional, |mesh| mesh.w_to_m[2].swap(0, 1)),
            "changed_final_width" => {
                mutate_context(&regional, |context| context.cellwidth_km[2] *= 1.5)
            }
            "changed_final_reference" => mutate_context(&regional, |context| {
                context.density_reference_width_km *= 1.25
            }),
            "duplicate_parent_lineage" => {
                mutate_lineage(&parent, "earthmesh_w_lineage", |values| {
                    values[2] = values[3]
                })
            }
            "changed_parent_coordinate" => corrupt_first_physical_width(&parent),
            "wrong_parent" => {
                let wrong_mesh = explicit_two_placeholder_mesh(&mesh);
                let (wrong_context, wrong_m, wrong_w) = parent_lineage_context(&wrong_mesh);
                write_gridfile_with_lineage(
                    &bad_parent,
                    &wrong_mesh,
                    Some(&wrong_context),
                    Some(&wrong_m),
                    Some(&wrong_w),
                );
            }
            "open_parent" => {
                let open = open_pentagon_mesh();
                let (open_context, open_m, open_w) = parent_lineage_context(&open);
                write_gridfile_with_lineage(
                    &bad_parent,
                    &open,
                    Some(&open_context),
                    Some(&open_m),
                    Some(&open_w),
                );
            }
            _ => unreachable!(),
        }
        let parent_arg = match case {
            "missing_parent_context" | "wrong_parent" | "open_parent" => &bad_parent,
            _ => &parent,
        };
        let out = root.join("mpas");
        seed_bundle(&out);
        let regional_before = fs::read(&regional).unwrap();
        let parent_before = fs::read(parent_arg).unwrap();

        let err = earthmesh_cli::mpas_gridfile_writers::write_mpas_from_final_gridfile_with_parent(
            &regional,
            parent_arg,
            &out,
            ModelFormat::Mpas,
        )
        .unwrap_err();

        assert!(matches!(
            err.kind(),
            io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData | io::ErrorKind::Unsupported
        ));
        assert_bundle_preserved(&out);
        assert_no_staged_artifacts(&out);
        assert_eq!(fs::read(&regional).unwrap(), regional_before);
        assert_eq!(fs::read(parent_arg).unwrap(), parent_before);
        let _ = fs::remove_dir_all(&root);
        drop(context);
    }
}

#[cfg(unix)]
#[test]
fn regional_mpas_final_delivery_rejects_parent_output_aliases_without_touching_sources() {
    use std::os::unix::fs::symlink;

    for case in ["parent_mesh_symlink", "parent_mesh_hardlink"] {
        let root = temp_root(case);
        let (parent, regional, _mesh, _context) = write_parent_and_region(&root);
        let out = root.join("mpas");
        fs::create_dir_all(&out).unwrap();
        match case {
            "parent_mesh_symlink" => symlink(&parent, out.join("mesh.nc4")).unwrap(),
            "parent_mesh_hardlink" => fs::hard_link(&parent, out.join("mesh.nc4")).unwrap(),
            _ => unreachable!(),
        }
        fs::write(out.join("graph.info"), "old graph").unwrap();
        let final_before = fs::read(&regional).unwrap();
        let parent_before = fs::read(&parent).unwrap();

        let err = earthmesh_cli::mpas_gridfile_writers::write_mpas_from_final_gridfile_with_parent(
            &regional,
            &parent,
            &out,
            ModelFormat::Mpas,
        )
        .unwrap_err();

        assert!(matches!(
            err.kind(),
            io::ErrorKind::AlreadyExists | io::ErrorKind::InvalidInput
        ));
        assert_eq!(fs::read(&regional).unwrap(), final_before);
        assert_eq!(fs::read(&parent).unwrap(), parent_before);
        assert_eq!(fs::read(out.join("graph.info")).unwrap(), b"old graph");
        assert_no_staged_artifacts(&out);
        let _ = fs::remove_dir_all(&root);
    }
}

fn fill_positive_lineages_with(path: &Path, value: i64) {
    for target in ["earthmesh_m_lineage", "earthmesh_w_lineage"] {
        mutate_lineage(path, target, |values| {
            for lineage in values {
                if *lineage > 0 {
                    *lineage = value;
                }
            }
        });
    }
}

fn assert_mpas_output_same(actual: &Path, expected: &Path, format: ModelFormat) {
    assert_vec_close(
        &read_f64(actual, "meshDensity"),
        &read_f64(expected, "meshDensity"),
    );
    if format == ModelFormat::MpasSimple {
        assert_eq!(
            read_i32(actual, "cellsOnVertex"),
            read_i32(expected, "cellsOnVertex")
        );
        for name in ["xCell", "yCell", "zCell", "xVertex", "yVertex", "zVertex"] {
            assert_vec_close(&read_f64(actual, name), &read_f64(expected, name));
        }
    } else {
        assert_full_matches_existing_builder(actual, expected);
    }
}

#[test]
fn regional_mpas_final_delivery_matches_parent_by_exact_identity_when_ancestry_is_shared() {
    for format in [
        ModelFormat::Mpas,
        ModelFormat::MpasOcean,
        ModelFormat::MpasSimple,
    ] {
        let root = temp_root("regional_same_ancestry_identity");
        let (parent, regional, _mesh, _context) = write_parent_and_region(&root);
        let baseline = root.join("baseline");
        let (expected, _) =
            earthmesh_cli::mpas_gridfile_writers::write_mpas_from_final_gridfile_with_parent(
                &regional, &parent, &baseline, format,
            )
            .unwrap();

        fill_positive_lineages_with(&parent, 7);
        fill_positive_lineages_with(&regional, 7);
        let out = root.join("mpas");
        let (mesh_out, graph_out) =
            earthmesh_cli::mpas_gridfile_writers::write_mpas_from_final_gridfile_with_parent(
                &regional, &parent, &out, format,
            )
            .unwrap();

        assert_eq!(graph_out.is_some(), format != ModelFormat::MpasSimple);
        assert_mpas_output_same(&mesh_out, &expected, format);
        let _ = fs::remove_dir_all(&root);
    }
}

#[test]
fn regional_mpas_final_delivery_rejects_ambiguous_same_ancestry_identity() {
    for case in [
        "changed_coordinate",
        "changed_corner_same_ancestry",
        "duplicate_exact_identity",
    ] {
        let root = temp_root(case);
        let (parent, regional, _mesh, _context) = write_parent_and_region(&root);
        fill_positive_lineages_with(&parent, 7);
        fill_positive_lineages_with(&regional, 7);
        match case {
            "changed_coordinate" => mutate_mesh(&parent, |mesh| mesh.w_points[2].lon += 0.125),
            "changed_corner_same_ancestry" => {
                mutate_mesh(&parent, |mesh| mesh.m_points[2].lat += 0.125)
            }
            "duplicate_exact_identity" => mutate_mesh(&parent, |mesh| {
                mesh.w_points[3] = mesh.w_points[2];
            }),
            _ => unreachable!(),
        }
        let out = root.join("mpas");
        seed_bundle(&out);

        let err = earthmesh_cli::mpas_gridfile_writers::write_mpas_from_final_gridfile_with_parent(
            &regional,
            &parent,
            &out,
            ModelFormat::Mpas,
        )
        .unwrap_err();

        assert!(matches!(
            err.kind(),
            io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData | io::ErrorKind::Unsupported
        ));
        assert_bundle_preserved(&out);
        assert_no_staged_artifacts(&out);
        let _ = fs::remove_dir_all(&root);
    }
}
