mod support;

use earthmesh_cli::project_quality::admit_project_final_gridfile as admit;
use earthmesh_project::{
    DomainConfig, MeshCellKind, MeshDomainKind, MeshIntentPreset, ProjectConfig, RefinementBackend,
    RegionShape, ResolutionSpec, ViolationPolicy,
};
use std::{fs, path::Path};

fn project(kind: MeshCellKind) -> ProjectConfig {
    let mut p = ProjectConfig::scaffold(
        "final_admission",
        MeshIntentPreset::Custom,
        DomainConfig::Regional {
            shape: RegionShape::Bbox {
                w: 90.0,
                e: 120.0,
                s: 10.0,
                n: 40.0,
            },
            sea_ratio: Some(0.5),
        },
        ResolutionSpec::Nxp(10),
    );
    p.target.cell = kind;
    p.target.kind = MeshDomainKind::Earth;
    p.quality.on_violation = ViolationPolicy::Warn;
    p
}

// Deliberately independent physical-cell views: TRI delivery must not audit
// auxiliary boundary W fans as if they were delivered HEX polygons.
fn grid(path: &Path, kind: MeshCellKind, vertices: &[(f64, f64)], cells: &[Vec<usize>]) {
    let mut vertex_points = vec![(0., 0.); 2];
    vertex_points.extend_from_slice(vertices);
    let mut centers = vec![(0., 0.); 2];
    centers.extend(cells.iter().map(|c| {
        (
            c.iter().map(|&i| vertices[i].0).sum::<f64>() / c.len() as f64,
            c.iter().map(|&i| vertices[i].1).sum::<f64>() / c.len() as f64,
        )
    }));
    let (m, w) = if kind == MeshCellKind::Hex {
        (vertex_points, centers)
    } else {
        (centers, vertex_points)
    };
    let width = cells.iter().map(Vec::len).max().unwrap().max(3);
    let mut mtow = vec![1_i32; m.len() * 3];
    if kind == MeshCellKind::Hex {
        mtow[6..].fill(2);
    }
    let mut wtom = vec![1_i32; w.len() * width];
    let mut counts = vec![1_i32; w.len()];
    for (i, cell) in cells.iter().enumerate() {
        if kind == MeshCellKind::Hex {
            counts[i + 2] = cell.len() as i32;
            for (j, &v) in cell.iter().enumerate() {
                wtom[(i + 2) * width + j] = (v + 2) as i32;
            }
        } else {
            assert_eq!(cell.len(), 3);
            for (j, &v) in cell.iter().enumerate() {
                mtow[(i + 2) * 3 + j] = (v + 2) as i32;
            }
        }
    }
    let mut f = earthmesh_cli::create_netcdf_quiet(path).unwrap();
    for (name, len) in [
        ("sjx_points", m.len()),
        ("lbx_points", w.len()),
        ("dimb", 3),
        ("dimc", width),
    ] {
        f.add_dimension(name, len).unwrap();
    }
    for (name, dim, values) in [
        (
            "GLONM",
            "sjx_points",
            m.iter().map(|p| p.0).collect::<Vec<_>>(),
        ),
        ("GLATM", "sjx_points", m.iter().map(|p| p.1).collect()),
        ("GLONW", "lbx_points", w.iter().map(|p| p.0).collect()),
        ("GLATW", "lbx_points", w.iter().map(|p| p.1).collect()),
    ] {
        f.add_variable::<f64>(name, &[dim])
            .unwrap()
            .put_values(&values, ..)
            .unwrap();
    }
    f.add_variable::<i32>("itab_m%iw", &["sjx_points", "dimb"])
        .unwrap()
        .put_values(&mtow, ..)
        .unwrap();
    f.add_variable::<i32>("itab_w%im", &["lbx_points", "dimc"])
        .unwrap()
        .put_values(&wtom, ..)
        .unwrap();
    f.add_variable::<i32>("n_ngrwm", &["lbx_points"])
        .unwrap()
        .put_values(&counts, ..)
        .unwrap();
    f.close().unwrap();
}

