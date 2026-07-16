use crate::landtype_file_is_real;
use crate::namelist_sets_landtype_file;
use crate::read_unstructured_mesh_netcdf;
use crate::unstructured_dimc;
use crate::unstructured_mesh_write_report_from_file;
use crate::write_clean_regional_ocean_gridfile;
use crate::write_colm_coupling_csv_from_mesh_with_options;
use crate::write_colm_coupling_netcdf_from_csv;
use crate::write_colm_package_delivery_manifest_with_quality;
use crate::write_coupling_quality_from_gridfile;
use crate::write_landtype_masked_gridfile_with_refine_levels;
use crate::write_method_c_mesh_with_optional_domain_and_metadata;
use crate::write_regional_gridfile_with_refine_levels;
use crate::CouplingCsvOptions;
use crate::GridRegion;
use crate::MethodCGridfileMetadataSlices;
use crate::RefineCoupledOutputReport;
use crate::UnstructuredMesh;
use crate::UnstructuredMeshWriteReport;
use std::io;
use std::path::{Path, PathBuf};

use earthmesh_core::EarthmeshConfig;

fn mkgrd_tmpfile_path(file_dir: &Path, nxp: usize, step: usize, suffix: &str) -> PathBuf {
    file_dir
        .join("tmpfile")
        .join(format!("gridfile_NXP{nxp:04}_{step:02}_{suffix}.nc4"))
}

pub(super) struct MethodCRefinedOutputReports {
    pub raw_output: Option<UnstructuredMeshWriteReport>,
    pub landtype_masked_cells: Option<usize>,
    pub coupled_outputs: Option<RefineCoupledOutputReport>,
    pub output: UnstructuredMeshWriteReport,
}

pub(super) struct MethodCMetadataSlices<'a> {
    pub m_refine_level: &'a [i32],
    pub m_refine_level_orig: &'a [i32],
    pub m_ngr: &'a [i32],
    pub w_refine_level: &'a [i32],
    pub w_refine_level_orig: &'a [i32],
    pub w_ngr: &'a [i32],
}

