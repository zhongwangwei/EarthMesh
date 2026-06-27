use std::io;

use super::{
    finalize_mask_postproc_layout_to_unstructured_mesh, mask_postproc_layout_from_unstructured_mesh,
};
use crate::{
    read_contain_netcdf, read_unstructured_mesh_netcdf, write_unstructured_mesh_netcdf,
    MaskPostprocDomainInputs, MaskPostprocDomainIoPlan, MaskPostprocLayout,
    UnstructuredMeshWriteReport,
};

/// Compose final mask-postprocess grid construction with the legacy NetCDF
/// result path selected by `plan_mask_postproc_domain_io`.
pub fn write_mask_postproc_final_gridfile(
    plan: &MaskPostprocDomainIoPlan,
    layout: &MaskPostprocLayout,
    is_in_domain_ustr: &[i32],
) -> io::Result<UnstructuredMeshWriteReport> {
    let mesh = finalize_mask_postproc_layout_to_unstructured_mesh(
        layout,
        is_in_domain_ustr,
        &plan.mode_grid,
    )?;
    write_unstructured_mesh_netcdf(&plan.result_gridfile, &mesh)
}

/// Load the two NetCDF inputs common to `mask_postproc_Earth`,
/// `mask_postproc_Lnd`, and `mask_postproc_Ocn`: the source unstructured
/// gridfile and the contain-domain mask table.
pub fn read_mask_postproc_domain_inputs(
    plan: &MaskPostprocDomainIoPlan,
) -> io::Result<MaskPostprocDomainInputs> {
    let source_mesh = read_unstructured_mesh_netcdf(&plan.source_gridfile)?;
    let contain = read_contain_netcdf(&plan.contain_domain)?;
    let layout = normalize_mask_postproc_layout_for_contain_domain(
        mask_postproc_layout_from_unstructured_mesh(&source_mesh, &plan.mode_grid)?,
        contain.is_in_area_ustr.len(),
    );
    let is_in_domain_ustr = contain.is_in_area_ustr.clone();

    Ok(MaskPostprocDomainInputs {
        layout,
        contain,
        is_in_domain_ustr,
    })
}

fn normalize_mask_postproc_layout_for_contain_domain(
    layout: MaskPostprocLayout,
    contain_ustr_len: usize,
) -> MaskPostprocLayout {
    let max_vertex_id = layout
        .center_neighbors
        .iter()
        .zip(layout.center_neighbor_counts.iter())
        .flat_map(|(row, &count)| row.iter().take(count).copied())
        .max()
        .unwrap_or(0);
    if contain_ustr_len <= layout.ustr_points || max_vertex_id < layout.ustr_bounds {
        return layout;
    }

    super::placeholder::add_leading_mask_postproc_placeholder(layout)
}
