//! Standalone final ownership, separate from raw algorithm/Project carriers.
use crate::project_delivery::LegacyDeliveryStage;
use crate::{MkgrdGridinitRunReport, RefinePipelineRunReport};
use earthmesh_core::{EarthmeshConfig, RefineConfig};
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

#[doc(hidden)]
pub fn run_refine_pipeline_with_delivery(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_tris: usize,
    source_gridnum_perdegree: Option<usize>,
    final_delivery: bool,
) -> io::Result<RefinePipelineRunReport> {
    let namelist = namelist_source.as_ref();
    let workdir = workdir.as_ref();
    if !final_delivery {
        return super::global_source::run_refine_pipeline_namelist(
            namelist,
            workdir,
            max_tris,
            source_gridnum_perdegree,
        );
    }
    let contents = fs::read_to_string(namelist)?;
    let config = EarthmeshConfig::from_mkgrd_namelist(&contents)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let file_dir = crate::workspace_apply::validate_read_nl_workspace_plan(
        &config.read_nl_workspace_plan(None),
        namelist,
        workdir,
    )?;
    let quality_dir = file_dir.join("result/final_quality/refinement");
    let mut inputs = vec![
        namelist.to_path_buf(),
        PathBuf::from(config.mode_file.trim()),
        PathBuf::from(config.landtype_file.trim()),
    ];
    if let Some(path) = std::env::var_os("EARTHMESH_CMRC_LOCAL_UPDATE") {
        inputs.push(PathBuf::from(path));
    }
    // Source discovery failures also retire the last successful attempt. Input
    // bytes remain protected even though no algorithm output exists yet.
    let discovery = refinement_inputs(&contents, &config, &mut inputs);
    inputs.sort();
    inputs.dedup();
    let input_refs = inputs.iter().map(PathBuf::as_path).collect::<Vec<_>>();
    let mut stage = LegacyDeliveryStage::new(&input_refs, &[], &quality_dir)?;
    discovery?;
    let scratch = stage.scratch_dir()?;
    let mut report = super::global_source::run_refine_pipeline_in_workspace(
        namelist,
        workdir,
        max_tris,
        source_gridnum_perdegree,
        Some(&scratch),
    )?;
    let native = report.output.output.clone();
    let parent = report.raw_output.as_ref().map(|raw| raw.output.clone());
    let cartesian = is_cartesian(&contents, &config, &report)?;
    let closed = config.mask_domain_global
        && !(matches!(config.mesh_type.trim(), "landmesh" | "oceanmesh")
            && report.landtype_masked_cells.is_some());
    let mut checks = vec![(native.clone(), closed)];
    if let Some(coupled) = &report.coupled_outputs {
        checks.extend([
            (coupled.land_output.output.clone(), false),
            (coupled.ocean_output.output.clone(), false),
        ]);
    }
    if let Some(lepp) = &report.lepp_post_quality {
        checks.push((lepp.output.output.clone(), closed));
        if let Some(coupled) = &lepp.coupled_outputs {
            checks.extend([
                (coupled.land_output.output.clone(), false),
                (coupled.ocean_output.output.clone(), false),
            ]);
        }
    }
    let mut models = BTreeMap::new();
    if let Some(certified) = &report.certified_run {
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&certified.manifest)?).map_err(io::Error::other)?;
        for (field, role) in [
            ("fvcom_2dm", "fvcom_2dm"),
            ("mpas", "mpas_mesh_input"),
            ("mpas_graph_info", "mpas_graph_info"),
        ] {
            if let Some(path) = manifest[field].as_str() {
                models.insert(role, PathBuf::from(path));
            }
        }
    }
    let mut files = BTreeMap::new();
    collect_report(&mut report, &scratch, &file_dir, &mut files)?;
    for name in [
        "obc.nc4",
        "obcv2.nc4",
        crate::refinement_demand::nest::ADAPTIVE_REFINEMENT_FILE,
    ] {
        let mut path = scratch.join("result").join(name);
        if path.is_file() {
            collect_path(&mut path, &scratch, &file_dir, &mut files)?;
        }
    }
    for path in models.values_mut() {
        collect_path(path, &scratch, &file_dir, &mut files)?;
    }
    let output_refs = files.values().map(PathBuf::as_path).collect::<Vec<_>>();
    stage.add_outputs(&input_refs, &output_refs)?;
    let cell_kind = if config.mode_grid.trim() == "tri" {
        earthmesh_project::MeshCellKind::Tri
    } else {
        earthmesh_project::MeshCellKind::Hex
    };
    let mut quality = None;
    let mut skipped_reason = None;
    if !cartesian {
        for (index, (source, full_sphere)) in checks.iter().enumerate() {
            let diagnostics = if index == 0 {
                quality_dir.clone()
            } else {
                scratch.join("checked_auxiliary").join(index.to_string())
            };
            let result = crate::project_quality::admit_staged_final_gridfile(
                &crate::project_quality::FinalAdmissionSpec {
                    cell_kind,
                    expected_euler_characteristic: full_sphere.then_some(2),
                    thresholds: earthmesh_quality::QualityThresholds::default(),
                    repair_level_cap: None,
                },
                source,
                &files[source],
                &diagnostics,
            )
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            if index == 0 {
                quality = Some(result);
            }
        }
        if models.is_empty() {
            let (generated, reason) = super::model_delivery::write_available_models(
                &config,
                &native,
                parent.as_deref(),
                quality.as_ref().unwrap(),
            )?;
            skipped_reason = reason;
            for (role, mut path) in generated {
                collect_path(&mut path, &scratch, &file_dir, &mut files)?;
                stage.add_outputs(&input_refs, &[&path])?;
                models.insert(role, path);
            }
        }
    }
    // References inside producer manifests/reports must survive scratch cleanup.
    // Only exact current-attempt artifact paths are rewritten, never input paths.
    for (source, published) in &files {
        let staged = stage.path(published)?;
        if source.extension().is_some_and(|ext| ext == "json") {
            let mut document: serde_json::Value =
                serde_json::from_slice(&fs::read(source)?).map_err(io::Error::other)?;
            remap_document(&mut document, &files);
            fs::write(
                staged,
                serde_json::to_vec_pretty(&document).map_err(io::Error::other)?,
            )?;
        } else {
            fs::copy(source, staged)?;
        }
    }
    if let Some(certified) = &report.certified_run {
        // Remapped JSON paths change byte lengths. Describe the staged final
        // artifacts, not the producer's pre-remap manifest/certificate sizes.
        let resources = stage.path(&certified.resources)?;
        let mut document: serde_json::Value =
            serde_json::from_slice(&fs::read(&resources)?).map_err(io::Error::other)?;
        for (role, path) in [
            ("gridfile", Some(&report.output.output)),
            (
                "remap",
                certified
                    .remap
                    .as_ref()
                    .or(certified.pre_export_remap.as_ref()),
            ),
            ("certificate", Some(&certified.certificate)),
            ("manifest", Some(&certified.manifest)),
            (
                "fvcom_2dm",
                models.get("fvcom_2dm").or(models.get("fvcom_mesh_input")),
            ),
            ("mpas", models.get("mpas_mesh_input")),
            ("mpas_graph_info", models.get("mpas_graph_info")),
        ] {
            if let Some(path) = path {
                document["artifact_bytes"][role] =
                    serde_json::json!(fs::metadata(stage.path(path)?)?.len());
            }
        }
        fs::write(
            resources,
            serde_json::to_vec_pretty(&document).map_err(io::Error::other)?,
        )?;
    }
    if cartesian {
        // Keep the existing XY carrier usable without mislabelling metres as
        // lon/lat or inventing a spherical certificate/model-ready marker.
        // The shared rollback helper withdraws its last artifact first and
        // restores it last. Use the selected native as that data-only boundary,
        // as the existing graph-first/mesh-last writers do; it is not readiness.
        let mut outputs = files.values().collect::<Vec<_>>();
        outputs.sort_by_key(|path| **path == report.output.output);
        let staged = outputs
            .iter()
            .map(|path| stage.path(path))
            .collect::<io::Result<Vec<_>>>()?;
        let publications = staged
            .iter()
            .zip(outputs)
            .map(|(a, b)| (a.as_path(), b.as_path()))
            .collect::<Vec<_>>();
        crate::atomic_output::publish_artifacts(&publications, &[])?;
        eprintln!("earthmesh_cli: Cartesian-XY carrier published transactionally; spherical final admission is not applicable and no final-delivery readiness is claimed");
    } else {
        let auxiliary = files
            .values()
            .filter(|path| {
                **path != report.output.output && !models.values().any(|model| model == *path)
            })
            .map(|path| {
                (
                    path.strip_prefix(&file_dir)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    path.clone(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let auxiliary_refs = auxiliary
            .iter()
            .map(|(key, path)| (key.as_str(), path.clone()))
            .collect();
        stage.publish(serde_json::json!({
            "kind":"earthmesh_legacy_delivery", "target":{"cell":cell_kind},
            "scope":if closed { "closed_sphere" } else { "regional_or_masked" },
            "capability":if models.is_empty() { "native_and_auxiliary" } else { "full" },
            "source_mesh_type":config.mesh_type, "source_mode_grid":config.mode_grid,
            "refine_backend":config.refine_backend, "requested_output_format":config.output_format,
            "patch_preprocessing_only":config.mask_patch_on,
            "skipped_reason":skipped_reason,
        }), &report.output.output, quality.unwrap().verdict, &models, &auxiliary_refs)?;
    }
    Ok(report)
}

fn refinement_inputs(
    contents: &str,
    config: &EarthmeshConfig,
    inputs: &mut Vec<PathBuf>,
) -> io::Result<()> {
    if let Some(path) = std::env::var_os("EARTHMESH_CMRC_LOCAL_UPDATE") {
        inputs.extend(super::cmrc_local_updates::input_paths(Path::new(&path))?);
    }
    let mut prefixes = Vec::new();
    if !config.mask_domain_global {
        prefixes.push(config.mask_domain_fprefix.as_str());
    }
    if config.mask_patch_on {
        prefixes.push(config.mask_patch_fprefix.as_str());
    }
    let refine = RefineConfig::from_mkrefine_namelist_with_external_field(
        contents,
        config.mesh_type.trim(),
        config.mode_grid.trim(),
        true,
    )
    .ok();
    if let Some(refine) = &refine {
        if refine.refine_spc {
            prefixes.push(&refine.mask_refine_spc_fprefix);
        }
        if refine.refine_cal {
            prefixes.push(&refine.mask_refine_cal_fprefix);
        }
        for field in crate::area_judge_threshold_inputs::enabled_mean_threshold_field_specs(
            refine,
            config.mesh_type.trim(),
        )
        .into_iter()
        .chain(
            crate::area_judge_threshold_inputs::enabled_std_threshold_field_specs(
                refine,
                config.mesh_type.trim(),
            ),
        ) {
            inputs.push(
                Path::new(refine.threshold_dir.trim()).join(format!("{}.nc", field.file_stem)),
            );
        }
    }
    for prefix in prefixes {
        let prefix = prefix.trim();
        if !matches!(prefix, "" | "none" | "/tmp") && !prefix.starts_with("inline:") {
            inputs.extend(crate::discover_mask_sources(prefix)?.files);
        }
    }
    if let Some(options) = crate::hfield_refine::read_hfield_refine_options(contents)? {
        if let Some((cells, levels)) = options.hydro_target_paths() {
            inputs.extend([cells.to_path_buf(), levels.to_path_buf()]);
        }
    }
    if config.coupling_identify_river_mouth {
        for name in [
            "params.txt",
            "uparea.bin",
            "rivlen.bin",
            "nextxy.bin",
            "rivwth.bin",
            "width.bin",
        ] {
            inputs.push(Path::new(config.coupling_cama_root.trim()).join(name));
        }
    }
    Ok(())
}

fn is_cartesian(
    contents: &str,
    config: &EarthmeshConfig,
    report: &RefinePipelineRunReport,
) -> io::Result<bool> {
    if report.certified_run.is_some() {
        return Ok(false);
    }
    let mdomain = crate::read_native_grid_mdomain(contents)?;
    let global_like = mdomain.map_or(config.mask_domain_global, |value| value < 2);
    let regions = crate::read_native_grid_refinement_regions(
        contents,
        matches!(config.mesh_type.trim(), "atmos" | "atmosmesh"),
        global_like,
    )?;
    let native_only = !regions.is_empty() && !report.refine.refine_spc && !report.refine.refine_cal;
    Ok(
        crate::native_spawn_uses_cartesian_xy(mdomain, config.mask_domain_global, native_only)
            || mdomain == Some(5),
    )
}

fn collect_path(
    path: &mut PathBuf,
    scratch: &Path,
    destination: &Path,
    files: &mut BTreeMap<PathBuf, PathBuf>,
) -> io::Result<()> {
    if let Some(published) = files.get(path) {
        *path = published.clone();
        return Ok(());
    }
    let suffix = path.strip_prefix(scratch).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "refined output escaped private workspace: {}",
                path.display()
            ),
        )
    })?;
    let published = destination.join(suffix);
    files.insert(path.clone(), published.clone());
    *path = published;
    Ok(())
}

