use crate::plan_mask_postproc_domain_io;
use crate::read_contain_netcdf;
use std::{io, path::PathBuf};

use earthmesh_core::EarthmeshConfig;

pub fn infer_mask_restart_ocean_num_vertex_from_config(
    config: &EarthmeshConfig,
) -> io::Result<usize> {
    if config.nxp <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mask_restart ocean postproc requires positive NL%NXP to infer num_vertex, got {}",
                config.nxp
            ),
        ));
    }
    let nxp = usize::try_from(config.nxp)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NXP must fit usize"))?;
    let postproc_plan = plan_mask_postproc_domain_io(
        PathBuf::from(config.file_dir()),
        nxp,
        config.mode_grid.trim(),
        config.mesh_type.trim(),
        config.mask_patch_on,
    )?;
    let contain = read_contain_netcdf(&postproc_plan.contain_domain)?;
    if contain.ustr_ii.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mask_restart ocean postproc cannot infer num_vertex from empty ustr_ii in {}",
                postproc_plan.contain_domain.display()
            ),
        ));
    }
    Ok(contain.ustr_ii.len())
}

pub fn maybe_infer_mask_restart_ocean_num_vertex_from_config(
    config: &EarthmeshConfig,
) -> io::Result<Option<usize>> {
    if config.nxp <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mask_restart ocean postproc requires positive NL%NXP to infer num_vertex, got {}",
                config.nxp
            ),
        ));
    }
    let nxp = usize::try_from(config.nxp)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NXP must fit usize"))?;
    let postproc_plan = plan_mask_postproc_domain_io(
        PathBuf::from(config.file_dir()),
        nxp,
        config.mode_grid.trim(),
        config.mesh_type.trim(),
        config.mask_patch_on,
    )?;
    if !postproc_plan.contain_domain.exists() {
        return Ok(None);
    }
    let contain = read_contain_netcdf(&postproc_plan.contain_domain)?;
    if contain.ustr_ii.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mask_restart ocean postproc cannot infer num_vertex from empty ustr_ii in {}",
                postproc_plan.contain_domain.display()
            ),
        ));
    }
    Ok(Some(contain.ustr_ii.len()))
}

pub fn maybe_infer_mask_restart_non_ocean_num_vertex_from_config(
    config: &EarthmeshConfig,
) -> io::Result<Option<usize>> {
    if config.nxp <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mask_restart non-ocean postproc requires positive NL%NXP to infer num_vertex, got {}",
                config.nxp
            ),
        ));
    }
    let nxp = usize::try_from(config.nxp)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NXP must fit usize"))?;
    let postproc_plan = plan_mask_postproc_domain_io(
        PathBuf::from(config.file_dir()),
        nxp,
        config.mode_grid.trim(),
        config.mesh_type.trim(),
        config.mask_patch_on,
    )?;
    if !postproc_plan.contain_domain.exists() {
        return Ok(None);
    }
    let contain = read_contain_netcdf(&postproc_plan.contain_domain)?;
    if contain.ustr_ii.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mask_restart non-ocean postproc cannot infer num_vertex from empty ustr_ii in {}",
                postproc_plan.contain_domain.display()
            ),
        ));
    }
    Ok(Some(contain.ustr_ii.len()))
}

pub fn restart_refine_initial_gridfile_path_from_config(
    config: &EarthmeshConfig,
) -> io::Result<PathBuf> {
    if config.nxp <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "restart-refine handoff requires positive NL%NXP to infer initial gridfile, got {}",
                config.nxp
            ),
        ));
    }
    let nxp = usize::try_from(config.nxp)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NXP must fit usize"))?;
    Ok(PathBuf::from(config.file_dir())
        .join("gridfile")
        .join(format!(
            "gridfile_NXP{nxp:04}_01_{}.nc4",
            config.mode_grid.trim()
        )))
}

pub fn infer_restart_refine_initial_gridfile_from_config(
    config: &EarthmeshConfig,
) -> io::Result<PathBuf> {
    let path = restart_refine_initial_gridfile_path_from_config(config)?;
    if path.exists() {
        Ok(path)
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "restart-refine handoff requires --restart-refine-initial-gridfile or existing initial gridfile {}",
                path.display()
            ),
        ))
    }
}

pub fn maybe_infer_restart_refine_initial_gridfile_from_config(
    config: &EarthmeshConfig,
) -> io::Result<Option<PathBuf>> {
    let path = restart_refine_initial_gridfile_path_from_config(config)?;
    Ok(path.exists().then_some(path))
}

pub fn namelist_sets_landtype_file(contents: &str) -> bool {
    contents
        .lines()
        .map(|line| line.split('!').next().unwrap_or(""))
        .any(|line| line.to_ascii_lowercase().contains("landtype_file"))
}

pub fn landtype_file_is_real(landtype_file: &str) -> bool {
    let trimmed = landtype_file.trim();
    !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("none") && trimmed != "/tmp"
}
