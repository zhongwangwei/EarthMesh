use std::io;

use earthmesh_core::{EarthmeshConfig, RefineConfig};
use earthmesh_quality::{HfieldConfigDiagnostics, MeshQualityReport, QualityMeshInput};

use super::gridfile::{
    hex_quality_cells_from_gridfile, read_gridfile_mesh_points, tri_quality_cells_from_gridfile,
};
use crate::{
    build_hfield_from_regions, olam_namelist_has_section, olam_native_method_c_uses_cartesian_xy,
    olam_native_refinement_depth, olam_native_refinement_requested, read_hfield_refine_options,
    read_olam_calculated_refinement_regions, read_olam_native_mdomain,
    read_olam_native_refine_controls, read_olam_native_refinement_regions,
    read_olam_native_sfcgrid_res_factor, read_olam_specified_refinement_regions,
    GridfileMeshPoints,
};

/// Attach h-field diagnostics to a mesh-quality report when `namelist_contents`
/// is a full mkgrd/mkrefine/hfield namelist. Plain `&quality` files return
/// `Ok(false)` and keep the legacy report shape.
pub fn attach_hfield_diagnostics_from_namelist(
    report: &mut MeshQualityReport,
    input: &QualityMeshInput,
    mesh: &GridfileMeshPoints,
    kind: &str,
    namelist_contents: &str,
) -> io::Result<bool> {
    let Some(hfield) = read_hfield_refine_options(namelist_contents)? else {
        return Ok(false);
    };
    if !olam_namelist_has_section(namelist_contents, "mkgrd") {
        return Ok(false);
    }

    let config = EarthmeshConfig::from_mkgrd_namelist(namelist_contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let is_atmosmesh = matches!(config.mesh_type.trim(), "atmos" | "atmosmesh");
    let native_mdomain = read_olam_native_mdomain(namelist_contents)?;
    let native_global_like_domain =
        native_mdomain.map_or(config.mask_domain_global, |mdomain| mdomain < 2);
    let native_surface_global_expansion =
        !is_atmosmesh && read_olam_native_sfcgrid_res_factor(namelist_contents)? > 1;
    let native_regions = read_olam_native_refinement_regions(
        namelist_contents,
        is_atmosmesh,
        native_global_like_domain,
    )?;
    let native_regions_requested =
        olam_native_refinement_requested(namelist_contents, config.mesh_type.trim())?;
    let refine = match RefineConfig::from_mkrefine_namelist(
        namelist_contents,
        config.mesh_type.trim(),
        config.mode_grid.trim(),
    ) {
        Ok(refine) => refine,
        Err(_err) if !native_regions.is_empty() || native_surface_global_expansion => {
            read_olam_native_refine_controls(namelist_contents)?
        }
        Err(err) => return Err(io::Error::new(io::ErrorKind::InvalidInput, err)),
    };
    let nxp = usize::try_from(config.nxp)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NL%NXP must fit usize"))?;
    if nxp == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NL%NXP must be positive for h-field diagnostics",
        ));
    }

    let max_spc_level = if refine.refine_spc {
        non_negative_usize(refine.max_iter_spc, "RL%max_iter_spc")?
    } else {
        0
    };
    let max_cal_level = if refine.refine_cal {
        non_negative_usize(refine.max_iter_cal, "RL%max_iter_cal")?
    } else {
        0
    };
    let max_native_level = olam_native_refinement_depth(namelist_contents, is_atmosmesh)?;
    let max_surface_expansion_level = usize::from(native_surface_global_expansion);
    let max_level = max_spc_level
        .max(max_cal_level)
        .max(max_native_level)
        .max(max_surface_expansion_level);
    let native_only_spawn = !native_regions.is_empty() && !refine.refine_spc && !refine.refine_cal;
    if olam_native_method_c_uses_cartesian_xy(
        native_mdomain,
        config.mask_domain_global,
        native_only_spawn,
    ) {
        return Ok(false);
    }

    let mut regions = native_regions;
    if refine.refine_spc {
        regions.extend(read_olam_specified_refinement_regions(
            &refine,
            max_spc_level,
            nxp,
            false,
        )?);
    }
    if refine.refine_cal {
        regions.extend(read_olam_calculated_refinement_regions(
            &refine,
            max_cal_level,
        )?);
    }
    if regions.is_empty() {
        let requested = if native_regions_requested {
            "native OLAM or mask-refine"
        } else {
            "mask-refine"
        };
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "h-field diagnostics found no {requested} region sources; check RL%mask_refine_*_fprefix/type or native OLAM ngrids/nsfcgrids"
            ),
        ));
    }

    let base_m = hfield.base_m.unwrap_or_else(|| {
        2.0 * std::f64::consts::PI * earthmesh_hfield::EARTH_RADIUS_METERS / (5.0 * nxp as f64)
    });
    let field_max_level = hfield.max_level.unwrap_or(max_level).clamp(1, 5);
    let field = build_hfield_from_regions(&regions, base_m, hfield.g, hfield.nlon, hfield.nlat)?;
    let target_levels = hfield_target_levels_for_quality_cells(mesh, kind, |lon, lat| {
        field.level_at(lon, lat, base_m, field_max_level as u8) as u32
    })?;
    if target_levels.len() != input.cells.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "h-field target levels ({}) do not match quality cells ({})",
                target_levels.len(),
                input.cells.len()
            ),
        ));
    }

    earthmesh_quality::attach_hfield_diagnostics(
        report,
        input,
        &target_levels,
        HfieldConfigDiagnostics {
            enabled: true,
            g: Some(hfield.g),
            max_level: Some(field_max_level as u32),
            base_m: Some(base_m),
        },
    );
    Ok(true)
}