fn collect_coupled(
    coupled: &mut crate::mkgrd_run_types::RefineCoupledOutputReport,
    scratch: &Path,
    destination: &Path,
    files: &mut BTreeMap<PathBuf, PathBuf>,
) -> io::Result<()> {
    for path in [
        &mut coupled.land_output.output,
        &mut coupled.ocean_output.output,
        &mut coupled.coupling_csv,
        &mut coupled.coupling_netcdf.output,
        &mut coupled.coupling_quality,
        &mut coupled.manifest,
    ] {
        collect_path(path, scratch, destination, files)?;
    }
    Ok(())
}

fn collect_report(
    report: &mut RefinePipelineRunReport,
    scratch: &Path,
    destination: &Path,
    files: &mut BTreeMap<PathBuf, PathBuf>,
) -> io::Result<()> {
    if let Some(gridinit) = &mut report.gridinit {
        // An unchecked refinement carrier must not replace a certified global base.
        let source = gridinit.gridfile.output.clone();
        let published = destination.join("tmpfile").join(format!(
            "refine_source_{}",
            source.file_name().unwrap().to_string_lossy()
        ));
        files.insert(source, published.clone());
        gridinit.gridfile.output = published;
        if let Some(raw) = &mut gridinit.raw_output {
            collect_path(&mut raw.output, scratch, destination, files)?;
        }
        if let Some(path) = &mut gridinit.workspace_mask.workspace.copied_namelist_to {
            collect_path(path, scratch, destination, files)?;
        }
        // The private scaffold is discarded; report only actual published files.
        gridinit
            .workspace_mask
            .workspace
            .created_directories
            .clear();
        for mask in &mut gridinit.workspace_mask.mask_reports {
            for path in &mut mask.outputs {
                collect_path(path, scratch, destination, files)?;
            }
        }
    }
    collect_path(&mut report.output.output, scratch, destination, files)?;
    if let Some(raw) = &mut report.raw_output {
        collect_path(&mut raw.output, scratch, destination, files)?;
    }
    if let Some(coupled) = &mut report.coupled_outputs {
        collect_coupled(coupled, scratch, destination, files)?;
    }
    if let Some(certified) = &mut report.certified_run {
        for path in [
            &mut certified.certificate,
            &mut certified.manifest,
            &mut certified.resources,
            &mut certified.ready_marker,
        ] {
            collect_path(path, scratch, destination, files)?;
        }
        for path in [&mut certified.remap, &mut certified.pre_export_remap]
            .into_iter()
            .flatten()
        {
            collect_path(path, scratch, destination, files)?;
        }
    }
    if let Some(lepp) = &mut report.lepp_adaptive_hybrid {
        for path in [&mut lepp.report, &mut lepp.unresolved_report] {
            collect_path(path, scratch, destination, files)?;
        }
    }
    if let Some(lepp) = &mut report.lepp_post_quality {
        for path in [&mut lepp.report, &mut lepp.output.output] {
            collect_path(path, scratch, destination, files)?;
        }
        if let Some(raw) = &mut lepp.raw_output {
            collect_path(&mut raw.output, scratch, destination, files)?;
        }
        if let Some(coupled) = &mut lepp.coupled_outputs {
            collect_coupled(coupled, scratch, destination, files)?;
        }
    }
    Ok(())
}

