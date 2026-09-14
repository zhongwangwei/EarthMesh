use std::{fs, path::Path};

use earthmesh_cli::{
    coordinate_types::LonLatPoint,
    mpas_gridfile_context::{read_mpas_gridfile_context, MpasGridfileContext},
    unstructured_mesh_support::{MethodCGridfileMetadataSlices, UnstructuredMesh},
};

fn root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_mpas_context_{name}_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn mesh() -> UnstructuredMesh {
    UnstructuredMesh {
        m_points: vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint {
                lon: 10.0,
                lat: 0.0,
            },
        ],
        w_points: vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 1.0 },
            LonLatPoint { lon: 1.0, lat: 0.0 },
            LonLatPoint { lon: 1.0, lat: 1.0 },
        ],
        m_to_w: vec![[1, 1, 1], [2, 3, 4]],
        w_to_m: vec![vec![1], vec![2], vec![2], vec![2]],
        n_w_to_m: vec![1, 1, 1, 1],
    }
}

fn context() -> MpasGridfileContext {
    MpasGridfileContext {
        cellwidth_km: vec![999.0, 100.0, 50.5, 25.25],
        base_nxp: 80,
        step: 2,
        density_reference_width_km: 25.25,
        source: "cmrc-test".to_string(),
    }
}

fn write_with_context(path: &Path, context: &MpasGridfileContext) {
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_method_c_metadata(
        path,
        &mesh(),
        MethodCGridfileMetadataSlices {
            mpas: Some(context),
            ..Default::default()
        },
    )
    .unwrap();
}

fn create_malformed(path: &Path, case: &str) {
    let mut file = netcdf::create(path).unwrap();
    file.add_dimension("lbx_points", 2).unwrap();
    file.add_dimension("wrong_points", 2).unwrap();
    match case {
        "partial" => {
            file.add_attribute("earthmesh_mpas_base_nxp", 80_i32)
                .unwrap();
            return;
        }
        "wrong_type" => {
            let mut var = file
                .add_variable::<f32>("earthmesh_w_cellwidth_km", &["lbx_points"])
                .unwrap();
            var.put_attribute("units", "km").unwrap();
            var.put_values(&[10.0_f32, 20.0], ..).unwrap();
        }
        "wrong_dim" => {
            let mut var = file
                .add_variable::<f64>("earthmesh_w_cellwidth_km", &["wrong_points"])
                .unwrap();
            var.put_attribute("units", "km").unwrap();
            var.put_values(&[10.0_f64, 20.0], ..).unwrap();
        }
        "wrong_units" => {
            let mut var = file
                .add_variable::<f64>("earthmesh_w_cellwidth_km", &["lbx_points"])
                .unwrap();
            var.put_attribute("units", "m").unwrap();
            var.put_values(&[10.0_f64, 20.0], ..).unwrap();
        }
        "missing_step" => {
            let mut var = file
                .add_variable::<f64>("earthmesh_w_cellwidth_km", &["lbx_points"])
                .unwrap();
            var.put_attribute("units", "km").unwrap();
            var.put_values(&[10.0_f64, 20.0], ..).unwrap();
            file.add_attribute("earthmesh_mpas_base_nxp", 80_i32)
                .unwrap();
            file.add_attribute("earthmesh_mpas_density_reference_width_km", 10.0_f64)
                .unwrap();
            file.add_attribute("earthmesh_mpas_cellwidth_source", "cmrc-test")
                .unwrap();
            return;
        }
        value_case => {
            let mut var = file
                .add_variable::<f64>("earthmesh_w_cellwidth_km", &["lbx_points"])
                .unwrap();
            var.put_attribute("units", "km").unwrap();
            let values = match value_case {
                "nan" => [f64::NAN, 20.0],
                "negative" => [-10.0, 20.0],
                _ => [10.0, 20.0],
            };
            var.put_values(&values, ..).unwrap();
        }
    }
    file.add_attribute(
        "earthmesh_mpas_base_nxp",
        if case == "zero_nxp" { 0_i32 } else { 80_i32 },
    )
    .unwrap();
    file.add_attribute(
        "earthmesh_mpas_step",
        if case == "zero_step" {
            0_i32
        } else if case == "oversized_step" {
            usize::BITS as i32 + 1
        } else {
            2_i32
        },
    )
    .unwrap();
    file.add_attribute(
        "earthmesh_mpas_density_reference_width_km",
        if case == "reference_nan" {
            f64::NAN
        } else if case == "reference_above_min_width" {
            15.0_f64
        } else {
            10.0_f64
        },
    )
    .unwrap();
    file.add_attribute(
        "earthmesh_mpas_cellwidth_source",
        if case == "empty_source" {
            " "
        } else {
            "cmrc-test"
        },
    )
    .unwrap();
}

