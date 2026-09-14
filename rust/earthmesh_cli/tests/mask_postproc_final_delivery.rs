use earthmesh_cli::{
    coordinate_types::LonLatPoint,
    mask_postproc_domain::*,
    mask_postproc_types::{
        MaskPostprocDomainIoPlan, MaskPostprocEarthRunOptions, MaskPostprocLandRunOptions,
        MaskPostprocOceanRunOptions,
    },
    unstructured_mesh_support::UnstructuredMesh,
};
use std::{fs, path::Path};

fn source_mesh() -> UnstructuredMesh {
    let state = earthmesh_mesh::gridinit_voronoi_state_canonical(1, 0, 1.0, 0.25, 100).unwrap();
    let mut mesh =
        earthmesh_cli::mesh_conversion_gridfile_state::gridfile_mesh_from_one_based_state(
            &state.grid,
            &state.tabs,
        )
        .unwrap();
    // Mask-postprocess arrays use explicit reserved rows 0 and 1, with physical IDs >=2.
    mesh.m_points.insert(0, LonLatPoint { lon: 0.0, lat: 0.0 });
    mesh.w_points.insert(0, LonLatPoint { lon: 0.0, lat: 0.0 });
    mesh.m_to_w.insert(0, [1; 3]);
    mesh.w_to_m.insert(0, vec![1]);
    mesh.n_w_to_m.insert(0, 0);
    mesh
}

fn prepare(root: &Path, kind: &str, mode: &str, defect: bool) -> MaskPostprocDomainIoPlan {
    fs::create_dir_all(root.join("result")).unwrap();
    fs::create_dir_all(root.join("contain")).unwrap();
    let plan = plan_mask_postproc_domain_io(root, 1, mode, kind, false).unwrap();
    let mut mesh = source_mesh();
    if defect {
        if mode == "hex" {
            mesh.n_w_to_m[2] = 4;
        } else {
            let v = mesh.m_to_w[2][0] as usize;
            let next = mesh.m_to_w[2][1] as usize;
            mesh.w_points[next] = mesh.w_points[v];
        }
    }
    let count = if mode == "hex" {
        mesh.w_points.len()
    } else {
        mesh.m_points.len()
    };
    let mut active = vec![1; count];
    active[..2].fill(0);
    let mut ids = vec![vec![0, 1, 1]; count];
    ids[2][0] = 1;
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(
        &plan.source_gridfile,
        &mesh,
    )
    .unwrap();
    earthmesh_cli::contain_io::write_contain_netcdf(
        &plan.contain_domain,
        &earthmesh_cli::contain_io::ContainMesh {
            ustr_id: ids,
            ustr_ii: vec![vec![1, 1, 1]],
            is_in_area_ustr: active,
        },
    )
    .unwrap();
    plan
}

fn run(plan: &MaskPostprocDomainIoPlan) -> std::io::Result<()> {
    run_with_scope(plan, Some(2))
}

fn earth_options<'a>(num_mp_step: &'a [usize]) -> MaskPostprocEarthRunOptions<'a> {
    static VERTICES: [f64; 3] = [0.0, 10.0, 11.0];
    static CENTERS: [f64; 2] = [0.0, 10.5];
    MaskPostprocEarthRunOptions {
        mask_sea_ratio: 0.5,
        minlon_dm_area: 1,
        maxlat_dm_area: 1,
        nlons_dm_select: 1,
        nlats_dm_select: 1,
        lon_vertex: &VERTICES,
        lat_vertex: &VERTICES,
        lon_i: &CENTERS,
        lat_i: &CENTERS,
        num_mp_step,
        sjx_points: 22,
    }
}

fn assert_no_delivery_staging(root: &Path) {
    fn walk(path: &Path) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            assert!(
                !path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(".earthmesh-delivery-"),
                "staging path leaked: {}",
                path.display()
            );
            if path.is_dir() {
                walk(&path);
            }
        }
    }
    if root.exists() {
        walk(root);
    }
}

