use std::io;

use earthmesh_core::RefineConfig;
use earthmesh_mesh::MethodCRefinementRegion;
use earthmesh_project::CloseBoundaryMode;

use super::bbox::{
    read_method_c_bbox_refinement_regions, read_method_c_calculated_bbox_refinement_regions,
};
use super::circle::{
    merge_refine_regions_by_shape, read_method_c_calculated_circle_refinement_regions,
    read_method_c_circle_refinement_regions,
};
use super::close::{
    read_method_c_calculated_close_refinement_regions, read_method_c_close_refinement_regions,
};
use super::shared::{parse_inline_mask_source, InlineMaskSource};
use crate::discover_mask_sources;

pub(crate) fn read_method_c_specified_refinement_regions(
    refine: &RefineConfig,
    max_level: usize,
    nxp: usize,
    apply_parent_halos: bool,
) -> io::Result<Vec<MethodCRefinementRegion>> {
    if let Some(source) = parse_inline_mask_source(&refine.mask_refine_spc_fprefix)? {
        let mut regions = Vec::new();
        match (refine.mask_refine_spc_type.trim(), source) {
            (
                "bbox",
                InlineMaskSource::Bbox {
                    west,
                    east,
                    south,
                    north,
                },
            ) => regions.push(MethodCRefinementRegion::Bbox {
                west_degrees: west,
                east_degrees: east,
                south_degrees: south,
                north_degrees: north,
                level: max_level,
            }),
            (
                "circle",
                InlineMaskSource::Circle {
                    center,
                    radius_meters,
                },
            ) => {
                let points = vec![center];
                let radii = vec![radius_meters];
                if apply_parent_halos {
                    super::circle::push_method_c_circle_or_corridor_region_with_parent_halos(
                        &mut regions,
                        points,
                        radii,
                        max_level,
                        refine,
                        nxp,
                    )?;
                } else {
                    super::circle::push_method_c_circle_or_corridor_region(
                        &mut regions,
                        points,
                        radii,
                        max_level,
                    );
                }
            }
            ("circle", InlineMaskSource::Circles(circles)) => {
                // Each member is pushed on its own. Handing the whole set to
                // `push_method_c_circle_or_corridor_region` would make one
                // Corridor — the swept tube that expresses a river — and a
                // chain is not a polyline: it is a set of independent circles,
                // so the tube would cut across whatever lies between them.
                // Pushing one at a time still reuses the parent-halo
                // derivation, per circle.
                for (center, radius_meters) in circles {
                    if apply_parent_halos {
                        super::circle::push_method_c_circle_or_corridor_region_with_parent_halos(
                            &mut regions,
                            vec![center],
                            vec![radius_meters],
                            max_level,
                            refine,
                            nxp,
                        )?;
                    } else {
                        super::circle::push_method_c_circle_or_corridor_region(
                            &mut regions,
                            vec![center],
                            vec![radius_meters],
                            max_level,
                        );
                    }
                }
            }
            (kind, _) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                    "inline specified refinement source does not match mask_refine_spc_type {kind}"
                ),
                ))
            }
        }
        merge_refine_regions_by_shape(&mut regions);
        return Ok(regions);
    }
    let discovery = discover_mask_sources(&refine.mask_refine_spc_fprefix)?;
    let close_boundary =
        CloseBoundaryMode::from_engine_spec(&refine.mask_refine_spc_close_boundary)
            .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;
    let mut regions = Vec::new();
    for source in discovery.files {
        match refine.mask_refine_spc_type.trim() {
            "circle" => read_method_c_circle_refinement_regions(
                &source,
                refine,
                max_level,
                nxp,
                &mut regions,
                apply_parent_halos,
            )?,
            "bbox" => read_method_c_bbox_refinement_regions(&source, max_level, &mut regions)?,
            "close" => read_method_c_close_refinement_regions(
                &source,
                max_level,
                &close_boundary,
                &mut regions,
            )?,
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "Method-C direct specified refine does not yet support {other} regions"
                    ),
                ));
            }
        }
    }
    merge_refine_regions_by_shape(&mut regions);
    Ok(regions)
}

pub(crate) fn read_method_c_calculated_refinement_regions(
    refine: &RefineConfig,
    max_level: usize,
) -> io::Result<Vec<MethodCRefinementRegion>> {
    let discovery = discover_mask_sources(&refine.mask_refine_cal_fprefix)?;
    let mut regions = Vec::new();
    for source in discovery.files {
        match refine.mask_refine_cal_type.trim() {
            "circle" => read_method_c_calculated_circle_refinement_regions(
                &source,
                max_level,
                &mut regions,
            )?,
            "bbox" => {
                read_method_c_calculated_bbox_refinement_regions(&source, max_level, &mut regions)?
            }
            "close" => {
                read_method_c_calculated_close_refinement_regions(&source, max_level, &mut regions)?
            }
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "Method-C direct calculated refine does not yet support {other} regions"
                    ),
                ));
            }
        }
    }
    Ok(regions)
}
