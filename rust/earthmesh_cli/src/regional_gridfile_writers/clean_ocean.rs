use crate::build_area_judge_close_area_source_cells_one_based;
use crate::build_global_source_axes_one_based;
use crate::plan_mask_postproc_domain_io;
use crate::read_obc_order_netcdf;
use crate::read_unstructured_mesh_netcdf;
use crate::run_getcontain_refine_file_one_based;
use crate::run_mask_postproc_ocean_domain;
use crate::write_area_judge_grid_netcdf;
use crate::write_close_mask_netcdf;
use crate::write_unstructured_mesh_netcdf_with_method_c_metadata;
use crate::AreaJudgeGridPayload;
use crate::CloseMask;
use crate::GetContainMeshKind;
use crate::GetContainRefineFileRunConfig;
use crate::LonLatPoint;
use crate::MaskPostprocDomainIoPlan;
use crate::MaskPostprocOceanRunOptions;
use std::fs;
use std::io;
use std::path::Path;

use super::fvcom::write_fvcom_2dm_from_carved;
use super::levels::{final_method_c_metadata_for_mask_postproc, refine_levels_from_gridfile};

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
    let plan = write_clean_regional_ocean_gridfile(
        global_gridfile,
        close_points,
        landtype_file,
        nxp,
        gridnum_perdegree,
        mask_sea_ratio,
        work_dir,
    )?;

    let carved = read_unstructured_mesh_netcdf(&plan.result_gridfile)?;
    let obc_order = match &plan.obc_output {
        Some(p) if p.exists() => read_obc_order_netcdf(p)?,
        _ => Vec::new(),
    };
    let report = write_fvcom_2dm_from_carved(&carved, &obc_order, output_2dm)?;
    Ok(report.triangles)
}

