use std::io;

use super::selection::compact_source_state_selected_matrix_fortran_order;
use super::types::{MkgrdCompactSourceState, MkgrdCompactSourceStateFinalPostproc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MkgrdCompactSourceStateLandPostprocContext {
    pub selected_seaorland: Vec<Vec<i32>>,
    pub minlon_dm_area: i32,
    pub maxlat_dm_area: i32,
    pub nlons_dm_select: usize,
    pub nlats_dm_select: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MkgrdCompactSourceStateEarthPostprocContext {
    pub minlon_dm_area: i32,
    pub maxlat_dm_area: i32,
    pub nlons_dm_select: usize,
    pub nlats_dm_select: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MkgrdCompactSourceStateFinalPostprocRequest {
    Land(MkgrdCompactSourceStateLandPostprocContext),
    Ocean { num_vertex: usize },
    Atmos,
    Earth(MkgrdCompactSourceStateEarthPostprocContext),
}

pub fn compact_source_state_final_postproc_request(
    state: &MkgrdCompactSourceState,
) -> io::Result<Option<MkgrdCompactSourceStateFinalPostprocRequest>> {
    let Some(postproc) = state.final_domain_postproc else {
        return Ok(None);
    };
    let require_contain = |kind: &str| {
        if state.final_domain_contain.is_none() {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("source-state final_domain_postproc={kind} requires final_domain_contain"),
            ))
        } else {
            Ok(())
        }
    };
    match postproc {
        MkgrdCompactSourceStateFinalPostproc::Ocean => {
            require_contain("ocean")?;
            Ok(Some(MkgrdCompactSourceStateFinalPostprocRequest::Ocean {
                num_vertex: state.num_vertex,
            }))
        }
        MkgrdCompactSourceStateFinalPostproc::Atmos => {
            require_contain("atmos")?;
            Ok(Some(MkgrdCompactSourceStateFinalPostprocRequest::Atmos))
        }
        MkgrdCompactSourceStateFinalPostproc::Earth => {
            require_contain("earth")?;
            Ok(Some(MkgrdCompactSourceStateFinalPostprocRequest::Earth(
                MkgrdCompactSourceStateEarthPostprocContext {
                    minlon_dm_area: 1,
                    maxlat_dm_area: 1,
                    nlons_dm_select: state.nlons_source,
                    nlats_dm_select: state.nlats_source,
                },
            )))
        }
        MkgrdCompactSourceStateFinalPostproc::Land => {
            require_contain("land")?;
            Ok(Some(MkgrdCompactSourceStateFinalPostprocRequest::Land(
                MkgrdCompactSourceStateLandPostprocContext {
                    selected_seaorland: compact_source_state_selected_matrix_fortran_order(
                        &state.seaorland,
                        state.nlons_source,
                        state.nlats_source,
                    )?,
                    minlon_dm_area: 1,
                    maxlat_dm_area: 1,
                    nlons_dm_select: state.nlons_source,
                    nlats_dm_select: state.nlats_source,
                },
            )))
        }
    }
}
