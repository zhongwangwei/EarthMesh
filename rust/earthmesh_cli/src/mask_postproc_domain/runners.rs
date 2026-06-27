use std::io;

use earthmesh_mesh::classify_boundary_orders_fortran_indexed;

use crate::*;

/// File-backed composition of the migrated
/// `MOD_mask_postproc.F90:mask_postproc_Earth` branch.
///
/// This runner intentionally composes already-migrated pure/data helpers:
/// contain-domain reading, Earth land/sea patchtype classification, `PatchID`
/// output, final clipped gridfile writing, and `earthmesh_info.nc4` output.
pub fn run_mask_postproc_earth_domain(
    plan: &MaskPostprocDomainIoPlan,
    options: MaskPostprocEarthRunOptions<'_>,
) -> io::Result<MaskPostprocEarthDomainReport> {
    if plan.mesh_type != "earthmesh" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "earth mask_postproc runner requires earthmesh plan, got {}",
                plan.mesh_type
            ),
        ));
    }

    let inputs = read_mask_postproc_domain_inputs(plan)?;
    let patchtypes = build_earth_patchtypes_fortran_indexed(
        &inputs.contain,
        options.mask_sea_ratio,
        options.minlon_dm_area,
        options.maxlat_dm_area,
        options.nlons_dm_select,
        options.nlats_dm_select,
    )?;
    let patchtype = write_mask_postproc_patchtype_netcdf(
        plan,
        patchtypes.patchtypes_select.clone(),
        options.minlon_dm_area,
        options.maxlat_dm_area,
        options.lon_vertex,
        options.lat_vertex,
        options.lon_i,
        options.lat_i,
    )?;
    let final_gridfile =
        write_mask_postproc_final_gridfile(plan, &inputs.layout, &inputs.is_in_domain_ustr)?;
    let earthmesh_info = write_mask_postproc_earth_info_netcdf(
        plan,
        options.num_mp_step,
        options.sjx_points,
        &inputs.layout,
        &inputs.is_in_domain_ustr,
        &patchtypes.seaorland_ustr,
    )?;

    Ok(MaskPostprocEarthDomainReport {
        patchtypes,
        patchtype,
        final_gridfile,
        earthmesh_info,
    })
}

/// File-backed composition of the migrated
/// `MOD_mask_postproc.F90:mask_postproc_Lnd` branch.
///
/// The source-grid clipping uses the contain-domain mask exactly like the
/// Fortran branch, while land-specific patchtype assignment is delegated to the
/// already-migrated pure `build_land_patchtypes_fortran_indexed` helper.
pub fn run_mask_postproc_land_domain(
    plan: &MaskPostprocDomainIoPlan,
    options: MaskPostprocLandRunOptions<'_>,
) -> io::Result<MaskPostprocLandDomainReport> {
    if plan.mesh_type != "landmesh" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "land mask_postproc runner requires landmesh plan, got {}",
                plan.mesh_type
            ),
        ));
    }

    let inputs = read_mask_postproc_domain_inputs(plan)?;
    let patchtypes = build_land_patchtypes_fortran_indexed(
        &inputs.contain,
        options.seaorland,
        options.minlon_dm_area,
        options.maxlat_dm_area,
        options.nlons_dm_select,
        options.nlats_dm_select,
    )?;
    let patchtype = write_mask_postproc_patchtype_netcdf(
        plan,
        patchtypes.patchtypes_select.clone(),
        options.minlon_dm_area,
        options.maxlat_dm_area,
        options.lon_vertex,
        options.lat_vertex,
        options.lon_i,
        options.lat_i,
    )?;
    let final_gridfile =
        write_mask_postproc_final_gridfile(plan, &inputs.layout, &inputs.is_in_domain_ustr)?;

    Ok(MaskPostprocLandDomainReport {
        patchtypes,
        patchtype,
        final_gridfile,
    })
}

/// File-backed composition of the migrated
/// `MOD_mask_postproc.F90:mask_postproc_Ocn` branch.
///
/// This runner composes contain-domain reading, the ocean sea-ratio mask
/// adjustment, tri/hex renewal, final gridfile writing, and tri-only boundary
/// outputs (`obc*.nc4`/`obcv2*.nc4`).
pub fn run_mask_postproc_ocean_domain(
    plan: &MaskPostprocDomainIoPlan,
    options: MaskPostprocOceanRunOptions,
) -> io::Result<MaskPostprocOceanDomainReport> {
    if plan.mesh_type != "oceanmesh" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "ocean mask_postproc runner requires oceanmesh plan, got {}",
                plan.mesh_type
            ),
        ));
    }

    let inputs = read_mask_postproc_domain_inputs(plan)?;
    let ocean_mask = apply_ocean_mask_sea_ratio_fortran_indexed(
        &inputs.contain,
        options.num_vertex,
        options.mask_sea_ratio,
    )?;
    let renewal = renew_mask_postproc_ocean_domain_fortran_indexed(
        &inputs.layout,
        &ocean_mask,
        &plan.mode_grid,
    )?;
    let finalization = finalize_mask_postproc_layout_with_reindex_report(
        &inputs.layout,
        &renewal.is_in_domain_ustr,
        &plan.mode_grid,
    )?;
    let final_gridfile = write_unstructured_mesh_netcdf(&plan.result_gridfile, &finalization.mesh)?;

    let mut boundary_orders = None;
    let mut obc = None;
    let mut obcv2 = None;
    if plan.mode_grid == "tri" {
        let boundary = renewal.boundary.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "tri ocean renewal did not produce boundary connection metadata",
            )
        })?;
        let isolated = renewal.isolated.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "tri ocean renewal did not produce isolated-ocean metadata",
            )
        })?;
        let obcv2_output = plan.obcv2_output.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "tri ocean plan is missing obcv2 output path",
            )
        })?;
        obcv2 = Some(write_obcv2_boundary_netcdf(obcv2_output, boundary)?);

        let orders = classify_boundary_orders_fortran_indexed(
            isolated.num_bdy_long,
            &isolated.bdy_long_order,
            &inputs.layout.vertex_neighbors,
            &inputs.layout.vertex_neighbor_counts,
            &finalization.vertex_reindex.vertex_mapping,
            &renewal.is_in_domain_ustr,
        )?;
        let obc_output = plan.obc_output.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "tri ocean plan is missing obc output path",
            )
        })?;
        obc = Some(write_obc_boundary_netcdf(obc_output, &orders)?);
        boundary_orders = Some(orders);
    }

    Ok(MaskPostprocOceanDomainReport {
        renewal,
        finalization,
        final_gridfile,
        boundary_orders,
        obc,
        obcv2,
    })
}
