use std::io;

use crate::{
    MkgrdRestartRefineFinalLandPostprocContext, MkgrdRestartRefineFinalPostprocRequest,
    SelectedLandDomainMatrix,
};

/// Build the reusable final postprocess request shared by source-state and
/// landtype-source Area_judge restart-refine handoffs.
pub fn restart_refine_final_postproc_request(
    mesh_type: &str,
    requested_num_vertex: Option<usize>,
    mask_sea_ratio: f64,
    selected_land_domain: Option<&SelectedLandDomainMatrix>,
) -> io::Result<Option<MkgrdRestartRefineFinalPostprocRequest>> {
    let Some(num_vertex) = requested_num_vertex else {
        return Ok(None);
    };
    match mesh_type.trim() {
        "earthmesh" => {
            let selected = selected_land_domain.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "restart-refine earth final postproc requires selected domain state",
                )
            })?;
            Ok(Some(MkgrdRestartRefineFinalPostprocRequest::Earth {
                mask_sea_ratio,
                minlon_dm_area: i32::try_from(selected.minlon_source).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "restart-refine earth minlon does not fit i32",
                    )
                })?,
                maxlat_dm_area: i32::try_from(selected.maxlat_source).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "restart-refine earth maxlat does not fit i32",
                    )
                })?,
                nlons_dm_select: selected.nlons,
                nlats_dm_select: selected.nlats,
            }))
        }
        "landmesh" => {
            let selected = selected_land_domain.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "restart-refine land final postproc requires selected land-domain state",
                )
            })?;
            Ok(Some(MkgrdRestartRefineFinalPostprocRequest::Land(
                MkgrdRestartRefineFinalLandPostprocContext {
                    selected_seaorland: selected.seaorland.clone(),
                    minlon_dm_area: i32::try_from(selected.minlon_source).map_err(|_| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "restart-refine land minlon does not fit i32",
                        )
                    })?,
                    maxlat_dm_area: i32::try_from(selected.maxlat_source).map_err(|_| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "restart-refine land maxlat does not fit i32",
                        )
                    })?,
                    nlons_dm_select: selected.nlons,
                    nlats_dm_select: selected.nlats,
                },
            )))
        }
        "oceanmesh" => Ok(Some(MkgrdRestartRefineFinalPostprocRequest::Ocean {
            mask_sea_ratio,
            num_vertex,
        })),
        "atmos" | "atmosmesh" => Ok(Some(MkgrdRestartRefineFinalPostprocRequest::Atmos)),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "restart-refine final postproc currently supports earthmesh/landmesh/oceanmesh/atmosmesh, got {other}"
            ),
        )),
    }
}
