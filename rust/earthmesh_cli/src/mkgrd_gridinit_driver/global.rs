use crate::apply_workspace_and_mask_operations;
use crate::convert_fvcom_mode_file_to_earthmesh;
use crate::convert_iap_ocean_mode_file_to_earthmesh;
use crate::convert_mpas_mode_file_to_earthmesh;
use crate::copy_existing_earthmesh_mode_file;
use crate::earthmesh_runtime_state_from_compact_mesh;
use crate::read_unstructured_mesh_netcdf;
use crate::MkgrdGridinitRunReport;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use earthmesh_core::{EarthmeshConfig, EarthmeshRuntimeState};

/// Run the Rust replacement path for the initial global `mkgrd.x` gridinit branch.
///
/// This mirrors the branch where `mode_grid` is `hex`/`tri` and `mode_file` does
/// not exist: parse the mkgrd namelist, apply the read_nl workspace/mask plan,
/// generate the in-memory global grid, and write
/// `gridfile/gridfile_NXP####_01_<mode_grid>.nc4`. Existing EarthMesh, MPAS,
/// FVCOM and IAP-Ocean mode files use the same import adapters. This is a raw
/// carrier API: final admission belongs to the selected delivery handoff.
pub fn run_mkgrd_gridinit_global_namelist(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_tris: usize,
) -> io::Result<MkgrdGridinitRunReport> {
    run_mkgrd_gridinit_global(namelist_source.as_ref(), workdir.as_ref(), max_tris, false)
}