#[test]
fn roundtrips_nonuniform_mpas_context_exactly() {
    let root = root("roundtrip");
    let output = root.join("grid.nc4");
    let expected = context();
    write_with_context(&output, &expected);

    assert_eq!(read_mpas_gridfile_context(&output).unwrap(), Some(expected));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn copied_gridfile_keeps_mpas_context_without_mutating_source() {
    let root = root("copy");
    let output = root.join("grid.nc4");
    let copy = root.join("copy.nc4");
    write_with_context(&output, &context());
    let source_before = fs::read(&output).unwrap();
    fs::copy(&output, &copy).unwrap();

    assert_eq!(fs::read(&copy).unwrap(), source_before);
    assert_eq!(
        read_mpas_gridfile_context(&copy).unwrap(),
        read_mpas_gridfile_context(&output).unwrap()
    );
    assert_eq!(fs::read(&output).unwrap(), source_before);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn returns_none_when_mpas_context_is_absent() {
    let root = root("missing");
    let output = root.join("grid.nc4");
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(&output, &mesh()).unwrap();

    assert_eq!(read_mpas_gridfile_context(&output).unwrap(), None);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn rejects_malformed_mpas_context_headers_and_values() {
    for case in [
        "partial",
        "wrong_type",
        "wrong_dim",
        "wrong_units",
        "missing_step",
        "nan",
        "negative",
        "zero_nxp",
        "zero_step",
        "oversized_step",
        "reference_nan",
        "reference_above_min_width",
        "empty_source",
    ] {
        let root = root(case);
        let output = root.join("bad.nc4");
        create_malformed(&output, case);

        assert!(
            read_mpas_gridfile_context(&output).is_err(),
            "case {case} should fail"
        );
        let _ = fs::remove_dir_all(&root);
    }
}

#[test]
fn invalid_mpas_metadata_does_not_overwrite_existing_gridfile() {
    let root = root("atomic");
    let output = root.join("grid.nc4");
    let old = context();
    write_with_context(&output, &old);
    let before = fs::read(&output).unwrap();
    let mut invalid = old.clone();
    invalid.cellwidth_km.pop();

    let err =
        earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_method_c_metadata(
            &output,
            &mesh(),
            MethodCGridfileMetadataSlices {
                mpas: Some(&invalid),
                ..Default::default()
            },
        )
        .unwrap_err();

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert_eq!(fs::read(&output).unwrap(), before);
    assert_eq!(read_mpas_gridfile_context(&output).unwrap(), Some(old));
    let _ = fs::remove_dir_all(&root);
}

fn canonical_hex_mesh() -> UnstructuredMesh {
    let state = earthmesh_mesh::gridinit_voronoi_state_canonical(1, 0, 1.0, 0.25, 100)
        .expect("gridinit voronoi state");
    earthmesh_cli::mesh_conversion_gridfile_state::gridfile_mesh_from_one_based_state(
        &state.grid,
        &state.tabs,
    )
    .expect("one-based gridinit fixture to unstructured mesh")
}

fn two_placeholder_mesh(mesh: &UnstructuredMesh) -> UnstructuredMesh {
    let point = LonLatPoint { lon: 0.0, lat: 0.0 };
    let mut explicit = mesh.clone();
    explicit.m_points.insert(0, point);
    explicit.w_points.insert(0, point);
    explicit.m_to_w.insert(0, [0, 0, 0]);
    explicit.w_to_m.insert(0, vec![0]);
    explicit.n_w_to_m.insert(0, 0);
    explicit
}

fn zero_placeholder_mesh(mesh: &UnstructuredMesh) -> UnstructuredMesh {
    let mut out = mesh.clone();
    out.m_points.remove(0);
    out.w_points.remove(0);
    out.m_to_w.remove(0);
    out.w_to_m.remove(0);
    out.n_w_to_m.remove(0);
    for row in &mut out.m_to_w {
        for id in row {
            *id -= 1;
        }
    }
    for ring in &mut out.w_to_m {
        for id in ring {
            *id -= 1;
        }
    }
    out
}
fn empty_physical_mesh() -> UnstructuredMesh {
    UnstructuredMesh {
        m_points: vec![LonLatPoint { lon: 0.0, lat: 0.0 }],
        w_points: vec![LonLatPoint { lon: 0.0, lat: 0.0 }],
        m_to_w: vec![[1, 1, 1]],
        w_to_m: vec![vec![1]],
        n_w_to_m: vec![1],
    }
}

fn adaptive_pass(
    level: usize,
    cell_meters: f64,
    regions: Vec<earthmesh_mesh::RefinementRegion>,
) -> earthmesh_cli::refinement_demand::nest::NestPassReport {
    earthmesh_cli::refinement_demand::nest::NestPassReport {
        level,
        circle_count: regions.len(),
        regions,
        cell_meters,
        demanded_cells: 1,
        faces_before: 10,
        faces_after: 11,
    }
}

fn adaptive_report(
    passes: Vec<earthmesh_cli::refinement_demand::nest::NestPassReport>,
) -> earthmesh_cli::refinement_demand::nest::AdaptiveNestReport {
    let deepest_level = passes.iter().map(|pass| pass.level).max().unwrap_or(0);
    earthmesh_cli::refinement_demand::nest::AdaptiveNestReport {
        passes,
        deepest_level,
        stopped_on_empty_demand: false,
        spring_passes: 0,
    }
}

fn circle(
    lon: f64,
    lat: f64,
    radius_meters: f64,
    level: usize,
) -> earthmesh_mesh::RefinementRegion {
    earthmesh_mesh::RefinementRegion::Circle {
        center: earthmesh_mesh::LonLatDegrees::new(lon, lat),
        radius_meters,
        level,
    }
}

fn bbox(
    west: f64,
    east: f64,
    south: f64,
    north: f64,
    level: usize,
) -> earthmesh_mesh::RefinementRegion {
    earthmesh_mesh::RefinementRegion::Bbox {
        west_degrees: west,
        east_degrees: east,
        south_degrees: south,
        north_degrees: north,
        level,
    }
}

fn polygon(points: &[(f64, f64)], level: usize) -> earthmesh_mesh::RefinementRegion {
    earthmesh_mesh::RefinementRegion::Polygon {
        points: points
            .iter()
            .map(|&(lon, lat)| earthmesh_mesh::LonLatDegrees::new(lon, lat))
            .collect(),
        level,
    }
}

fn corridor(
    points: &[(f64, f64)],
    radius_meters: f64,
    level: usize,
) -> earthmesh_mesh::RefinementRegion {
    earthmesh_mesh::RefinementRegion::Corridor {
        points: points
            .iter()
            .map(|&(lon, lat)| earthmesh_mesh::LonLatDegrees::new(lon, lat))
            .collect(),
        radius_meters: vec![radius_meters; points.len()],
        level,
    }
}

fn first_physical_w_row(mesh: &UnstructuredMesh) -> usize {
    mesh.n_w_to_m
        .iter()
        .zip(&mesh.w_to_m)
        .zip(&mesh.w_points)
        .position(|((&count, ring), point)| {
            !(point.lon == 0.0
                && point.lat == 0.0
                && count <= 1
                && ring.windows(2).all(|pair| pair[0] == pair[1]))
        })
        .unwrap_or(0)
}

#[test]
fn builds_mpas_context_from_adaptive_region_demand_and_roundtrips() {
    let base = 640_000.0;
    let reference = base / 8.0 / 1000.0;
    for (name, mesh, first) in [
        (
            "zero_placeholder",
            zero_placeholder_mesh(&canonical_hex_mesh()),
            0_usize,
        ),
        ("one_placeholder", canonical_hex_mesh(), 1_usize),
        (
            "two_placeholder",
            two_placeholder_mesh(&canonical_hex_mesh()),
            2_usize,
        ),
    ] {
        assert_eq!(first_physical_w_row(&mesh), first, "{name} layout changed");
        let inside = mesh.w_points[first];
        let outside = mesh.w_points[first + 1];
        let report = adaptive_report(vec![
            adaptive_pass(
                1,
                base,
                vec![circle(inside.lon, inside.lat, 2_000_000.0, 1)],
            ),
            adaptive_pass(
                2,
                base / 2.0,
                vec![bbox(
                    outside.lon - 0.001,
                    outside.lon + 0.001,
                    outside.lat - 0.001,
                    outside.lat + 0.001,
                    4,
                )],
            ),
            adaptive_pass(3, base / 4.0, vec![circle(179.0, 80.0, 1_000.0, 5)]),
        ]);

        let context =
            MpasGridfileContext::from_adaptive_region_demand(&mesh, &report, base, 42).unwrap();

        assert_eq!(context.source, "adaptive_region_pass_w_demand_v1");
        assert_eq!(context.base_nxp, 42);
        assert_eq!(context.step, 4);
        assert_eq!(context.density_reference_width_km, reference);
        assert_eq!(context.cellwidth_km.len(), mesh.w_points.len());
        for width in &context.cellwidth_km[..first] {
            assert_eq!(*width, reference, "{name} placeholder width");
        }
        assert_eq!(context.cellwidth_km[first], base / 2.0 / 1000.0);
        assert_eq!(context.cellwidth_km[first + 1], base / 4.0 / 1000.0);
        assert_eq!(
            context.cellwidth_km[first + 2],
            base / 1000.0,
            "unsampled finest pass only changes the global reference"
        );

        let root = root(name);
        let output = root.join("grid.nc4");
        earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_method_c_metadata(
            &output,
            &mesh,
            MethodCGridfileMetadataSlices {
                mpas: Some(&context),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(read_mpas_gridfile_context(&output).unwrap(), Some(context));
        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn adaptive_region_demand_uses_shared_region_shapes_and_empty_report() {
    let base = 320_000.0;
    let mesh = canonical_hex_mesh();
    let first = first_physical_w_row(&mesh);
    let p0 = mesh.w_points[first];
    let p1 = mesh.w_points[first + 1];
    let p2 = mesh.w_points[first + 2];
    let report = adaptive_report(vec![
        adaptive_pass(1, base, vec![circle(p0.lon, p0.lat, 1_500_000.0, 1)]),
        adaptive_pass(
            2,
            base / 2.0,
            vec![polygon(
                &[
                    (p1.lon - 0.01, p1.lat - 0.01),
                    (p1.lon + 0.01, p1.lat - 0.01),
                    (p1.lon + 0.01, p1.lat + 0.01),
                    (p1.lon - 0.01, p1.lat + 0.01),
                ],
                2,
            )],
        ),
        adaptive_pass(
            3,
            base / 4.0,
            vec![corridor(
                &[(p2.lon - 0.05, p2.lat), (p2.lon + 0.05, p2.lat)],
                5_000.0,
                3,
            )],
        ),
    ]);

    let context =
        MpasGridfileContext::from_adaptive_region_demand(&mesh, &report, base, 42).unwrap();

    assert_eq!(context.cellwidth_km[first], base / 2.0 / 1000.0);
    assert_eq!(context.cellwidth_km[first + 1], base / 4.0 / 1000.0);
    assert_eq!(context.cellwidth_km[first + 2], base / 8.0 / 1000.0);
    let empty = adaptive_report(Vec::new());
    let empty_context =
        MpasGridfileContext::from_adaptive_region_demand(&mesh, &empty, base, 42).unwrap();
    assert_eq!(empty_context.step, 1);
    assert_eq!(empty_context.density_reference_width_km, base / 1000.0);
    assert!(empty_context
        .cellwidth_km
        .iter()
        .all(|width| *width == base / 1000.0));
}

#[test]
fn rejects_invalid_adaptive_region_demand_inputs_and_reserved_versions() {
    let mesh = canonical_hex_mesh();
    let base = 640_000.0;
    for case in [
        "zero_base",
        "nan_base",
        "inf_base",
        "underflow_base",
        "base_nxp0",
        "noncontiguous_passes",
        "bad_depth",
        "cap6",
        "bad_cell_meters",
        "empty_active_region",
        "invalid_region",
        "region_level_below_pass",
        "bad_w_site",
        "nan_lon",
        "empty_physical_mesh",
    ] {
        let mut local_mesh = mesh.clone();
        let mut report = adaptive_report(vec![
            adaptive_pass(1, base, vec![circle(0.0, 0.0, 1_000.0, 1)]),
            adaptive_pass(2, base / 2.0, vec![circle(0.0, 0.0, 1_000.0, 2)]),
        ]);
        let mut local_base = base;
        let mut base_nxp = 42;
        match case {
            "zero_base" => local_base = 0.0,
            "nan_base" => local_base = f64::NAN,
            "inf_base" => local_base = f64::INFINITY,
            "underflow_base" => local_base = f64::from_bits(1),
            "base_nxp0" => base_nxp = 0,
            "noncontiguous_passes" => report.passes[1].level = 3,
            "bad_depth" => report.deepest_level = 1,
            "cap6" => {
                report = adaptive_report(
                    (1..=6)
                        .map(|level| {
                            adaptive_pass(
                                level,
                                base / 2.0_f64.powi((level - 1) as i32),
                                vec![circle(0.0, 0.0, 1_000.0, level)],
                            )
                        })
                        .collect(),
                );
            }
            "bad_cell_meters" => report.passes[1].cell_meters *= 1.25,
            "empty_active_region" => report.passes[0].regions.clear(),
            "invalid_region" => report.passes[0].regions = vec![circle(0.0, 0.0, f64::NAN, 1)],
            "region_level_below_pass" => {
                report.passes[1].regions = vec![circle(0.0, 0.0, 1_000.0, 1)]
            }
            "bad_w_site" => {
                let first = first_physical_w_row(&local_mesh);
                local_mesh.w_points[first].lat = 91.0;
            }
            "nan_lon" => {
                let first = first_physical_w_row(&local_mesh);
                local_mesh.w_points[first].lon = f64::NAN;
            }
            "empty_physical_mesh" => local_mesh = empty_physical_mesh(),
            _ => unreachable!(),
        }
        assert!(
            MpasGridfileContext::from_adaptive_region_demand(
                &local_mesh,
                &report,
                local_base,
                base_nxp,
            )
            .is_err(),
            "case {case} should fail"
        );
    }

    let report = adaptive_report(vec![adaptive_pass(
        1,
        base,
        vec![circle(0.0, 0.0, 1_000.0, 1)],
    )]);
    let mut reserved =
        MpasGridfileContext::from_adaptive_region_demand(&mesh, &report, base, 42).unwrap();
    reserved.source = "adaptive_region_pass_w_demand_v2".to_string();
    let root = root("reserved_adaptive_source");
    let output = root.join("bad.nc4");
    let err =
        earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_method_c_metadata(
            &output,
            &mesh,
            MethodCGridfileMetadataSlices {
                mpas: Some(&reserved),
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(err.to_string().contains("unsupported"), "{err}");

    let valid = MpasGridfileContext::from_adaptive_region_demand(&mesh, &report, base, 42).unwrap();
    let native = root.join("native.nc4");
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_method_c_metadata(
        &native,
        &mesh,
        MethodCGridfileMetadataSlices {
            mpas: Some(&valid),
            ..Default::default()
        },
    )
    .unwrap();
    for (case, source, step) in [
        (
            "unknown_source",
            "adaptive_region_pass_w_demand_v2",
            valid.step as i32,
        ),
        ("step7", valid.source.as_str(), 7_i32),
    ] {
        let bad = root.join(format!("{case}.nc4"));
        fs::copy(&native, &bad).unwrap();
        let mut file = netcdf::append(&bad).unwrap();
        file.add_attribute("earthmesh_mpas_cellwidth_source", source)
            .unwrap();
        file.add_attribute("earthmesh_mpas_step", step).unwrap();
        drop(file);
        assert!(
            read_mpas_gridfile_context(&bad).is_err(),
            "case {case} should fail"
        );
    }
    let _ = fs::remove_dir_all(root);
}

fn lepp_report(
    targets: Vec<earthmesh_refine_method_c::AdaptiveHybridResolvedTarget>,
) -> earthmesh_refine_method_c::AdaptiveHybridReport {
    earthmesh_refine_method_c::AdaptiveHybridReport {
        resolved_targets: targets,
        target_radius_m: earthmesh_core::EARTH_RADIUS_METERS,
        cycles: 0,
        insertions: Vec::new(),
        insertion_counts: earthmesh_refine_method_c::AdaptiveHybridInsertionCounts::default(),
        path_stats: earthmesh_refine_method_c::AdaptiveHybridPathStats::default(),
        initial_vertices: 0,
        final_vertices: 0,
        initial_faces: 0,
        final_faces: 0,
        target_satisfaction: earthmesh_refine_method_c::AdaptiveHybridTargetSatisfaction::default(),
        unresolved_demand_count: 0,
        unresolved_demands: Vec::new(),
        rejections: Vec::new(),
        stop_reason: earthmesh_refine_method_c::AdaptiveHybridStopReason::Satisfied,
    }
}

fn lepp_target(
    id: &str,
    region: earthmesh_mesh::RefinementRegion,
    target_edge_m: f64,
) -> earthmesh_refine_method_c::AdaptiveHybridResolvedTarget {
    earthmesh_refine_method_c::AdaptiveHybridResolvedTarget {
        demand: earthmesh_refine_method_c::AdaptiveHybridDemand::user_region(id, region),
        target_edge_m,
    }
}

fn lepp_site_circle(point: LonLatPoint, level: usize) -> earthmesh_mesh::RefinementRegion {
    earthmesh_mesh::RefinementRegion::Circle {
        center: earthmesh_mesh::LonLatDegrees::new(point.lon, point.lat),
        radius_meters: 1.0,
        level,
    }
}

fn lepp_complete_report(
    mesh: &UnstructuredMesh,
    first: usize,
) -> earthmesh_refine_method_c::AdaptiveHybridReport {
    let mut targets = mesh.w_points[first..]
        .iter()
        .enumerate()
        .map(|(idx, point)| {
            lepp_target(
                &format!("site-{idx}"),
                lepp_site_circle(*point, 1),
                200_000.0,
            )
        })
        .collect::<Vec<_>>();
    targets.push(lepp_target(
        "overlap",
        lepp_site_circle(mesh.w_points[first], 2),
        100_000.0,
    ));
    targets.push(lepp_target(
        "unsampled-reference",
        earthmesh_mesh::RefinementRegion::Circle {
            center: earthmesh_mesh::LonLatDegrees::new(179.0, 80.0),
            radius_meters: 1_000.0,
            level: 3,
        },
        50_000.0,
    ));
    lepp_report(targets)
}

#[test]
fn builds_mpas_context_from_complete_lepp_resolved_demand_and_roundtrips() {
    for (name, mesh, first) in [
        (
            "zero_placeholder",
            zero_placeholder_mesh(&canonical_hex_mesh()),
            0_usize,
        ),
        ("one_placeholder", canonical_hex_mesh(), 1_usize),
        (
            "two_placeholder",
            two_placeholder_mesh(&canonical_hex_mesh()),
            2_usize,
        ),
    ] {
        assert_eq!(first_physical_w_row(&mesh), first, "{name} layout changed");
        let report = lepp_complete_report(&mesh, first);

        let context = MpasGridfileContext::from_lepp_resolved_demand(&mesh, &report, 6)
            .unwrap()
            .unwrap();

        assert_eq!(context.source, "lepp_resolved_region_w_demand_v1");
        assert_eq!(context.base_nxp, 6);
        assert_eq!(context.step, 4);
        assert_eq!(context.density_reference_width_km, 50.0);
        for width in &context.cellwidth_km[..first] {
            assert_eq!(*width, 50.0, "{name} placeholder width");
        }
        assert_eq!(context.cellwidth_km[first], 100.0);
        assert!(context.cellwidth_km[first + 1..]
            .iter()
            .all(|width| *width == 200.0));

        let root = root(&format!("lepp_{name}"));
        let output = root.join("grid.nc4");
        earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_method_c_metadata(
            &output,
            &mesh,
            MethodCGridfileMetadataSlices {
                mpas: Some(&context),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(read_mpas_gridfile_context(&output).unwrap(), Some(context));
        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn lepp_resolved_demand_requires_complete_valid_coverage_and_supported_version() {
    let mesh = canonical_hex_mesh();
    let first = first_physical_w_row(&mesh);
    let empty = lepp_report(Vec::new());
    assert_eq!(
        MpasGridfileContext::from_lepp_resolved_demand(&mesh, &empty, 6).unwrap(),
        None
    );
    let partial = lepp_report(vec![lepp_target(
        "partial",
        lepp_site_circle(mesh.w_points[first], 1),
        200_000.0,
    )]);
    assert_eq!(
        MpasGridfileContext::from_lepp_resolved_demand(&mesh, &partial, 6).unwrap(),
        None
    );

    for case in [
        "bad_radius",
        "bad_scale",
        "bad_site",
        "bad_lat",
        "bad_nxp",
        "empty_physical_mesh",
        "explicit_mismatch",
        "level6",
        "underflow_km",
        "overflow_radius",
        "bad_source_floor",
    ] {
        let mut local_mesh = mesh.clone();
        let mut report = lepp_complete_report(&local_mesh, first);
        let mut nxp = 6;
        match case {
            "bad_radius" => report.target_radius_m = 0.0,
            "bad_scale" => report.resolved_targets[0].target_edge_m = f64::NAN,
            "bad_site" => local_mesh.w_points[first].lon = f64::NAN,
            "bad_lat" => local_mesh.w_points[first].lat = 91.0,
            "bad_nxp" => nxp = 0,
            "empty_physical_mesh" => local_mesh.w_points.truncate(first),
            "explicit_mismatch" => report.resolved_targets[0].demand.target_edge_m = Some(123.0),
            "level6" => {
                report.resolved_targets[0].demand.region =
                    lepp_site_circle(local_mesh.w_points[first], 6);
            }
            "underflow_km" => report.resolved_targets[0].target_edge_m = f64::from_bits(1),
            "overflow_radius" => report.target_radius_m = f64::MAX,
            "bad_source_floor" => {
                report.resolved_targets[0].demand.source_resolution_m = Some(f64::NAN);
            }
            _ => unreachable!(),
        }
        assert!(
            MpasGridfileContext::from_lepp_resolved_demand(&local_mesh, &report, nxp).is_err(),
            "case {case} should fail instead of silently returning None"
        );
    }

    let root = root("reserved_lepp_source");
    let output = root.join("bad.nc4");
    let mut context = MpasGridfileContext::from_lepp_resolved_demand(
        &mesh,
        &lepp_complete_report(&mesh, first),
        6,
    )
    .unwrap()
    .unwrap();
    context.source = "lepp_resolved_region_w_demand_v2".to_string();
    let err =
        earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_method_c_metadata(
            &output,
            &mesh,
            MethodCGridfileMetadataSlices {
                mpas: Some(&context),
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(err.to_string().contains("unsupported"), "{err}");

    let mut valid = MpasGridfileContext::from_lepp_resolved_demand(
        &mesh,
        &lepp_complete_report(&mesh, first),
        6,
    )
    .unwrap()
    .unwrap();
    valid.source = "lepp_resolved_region_w_demand_v1".to_string();
    let native = root.join("native.nc4");
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_method_c_metadata(
        &native,
        &mesh,
        MethodCGridfileMetadataSlices {
            mpas: Some(&valid),
            ..Default::default()
        },
    )
    .unwrap();
    for (case, source, step) in [
        (
            "unknown_source",
            "lepp_resolved_region_w_demand_v2",
            valid.step as i32,
        ),
        ("step7", valid.source.as_str(), 7_i32),
    ] {
        let bad = root.join(format!("{case}.nc4"));
        fs::copy(&native, &bad).unwrap();
        let mut file = netcdf::append(&bad).unwrap();
        file.add_attribute("earthmesh_mpas_cellwidth_source", source)
            .unwrap();
        file.add_attribute("earthmesh_mpas_step", step).unwrap();
        drop(file);
        assert!(
            read_mpas_gridfile_context(&bad).is_err(),
            "case {case} should fail"
        );
    }
    let _ = fs::remove_dir_all(root);
}
