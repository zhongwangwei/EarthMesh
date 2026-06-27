use std::io;

use earthmesh_mesh::AreaJudgeSourceBounds;

use crate::*;

/// File-backed adapter for the refinement branch of
/// `MOD_GetContain.F90:Get_Contain(iter)`.
///
/// This covers the `refine .and. step <= max_iter` branch for triangular
/// refinement grids: read the current `Unstructured_Mesh_Save` gridfile, expand
/// the selected `Area_judge_refine` grid back into the full source grid, compute
/// `IsInRfArea_sjx` plus containment rows, and persist the legacy
/// `Contain_Save` schema selected by the caller.
pub fn run_getcontain_refine_file_fortran_indexed(
    config: GetContainRefineFileRunConfig<'_>,
) -> io::Result<GetContainRefineFileRunReport> {
    let mesh = read_unstructured_mesh_netcdf(config.gridfile)?;
    let area_payload = read_area_judge_grid_netcdf(config.area_grid_file)?;
    let is_in_refine_grid = expand_area_judge_selected_grid_only_fortran_indexed(
        &area_payload,
        config.lon_i.len().saturating_sub(1),
        config.lat_i.len().saturating_sub(1),
    )?;
    getcontain_validate_source_matrix(
        "seaorland",
        config.seaorland,
        config.lon_i.len(),
        config.lat_i.len(),
    )?;

    let bounds = area_judge_bounds_to_getcontain_bounds(
        area_payload.bounds,
        config.lon_vertex,
        config.lat_vertex,
    )?;
    let mut vertices = Vec::with_capacity(mesh.w_points.len() + 1);
    vertices.push(LonLatPoint {
        lon: f64::NAN,
        lat: f64::NAN,
    });
    vertices.extend(mesh.w_points.iter().copied());

    let mut cell_to_vertices = Vec::with_capacity(mesh.m_to_w.len() + 1);
    cell_to_vertices.push(Vec::new());
    cell_to_vertices.extend(mesh.m_to_w.iter().map(|row| row.to_vec()));
    let mut n_edges = Vec::with_capacity(mesh.m_to_w.len() + 1);
    n_edges.push(0);
    n_edges.extend(std::iter::repeat_n(3, mesh.m_to_w.len()));

    let is_in_area_ustr = getcontain_is_in_area_ustr_fortran_indexed(
        bounds,
        &vertices,
        &cell_to_vertices,
        &n_edges,
        config.num_vertex,
    )?;
    let contain = getcontain_containment_matrix_flat_fortran_indexed(
        config.mesh_kind,
        &vertices,
        &cell_to_vertices,
        &n_edges,
        &is_in_area_ustr,
        &is_in_refine_grid,
        config.seaorland,
        config.lon_i,
        config.lat_i,
        config.num_vertex,
    )?;
    let active_unstructured_cells = contain
        .is_in_area_ustr
        .iter()
        .filter(|value| **value != 0)
        .count();
    let contained_source_pixels = contain.num_ii();
    let write = write_flat_contain_netcdf(config.output, &contain)?;

    Ok(GetContainRefineFileRunReport {
        output: config.output.to_path_buf(),
        active_unstructured_cells,
        contained_source_pixels,
        runtime_counts: GetContainRuntimeCounts {
            current_num_mp_step: mesh.m_points.len(),
            current_num_wp_step: mesh.w_points.len(),
            previous_num_vertex: config.num_vertex,
        },
        write,
    })
}

fn expand_area_judge_selected_grid_only_fortran_indexed(
    payload: &AreaJudgeGridPayload,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<Vec<Vec<i32>>> {
    validate_area_judge_grid_payload(payload)?;
    if payload.bounds.maxlon_source > nlons_source || payload.bounds.minlat_source > nlats_source {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Area_judge bounds lon {}..{} lat {}..{} exceed source dimensions {}x{}",
                payload.bounds.minlon_source,
                payload.bounds.maxlon_source,
                payload.bounds.maxlat_source,
                payload.bounds.minlat_source,
                nlons_source,
                nlats_source
            ),
        ));
    }

    let mut full = vec![vec![0_i32; nlats_source + 1]; nlons_source + 1];
    for (lon_offset, lon_index) in
        (payload.bounds.minlon_source..=payload.bounds.maxlon_source).enumerate()
    {
        for (lat_offset, lat_index) in
            (payload.bounds.maxlat_source..=payload.bounds.minlat_source).enumerate()
        {
            full[lon_index][lat_index] = payload.is_in_area_select[lon_offset][lat_offset];
        }
    }
    Ok(full)
}

fn area_judge_bounds_to_getcontain_bounds(
    bounds: AreaJudgeSourceBounds,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
) -> io::Result<GetContainAreaBounds> {
    let west = *lon_vertex.get(bounds.minlon_source).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "minlon_source exceeds lon_vertex",
        )
    })?;
    let east = *lon_vertex.get(bounds.maxlon_source).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "maxlon_source exceeds lon_vertex",
        )
    })?;
    let north = *lat_vertex.get(bounds.maxlat_source).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "maxlat_source exceeds lat_vertex",
        )
    })?;
    let south = *lat_vertex.get(bounds.minlat_source).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "minlat_source exceeds lat_vertex",
        )
    })?;
    Ok(GetContainAreaBounds {
        west,
        east,
        south,
        north,
    })
}
