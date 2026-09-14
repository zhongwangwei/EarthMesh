mod support;

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use earthmesh_cli::unstructured_mesh_support::{
    check_unstructured_mesh_topology, UnstructuredMesh,
};
use earthmesh_project::{
    AdaptiveRefinementRecipe, DomainConfig, MeshCellKind, MeshDomainKind, MeshIntentPreset,
    MethodCAlgorithm, ModelFormat, ProjectConfig, ProjectDataLayer, ProjectLayerRole,
    RefinementBackend, RegionShape, ResolutionSpec, ThresholdCriterionConfig, ThresholdField,
    ViolationPolicy,
};

fn root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_adaptive_window_{name}_{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

fn write_lai(path: &Path, inside: bool, outside: bool) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).unwrap();
    file.add_dimension("longitude", 32).unwrap();
    file.add_dimension("latitude", 16).unwrap();
    let mut values = Vec::new();
    for i in 0..32 {
        for j in 0..16 {
            let lon = -180.0 + (i as f64 + 0.5) * 11.25;
            let lat = 90.0 - (j as f64 + 0.5) * 11.25;
            let hit = lat.abs() < 11.25
                && ((inside && lon.abs() < 11.25) || (outside && (-34.0..-22.5).contains(&lon)));
            values.push(if hit { 10.0 } else { 0.0 });
        }
    }
    file.add_variable::<f64>("lai", &["longitude", "latitude"])
        .unwrap()
        .put_values(&values, (.., ..))
        .unwrap();
}

fn project(root: &Path, backend: &str) -> ProjectConfig {
    let source = root.join("lai.nc");
    write_lai(&source, false, false);
    let mut cfg = ProjectConfig::scaffold(
        "window",
        MeshIntentPreset::Custom,
        DomainConfig::Regional {
            shape: RegionShape::Bbox {
                w: -30.0,
                e: 30.0,
                s: -20.0,
                n: 20.0,
            },
            sea_ratio: None,
        },
        ResolutionSpec::Nxp(12),
    );
    cfg.target.kind = MeshDomainKind::Earth;
    cfg.target.cell = MeshCellKind::Tri;
    cfg.target.model_format = ModelFormat::CoLM;
    cfg.data_layers = vec![ProjectDataLayer {
        id: "lai".into(),
        role: ProjectLayerRole::Threshold(ThresholdField::Lai),
        path: source.to_string_lossy().into_owned(),
        enabled: true,
        threshold_value: Some(1.0),
    }];
    cfg.refinement.enabled = true;
    cfg.refinement.threshold_enabled = true;
    cfg.refinement.max_passes = 1;
    // Both backends reject a requested refinement that splits nothing. Keep a
    // hard region outside the evaluation window as the identical control.
    cfg.refinement.specified_bbox = Some(earthmesh_project::SpecifiedBboxRefinement {
        w: 20.0,
        e: 30.0,
        s: -15.0,
        n: 15.0,
    });
    cfg.refinement.backend = if backend == "red_green" {
        RefinementBackend::RedGreen
    } else {
        RefinementBackend::MethodC
    };
    cfg.refinement.method_c.algorithm = if backend == "red_green" {
        MethodCAlgorithm::Canonical
    } else {
        MethodCAlgorithm::LeppDelaunay
    };
    cfg.refinement.method_c.max_cycles = 1;
    cfg.refinement.method_c.maximum_insertions_per_cycle = 64;
    cfg.refinement.method_c.maximum_neighbor_size_ratio = 10.0;
    cfg.refinement.adaptive = Some(AdaptiveRefinementRecipe {
        enabled: true,
        max_level: 1,
        base_m: None,
        coastline: false,
    });
    cfg.refinement.threshold_criteria = vec![
        ThresholdCriterionConfig {
            id: "lai_mean".into(),
            enabled: true,
            value: Some(1.0),
        },
        ThresholdCriterionConfig {
            id: "lai_std".into(),
            enabled: false,
            value: Some(1.0),
        },
    ];
    cfg.quality.on_violation = ViolationPolicy::Warn;
    cfg.expert.niter = Some(1);
    cfg.expert.niter_refine = Some(1);
    cfg.expert.openmp = Some(1);
    cfg
}

fn value<'a>(stdout: &'a str, key: &str) -> &'a str {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix(key))
        .unwrap_or_else(|| panic!("missing {key}: {stdout}"))
}

fn run_raw(
    root: &Path,
    cfg: &ProjectConfig,
    case: &str,
    degree: Option<usize>,
    adaptive: bool,
) -> UnstructuredMesh {
    let mut lowered = cfg.lower();
    lowered.mkgrd.experiment_name = case.into();
    lowered.mkgrd.base_dir = format!("{}/", root.display());
    if let Some(degree) = degree {
        let mask = root.join(format!("{case}-mask.nml"));
        fs::write(
            &mask,
            format!("bbox_num = 1\nbbox_refine = {degree}\n-15 15 15 -15\n"),
        )
        .unwrap();
        lowered.refine.mask_refine_cal_type = "bbox".into();
        lowered.refine.mask_refine_cal_fprefix = mask.to_string_lossy().into_owned();
    }
    if !adaptive {
        lowered.adaptive = None;
    }
    let nml = root.join(format!("{case}.nml"));
    fs::write(&nml, lowered.to_namelist()).unwrap();
    let output = support::output(
        Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
            .current_dir(root)
            .args([
                nml.to_str().unwrap(),
                "--run-refine-passthrough",
                "--max-tris",
                "100000",
                "--quiet",
            ]),
    )
    .unwrap();
    mesh_from_output(root, case, &output)
}

