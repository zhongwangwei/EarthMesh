use std::io;
use std::path::Path;

use earthmesh_core::EarthmeshConfig;

use crate::*;

pub(super) struct OlamRefinedOutputReports {
    pub raw_output: Option<UnstructuredMeshWriteReport>,
    pub landtype_masked_cells: Option<usize>,
    pub coupled_outputs: Option<MkgrdOlamCoupledOutputReport>,
    pub output: UnstructuredMeshWriteReport,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn write_olam_refined_outputs(
    namelist_contents: &str,
    config: &EarthmeshConfig,
    source_gridnum_perdegree: Option<usize>,
    file_dir: &Path,
    nxp: usize,
    max_level: usize,
    output_mesh: &UnstructuredMesh,
    domain_region: Option<&GridRegion>,
) -> io::Result<OlamRefinedOutputReports> {
    let output_path = file_dir.join("result").join(format!(
        "gridfile_NXP{nxp:04}_{}.nc4",
        config.mode_grid.trim()
    ));
    let has_landtype_file = namelist_sets_landtype_file(namelist_contents)
        && landtype_file_is_real(&config.landtype_file);

    let (raw_output, landtype_masked_cells, coupled_outputs, output) = if has_landtype_file
        && matches!(config.mesh_type.trim(), "landmesh" | "oceanmesh")
    {
        let gridnum_perdegree =
            olam_source_gridnum_perdegree(source_gridnum_perdegree, config, "OLAM landtype mask")?;
        let raw_path = mkgrd_tmpfile_path(
            file_dir,
            nxp,
            max_level,
            &format!("olam_raw_{}", config.mode_grid.trim()),
        );
        let raw_output = write_unstructured_mesh_netcdf(&raw_path, output_mesh)?;
        let landtype_input = if let Some(region) = domain_region {
            let domain_path = mkgrd_tmpfile_path(
                file_dir,
                nxp,
                max_level,
                &format!("olam_domain_{}", config.mode_grid.trim()),
            );
            let kept = write_regional_gridfile(
                &raw_output.output,
                &domain_path,
                region,
                config.mode_grid.trim(),
            )?;
            if kept == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "OLAM domain mask kept no cells",
                ));
            }
            domain_path
        } else {
            raw_output.output.clone()
        };
        let kept = write_landtype_masked_gridfile(
            &landtype_input,
            &output_path,
            &config.landtype_file,
            gridnum_perdegree,
            config.mode_grid.trim(),
            config.mesh_type.trim(),
        )?;
        let masked_mesh = read_unstructured_mesh_netcdf(&output_path)?;
        let output = UnstructuredMeshWriteReport {
            output: output_path.clone(),
            sjx_points: masked_mesh.m_points.len(),
            lbx_points: masked_mesh.w_points.len(),
            dimc: unstructured_dimc(&masked_mesh),
        };
        (Some(raw_output), Some(kept), None, output)
    } else if config.mesh_type.trim() == "LOCmesh" {
        if !has_landtype_file {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "LOCmesh OLAM specified refine requires a real NL%landtype_file",
            ));
        }
        let gridnum_perdegree =
            olam_source_gridnum_perdegree(source_gridnum_perdegree, config, "OLAM LOC coupling")?;
        let raw_path = mkgrd_tmpfile_path(
            file_dir,
            nxp,
            max_level,
            &format!("olam_raw_{}", config.mode_grid.trim()),
        );
        let (raw_output, output) = write_olam_mesh_with_optional_domain(
            output_mesh,
            &raw_path,
            &output_path,
            domain_region,
            config.mode_grid.trim(),
        )?;
        let land_output_path = file_dir.join("result").join(format!(
            "gridfile_NXP{nxp:04}_{}_landmesh.nc4",
            config.mode_grid.trim()
        ));
        let ocean_output_path = file_dir.join("result").join(format!(
            "gridfile_NXP{nxp:04}_{}_oceanmesh.nc4",
            config.mode_grid.trim()
        ));
        let land_kept = write_landtype_masked_gridfile(
            &output.output,
            &land_output_path,
            &config.landtype_file,
            gridnum_perdegree,
            config.mode_grid.trim(),
            "landmesh",
        )?;
        let ocean_kept = write_landtype_masked_gridfile(
            &output.output,
            &ocean_output_path,
            &config.landtype_file,
            gridnum_perdegree,
            config.mode_grid.trim(),
            "oceanmesh",
        )?;
        let land_mesh = read_unstructured_mesh_netcdf(&land_output_path)?;
        let ocean_mesh = read_unstructured_mesh_netcdf(&ocean_output_path)?;
        let land_output = UnstructuredMeshWriteReport {
            output: land_output_path,
            sjx_points: land_mesh.m_points.len(),
            lbx_points: land_mesh.w_points.len(),
            dimc: unstructured_dimc(&land_mesh),
        };
        let ocean_output = UnstructuredMeshWriteReport {
            output: ocean_output_path,
            sjx_points: ocean_mesh.m_points.len(),
            lbx_points: ocean_mesh.w_points.len(),
            dimc: unstructured_dimc(&ocean_mesh),
        };
        let output_stem = output_path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("gridfile_NXP{nxp:04}_{}", config.mode_grid.trim()));
        let standard_dir = file_dir.join("standard");
        let coupling_csv = standard_dir.join(format!("CoLM_{output_stem}_cells.csv"));
        let coupling_netcdf_path = standard_dir.join(format!("CoLM_{output_stem}_coupling.nc4"));
        let manifest_path = standard_dir.join(format!("CoLM_{output_stem}_manifest.json"));
        let case_name = config.experiment_name.trim();
        let counts = write_colm_coupling_csv_from_mesh(
            &output.output,
            &config.landtype_file,
            gridnum_perdegree,
            case_name,
            config.mode_grid.trim(),
            &coupling_csv,
        )?;
        let coupling_netcdf = write_colm_coupling_netcdf_from_csv(
            &coupling_csv,
            &coupling_netcdf_path,
            case_name,
            &manifest_path,
        )?;
        let manifest = write_colm_package_delivery_manifest(
            &manifest_path,
            case_name,
            coupling_netcdf.rows,
            &coupling_netcdf.output,
            None,
            None,
        )?;
        let coupled_outputs = MkgrdOlamCoupledOutputReport {
            land_output,
            ocean_output,
            coupling_csv,
            coupling_netcdf,
            manifest,
            counts,
        };
        (
            raw_output.or_else(|| Some(output.clone())),
            Some(land_kept + ocean_kept),
            Some(coupled_outputs),
            output,
        )
    } else {
        let raw_path = mkgrd_tmpfile_path(
            file_dir,
            nxp,
            max_level,
            &format!("olam_raw_{}", config.mode_grid.trim()),
        );
        let (raw_output, output) = write_olam_mesh_with_optional_domain(
            output_mesh,
            &raw_path,
            &output_path,
            domain_region,
            config.mode_grid.trim(),
        )?;
        (raw_output, None, None, output)
    };

    Ok(OlamRefinedOutputReports {
        raw_output,
        landtype_masked_cells,
        coupled_outputs,
        output,
    })
}

fn olam_source_gridnum_perdegree(
    source_gridnum_perdegree: Option<usize>,
    config: &EarthmeshConfig,
    purpose: &str,
) -> io::Result<usize> {
    let value = match source_gridnum_perdegree {
        Some(value) => value,
        None => usize::try_from(config.gridnum_perdegree).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "NL%gridnum_perdegree must be positive for {purpose}, got {}",
                    config.gridnum_perdegree
                ),
            )
        })?,
    };
    if value == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("gridnum_perdegree must be positive for {purpose}"),
        ));
    }
    Ok(value)
}
