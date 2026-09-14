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
            if kind == "oceanmesh" && mode == "tri" {
                assert!(earthmesh_cli::obc_boundary_io::read_gridfile_obc_order(
                    &plan.result_gridfile
                )
                .unwrap()
                .is_some());
            }
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
    plan.patchtype_output = Some(plan.source_gridfile.clone());
    assert!(run(&plan).is_err());
    assert_eq!(fs::read(&plan.source_gridfile).unwrap(), original);
    plan.patchtype_output = None;
    assert!(run(&plan).unwrap_err().to_string().contains("patchtype"));
    assert!(quality_dir(&plan).join("quality_summary.json").is_file());
    assert!(!quality_dir(&plan).join("legacy_delivery.json").exists());
    fs::remove_dir_all(root).unwrap();
}