fn run_with_scope(
    plan: &MaskPostprocDomainIoPlan,
    expected_euler: Option<isize>,
) -> std::io::Result<()> {
    let vertices = [0.0, 10.0, 11.0];
    let centers = [0.0, 10.5];
    match plan.mesh_type.as_str() {
        "earthmesh" => run_final_mask_postproc_earth_domain(
            plan,
            MaskPostprocEarthRunOptions {
                mask_sea_ratio: 0.5,
                minlon_dm_area: 1,
                maxlat_dm_area: 1,
                nlons_dm_select: 1,
                nlats_dm_select: 1,
                lon_vertex: &vertices,
                lat_vertex: &vertices,
                lon_i: &centers,
                lat_i: &centers,
                num_mp_step: &[22],
                sjx_points: 22,
            },
            expected_euler,
        )
        .map(|_| ()),
        "landmesh" => run_final_mask_postproc_land_domain(
            plan,
            MaskPostprocLandRunOptions {
                seaorland: &[vec![false]],
                minlon_dm_area: 1,
                maxlat_dm_area: 1,
                nlons_dm_select: 1,
                nlats_dm_select: 1,
                lon_vertex: &vertices,
                lat_vertex: &vertices,
                lon_i: &centers,
                lat_i: &centers,
            },
        )
        .map(|_| ()),
        _ => run_final_mask_postproc_ocean_domain(
            plan,
            MaskPostprocOceanRunOptions {
                mask_sea_ratio: 0.5,
                num_vertex: 1,
            },
        )
        .map(|_| ()),
    }
}

fn quality_dir(plan: &MaskPostprocDomainIoPlan) -> std::path::PathBuf {
    plan.result_gridfile
        .parent()
        .unwrap()
        .join("final_quality")
        .join(plan.result_gridfile.file_stem().unwrap())
}

