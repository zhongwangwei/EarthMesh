use std::io;
use std::path::Path;

use earthmesh_mesh::AreaJudgeSourceBounds;

use crate::*;

/// Build the final-domain Area_judge payload requested by compact source-state
/// metadata. This mirrors the full-source selected window used by the migrated
/// source-state CLI path before final `Get_Contain(0)`.
pub fn compact_source_state_final_domain_area_payload_fortran_indexed(
    state: &MkgrdCompactSourceState,
    axes: &GlobalSourceAxes,
) -> io::Result<Option<AreaJudgeGridPayload>> {
    if state.final_domain_contain.is_none() {
        return Ok(None);
    }
    select_area_judge_grid_fortran_indexed(
        &state.is_in_domain,
        None,
        &axes.lon_i,
        &axes.lat_i,
        AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: state.nlons_source,
            maxlat_source: 1,
            minlat_source: state.nlats_source,
        },
    )
    .map(Some)
}

/// Build final-domain contain options requested by compact source-state
/// metadata while borrowing caller-owned axes and output paths.
pub fn compact_source_state_final_contain_options<'a>(
    state: &'a MkgrdCompactSourceState,
    axes: &'a GlobalSourceAxes,
    area_grid_file: &'a Path,
) -> Option<MkgrdFinalDomainContainOptions<'a>> {
    state
        .final_domain_contain
        .map(|mesh_kind| MkgrdFinalDomainContainOptions {
            area_grid_file,
            mesh_kind,
            seaorland: &state.seaorland,
            lon_vertex: &axes.lon_vertex,
            lat_vertex: &axes.lat_vertex,
            lon_i: &axes.lon_i,
            lat_i: &axes.lat_i,
            num_vertex: state.num_vertex,
        })
}

/// Build the full-source final-domain Area_judge payload for a
/// data_preprocess-derived source-state handoff.
pub fn data_preprocess_source_state_final_domain_area_payload_fortran_indexed(
    state: &MkgrdDataPreprocessSourceState,
) -> io::Result<AreaJudgeGridPayload> {
    select_area_judge_grid_fortran_indexed(
        &state.is_in_domain,
        None,
        &state.lon_i,
        &state.lat_i,
        AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: state.nlons_source,
            maxlat_source: 1,
            minlat_source: state.nlats_source,
        },
    )
}

/// Build final-domain contain options for a data_preprocess-derived
/// source-state handoff while borrowing caller-owned output paths.
pub fn data_preprocess_source_state_final_contain_options<'a>(
    state: &'a MkgrdDataPreprocessSourceState,
    mesh_type: &str,
    area_grid_file: &'a Path,
) -> io::Result<Option<MkgrdFinalDomainContainOptions<'a>>> {
    let mesh_kind = match mesh_type.trim() {
        "earthmesh" => GetContainMeshKind::Loc,
        "landmesh" => GetContainMeshKind::Land,
        "oceanmesh" => GetContainMeshKind::Ocean,
        "atmos" | "atmosmesh" => GetContainMeshKind::Atmos,
        "LOCmesh" => GetContainMeshKind::Loc,
        _ => return Ok(None),
    };
    Ok(Some(MkgrdFinalDomainContainOptions {
        area_grid_file,
        mesh_kind,
        seaorland: &state.seaorland,
        lon_vertex: &state.lon_vertex,
        lat_vertex: &state.lat_vertex,
        lon_i: &state.lon_i,
        lat_i: &state.lat_i,
        num_vertex: state.num_vertex,
    }))
}

/// Write the full-source final-domain Area_judge payload and return the
/// matching contain options for a data_preprocess-derived source-state handoff.
pub fn write_data_preprocess_source_state_final_domain_contain_options<'a>(
    state: &'a MkgrdDataPreprocessSourceState,
    mesh_type: &str,
    area_grid_file: &'a Path,
) -> io::Result<Option<MkgrdFinalDomainContainOptions<'a>>> {
    let payload = data_preprocess_source_state_final_domain_area_payload_fortran_indexed(state)?;
    crate::ensure_parent_dir(area_grid_file)?;
    write_area_judge_grid_netcdf(area_grid_file, &payload)?;
    data_preprocess_source_state_final_contain_options(state, mesh_type, area_grid_file)
}

/// Build final-domain contain options shared by Area_judge restart-refine
/// source-state and landtype-source handoffs while borrowing caller-owned axes.
pub fn restart_refine_final_contain_options<'a>(
    area_grid_file: &'a Path,
    mesh_type: &str,
    requested_num_vertex: Option<usize>,
    seaorland: &'a [Vec<i32>],
    lon_vertex: &'a [f64],
    lat_vertex: &'a [f64],
    lon_i: &'a [f64],
    lat_i: &'a [f64],
) -> io::Result<Option<MkgrdFinalDomainContainOptions<'a>>> {
    let Some(num_vertex) = requested_num_vertex else {
        return Ok(None);
    };
    let mesh_kind = match mesh_type.trim() {
        "earthmesh" => GetContainMeshKind::Loc,
        "landmesh" => GetContainMeshKind::Land,
        "oceanmesh" => GetContainMeshKind::Ocean,
        "atmos" | "atmosmesh" => GetContainMeshKind::Atmos,
        "LOCmesh" => GetContainMeshKind::Loc,
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("restart-refine final contain does not support mesh_type={other}"),
            ))
        }
    };
    Ok(Some(MkgrdFinalDomainContainOptions {
        area_grid_file,
        mesh_kind,
        seaorland,
        lon_vertex,
        lat_vertex,
        lon_i,
        lat_i,
        num_vertex,
    }))
}