pub fn attach_hfield_diagnostics_from_gridfile_namelist(
    report: &mut MeshQualityReport,
    input: &QualityMeshInput,
    gridfile: impl AsRef<std::path::Path>,
    kind: &str,
    namelist_contents: &str,
) -> io::Result<bool> {
    let mesh = read_gridfile_mesh_points(gridfile)?;
    attach_hfield_diagnostics_from_namelist(report, input, &mesh, kind, namelist_contents)
}

fn hfield_target_levels_for_quality_cells(
    mesh: &GridfileMeshPoints,
    kind: &str,
    mut level_at: impl FnMut(f64, f64) -> u32,
) -> io::Result<Vec<u32>> {
    match kind.trim() {
        "tri" => Ok(tri_quality_cells_from_gridfile(mesh)
            .into_iter()
            .map(|(mi, _)| level_at(mesh.m_lon[mi], mesh.m_lat[mi]))
            .collect()),
        "hex" => Ok(hex_quality_cells_from_gridfile(mesh)
            .into_iter()
            .map(|(_wi, corners)| max_corner_level(mesh, &corners, &mut level_at))
            .collect()),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("h-field quality diagnostics support tri or hex view, got {other}"),
        )),
    }
}

fn max_corner_level(
    mesh: &GridfileMeshPoints,
    corners: &[usize],
    level_at: &mut impl FnMut(f64, f64) -> u32,
) -> u32 {
    corners
        .iter()
        .filter_map(|&mi| Some(level_at(*mesh.m_lon.get(mi)?, *mesh.m_lat.get(mi)?)))
        .max()
        .unwrap_or(0)
}

fn non_negative_usize(value: i32, field: &str) -> io::Result<usize> {
    usize::try_from(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{field} must be non-negative, got {value}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_hfield_targets_follow_corner_demand_not_cell_center_only() {
        let mesh = GridfileMeshPoints {
            m_lon: vec![0.0, 0.0, 1.0, 2.0],
            m_lat: vec![0.0, 1.0, 0.0, 0.0],
            w_lon: vec![100.0, 101.0, 102.0],
            w_lat: vec![0.0, 0.0, 0.0],
            m_to_w: vec![1, 2, 3, 1, 2, 3, 1, 2, 3, 2, 3, 1],
            m_refine_level: Vec::new(),
            w_to_m: Vec::new(),
            w_to_m_width: 0,
            n_w: Vec::new(),
            w_refine_level: Vec::new(),
        };

        let targets = hfield_target_levels_for_quality_cells(&mesh, "hex", |lon, _lat| {
            if (lon - 2.0).abs() < f64::EPSILON {
                2
            } else {
                0
            }
        })
        .unwrap();

        assert!(targets.contains(&2));
    }
}