fn mesh_from_output(root: &Path, case: &str, output: &Output) -> UnstructuredMesh {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    fs::write(
        root.join(format!("{case}.log")),
        format!("{stdout}\n{stderr}"),
    )
    .unwrap();
    assert!(output.status.success(), "{case}: {stdout}\n{stderr}");
    let mesh = earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(value(
        &stdout,
        "gridfile=",
    ))
    .unwrap();
    let topology = check_unstructured_mesh_topology(&mesh);
    assert!(
        topology.is_consistent(),
        "{case}: {:?}",
        topology.violations
    );
    mesh
}

#[test]
fn adaptive_zero_degree_masks_are_evaluation_windows_not_hard_demands() {
    for backend in ["red_green", "lepp_delaunay"] {
        let root = root(backend);
        let cfg = project(&root, backend);
        let baseline = run_raw(&root, &cfg, "unscoped", None, true);
        let scoped = run_raw(&root, &cfg, "evaluation", Some(0), true);
        assert!(scoped == baseline,
            "{backend}: zero threshold hits must not refine the evaluation window (baseline {} M rows, scoped {})", baseline.m_points.len(), scoped.m_points.len());
        let hard = run_raw(&root, &cfg, "positive_degree", Some(1), true);
        assert!(
            hard.m_points.len() > baseline.m_points.len(),
            "{backend}: positive degree remains hard demand"
        );
        let legacy = run_raw(&root, &cfg, "legacy_adaptive_off", Some(0), false);
        let legacy_hard = run_raw(&root, &cfg, "legacy_positive", Some(1), false);
        assert!(
            legacy == legacy_hard,
            "{backend}: without a statistical consumer, legacy degree zero still selects max level"
        );
        fs::remove_dir_all(root).unwrap();
    }
}

fn run_project(root: &Path, cfg: &ProjectConfig, case: &str) -> UnstructuredMesh {
    let path = root.join(format!("{case}.yaml"));
    fs::write(&path, cfg.to_yaml().unwrap()).unwrap();
    let output = support::output(
        Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
            .current_dir(root)
            .args(["--project", path.to_str().unwrap(), "--max-tris", "100000"]),
    )
    .unwrap();
    let mesh = mesh_from_output(root, case, &output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let delivered = netcdf::open(value(&stdout, "colm_mesh_input=")).unwrap();
    let nlon = delivered.dimension("nlon").unwrap().len();
    let nlat = delivered.dimension("nlat").unwrap().len();
    assert!(nlon > 0 && nlon <= 360 && nlat > 0 && nlat <= 180);
    let ids = delivered
        .variable("cell_id")
        .unwrap()
        .get_values::<i32, _>(..)
        .unwrap();
    assert!(ids
        .iter()
        .all(|&id| id > 0 && (id as usize) < mesh.m_points.len()));
    let pixels = delivered
        .variable("elmindex")
        .unwrap()
        .get_values::<i32, _>(..)
        .unwrap();
    assert_eq!(pixels.len(), nlon * nlat);
    assert!(pixels.iter().any(|&id| id > 0));
    assert!(pixels.iter().all(|&id| id == 0 || ids.contains(&id)));
    mesh
}

#[test]
fn project_adaptive_windows_filter_hits_without_clipping_hard_regions_or_delivery() {
    for backend in ["red_green", "lepp_delaunay"] {
        let root = root(&format!("project_{backend}"));
        let mut cfg = project(&root, backend);
        cfg.delivery.colm_mesh = Some(earthmesh_project::ColmMeshDeliveryConfig {
            pixels_per_degree: 1,
        });
        cfg.refinement.threshold_region = Some(RegionShape::Bbox {
            w: -15.0,
            e: 15.0,
            s: -15.0,
            n: 15.0,
        });
        cfg.refinement.enabled = false;
        let coarse = run_project(&root, &cfg, "no_refinement");
        cfg.refinement.enabled = true;
        let baseline = run_project(&root, &cfg, "no_hits");
        assert!(
            baseline.m_points.len() > coarse.m_points.len(),
            "{backend}: specified region outside the threshold window must still refine"
        );
        write_lai(&root.join("lai.nc"), false, true);
        let outside = run_project(&root, &cfg, "outside_hits");
        assert!(
            outside == baseline,
            "{backend}: outside hits must not add demand"
        );
        write_lai(&root.join("lai.nc"), true, true);
        let both = run_project(&root, &cfg, "both_hits");
        assert!(
            both.m_points.len() > baseline.m_points.len(),
            "{backend}: inside hits must refine"
        );
        write_lai(&root.join("lai.nc"), true, false);
        let inside = run_project(&root, &cfg, "inside_hits");
        assert!(
            inside == both,
            "{backend}: outside hits must not change scoped mesh"
        );
        // Removing the window makes the western hotspot effective too.
        write_lai(&root.join("lai.nc"), false, true);
        cfg.refinement.threshold_region = None;
        let unscoped = run_project(&root, &cfg, "unscoped_hits");
        assert!(
            unscoped.m_points.len() > baseline.m_points.len(),
            "{backend}: outside fixture must actually trigger refinement when unmasked"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