fn remap_document(document: &mut serde_json::Value, files: &BTreeMap<PathBuf, PathBuf>) {
    match document {
        serde_json::Value::String(value) => {
            if let Some(path) = files.get(Path::new(value)) {
                *value = path.to_string_lossy().into_owned();
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                remap_document(value, files);
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values_mut() {
                remap_document(value, files);
            }
        }
        _ => {}
    }
}

pub(super) fn generate_carrier(
    namelist: &Path,
    workdir: &Path,
    max_tris: usize,
    config: &EarthmeshConfig,
    output_dir: &Path,
) -> io::Result<MkgrdGridinitRunReport> {
    let mut plan = config.read_nl_workspace_plan(None);
    let original = plan.file_dir.clone();
    for directory in &mut plan.directories_to_create {
        let suffix = Path::new(directory)
            .strip_prefix(&original)
            .map_err(io::Error::other)?;
        *directory = output_dir.join(suffix).to_string_lossy().into_owned();
    }
    plan.file_dir = output_dir.to_string_lossy().into_owned();
    plan.namelist_save_path = output_dir
        .join("result/namelist.save")
        .to_string_lossy()
        .into_owned();
    plan.remove_existing_file_dir = false;
    plan.remove_filelists = false;
    plan.mask_operations
        .retain(|op| !op.mask_fprefix.trim().starts_with("inline:"));
    let workspace_mask =
        crate::apply_workspace_and_mask_operations(&plan, namelist, workdir, 9, false)?;
    let (gridfile, runtime_state) =
        crate::mkgrd_gridinit_driver::generate_gridinit_carrier(config, output_dir, max_tris)?;
    Ok(MkgrdGridinitRunReport {
        config: config.clone(),
        runtime_state,
        workspace_mask,
        raw_output: None,
        gridfile,
        fvcom_2dm: None,
    })
}
