//! Raw base generation/import shared by carrier and final-delivery handoffs.
//! Only output destinations are supplied here; input paths/config stay unchanged.
use crate::{
    convert_fvcom_mode_file_to_earthmesh, convert_iap_ocean_mode_file_to_earthmesh,
    convert_mpas_mode_file_to_earthmesh, copy_existing_earthmesh_mode_file,
    earthmesh_runtime_state_from_compact_mesh, read_unstructured_mesh_netcdf,
    UnstructuredMeshWriteReport,
};
use earthmesh_core::{EarthmeshConfig, EarthmeshRuntimeState};
use std::{
    io,
    path::{Path, PathBuf},
};

pub(super) fn gridinit_sizes(config: &EarthmeshConfig) -> io::Result<(usize, usize)> {
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

    Ok((nxp, niter))
}

pub(crate) fn generate_gridinit_carrier(
    config: &EarthmeshConfig,
    output_dir: &Path,
    max_tris: usize,
) -> io::Result<(UnstructuredMeshWriteReport, Option<EarthmeshRuntimeState>)> {
    let (nxp, niter) = gridinit_sizes(config)?;
    let mode_file = PathBuf::from(config.mode_file.trim());
    let result = if mode_file.exists() {
        let gridfile = match config.mode_file_description.trim() {
            "EarthMesh" => {
                copy_existing_earthmesh_mode_file(&mode_file, output_dir, nxp, &config.mode_grid)?
            }
            "MPAS" => {
                convert_mpas_mode_file_to_earthmesh(&mode_file, output_dir, nxp, &config.mode_grid)?
            }
            "FVCOM" => convert_fvcom_mode_file_to_earthmesh(
                &mode_file,
                output_dir,
                nxp,
                &config.mode_grid,
            )?,
            "IAP-Ocean" => convert_iap_ocean_mode_file_to_earthmesh(
                &mode_file,
                output_dir,
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
        let runtime_state = Some(earthmesh_runtime_state_from_compact_mesh(config, &mesh)?);
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
        let output_path = crate::gridfile_output_path(output_dir, nxp, 1, &config.mode_grid);
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
    Ok(result)
}
