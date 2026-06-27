use std::io;

use crate::*;

/// Build the reusable final-domain postprocess request for
/// data_preprocess-derived source-state handoffs.
pub fn data_preprocess_source_state_final_postproc_request(
    state: &MkgrdDataPreprocessSourceState,
    mesh_type: &str,
) -> io::Result<Option<MkgrdDataPreprocessSourceStateFinalPostprocRequest>> {
    match mesh_type.trim() {
        "earthmesh" => {
            let selected = selected_land_domain_from_full_source_seaorland_fortran_order(
                &state.seaorland,
                state.nlons_source,
                state.nlats_source,
            )?;
            Ok(Some(
                MkgrdDataPreprocessSourceStateFinalPostprocRequest::Earth(
                    MkgrdDataPreprocessSourceStateEarthPostprocContext {
                        minlon_dm_area: i32::try_from(selected.minlon_source).map_err(|_| {
                            io::Error::new(
                                io::ErrorKind::InvalidInput,
                                "earth source-state minlon does not fit i32",
                            )
                        })?,
                        maxlat_dm_area: i32::try_from(selected.maxlat_source).map_err(|_| {
                            io::Error::new(
                                io::ErrorKind::InvalidInput,
                                "earth source-state maxlat does not fit i32",
                            )
                        })?,
                        nlons_dm_select: selected.nlons,
                        nlats_dm_select: selected.nlats,
                    },
                ),
            ))
        }
        "landmesh" => {
            let selected = selected_land_domain_from_full_source_seaorland_fortran_order(
                &state.seaorland,
                state.nlons_source,
                state.nlats_source,
            )?;
            Ok(Some(
                MkgrdDataPreprocessSourceStateFinalPostprocRequest::Land(
                    MkgrdDataPreprocessSourceStateLandPostprocContext {
                        selected_seaorland: selected.seaorland,
                        minlon_dm_area: i32::try_from(selected.minlon_source).map_err(|_| {
                            io::Error::new(
                                io::ErrorKind::InvalidInput,
                                "landtype-source minlon does not fit i32",
                            )
                        })?,
                        maxlat_dm_area: i32::try_from(selected.maxlat_source).map_err(|_| {
                            io::Error::new(
                                io::ErrorKind::InvalidInput,
                                "landtype-source maxlat does not fit i32",
                            )
                        })?,
                        nlons_dm_select: selected.nlons,
                        nlats_dm_select: selected.nlats,
                    },
                ),
            ))
        }
        "oceanmesh" => Ok(Some(
            MkgrdDataPreprocessSourceStateFinalPostprocRequest::Ocean {
                num_vertex: state.num_vertex,
            },
        )),
        "atmos" | "atmosmesh" => Ok(Some(
            MkgrdDataPreprocessSourceStateFinalPostprocRequest::Atmos,
        )),
        _ => Ok(None),
    }
}

/// Map a typed data_preprocess final postprocess request into the borrowed
/// runner options consumed by the migrated final-domain handoff.
pub fn data_preprocess_source_state_final_postproc_options<'a>(
    request: Option<&'a MkgrdDataPreprocessSourceStateFinalPostprocRequest>,
    state: &'a MkgrdDataPreprocessSourceState,
    mask_sea_ratio: f64,
    output_format: &'a str,
) -> io::Result<Option<MkgrdFinalDomainPostprocOptions<'a>>> {
    match request {
        Some(MkgrdDataPreprocessSourceStateFinalPostprocRequest::Earth(context)) => {
            Ok(Some(MkgrdFinalDomainPostprocOptions::EarthFromFinalGrid(
                MkgrdFinalDomainEarthAutoPostprocOptions {
                    mask_sea_ratio,
                    minlon_dm_area: context.minlon_dm_area,
                    maxlat_dm_area: context.maxlat_dm_area,
                    nlons_dm_select: context.nlons_dm_select,
                    nlats_dm_select: context.nlats_dm_select,
                    lon_vertex: &state.lon_vertex,
                    lat_vertex: &state.lat_vertex,
                    lon_i: &state.lon_i,
                    lat_i: &state.lat_i,
                },
            )))
        }
        Some(MkgrdDataPreprocessSourceStateFinalPostprocRequest::Land(context)) => Ok(Some(
            MkgrdFinalDomainPostprocOptions::Land(MaskPostprocLandRunOptions {
                seaorland: &context.selected_seaorland,
                minlon_dm_area: context.minlon_dm_area,
                maxlat_dm_area: context.maxlat_dm_area,
                nlons_dm_select: context.nlons_dm_select,
                nlats_dm_select: context.nlats_dm_select,
                lon_vertex: &state.lon_vertex,
                lat_vertex: &state.lat_vertex,
                lon_i: &state.lon_i,
                lat_i: &state.lat_i,
            }),
        )),
        Some(MkgrdDataPreprocessSourceStateFinalPostprocRequest::Ocean { num_vertex }) => Ok(Some(
            MkgrdFinalDomainPostprocOptions::Ocean(MaskPostprocOceanRunOptions {
                mask_sea_ratio,
                num_vertex: *num_vertex,
            }),
        )),
        Some(MkgrdDataPreprocessSourceStateFinalPostprocRequest::Atmos) => {
            Ok(Some(MkgrdFinalDomainPostprocOptions::Atmos {
                output_format: output_format.trim(),
            }))
        }
        None => Ok(None),
    }
}

/// Infer the legacy `num_vertex` boundary from `NL%mode_grid` for migrated
/// mkgrd paths that do not have a persisted mesh-count handoff yet.
pub fn mkgrd_mode_grid_num_vertex(mode_grid: &str) -> io::Result<usize> {
    match mode_grid.trim() {
        "tri" => Ok(3),
        "hex" => Ok(6),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported NL%mode_grid for migrated landtype-source execution: {other}"),
        )),
    }
}

/// Convert a Fortran-indexed landtype raster into the one-based sea/land mask
/// consumed by restarted final-domain `Get_Contain` and land postprocess
/// handoffs: `0` stays ocean/empty, any non-zero landtype becomes land `1`.
pub fn seaorland_from_landtypes_global_fortran_indexed(landtypes: &[Vec<i32>]) -> Vec<Vec<i32>> {
    landtypes
        .iter()
        .map(|row| {
            row.iter()
                .map(|&landtype| if landtype == 0 { 0 } else { 1 })
                .collect()
        })
        .collect()
}