#[test]
fn invalid_final_cells_do_not_publish_domain_sidecars() {
    let root = std::env::temp_dir().join(format!("legacy_domain_reject_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    for kind in ["earthmesh", "landmesh", "oceanmesh"] {
        for mode in ["hex", "tri"] {
            let plan = prepare(&root.join(kind).join(mode), kind, mode, true);
            let dir = quality_dir(&plan);
            fs::create_dir_all(&dir).unwrap();
            let marker = dir.join("legacy_delivery.json");
            fs::write(&marker, "old success").unwrap();
            let error = run(&plan)
                .expect_err("invalid final cells must block sidecars")
                .to_string();
            assert!(
                error.contains("admission") || error.contains("invalid polygon geometry"),
                "{kind}/{mode}: {error}"
            );
            assert!(!marker.exists());
            assert!(
                !plan.result_gridfile.exists(),
                "native {} escaped failed admission",
                plan.result_gridfile.display()
            );
            for path in [&plan.patchtype_output, &plan.obc_output, &plan.obcv2_output]
                .into_iter()
                .flatten()
            {
                assert!(
                    !path.exists(),
                    "sidecar {} escaped failed admission",
                    path.display()
                );
            }
            assert!(!plan.file_dir.join("result/earthmesh_info.nc4").exists());
            assert_no_delivery_staging(&root);
        }
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_final_attempt_preserves_previous_native_and_auxiliary_bundle() {
    let root = std::env::temp_dir().join(format!("legacy_domain_preserve_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);

    for kind in ["earthmesh", "landmesh", "oceanmesh"] {
        for mode in ["hex", "tri"] {
            let case = root.join(kind).join(mode);
            let plan = prepare(&case, kind, mode, false);
            run(&plan).unwrap_or_else(|e| panic!("seed {kind}/{mode}: {e}"));
            let native = fs::read(&plan.result_gridfile).unwrap();
            let mut aux = Vec::new();
            for path in [&plan.patchtype_output, &plan.obc_output, &plan.obcv2_output]
                .into_iter()
                .flatten()
            {
                aux.push((path.clone(), fs::read(path).unwrap()));
            }
            let info = plan.file_dir.join("result/earthmesh_info.nc4");
            let info_bytes = info.exists().then(|| fs::read(&info).unwrap());

            let plan = prepare(&case, kind, mode, true);
            let error = run(&plan).unwrap_err().to_string();
            assert!(
                error.contains("admission") || error.contains("invalid polygon geometry"),
                "{kind}/{mode}: {error}"
            );
            assert_eq!(
                fs::read(&plan.result_gridfile).unwrap(),
                native,
                "{kind}/{mode} native changed"
            );
            for (path, bytes) in aux {
                assert_eq!(
                    fs::read(&path).unwrap(),
                    bytes,
                    "{} changed",
                    path.display()
                );
            }
            if let Some(bytes) = info_bytes {
                assert_eq!(
                    fs::read(&info).unwrap(),
                    bytes,
                    "{} changed",
                    info.display()
                );
            }
            assert!(!quality_dir(&plan).join("legacy_delivery.json").exists());
            assert!(quality_dir(&plan).join("quality_summary.json").is_file());
            assert_no_delivery_staging(&root);
        }
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn final_domain_delivery_records_native_and_auxiliary_without_claiming_model_delivery() {
    let root = std::env::temp_dir().join(format!("legacy_domain_valid_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    for kind in ["earthmesh", "landmesh", "oceanmesh"] {
        for mode in ["tri", "hex"] {
            let plan = prepare(&root.join(kind).join(mode), kind, mode, false);
            let before = fs::read(&plan.source_gridfile).unwrap();
            run(&plan).unwrap_or_else(|e| panic!("{kind}/{mode}: {e}"));
            let record: serde_json::Value = serde_json::from_slice(
                &fs::read(quality_dir(&plan).join("legacy_delivery.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(record["gridfile"], plan.result_gridfile.to_str().unwrap());
            assert_eq!(record["model_delivery_status"], "native_only");
            assert!(record["model_artifacts"].as_object().unwrap().is_empty());
            let expected = match (kind, mode) {
                ("earthmesh", _) | ("oceanmesh", "tri") => 2,
                ("landmesh", _) => 1,
                _ => 0,
            };
            assert_eq!(
                record["auxiliary_artifacts"]
                    .as_object()
                    .map_or(0, |m| m.len()),
                expected
            );
            if let Some(files) = record["auxiliary_artifacts"].as_object() {
                for file in files.values() {
                    assert!(Path::new(file.as_str().unwrap()).is_file());
                }
            }
            assert_eq!(fs::read(&plan.source_gridfile).unwrap(), before);
        }
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn post_admission_auxiliary_failure_preserves_previous_final_bundle() {
    let root = std::env::temp_dir().join(format!("legacy_domain_aux_fail_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let plan = prepare(&root, "earthmesh", "tri", false);
    run(&plan).unwrap();
    let native = fs::read(&plan.result_gridfile).unwrap();
    let patch = plan.patchtype_output.as_ref().unwrap();
    fs::write(patch, b"old patch sentinel").unwrap();
    let patch_bytes = fs::read(patch).unwrap();
    let info = plan.file_dir.join("result/earthmesh_info.nc4");
    let info_bytes = fs::read(&info).unwrap();

    let error = run_final_mask_postproc_earth_domain(&plan, earth_options(&[usize::MAX]), Some(2))
        .unwrap_err()
        .to_string();

    assert!(error.contains("num_mp_step"), "{error}");
    assert_eq!(fs::read(&plan.result_gridfile).unwrap(), native);
    assert_eq!(fs::read(patch).unwrap(), patch_bytes);
    assert_eq!(fs::read(&info).unwrap(), info_bytes);
    assert!(!quality_dir(&plan).join("legacy_delivery.json").exists());
    assert!(quality_dir(&plan).join("quality_summary.json").is_file());
    assert_no_delivery_staging(&root);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn final_domain_success_reports_published_paths_across_output_directories() {
    let root = std::env::temp_dir().join(format!("legacy_domain_crossdir_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let mut plan = prepare(&root.join("case"), "landmesh", "tri", false);
    let source = fs::read(&plan.source_gridfile).unwrap();
    let contain = fs::read(&plan.contain_domain).unwrap();
    plan.result_gridfile = root.join("native").join("final_land.nc4");
    plan.patchtype_output = Some(root.join("aux").join("patchtype_land.nc4"));

    let report = run_final_mask_postproc_land_domain(
        &plan,
        MaskPostprocLandRunOptions {
            seaorland: &[vec![false]],
            minlon_dm_area: 1,
            maxlat_dm_area: 1,
            nlons_dm_select: 1,
            nlats_dm_select: 1,
            lon_vertex: &[0.0, 10.0, 11.0],
            lat_vertex: &[0.0, 10.0, 11.0],
            lon_i: &[0.0, 10.5],
            lat_i: &[0.0, 10.5],
        },
    )
    .unwrap();

    assert_eq!(report.final_gridfile.output, plan.result_gridfile);
    assert_eq!(
        report.patchtype.output,
        plan.patchtype_output.clone().unwrap()
    );
    let record_path = quality_dir(&plan).join("legacy_delivery.json");
    let record_text = fs::read_to_string(&record_path).unwrap();
    assert!(!record_text.contains(".earthmesh-delivery-"));
    let record: serde_json::Value = serde_json::from_str(&record_text).unwrap();
    assert_eq!(record["gridfile"], plan.result_gridfile.to_str().unwrap());
    assert_eq!(
        record["auxiliary_artifacts"]["patchtype"],
        plan.patchtype_output.as_ref().unwrap().to_str().unwrap()
    );
    assert_eq!(fs::read(&plan.source_gridfile).unwrap(), source);
    assert_eq!(fs::read(&plan.contain_domain).unwrap(), contain);
    assert_no_delivery_staging(&root);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn complete_masked_islands_are_allowed_but_not_a_closed_earth_sphere() {
    let root = std::env::temp_dir().join(format!("legacy_domain_islands_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    for kind in ["earthmesh", "landmesh", "oceanmesh"] {
        let plan = prepare(&root.join(kind), kind, "hex", false);
        let mut contain =
            earthmesh_cli::contain_io::read_contain_netcdf(&plan.contain_domain).unwrap();
        contain.is_in_area_ustr[3..].fill(-1);
        earthmesh_cli::contain_io::write_contain_netcdf(&plan.contain_domain, &contain).unwrap();
        run_with_scope(&plan, None).unwrap_or_else(|e| panic!("{kind}: {e}"));
        let report: serde_json::Value = serde_json::from_slice(
            &fs::read(quality_dir(&plan).join("quality_summary.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(report["geometry"]["cell_count"], 1);
        assert!(report["topology"]["boundary_edge_count"].as_u64().unwrap() > 0);
        if kind == "earthmesh" {
            let error = run(&plan).unwrap_err().to_string();
            assert!(error.contains("closed sphere"), "{error}");
            assert!(!quality_dir(&plan).join("legacy_delivery.json").exists());
        }
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn final_domain_delivery_rejects_diagnostic_input_aliases_before_writing() {
    let root = std::env::temp_dir().join(format!(
        "legacy_domain_diagnostic_alias_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);

    for diagnostic in ["quality_summary.json", "legacy_delivery.json"] {
        let mut plan = prepare(&root.join(diagnostic), "landmesh", "tri", false);
        let alias = quality_dir(&plan).join(diagnostic);
        fs::create_dir_all(alias.parent().unwrap()).unwrap();
        fs::copy(&plan.source_gridfile, &alias).unwrap();
        let input_bytes = fs::read(&alias).unwrap();
        plan.source_gridfile = alias.clone();

        let error = run(&plan).unwrap_err().to_string();

        assert!(
            error.contains("input") || error.contains("alias") || error.contains("distinct"),
            "{diagnostic}: {error}"
        );
        assert_eq!(
            fs::read(&alias).unwrap(),
            input_bytes,
            "{diagnostic} changed"
        );
        assert!(!plan.result_gridfile.exists());
        assert_no_delivery_staging(&root);
    }

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn final_domain_delivery_does_not_clobber_inputs_or_keep_success_on_sidecar_failure() {
    let root = std::env::temp_dir().join(format!("legacy_domain_outputs_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let mut plan = prepare(&root, "landmesh", "tri", false);
    run(&plan).unwrap();
    let native = fs::read(&plan.result_gridfile).unwrap();
    plan.patchtype_output = Some(plan.result_gridfile.clone());
    assert!(run(&plan).unwrap_err().to_string().contains("distinct"));
    assert_eq!(fs::read(&plan.result_gridfile).unwrap(), native);
    assert!(!quality_dir(&plan).join("legacy_delivery.json").exists());
    let original = fs::read(&plan.source_gridfile).unwrap();
    let marker = quality_dir(&plan).join("legacy_delivery.json");
    fs::write(&marker, "old success").unwrap();
    plan.patchtype_output = Some(plan.source_gridfile.clone());
    assert!(run(&plan).is_err());
    assert_eq!(fs::read(&plan.source_gridfile).unwrap(), original);
    assert!(!marker.exists());
    plan.patchtype_output = None;
    assert!(run(&plan).unwrap_err().to_string().contains("patchtype"));
    assert!(quality_dir(&plan).join("quality_summary.json").is_file());
    assert!(!quality_dir(&plan).join("legacy_delivery.json").exists());
    fs::remove_dir_all(root).unwrap();
}