/// Only the standalone, unmasked global base handoff sets `final_delivery`.
/// Project, regional extraction and refinement retain their unchecked carriers.
pub(crate) fn run_mkgrd_gridinit_global(
    namelist_source: &Path,
    workdir: &Path,
    max_tris: usize,
    final_delivery: bool,
) -> io::Result<MkgrdGridinitRunReport> {
    let contents = fs::read_to_string(namelist_source)?;
    let config = EarthmeshConfig::from_mkgrd_namelist(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

    if config.mask_restart {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "mask_restart mkgrd branch is not yet current to Rust",
        ));
    }
    if !matches!(config.mode_grid.as_str(), "hex" | "tri") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mode_grid {} is not supported by the gridinit branch",
                config.mode_grid
            ),
        ));
    }
    if config.nxp <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NXP must be positive for gridinit",
        ));
    }
    if config.niter < 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "niter must be non-negative for gridinit",
        ));
    }
    let nxp = usize::try_from(config.nxp)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NXP must fit usize"))?;
    let niter = usize::try_from(config.niter)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "niter must fit usize"))?;

    let mut plan = config.read_nl_workspace_plan(None);
    // Inline Project geometry is consumed by the Method-C region adapters and
    // subsequent regional clip; it is not a file prefix for legacy Mask_make.
    plan.mask_operations
        .retain(|operation| !operation.mask_fprefix.trim().starts_with("inline:"));
    let mode_file = PathBuf::from(config.mode_file.trim());
    let mut output_dir = PathBuf::from(config.file_dir());
    let mut delivery = None;
    if final_delivery {
        // This branch publishes the full sphere, not the masked/refined carrier.
        if !config.mask_domain_global || config.mask_patch_on {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "final base delivery requires an unmasked global grid",
            ));
        }
        output_dir = crate::workspace_apply::validate_read_nl_workspace_plan(
            &plan,
            namelist_source,
            workdir,
        )?;
        let published = crate::gridfile_output_path(&output_dir, nxp, 1, &config.mode_grid);
        let quality_dir = published
            .parent()
            .unwrap()
            .join("final_quality")
            .join(published.file_stem().unwrap());
        let stage = crate::project_delivery::LegacyDeliveryStage::new(
            &[namelist_source, &mode_file],
            &[&published],
            &quality_dir,
        )?;
        // Preserve previous deliveries and inputs located inside file_dir.
        plan.remove_existing_file_dir = false;
        plan.remove_filelists = false;
        let saved_namelist = workdir.join(&plan.namelist_save_path);
        for input in [namelist_source, mode_file.as_path(), published.as_path()]
            .into_iter()
            .filter(|path| path.exists())
        {
            crate::atomic_output::validate_output_path(input, &saved_namelist)?;
        }
        let staged = stage.path(&published)?;
        // Existing import converters append gridfile/<name> to file_dir.
        // Redirect only their output root; workspace and inputs stay canonical.
        output_dir = staged
            .parent()
            .and_then(Path::parent)
            .unwrap()
            .to_path_buf();
        delivery = Some((stage, published, staged, quality_dir));
    }
    let workspace_mask =
        apply_workspace_and_mask_operations(&plan, namelist_source, workdir, 9, false)?;

    let (mut gridfile, runtime_state) = if mode_file.exists() {
        let gridfile = match config.mode_file_description.trim() {
            "EarthMesh" => {
                copy_existing_earthmesh_mode_file(&mode_file, &output_dir, nxp, &config.mode_grid)?
            }
            "MPAS" => convert_mpas_mode_file_to_earthmesh(
                &mode_file,
                &output_dir,
                nxp,
                &config.mode_grid,
            )?,
            "FVCOM" => convert_fvcom_mode_file_to_earthmesh(
                &mode_file,
                &output_dir,
                nxp,
                &config.mode_grid,
            )?,
            "IAP-Ocean" => convert_iap_ocean_mode_file_to_earthmesh(
                &mode_file,
                &output_dir,
                nxp,
                &config.mode_grid,
            )?,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "only existing EarthMesh, MPAS, FVCOM, and IAP-Ocean mode_file ingestion are current to Rust",
                ));
            }
        };
        let mesh = read_unstructured_mesh_netcdf(&gridfile.output)?;
        let runtime_state = Some(earthmesh_runtime_state_from_compact_mesh(&config, &mesh)?);
        (gridfile, runtime_state)
    } else {
        let state = earthmesh_mesh::gridinit_voronoi_state_canonical(
            nxp,
            niter,
            config.beta,
            config.relax,
            max_tris,
        )?;
        let mesh = crate::gridfile_mesh_from_one_based_state(&state.grid, &state.tabs)?;
        crate::validate_published_cell_degrees(&mesh, &config.mode_grid)?;
        let output_path = crate::gridfile_output_path(&output_dir, nxp, 1, &config.mode_grid);
        let context = crate::mpas_gridfile_context::MpasGridfileContext::from_producer(
            &mesh,
            vec![7680.0 / nxp as f64; mesh.w_points.len()],
            nxp,
            1,
            "gridinit_uniform_base",
        )?;
        // Fresh snapshot identities survive whole-cell extraction. Imported
        // mode files retain only their own metadata; do not invent ancestry.
        let m_lineage = (1..=mesh.m_points.len())
            .map(|row| row as i64)
            .collect::<Vec<_>>();
        let w_lineage = (1..=mesh.w_points.len())
            .map(|row| row as i64)
            .collect::<Vec<_>>();
        let gridfile =
            crate::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_method_c_metadata(
                output_path,
                &mesh,
                crate::MethodCGridfileMetadataSlices {
                    mpas: Some(&context),
                    m_lineage: Some(&m_lineage),
                    w_lineage: Some(&w_lineage),
                    ..Default::default()
                },
            )?;
        let mut generated_runtime_state = EarthmeshRuntimeState::new(config.clone());
        generated_runtime_state.grid = state.grid;
        generated_runtime_state.ijtabs = state.tabs;
        generated_runtime_state
            .record_pentagon_indices_from_icosahedron(state.impent)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        generated_runtime_state
            .record_mesh_counts_for_step(
                1,
                generated_runtime_state.grid.nma,
                generated_runtime_state.grid.nwa,
            )
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        (gridfile, Some(generated_runtime_state))
    };

    if let Some((stage, published, staged, quality_dir)) = delivery {
        fs::rename(&gridfile.output, &staged)?;
        let cell_kind = if config.mode_grid == "tri" {
            earthmesh_project::MeshCellKind::Tri
        } else {
            earthmesh_project::MeshCellKind::Hex
        };
        let quality = crate::project_quality::admit_staged_final_gridfile(
            &crate::project_quality::FinalAdmissionSpec {
                cell_kind,
                expected_euler_characteristic: Some(2),
                thresholds: earthmesh_quality::QualityThresholds::default(),
                repair_level_cap: None,
            },
            &staged,
            &published,
            &quality_dir,
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        stage.publish(
            serde_json::json!({
                "kind": "earthmesh_legacy_delivery",
                "target": {"cell": cell_kind},
                "capability": "native_only",
                "source_mesh_type": config.mesh_type,
                "source_mode_grid": config.mode_grid,
                "skipped_reason": "Native global base grid only; no specialized model adapter was run",
            }),
            &published, quality.verdict, &BTreeMap::new(), &BTreeMap::new(),
        )?;
        gridfile.output = published;
    }

    Ok(MkgrdGridinitRunReport {
        config,
        runtime_state,
        workspace_mask,
        raw_output: None,
        gridfile,
        fvcom_2dm: None,
    })
}
