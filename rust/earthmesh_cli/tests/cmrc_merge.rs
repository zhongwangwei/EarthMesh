//! CMRC's merge criteria end to end (guide 11.111): synthetic layers around
//! Kunming -- a DEM in two 5-degree tiles, a gentle slope with a rough
//! patch, and a two-class land cover -- read in the domain's window; a face
//! merges only where the relief is gentle and one class covers it.

mod support;

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use earthmesh_project::{
    CertifiedMaterialization, CertifiedMergeCriterion, CertifiedMergeRecipe,
    CertifiedMergeStatistic, CertifiedMode, DomainConfig, MeshCellKind, MeshDomainKind,
    MeshIntentPreset, ModelFormat, ProjectConfig, RefinementBackend, RegionShape, ResolutionSpec,
    ViolationPolicy,
};

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cmrc_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

/// A gentle slope (10 m per degree of longitude) with 30 m of pseudo-random
/// relief within 0.8 degrees of 102.7E 25N.
fn height(lon: f64, lat: f64) -> f32 {
    let slope = 1000.0 + 10.0 * (lon - 102.7);
    let (dlon, dlat) = ((lon - 102.7) * 25.0f64.to_radians().cos(), lat - 25.0);
    if dlon * dlon + dlat * dlat > 0.8 * 0.8 {
        return slope as f32;
    }
    let hash = ((lon * 1.0e4).round() as i64).wrapping_mul(0x9e37_79b9)
        ^ ((lat * 1.0e4).round() as i64).wrapping_mul(0x85eb_ca6b);
    (slope + 30.0 * (hash.rem_euclid(1000) as f64 / 1000.0)) as f32
}

/// `merit/n20e100.nc` and `merit/n25e100.nc` (rows north to south) and
/// `landcover.nc` (class 1 west of 103.5E, 2 east of it), at 0.05 degrees.
fn write_layers(root: &Path) -> (PathBuf, PathBuf) {
    let step = 0.05;
    let merit = root.join("merit");
    fs::create_dir_all(&merit).unwrap();
    for lat0 in [20.0, 25.0] {
        let mut file = netcdf::create(merit.join(format!("n{:02}e100.nc", lat0 as i32))).unwrap();
        let n = (5.0 / step) as usize;
        file.add_dimension("lat", n).unwrap();
        file.add_dimension("lon", n).unwrap();
        let lats = (0..n)
            .map(|j| lat0 + 5.0 - (j as f64 + 0.5) * step)
            .collect::<Vec<_>>();
        let lons = (0..n)
            .map(|i| 100.0 + (i as f64 + 0.5) * step)
            .collect::<Vec<_>>();
        file.add_variable::<f64>("lat", &["lat"])
            .unwrap()
            .put_values(&lats, ..)
            .unwrap();
        file.add_variable::<f64>("lon", &["lon"])
            .unwrap()
            .put_values(&lons, ..)
            .unwrap();
        let values = lats
            .iter()
            .flat_map(|&lat| lons.iter().map(move |&lon| height(lon, lat)))
            .collect::<Vec<_>>();
        let mut elevation = file.add_variable::<f32>("elv", &["lat", "lon"]).unwrap();
        elevation.set_fill_value(-9999.0f32).unwrap();
        elevation.put_values(&values, (.., ..)).unwrap();
    }
    let landcover = root.join("landcover.nc");
    let mut file = netcdf::create(&landcover).unwrap();
    let n = 120usize;
    file.add_dimension("latitude", n).unwrap();
    file.add_dimension("longitude", n).unwrap();
    let lats = (0..n)
        .map(|j| 22.0 + (j as f64 + 0.5) * step)
        .collect::<Vec<_>>();
    let lons = (0..n)
        .map(|i| 100.0 + (i as f64 + 0.5) * step)
        .collect::<Vec<_>>();
    file.add_variable::<f64>("latitude", &["latitude"])
        .unwrap()
        .put_values(&lats, ..)
        .unwrap();
    file.add_variable::<f64>("longitude", &["longitude"])
        .unwrap()
        .put_values(&lons, ..)
        .unwrap();
    let values = lats
        .iter()
        .flat_map(|_| lons.iter().map(|&lon| if lon < 103.5 { 1i32 } else { 2 }))
        .collect::<Vec<_>>();
    file.add_variable::<i32>("landtype", &["latitude", "longitude"])
        .unwrap()
        .put_values(&values, (.., ..))
        .unwrap();
    (merit, landcover)
}