#[test]
fn final_admission_uses_native_cells_scope_and_policy_not_backend_or_intent() {
    let root = std::env::temp_dir().join(format!("project_final_admission_{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let file = root.join("final.nc4");
    let out = root.join("quality");
    let vertices = [
        (100., 20.),
        (100., 22.),
        (102., 23.),
        (104., 22.),
        (104., 20.),
        (102., 19.),
        (106., 23.),
        (108., 22.),
        (108., 20.),
        (106., 19.),
    ];
    let cells = vec![vec![0, 1, 2, 3, 4, 5], vec![4, 3, 6, 7, 8, 9]];
    grid(&file, MeshCellKind::Hex, &vertices, &cells);
    let mut p = project(MeshCellKind::Hex);
    let good = admit(&p, &file, &out, None).unwrap();
    assert_eq!(good.topology.boundary_loop_count, 1);
    assert_eq!(good.topology.misoriented_shared_edge_count, 0);
    let before = fs::read(&file).unwrap();
    for backend in [
        RefinementBackend::Certified,
        RefinementBackend::MethodC,
        RefinementBackend::RedGreen,
    ] {
        p.refinement.backend = backend;
        for kind in [
            MeshDomainKind::Land,
            MeshDomainKind::Ocean,
            MeshDomainKind::Atmosphere,
        ] {
            p.target.kind = kind;
            assert!(admit(&p, &file, &out, None).is_ok());
        }
    }
    assert_eq!(before, fs::read(&file).unwrap());
    let reversed = cells
        .iter()
        .map(|c| c.iter().copied().rev().collect())
        .collect::<Vec<Vec<usize>>>();
    grid(&file, MeshCellKind::Hex, &vertices, &reversed);
    assert!(admit(&p, &file, &out, None).is_ok());
    // Two disconnected disks have Euler 2, but are not a closed global sphere.
    let mut islands = vertices.to_vec();
    islands.extend(vertices.iter().map(|&(x, y)| (x + 15., y)));
    let mut island_cells = cells.clone();
    island_cells.extend(
        cells
            .iter()
            .map(|c| c.iter().map(|v| v + vertices.len()).collect::<Vec<_>>()),
    );
    grid(&file, MeshCellKind::Hex, &islands, &island_cells);
    let islands_report = admit(&p, &file, &out, None).unwrap();
    assert_eq!(islands_report.topology.euler_characteristic, 2);
    assert_eq!(islands_report.topology.connected_component_count, 2);

    p.quality.on_violation = ViolationPolicy::Block;
    assert!(
        admit(&p, &file, &out, None).is_ok(),
        "valid regional components are not a topology failure"
    );
    p.quality.on_violation = ViolationPolicy::Warn;
    p.domain = DomainConfig::Global;
    let err = admit(&p, &file, &out, None).unwrap_err();
    assert!(err.contains("closed sphere"), "{err}");
    p = project(MeshCellKind::Hex);
    let mut broken = cells.clone();
    broken[1].reverse();
    grid(&file, MeshCellKind::Hex, &vertices, &broken);
    let err = admit(&p, &file, &out, None).unwrap_err();
    assert!(err.contains("misoriented_shared_edge"), "{err}");
    assert!(out.join("quality_summary.json").is_file());
    broken.iter_mut().for_each(|c| c.reverse());
    grid(&file, MeshCellKind::Hex, &vertices, &broken);
    assert!(admit(&p, &file, &out, None)
        .unwrap_err()
        .contains("misoriented_shared_edge"));
    // A crossed native ring cannot be repaired by sorting its corners.
    let mut crossed = cells.clone();
    crossed[0].swap(1, 3);
    grid(&file, MeshCellKind::Hex, &vertices, &crossed);
    assert!(admit(&p, &file, &out, None).is_err());
    grid(&file, MeshCellKind::Hex, &vertices, &[vec![0, 1, 3, 4]]);
    assert!(admit(&p, &file, &out, None).unwrap_err().contains("5..=7"));
    grid(&file, MeshCellKind::Hex, &vertices, &[cells[0].clone()]);
    p.quality.on_violation = ViolationPolicy::Block;
    let island = admit(&p, &file, &out, None).unwrap();
    assert_eq!(island.topology.orphan_cell_count, 1);
    assert_ne!(island.verdict, earthmesh_quality::QualityLevel::Fail);
    p.quality.on_violation = ViolationPolicy::Warn;
    // Two connected M triangles are valid; their auxiliary W degrees are irrelevant.
    p.target.cell = MeshCellKind::Tri;
    grid(
        &file,
        MeshCellKind::Tri,
        &vertices,
        &[vec![0, 1, 3], vec![0, 3, 4]],
    );
    assert!(admit(&p, &file, &out, None).is_ok());
    grid(
        &file,
        MeshCellKind::Tri,
        &vertices,
        &[vec![3, 1, 0], vec![4, 3, 0]],
    );
    assert!(admit(&p, &file, &out, None).is_ok());
    grid(
        &file,
        MeshCellKind::Tri,
        &vertices,
        &[vec![3, 1, 0], vec![0, 3, 4]],
    );
    assert!(admit(&p, &file, &out, None)
        .unwrap_err()
        .contains("misoriented_shared_edge"));
    let skinny = [(100., 20.), (100., 20.001), (104., 20.001), (104., 20.)];
    grid(
        &file,
        MeshCellKind::Tri,
        &skinny,
        &[vec![0, 1, 2], vec![0, 2, 3]],
    );
    let numeric_fail = admit(&p, &file, &out, None).unwrap();
    assert_eq!(numeric_fail.verdict, earthmesh_quality::QualityLevel::Fail);
    assert_eq!(
        numeric_fail
            .gates
            .iter()
            .find(|g| g.metric == "final_mesh_admission")
            .unwrap()
            .level,
        earthmesh_quality::QualityLevel::Pass
    );
    for policy in [ViolationPolicy::Block, ViolationPolicy::AutoRefine] {
        p.quality.on_violation = policy;
        assert!(admit(&p, &file, &out, None)
            .unwrap_err()
            .contains("project quality gate failed"));
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_cli_backends_admit_the_selected_mesh_before_configured_model_delivery() {
    use earthmesh_project::{
        ColmMeshDeliveryConfig, MethodCAlgorithm, ModelFormat, SpecifiedCircleRefinement,
        SpecifiedCircleRefinements,
    };
    for (name, backend, algorithm) in [
        (
            "cmrc",
            RefinementBackend::Certified,
            MethodCAlgorithm::Canonical,
        ),
        (
            "method_c",
            RefinementBackend::MethodC,
            MethodCAlgorithm::Canonical,
        ),
        (
            "redgreen",
            RefinementBackend::RedGreen,
            MethodCAlgorithm::Canonical,
        ),
        (
            "lepp",
            RefinementBackend::MethodC,
            MethodCAlgorithm::LeppDelaunay,
        ),
    ] {
        let root =
            std::env::temp_dir().join(format!("project_final_cli_{name}_{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let mut p = project(MeshCellKind::Tri);
        p.domain = DomainConfig::Global;
        p.target.model_format = ModelFormat::CoLM;
        p.refinement.enabled = true;
        p.refinement.max_passes = 1;
        p.refinement.backend = backend;
        p.refinement.method_c.algorithm = algorithm;
        p.refinement.method_c.max_cycles = 1;
        p.refinement.method_c.maximum_insertions_per_cycle = 2;
        p.refinement.specified_circle =
            Some(SpecifiedCircleRefinements::One(SpecifiedCircleRefinement {
                lon: 110.,
                lat: 20.,
                radius_km: 500.,
            }));
        p.expert.niter = Some(1);
        p.expert.niter_refine = Some(1);
        p.delivery.colm_mesh = Some(ColmMeshDeliveryConfig {
            pixels_per_degree: 1,
        });
        let path = root.join("project.yaml");
        fs::write(&path, p.to_yaml().unwrap()).unwrap();
        let result = support::output(
            std::process::Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
                .current_dir(&root)
                .args([
                    "--project",
                    path.to_str().unwrap(),
                    "--max-tris",
                    "100000",
                    "--quiet",
                ]),
        )
        .unwrap();
        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(result.status.success(), "{name}\n{stdout}\n{stderr}");
        let field = |key: &str| {
            stdout
                .lines()
                .find_map(|l| l.strip_prefix(key))
                .unwrap()
                .to_string()
        };
        let report_path = field("project_final_quality=");
        let report: serde_json::Value =
            serde_json::from_slice(&fs::read(&report_path).unwrap()).unwrap();
        assert_eq!(report["cell_view"], "tri");
        assert_eq!(report["topology"]["euler_characteristic"], 2);
        assert_eq!(report["topology"]["boundary_edge_count"], 0);
        let gridfile = Path::new(report["mesh_name"].as_str().unwrap());
        let delivered = field("colm_mesh_input=");
        assert!(Path::new(&delivered).is_file());
        assert_eq!(
            Path::new(&delivered).parent().unwrap().parent(),
            gridfile.parent()
        );
        assert!(
            stdout.find("project_final_quality=").unwrap()
                < stdout.find("colm_mesh_input=").unwrap()
        );
        // Same backend and physical demand, only model-format configuration
        // changes. Closed global TRI does not require an OBC sidecar.
        p.target.model_format = ModelFormat::Fvcom;
        p.delivery.colm_mesh = None;
        fs::write(&path, p.to_yaml().unwrap()).unwrap();
        let fvcom = support::output(
            std::process::Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
                .current_dir(&root)
                .args([
                    "--project",
                    path.to_str().unwrap(),
                    "--max-tris",
                    "100000",
                    "--quiet",
                ]),
        )
        .unwrap();
        let fvcom_stdout = String::from_utf8_lossy(&fvcom.stdout);
        assert!(
            fvcom.status.success(),
            "{name} FVCOM\n{fvcom_stdout}\n{}",
            String::from_utf8_lossy(&fvcom.stderr)
        );
        let delivered = fvcom_stdout
            .lines()
            .find_map(|l| l.strip_prefix("fvcom_mesh_input="))
            .expect("selected final FVCOM artifact");
        let mesh_text = fs::read_to_string(delivered).unwrap();
        assert_eq!(
            mesh_text.lines().filter(|l| l.starts_with("E3T ")).count(),
            report["geometry"]["cell_count"].as_u64().unwrap() as usize
        );
        assert!(!mesh_text.lines().any(|l| l.starts_with("NS ")));
        assert!(
            fvcom_stdout.find("project_final_quality=").unwrap()
                < fvcom_stdout.find("fvcom_mesh_input=").unwrap()
        );
        fs::remove_dir_all(root).unwrap();
    }
}
