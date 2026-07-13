use crate::apply_workspace_and_mask_operations;
use crate::convert_fvcom_mode_file_to_earthmesh;
use crate::convert_iap_ocean_mode_file_to_earthmesh;
use crate::convert_mpas_mode_file_to_earthmesh;
use crate::copy_existing_earthmesh_mode_file;
use crate::earthmesh_runtime_state_from_compact_mesh;
use crate::read_unstructured_mesh_netcdf;
use crate::write_gridfile_from_one_based_state;
use crate::MkgrdGridinitRunReport;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use earthmesh_core::{EarthmeshConfig, EarthmeshRuntimeState};

/// Run the Rust replacement path for the initial global `mkgrd.x` gridinit branch.
///
/// This mirrors the branch where `mode_grid` is `hex`/`tri` and `mode_file` does
/// not exist: parse the mkgrd namelist, apply the read_nl workspace/mask plan,
/// generate the in-memory global grid, and write
/// `gridfile/gridfile_NXP####_01_<mode_grid>.nc4`.  Restart mode and reading an
/// existing `mode_file` remain explicit `InvalidInput` errors until those compatibility
/// branches are current behind tests.
pub fn run_mkgrd_gridinit_global_namelist(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_tris: usize,
) -> io::Result<MkgrdGridinitRunReport> {
    let namelist_source = namelist_source.as_ref();
    let workdir = workdir.as_ref();
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
    let workspace_mask =
        apply_workspace_and_mask_operations(&plan, namelist_source, workdir, 9, false)?;

    let mode_file = PathBuf::from(config.mode_file.trim());
    let (gridfile, runtime_state) = if mode_file.exists() {
        let gridfile = match config.mode_file_description.trim() {
            "EarthMesh" => copy_existing_earthmesh_mode_file(
                &mode_file,
                config.file_dir(),
                nxp,
                &config.mode_grid,
            )?,
            "MPAS" => convert_mpas_mode_file_to_earthmesh(
                &mode_file,
                config.file_dir(),
                nxp,
                &config.mode_grid,
            )?,
            "FVCOM" => convert_fvcom_mode_file_to_earthmesh(
                &mode_file,
                config.file_dir(),
                nxp,
                &config.mode_grid,
            )?,
            "IAP-Ocean" => convert_iap_ocean_mode_file_to_earthmesh(
                &mode_file,
                config.file_dir(),
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
            f64::from(config.beta),
            f64::from(config.relax),
            max_tris,
        )?;
        let gridfile = write_gridfile_from_one_based_state(
            config.file_dir(),
            nxp,
            1,
            &config.mode_grid,
            &state.grid,
            &state.tabs,
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

    Ok(MkgrdGridinitRunReport {
        config,
        runtime_state,
        workspace_mask,
        gridfile,
        fvcom_2dm: None,
    })
}
