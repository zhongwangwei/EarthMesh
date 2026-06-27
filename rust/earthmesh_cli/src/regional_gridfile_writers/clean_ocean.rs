use std::fs;
use std::io;
use std::path::Path;

use crate::*;

use super::fvcom::write_fvcom_2dm_from_carved;

/// Carve a CLEAN regional ocean (FVCOM) mesh from a global gridfile + a close
/// polygon + a landtype NetCDF - in pure Rust, WITHOUT the refine pipeline and
/// WITHOUT reading any criterion data (LAI/slope/...). It reuses the engine's
/// Area_judge -> Get_Contain -> ocean `mask_postproc` chain (the same clean tri
/// boundary peeling the production path uses), so the result has a proper 0/2
/// boundary that the FVCOM writer accepts. Reads only: the gridfile, the close
/// polygon, and the landtype file. Writes intermediates under `work_dir`.
pub fn write_clean_regional_ocean_fvcom(
    global_gridfile: &Path,
    close_points: &[LonLatPoint],
    landtype_file: &Path,
    nxp: usize,
    gridnum_perdegree: usize,
    mask_sea_ratio: f64,
    work_dir: &Path,
    output_2dm: &Path,
) -> io::Result<usize> {
    let pre = read_landtype_data_preprocess_fortran_indexed(landtype_file, gridnum_perdegree)?;

    let close_path = work_dir.join("tmpfile").join("mask_domain_close_0_001.nc4");
    if let Some(p) = close_path.parent() {
        fs::create_dir_all(p)?;
    }
    write_close_mask_netcdf(
        &close_path,
        &CloseMask {
            refine_degree: 0,
            points: close_points.to_vec(),
        },
    )?;

    let domain = build_area_judge_domain_fortran_indexed(
        work_dir,
        false,
        "close",
        1,
        &pre.lon_vertex,
        &pre.lat_vertex,
        &pre.lon_i,
        &pre.lat_i,
        gridnum_perdegree,
        pre.nlons_source,
        pre.nlats_source,
    )?;
    let bounds = domain.bounds;

    let sol = build_area_judge_seaorland_fortran_indexed(
        &domain.is_in_domain,
        &pre.landtypes_global,
        bounds,
        "oceanmesh",
        false,
    )?;

    let plan = plan_mask_postproc_domain_io(work_dir, nxp, "tri", "oceanmesh", false)?;
    if let Some(p) = plan.source_gridfile.parent() {
        fs::create_dir_all(p)?;
    }
    fs::copy(global_gridfile, &plan.source_gridfile)?;

    let payload = select_area_judge_grid_fortran_indexed(
        &domain.is_in_domain,
        None,
        &pre.lon_i,
        &pre.lat_i,
        bounds,
    )?;
    let area_grid_file = work_dir.join("tmpfile").join("area_judge_domain.nc4");
    write_area_judge_grid_netcdf(&area_grid_file, &payload)?;
    run_getcontain_refine_file_fortran_indexed(GetContainRefineFileRunConfig {
        gridfile: &plan.source_gridfile,
        area_grid_file: &area_grid_file,
        output: &plan.contain_domain,
        mesh_kind: GetContainMeshKind::Ocean,
        seaorland: &sol.seaorland,
        lon_vertex: &pre.lon_vertex,
        lat_vertex: &pre.lat_vertex,
        lon_i: &pre.lon_i,
        lat_i: &pre.lat_i,
        num_vertex: 0,
    })?;

    run_mask_postproc_ocean_domain(
        &plan,
        MaskPostprocOceanRunOptions {
            mask_sea_ratio,
            num_vertex: 0,
        },
    )?;

    let carved = read_unstructured_mesh_netcdf(&plan.result_gridfile)?;
    let obc_order = match &plan.obc_output {
        Some(p) if p.exists() => read_obc_order_netcdf(p)?,
        _ => Vec::new(),
    };
    write_fvcom_2dm_from_carved(&carved, &obc_order, output_2dm)
}