pub fn write_clean_regional_ocean_gridfile(
    global_gridfile: &Path,
    close_points: &[LonLatPoint],
    landtype_file: &Path,
    nxp: usize,
    gridnum_perdegree: usize,
    mask_sea_ratio: f64,
    work_dir: &Path,
) -> io::Result<MaskPostprocDomainIoPlan> {
    let nlons_source = gridnum_perdegree.checked_mul(360).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "gridnum_perdegree * 360 overflows usize",
        )
    })?;
    let nlats_source = gridnum_perdegree.checked_mul(180).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "gridnum_perdegree * 180 overflows usize",
        )
    })?;
    let axes = build_global_source_axes_one_based(gridnum_perdegree, nlons_source, nlats_source)?;

    let close_path = work_dir.join("tmpfile").join("mask_domain_close_0_001.nc4");
    crate::ensure_parent_dir(&close_path)?;
    write_close_mask_netcdf(
        &close_path,
        &CloseMask {
            refine_degree: 0,
            points: close_points.to_vec(),
        },
    )?;

    let domain = build_area_judge_close_area_source_cells_one_based(
        &close_path,
        &axes.lon_vertex,
        &axes.lat_vertex,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
    )?;
    let bounds = domain.bounds;
    let nlons_select = bounds.maxlon_source - bounds.minlon_source + 1;
    let nlats_select = bounds.minlat_source - bounds.maxlat_source + 1;
    let landtype = crate::mkgrd_data_preprocess_source::read_landtype_bbox_window_one_based(
        landtype_file,
        gridnum_perdegree,
        bounds,
    )?;
    let mut is_in_area_select = vec![vec![0_i32; nlats_select]; nlons_select];
    for (lon_index, lat_index) in domain.cells {
        if lon_index >= bounds.minlon_source
            && lon_index <= bounds.maxlon_source
            && lat_index >= bounds.maxlat_source
            && lat_index <= bounds.minlat_source
        {
            is_in_area_select[lon_index - bounds.minlon_source][lat_index - bounds.maxlat_source] =
                1;
        }
    }
    let mut seaorland = vec![vec![0_i32; nlats_select + 1]; nlons_select + 1];
    for lon_offset in 0..nlons_select {
        for lat_offset in 0..nlats_select {
            if is_in_area_select[lon_offset][lat_offset] == 0 {
                continue;
            }
            let value = landtype.values[lon_offset * nlats_select + lat_offset];
            if value != 0 {
                seaorland[lon_offset + 1][lat_offset + 1] = 1;
            }
        }
    }

    let longitude = axes.lon_i[bounds.minlon_source..=bounds.maxlon_source].to_vec();
    let latitude = axes.lat_i[bounds.maxlat_source..=bounds.minlat_source].to_vec();
    let lon_i = std::iter::once(f64::NAN)
        .chain(longitude.iter().copied())
        .collect::<Vec<_>>();
    let lat_i = std::iter::once(f64::NAN)
        .chain(latitude.iter().copied())
        .collect::<Vec<_>>();
    let lon_vertex = std::iter::once(f64::NAN)
        .chain(
            axes.lon_vertex[bounds.minlon_source..=bounds.maxlon_source]
                .iter()
                .copied(),
        )
        .collect::<Vec<_>>();
    let lat_vertex = std::iter::once(f64::NAN)
        .chain(
            axes.lat_vertex[bounds.maxlat_source..=bounds.minlat_source]
                .iter()
                .copied(),
        )
        .collect::<Vec<_>>();
    let local_bounds = earthmesh_mesh::AreaJudgeSourceBounds {
        minlon_source: 1,
        maxlon_source: nlons_select,
        maxlat_source: 1,
        minlat_source: nlats_select,
    };
    let payload = AreaJudgeGridPayload {
        bounds: local_bounds,
        longitude,
        latitude,
        is_in_area_select,
        seaorland_select: None,
    };

    let plan = plan_mask_postproc_domain_io(work_dir, nxp, "tri", "oceanmesh", false)?;
    crate::ensure_parent_dir(&plan.source_gridfile)?;
    fs::copy(global_gridfile, &plan.source_gridfile)?;

    let area_grid_file = work_dir.join("tmpfile").join("area_judge_domain.nc4");
    write_area_judge_grid_netcdf(&area_grid_file, &payload)?;
    run_getcontain_refine_file_one_based(GetContainRefineFileRunConfig {
        gridfile: &plan.source_gridfile,
        area_grid_file: &area_grid_file,
        output: &plan.contain_domain,
        mesh_kind: GetContainMeshKind::Ocean,
        seaorland: &seaorland,
        lon_vertex: &lon_vertex,
        lat_vertex: &lat_vertex,
        lon_i: &lon_i,
        lat_i: &lat_i,
        num_vertex: 0,
    })?;
    rebase_clean_ocean_contain_indices(&plan.contain_domain, bounds)?;

    let report = run_mask_postproc_ocean_domain(
        &plan,
        MaskPostprocOceanRunOptions {
            mask_sea_ratio,
            num_vertex: 0,
        },
    )?;
    let source_levels = refine_levels_from_gridfile(&plan.source_gridfile)?;
    let final_metadata = final_method_c_metadata_for_mask_postproc(
        "tri",
        &report.finalization,
        &report.renewal.is_in_domain_ustr,
        report.renewal.is_in_domain_ustr.len(),
        &source_levels,
    )?;
    if final_metadata.m.is_some() || final_metadata.w.is_some() {
        write_unstructured_mesh_netcdf_with_method_c_metadata(
            &plan.result_gridfile,
            &report.finalization.mesh,
            final_metadata.slices(),
        )?;
    }

    Ok(plan)
}

fn rebase_clean_ocean_contain_indices(
    contain_path: &Path,
    bounds: earthmesh_mesh::AreaJudgeSourceBounds,
) -> io::Result<()> {
    let lon_offset = i32::try_from(bounds.minlon_source - 1).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "clean-ocean longitude source offset exceeds i32",
        )
    })?;
    let lat_offset = i32::try_from(bounds.maxlat_source - 1).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "clean-ocean latitude source offset exceeds i32",
        )
    })?;
    if lon_offset == 0 && lat_offset == 0 {
        return Ok(());
    }
    let mut contain = crate::contain_io::read_contain_netcdf(contain_path)?;
    for row in &mut contain.ustr_ii {
        if row.len() != 2 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "clean-ocean containment source row width {} must be 2",
                    row.len()
                ),
            ));
        }
        row[0] = row[0].checked_add(lon_offset).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "clean-ocean longitude source index exceeds i32",
            )
        })?;
        row[1] = row[1].checked_add(lat_offset).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "clean-ocean latitude source index exceeds i32",
            )
        })?;
    }
    crate::contain_io::write_contain_netcdf(contain_path, &contain)?;
    Ok(())
}
