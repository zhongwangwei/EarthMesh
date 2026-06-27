use std::io;

use earthmesh_mesh::OlamRefinementRegion;

use super::{
    olam_native_grid_count, read_olam_native_mdomain, read_olam_native_sfcgrid_res_factor,
};

pub(crate) fn olam_native_atmosphere_grid_count_spawns(
    native_mdomain: Option<usize>,
    grid_count: usize,
) -> bool {
    grid_count > 1 && native_mdomain.is_none_or(|mdomain| matches!(mdomain, 0 | 5))
}

pub(crate) fn olam_native_surface_global_expansion_requested(
    contents: &str,
    mesh_type: &str,
) -> io::Result<bool> {
    if matches!(mesh_type, "atmos" | "atmosmesh") {
        return Ok(false);
    }
    Ok(read_olam_native_sfcgrid_res_factor(contents)? > 1)
}

pub(crate) fn olam_native_refinement_requested(
    contents: &str,
    mesh_type: &str,
) -> io::Result<bool> {
    let native_mdomain = read_olam_native_mdomain(contents)?;
    let atmosphere_requested = match olam_native_grid_count(contents, "ngrids")? {
        Some(0) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "native OLAM ngrids must be at least 1",
            ))
        }
        Some(count) => olam_native_atmosphere_grid_count_spawns(native_mdomain, count),
        None => false,
    };
    if matches!(mesh_type, "atmos" | "atmosmesh") {
        return Ok(atmosphere_requested);
    }
    let surface_requested =
        olam_native_grid_count(contents, "nsfcgrids")?.is_some_and(|count| count > 0);
    Ok(atmosphere_requested || surface_requested)
}

pub(crate) fn olam_native_refinement_depth(
    contents: &str,
    is_atmosmesh: bool,
) -> io::Result<usize> {
    let native_mdomain = read_olam_native_mdomain(contents)?;
    let atmosphere_grid_count = olam_native_grid_count(contents, "ngrids")?.unwrap_or(1);
    let atmosphere_depth =
        if olam_native_atmosphere_grid_count_spawns(native_mdomain, atmosphere_grid_count) {
            atmosphere_grid_count.saturating_sub(1)
        } else {
            0
        };
    if is_atmosmesh {
        return Ok(atmosphere_depth);
    }
    let surface_grid_count = olam_native_grid_count(contents, "nsfcgrids")?.unwrap_or(0);
    Ok(atmosphere_depth + surface_grid_count)
}

pub(crate) fn olam_refinement_region_level(region: &OlamRefinementRegion) -> usize {
    match region {
        OlamRefinementRegion::Circle { level, .. }
        | OlamRefinementRegion::Bbox { level, .. }
        | OlamRefinementRegion::Corridor { level, .. }
        | OlamRefinementRegion::Polygon { level, .. } => *level,
    }
}