fn preview(source: &Path, cwd: &Path) -> serde_json::Value {
    let output = support::output(
        Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
            .current_dir(cwd)
            .arg("--cmrc-merge-preview")
            .arg(source),
    )
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

/// The faces of the criterion mesh per level, from a preview.
fn criterion_faces(preview: &serde_json::Value) -> Vec<u64> {
    preview["criterion_mesh"]
        .as_array()
        .unwrap()
        .iter()
        .map(|level| level["faces"].as_u64().unwrap())
        .collect()
}

/// The rough patch keeps the finer level, the gentle slope merges to the
/// base, and the certificate says the lattice was the requirement; the
/// preview reads what the run read.
#[test]
fn merge_criteria_drive_a_regional_reverse_coarsening() {
    let root = temp_root("merge_criteria");
    let (merit, landcover) = write_layers(&root);
    let path = root.join("cmrc.nml");
    fs::write(
        &path,
        format!(
            "&mkgrd\n  NL%EXPNME='merge'\n  NL%base_dir='{}/'\n  NL%NXP=120\n  \
             NL%mesh_type='earthmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  \
             NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  \
             NL%refine_backend='certified'\n  NL%mask_domain_global=.false.\n  \
             NL%mask_domain_type='bbox'\n  \
             NL%mask_domain_fprefix='inline:bbox:w=101.5,e=104,s=23.5,n=26.5'\n  \
             NL%landtype_file='none'\n/\n\
             &certified\n  NL%mode='reverse_coarsening'\n  NL%delivery='coupled'\n  \
             NL%maximum_level=2\n  NL%maximum_cells=200000\n  \
             NL%gradation_rings_per_level=3\n  NL%search_budget=100\n  \
             NL%materialization='regional'\n/\n\
             &certified_merge\n  NL%levels=1\n  NL%minimum_samples=4\n  \
             NL%layer_file(1)='{}'\n  NL%layer_variable(1)='elv'\n  \
             NL%statistic(1)='std'\n  NL%threshold(1)=5.\n  \
             NL%layer_file(2)='{}'\n  NL%layer_variable(2)='landtype'\n  \
             NL%statistic(2)='purity'\n  NL%threshold(2)=0.9\n/\n",
            root.display(),
            merit.display(),
            landcover.display()
        ),
    )
    .unwrap();
    let certified = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 200_000, None)
        .unwrap()
        .certified_run
        .unwrap();
    assert_eq!(certified.product_outcome, "certified_adaptive");
    let certificate: serde_json::Value =
        serde_json::from_slice(&fs::read(&certified.certificate).unwrap()).unwrap();
    let layers = &certificate["requirement_layers"];
    assert_eq!(layers["policy"], "lattice_requirement_remains_hard");
    let lattice = &layers["lattice"];
    assert_eq!(lattice["finest_level_required"], 1);
    assert_eq!(lattice["criteria"].as_array().unwrap().len(), 2);
    for layer in lattice["layers"].as_array().unwrap() {
        assert!(layer["samples_read"].as_u64().unwrap() > 1000, "{layer}");
    }
    assert!(certificate["requirement_grid"].is_null());

    // The rough patch's cells keep level 1; cells far from the patch and
    // from the class boundary merged to the base.
    let result_dir = certified.certificate.parent().unwrap().to_path_buf();
    let grid = fs::read_dir(&result_dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .find(|name| name.starts_with("gridfile_") && !name.contains("parent"))
        .unwrap();
    let points =
        earthmesh_cli::grid_quality_pipeline::read_gridfile_mesh_points(result_dir.join(&grid))
            .unwrap();
    let (mut patch, mut far) = (Vec::new(), Vec::new());
    for ((&lon, &lat), &level) in points
        .w_lon
        .iter()
        .zip(&points.w_lat)
        .zip(&points.w_refine_level)
    {
        let (dlon, dlat) = ((lon - 102.7) * 25.0f64.to_radians().cos(), lat - 25.0);
        let apart = (dlon * dlon + dlat * dlat).sqrt();
        if apart < 0.3 {
            patch.push(level);
        } else if apart > 1.4 && (lon - 103.5).abs() > 0.6 {
            far.push(level);
        }
    }
    assert!(
        !patch.is_empty() && patch.iter().all(|&level| level == 1),
        "{patch:?}"
    );
    assert!(far.contains(&0), "{far:?}");

    // The preview is the record the run published, field for field; every
    // finest face lies under one face of its criterion mesh.
    let preview = preview(&path, &root);
    assert_eq!(&preview, lattice);
    assert_eq!(preview["base_nxp"], 120);
    let faces = criterion_faces(&preview);
    assert_eq!(
        faces[0] * 4 + faces[1],
        lattice["base_faces"].as_u64().unwrap() * 4
    );
    assert!(faces[0] > 0 && faces[1] > 0, "{faces:?}");
    fs::remove_dir_all(root).unwrap();
}

/// A Studio project: criteria paths relative to the project file resolve
/// there, not in the working directory; the preview leaves no run directory
/// behind; the project runs to a certified regional mesh.
#[test]
fn a_project_previews_and_runs_its_merge_criteria() {
    let root = temp_root("merge_project");
    write_layers(&root);
    let mut project = ProjectConfig::scaffold(
        "merge",
        MeshIntentPreset::Custom,
        DomainConfig::Regional {
            shape: RegionShape::Bbox {
                w: 101.5,
                e: 104.0,
                s: 23.5,
                n: 26.5,
            },
            sea_ratio: None,
        },
        ResolutionSpec::Nxp(120),
    );
    project.target.kind = MeshDomainKind::Earth;
    project.target.cell = MeshCellKind::Hex;
    project.target.model_format = ModelFormat::CoLM;
    project.data_layers.clear();
    project.quality.on_violation = ViolationPolicy::Warn;
    project.refinement.enabled = true;
    project.refinement.backend = RefinementBackend::Certified;
    project.refinement.certified.mode = CertifiedMode::ReverseCoarsening;
    project.refinement.certified.materialization = CertifiedMaterialization::Regional;
    project.refinement.certified.maximum_cells = 200_000;
    // The base is about 67 km; 33 km is one level below it.
    project.refinement.certified.merge = Some(CertifiedMergeRecipe {
        finest_m: 33_000.0,
        minimum_samples: 4,
        criteria: vec![
            CertifiedMergeCriterion {
                path: "merit".into(),
                variable: "elv".into(),
                statistic: CertifiedMergeStatistic::Std,
                threshold: 5.0,
            },
            CertifiedMergeCriterion {
                path: "landcover.nc".into(),
                variable: "landtype".into(),
                statistic: CertifiedMergeStatistic::Purity,
                threshold: 0.9,
            },
        ],
    });
    let project_path = root.join("project.yaml");
    fs::write(&project_path, project.to_yaml().unwrap()).unwrap();

    let elsewhere = temp_root("merge_project_cwd");
    let preview = preview(&project_path, &elsewhere);
    assert!(!elsewhere.join("run_manifest.json").exists());
    assert_eq!(preview["levels_requested"], 1);
    assert_eq!(preview["finest_level_required"], 1);
    for layer in preview["layers"].as_array().unwrap() {
        let file = PathBuf::from(layer["file"].as_str().unwrap());
        assert!(file.is_absolute() && file.starts_with(&root), "{layer}");
        assert!(layer["samples_read"].as_u64().unwrap() > 1000, "{layer}");
    }
    let mut entries = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    entries.sort();
    assert_eq!(entries, ["landcover.nc", "merit", "project.yaml"]);

    let output = support::output(
        Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
            .current_dir(&elsewhere)
            .arg("--project")
            .arg(&project_path)
            .args(["--max-tris", "200000", "--quiet"]),
    )
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut pending = vec![root.clone()];
    let mut certificates = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path
                .file_name()
                .is_some_and(|name| name == "certified_certificate.json")
            {
                certificates.push(path);
            }
        }
    }
    assert_eq!(certificates.len(), 1, "{certificates:?}");
    let certificate: serde_json::Value =
        serde_json::from_slice(&fs::read(&certificates[0]).unwrap()).unwrap();
    assert_eq!(
        certificate["requirement_layers"]["policy"],
        "lattice_requirement_remains_hard"
    );
    assert_eq!(certificate["product_outcome"], "certified_adaptive");
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(elsewhere).unwrap();
}
