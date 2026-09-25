//! The run's nominal demand, as a cell width at any site.
//!
//! MPAS delivery needs the width each cell was *asked* to have -- not the width
//! it came out at -- and three different demand producers can have run: the
//! h-field, the adaptive region passes, or LEPP's resolved region targets.
//! Which one ran is the request's business. Output code takes a
//! `NominalDemandWidth` and asks it for the MPAS context, so the writer never
//! branches on how the demand was produced.

use std::io;

use crate::hfield_gridfile_context::HfieldGridfileContext;
use crate::mpas_gridfile_context::MpasGridfileContext;
use crate::refinement_demand::nest::AdaptiveNestReport;

/// Region targets a backend has resolved to edge lengths.
///
/// The request layer states what the MPAS width needs from them and the
/// backend adapter implements it, so this module never names the backend.
pub trait ResolvedTargetWidths {
    /// The nominal target edge, in metres, at each site; `None` where no
    /// resolved target covers it.
    fn nominal_target_edges_m_at(
        &self,
        sites: &[earthmesh_mesh::LonLatDegrees],
    ) -> io::Result<Vec<Option<f64>>>;
    /// Every resolved target's edge, in metres.
    fn target_edges_m(&self) -> Vec<f64>;
    /// The deepest level any resolved target asks for.
    fn deepest_target_level(&self) -> usize;
}

#[derive(Clone, Copy)]
pub(crate) enum NominalDemandWidth<'a> {
    /// The gradient-limited target-level field.
    Hfield(&'a HfieldGridfileContext),
    /// The point+radius regions each emitted pass judged, at its own scale.
    RegionPasses {
        report: &'a AdaptiveNestReport,
        base_m: f64,
    },
    /// Region targets resolved to edge lengths.
    ResolvedTargets(&'a dyn ResolvedTargetWidths),
}

impl<'a> NominalDemandWidth<'a> {
    /// The one producer that ran, if any. Two at once is a pipeline defect:
    /// there would be no single nominal width to deliver.
    pub(crate) fn from_producers(
        hfield: Option<&'a HfieldGridfileContext>,
        region_passes: Option<(&'a AdaptiveNestReport, f64)>,
        resolved_targets: Option<&'a dyn ResolvedTargetWidths>,
    ) -> io::Result<Option<Self>> {
        match (hfield, region_passes, resolved_targets) {
            (Some(field), None, None) => Ok(Some(Self::Hfield(field))),
            (None, Some((report, base_m)), None) => Ok(Some(Self::RegionPasses { report, base_m })),
            (None, None, Some(report)) => Ok(Some(Self::ResolvedTargets(report))),
            (None, None, None) => Ok(None),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "refined output cannot have two competing MPAS demand producers",
            )),
        }
    }

    /// The width field sampled at the mesh's final W sites. `None` when the
    /// demand does not cover every site and nothing may be guessed for the rest.
    pub(crate) fn mpas_context(
        &self,
        mesh: &crate::UnstructuredMesh,
        base_nxp: usize,
    ) -> io::Result<Option<MpasGridfileContext>> {
        match *self {
            Self::Hfield(field) => {
                MpasGridfileContext::from_hfield_quantized_demand(mesh, field, base_nxp).map(Some)
            }
            Self::RegionPasses { report, base_m } => {
                MpasGridfileContext::from_adaptive_region_demand(mesh, report, base_m, base_nxp)
                    .map(Some)
            }
            Self::ResolvedTargets(report) => {
                let context =
                    MpasGridfileContext::from_resolved_target_demand(mesh, report, base_nxp)?;
                if context.is_none() {
                    eprintln!("earthmesh_cli: MPAS nominal context unavailable: resolved regions do not cover every parent W site; no background width was supplied");
                }
                Ok(context)
            }
        }
    }
}