impl<'a> MethodCMetadataSlices<'a> {
    fn gridfile(&self) -> MethodCGridfileMetadataSlices<'a> {
        MethodCGridfileMetadataSlices {
            m_refine_level: Some(self.m_refine_level),
            m_refine_level_orig: Some(self.m_refine_level_orig),
            m_ngr: Some(self.m_ngr),
            w_refine_level: Some(self.w_refine_level),
            w_refine_level_orig: Some(self.w_refine_level_orig),
            w_ngr: Some(self.w_ngr),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn write_method_c_refined_outputs(
    namelist_contents: &str,
    config: &EarthmeshConfig,
    source_gridnum_perdegree: Option<usize>,
    file_dir: &Path,
    nxp: usize,
    max_level: usize,
    output_mesh: &UnstructuredMesh,
    domain_region: Option<&GridRegion>,
    metadata: Option<MethodCMetadataSlices<'_>>,
) -> io::Result<MethodCRefinedOutputReports> {
    let output_path = file_dir.join("result").join(format!(
        "gridfile_NXP{nxp:04}_{}.nc4",
        config.mode_grid.trim()
    ));
    let has_landtype_file = namelist_sets_landtype_file(namelist_contents)
        && landtype_file_is_real(&config.landtype_file);

    let (raw_output, landtype_masked_cells, coupled_outputs, output) = if has_landtype_file
        && matches!(config.mesh_type.trim(), "landmesh" | "oceanmesh")
    {
        let gridnum_perdegree = method_c_source_gridnum_perdegree(
            source_gridnum_perdegree,
            config,
            "Method-C landtype mask",
        )?;
        let raw_path = mkgrd_tmpfile_path(
            file_dir,
            nxp,
            max_level,
            &format!("refine_raw_{}", config.mode_grid.trim()),
        );
        let raw_output = crate::write_unstructured_mesh_netcdf_with_method_c_metadata(
            &raw_path,
            output_mesh,
            metadata
                .as_ref()
                .map(MethodCMetadataSlices::gridfile)
                .unwrap_or_default(),
        )?;
        if config.mesh_type.trim() == "oceanmesh" && config.mode_grid.trim() == "tri" {
            if let Some(GridRegion::Close { points }) = domain_region {
                let plan = write_clean_regional_ocean_gridfile(
                    &raw_output.output,
                    points,
                    Path::new(&config.landtype_file),
                    nxp,
                    gridnum_perdegree,
                    config.mask_sea_ratio,
                    file_dir,
                )?;
                let output = unstructured_mesh_write_report_from_file(&plan.result_gridfile)?;
                return Ok(MethodCRefinedOutputReports {
                    raw_output: Some(raw_output),
                    landtype_masked_cells: Some(output.sjx_points.saturating_sub(2)),
                    coupled_outputs: None,
                    output,
                });
            }
        }
        let landtype_input = if let Some(region) = domain_region {
            let domain_path = mkgrd_tmpfile_path(
                file_dir,
                nxp,
                max_level,
                &format!("refine_domain_{}", config.mode_grid.trim()),
            );
            let kept = write_regional_gridfile_with_refine_levels(
                &raw_output.output,
                &domain_path,
                region,
                config.mode_grid.trim(),
                metadata.as_ref().map(|fields| fields.m_refine_level),
                metadata.as_ref().map(|fields| fields.w_refine_level),
            )?;
            if kept == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Method-C domain mask kept no cells",
                ));
            }
            domain_path
        } else {
            raw_output.output.clone()
        };
        let kept = write_landtype_masked_gridfile_with_refine_levels(
            &landtype_input,
            &output_path,
            &config.landtype_file,
            gridnum_perdegree,
            config.mode_grid.trim(),
            config.mesh_type.trim(),
            None,
            None,
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
                "LOCmesh Method-C specified refine requires a real NL%landtype_file",
            ));
        }
        let gridnum_perdegree = method_c_source_gridnum_perdegree(
            source_gridnum_perdegree,
            config,
            "Method-C LOC coupling",
        )?;
        let raw_path = mkgrd_tmpfile_path(
            file_dir,
            nxp,
            max_level,
            &format!("refine_raw_{}", config.mode_grid.trim()),
        );
        let (raw_output, output) = write_method_c_mesh_with_optional_domain_and_metadata(
            output_mesh,
            &raw_path,
            &output_path,
            domain_region,
            config.mode_grid.trim(),
            metadata
                .as_ref()
                .map(MethodCMetadataSlices::gridfile)
                .unwrap_or_default(),
        )?;
        let land_output_path = file_dir.join("result").join(format!(
            "gridfile_NXP{nxp:04}_{}_landmesh.nc4",
            config.mode_grid.trim()
        ));
        let ocean_output_path = file_dir.join("result").join(format!(
            "gridfile_NXP{nxp:04}_{}_oceanmesh.nc4",
            config.mode_grid.trim()
        ));
        let land_kept = write_landtype_masked_gridfile_with_refine_levels(
            &output.output,
            &land_output_path,
            &config.landtype_file,
            gridnum_perdegree,
            config.mode_grid.trim(),
            "landmesh",
            None,
            None,
        )?;
        let ocean_kept = write_landtype_masked_gridfile_with_refine_levels(
            &output.output,
            &ocean_output_path,
            &config.landtype_file,
            gridnum_perdegree,
            config.mode_grid.trim(),
            "oceanmesh",
            None,
            None,
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
        let coupling_quality = standard_dir.join(format!("CoLM_{output_stem}_quality.json"));
        let case_name = config.experiment_name.trim();
        let counts = write_colm_coupling_csv_from_mesh_with_options(
            &output.output,
            &config.landtype_file,
            gridnum_perdegree,
            case_name,
            config.mode_grid.trim(),
            &coupling_csv,
            CouplingCsvOptions {
                fraction_method: config.coupling_fraction_method.trim(),
                identify_coastline: config.coupling_identify_coastline,
                identify_river_mouth: config.coupling_identify_river_mouth,
                cama_root: config
                    .coupling_identify_river_mouth
                    .then(|| Path::new(config.coupling_cama_root.trim())),
                target_dx_km: earthmesh_project::nxp_to_km(nxp as i32),
            },
        )?;
        write_coupling_quality_from_gridfile(
            &output.output,
            &config.landtype_file,
            gridnum_perdegree,
            &coupling_quality,
        )?;
        let coupling_netcdf = write_colm_coupling_netcdf_from_csv(
            &coupling_csv,
            &coupling_netcdf_path,
            case_name,
            &manifest_path,
        )?;
        let manifest = write_colm_package_delivery_manifest_with_quality(
            &manifest_path,
            case_name,
            coupling_netcdf.rows,
            &coupling_netcdf.output,
            None,
            None,
            Some(&coupling_quality),
        )?;
        let coupled_outputs = RefineCoupledOutputReport {
            land_output,
            ocean_output,
            coupling_csv,
            coupling_netcdf,
            coupling_quality,
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
            &format!("refine_raw_{}", config.mode_grid.trim()),
        );
        let (raw_output, output) = write_method_c_mesh_with_optional_domain_and_metadata(
            output_mesh,
            &raw_path,
            &output_path,
            domain_region,
            config.mode_grid.trim(),
            metadata
                .as_ref()
                .map(MethodCMetadataSlices::gridfile)
                .unwrap_or_default(),
        )?;
        (raw_output, None, None, output)
    };

    Ok(MethodCRefinedOutputReports {
        raw_output,
        landtype_masked_cells,
        coupled_outputs,
        output,
    })
}

fn method_c_source_gridnum_perdegree(
    source_gridnum_perdegree: Option<usize>,
    config: &EarthmeshConfig,
    purpose: &str,
) -> io::Result<usize> {
    let value = match source_gridnum_perdegree {
        Some(value) => value,
        None => crate::mkgrd_gridinit_driver::landtype_gridnum_perdegree(Path::new(
            config.landtype_file.trim(),
        ))?,
    };
    if value == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("gridnum_perdegree must be positive for {purpose}"),
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn method_c_infers_landtype_resolution_instead_of_using_the_source_grid_default() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "earthmesh_method_c_landtype_resolution_{}_{}.nc",
            std::process::id(),
            stamp
        ));
        let mut file = netcdf::create_with(&path, netcdf::Options::default()).unwrap();
        file.add_dimension("lon", 720).unwrap();
        file.add_dimension("lat", 360).unwrap();
        drop(file);

        let config = EarthmeshConfig {
            gridnum_perdegree: 120,
            landtype_file: path.display().to_string(),
            ..EarthmeshConfig::default()
        };
        assert_eq!(
            method_c_source_gridnum_perdegree(None, &config, "test").unwrap(),
            2
        );
        assert_eq!(
            method_c_source_gridnum_perdegree(Some(3), &config, "test").unwrap(),
            3
        );

        let _ = std::fs::remove_file(path);
    }
}
