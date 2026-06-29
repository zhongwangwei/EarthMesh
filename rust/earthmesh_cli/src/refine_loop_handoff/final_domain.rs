use std::fs;
use std::io;

use crate::*;

/// Execute the final file-backed handoff after the top-level `mkgrd.F90` refine
/// loop: copy the final step gridfile to `result/gridfile_NXP####_<mode>.nc4`
/// like `Get_Contain(0)` does before containment calculation, then optionally
/// run the migrated domain `mask_postproc` branch using the planned files.
pub fn run_mkgrd_refine_loop_final_domain_handoff(
    plan: &MkgrdRefineLoopIoPlan,
    postproc_options: Option<MkgrdFinalDomainPostprocOptions<'_>>,
) -> io::Result<MkgrdRefineLoopFinalDomainHandoffReport> {
    run_mkgrd_refine_loop_final_domain_handoff_with_domain_contain(plan, None, postproc_options)
}

/// Execute the final file-backed handoff after the top-level `mkgrd.F90` refine
/// loop and, when requested, generate the final `Get_Contain(0)` domain
/// containment file before optional `mask_postproc(mesh_type)`.
pub fn run_mkgrd_refine_loop_final_domain_handoff_with_domain_contain(
    plan: &MkgrdRefineLoopIoPlan,
    contain_options: Option<MkgrdFinalDomainContainOptions<'_>>,
    postproc_options: Option<MkgrdFinalDomainPostprocOptions<'_>>,
) -> io::Result<MkgrdRefineLoopFinalDomainHandoffReport> {
    crate::ensure_parent_dir(&plan.final_result_gridfile)?;
    let copied_bytes = fs::copy(&plan.final_domain_gridfile, &plan.final_result_gridfile)?;

    let generated_contain = contain_options
        .map(|options| {
            run_getcontain_refine_file_fortran_indexed(GetContainRefineFileRunConfig {
                gridfile: &plan.final_result_gridfile,
                area_grid_file: options.area_grid_file,
                output: &plan.final_domain_contain_output,
                mesh_kind: options.mesh_kind,
                seaorland: options.seaorland,
                lon_vertex: options.lon_vertex,
                lat_vertex: options.lat_vertex,
                lon_i: options.lon_i,
                lat_i: options.lat_i,
                num_vertex: options.num_vertex,
            })
        })
        .transpose()?;

    let postproc = match postproc_options {
        None => None,
        Some(options) => Some(match options {
            MkgrdFinalDomainPostprocOptions::Atmos { output_format } => {
                match output_format.trim() {
                    "MPAS" => MkgrdFinalDomainPostprocReport::AtmosFull(
                        write_mask_postproc_atmos_mpas_netcdf(
                            &plan.file_dir,
                            plan.nxp,
                            plan.final_mask_postproc_step,
                            &plan.mode_grid,
                            &plan.mesh_type,
                            output_format,
                        )?,
                    ),
                    "MPAS-Simple" => MkgrdFinalDomainPostprocReport::Atmos(
                        write_mask_postproc_atmos_mpas_simple_netcdf(
                            &plan.file_dir,
                            plan.nxp,
                            &plan.mode_grid,
                            &plan.mesh_type,
                            output_format,
                        )?,
                    ),
                    other => {
                        return Err(io::Error::new(
                                io::ErrorKind::InvalidInput,
                                format!(
                                    "atmosmesh final postproc supports output_format MPAS/MPAS-Simple, got {other}"
                                ),
                            ));
                    }
                }
            }
            MkgrdFinalDomainPostprocOptions::Earth(options) => {
                let postproc_plan = plan.final_mask_postproc_domain.as_ref().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "final domain mask_postproc plan is unavailable for this mesh_type",
                    )
                })?;
                MkgrdFinalDomainPostprocReport::Earth(run_mask_postproc_earth_domain(
                    postproc_plan,
                    options,
                )?)
            }
            MkgrdFinalDomainPostprocOptions::EarthFromFinalGrid(options) => {
                let postproc_plan = plan.final_mask_postproc_domain.as_ref().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "final domain mask_postproc plan is unavailable for this mesh_type",
                    )
                })?;
                let source_mesh = read_unstructured_mesh_netcdf(&postproc_plan.source_gridfile)?;
                let num_mp_step = vec![source_mesh.m_points.len()];
                MkgrdFinalDomainPostprocReport::Earth(run_mask_postproc_earth_domain(
                    postproc_plan,
                    MaskPostprocEarthRunOptions {
                        mask_sea_ratio: options.mask_sea_ratio,
                        minlon_dm_area: options.minlon_dm_area,
                        maxlat_dm_area: options.maxlat_dm_area,
                        nlons_dm_select: options.nlons_dm_select,
                        nlats_dm_select: options.nlats_dm_select,
                        lon_vertex: options.lon_vertex,
                        lat_vertex: options.lat_vertex,
                        lon_i: options.lon_i,
                        lat_i: options.lat_i,
                        num_mp_step: &num_mp_step,
                        sjx_points: source_mesh.m_points.len(),
                    },
                )?)
            }
            MkgrdFinalDomainPostprocOptions::Land(options) => {
                let postproc_plan = plan.final_mask_postproc_domain.as_ref().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "final domain mask_postproc plan is unavailable for this mesh_type",
                    )
                })?;
                MkgrdFinalDomainPostprocReport::Land(run_mask_postproc_land_domain(
                    postproc_plan,
                    options,
                )?)
            }
            MkgrdFinalDomainPostprocOptions::Ocean(options) => {
                let postproc_plan = plan.final_mask_postproc_domain.as_ref().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "final domain mask_postproc plan is unavailable for this mesh_type",
                    )
                })?;
                MkgrdFinalDomainPostprocReport::Ocean(run_mask_postproc_ocean_domain(
                    postproc_plan,
                    options,
                )?)
            }
        }),
    };

    Ok(MkgrdRefineLoopFinalDomainHandoffReport {
        copied_result_gridfile: plan.final_result_gridfile.clone(),
        copied_bytes,
        contain_domain: plan.final_domain_contain_output.clone(),
        generated_contain,
        postproc,
    })
}
