use std::io;

use earthmesh_core::RefineConfig;
use earthmesh_mesh::OlamRefinementRegion;

use super::bbox::{
    read_olam_bbox_refinement_regions, read_olam_calculated_bbox_refinement_regions,
};
use super::circle::{
    merge_olam_method_c_regions_by_shape, read_olam_calculated_circle_refinement_regions,
    read_olam_circle_refinement_regions,
};
use super::close::{
    read_olam_calculated_close_refinement_regions, read_olam_close_refinement_regions,
};
use crate::discover_mask_sources;

pub(crate) fn read_olam_specified_refinement_regions(
    refine: &RefineConfig,
    max_level: usize,
    nxp: usize,
    apply_parent_halos: bool,
) -> io::Result<Vec<OlamRefinementRegion>> {
    let discovery = discover_mask_sources(&refine.mask_refine_spc_fprefix)?;
    let mut regions = Vec::new();
    for source in discovery.files {
        match refine.mask_refine_spc_type.trim() {
            "circle" => read_olam_circle_refinement_regions(
                &source,
                refine,
                max_level,
                nxp,
                &mut regions,
                apply_parent_halos,
            )?,
            "bbox" => read_olam_bbox_refinement_regions(&source, max_level, &mut regions)?,
            "close" => read_olam_close_refinement_regions(&source, max_level, &mut regions)?,
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("OLAM direct specified refine does not yet support {other} regions"),
                ));
            }
        }
    }
    merge_olam_method_c_regions_by_shape(&mut regions);
    Ok(regions)
}

pub(crate) fn read_olam_calculated_refinement_regions(
    refine: &RefineConfig,
    max_level: usize,
) -> io::Result<Vec<OlamRefinementRegion>> {
    let discovery = discover_mask_sources(&refine.mask_refine_cal_fprefix)?;
    let mut regions = Vec::new();
    for source in discovery.files {
        match refine.mask_refine_cal_type.trim() {
            "circle" => {
                read_olam_calculated_circle_refinement_regions(&source, max_level, &mut regions)?
            }
            "bbox" => {
                read_olam_calculated_bbox_refinement_regions(&source, max_level, &mut regions)?
            }
            "close" => {
                read_olam_calculated_close_refinement_regions(&source, max_level, &mut regions)?
            }
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("OLAM direct calculated refine does not yet support {other} regions"),
                ));
            }
        }
    }
    Ok(regions)
}
