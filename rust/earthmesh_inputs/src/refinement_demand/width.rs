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
use crate::mpas_gridfile_context::{invalid, MpasGridfileContext};
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
pub enum NominalDemandWidth<'a> {
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
    pub fn from_producers(
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
    pub fn mpas_context(
        &self,
        mesh: &crate::UnstructuredMesh,
        base_nxp: usize,
    ) -> io::Result<Option<MpasGridfileContext>> {
        match *self {
            Self::Hfield(field) => mpas_context_from_hfield(mesh, field, base_nxp).map(Some),
            Self::RegionPasses { report, base_m } => {
                mpas_context_from_region_passes(mesh, report, base_m, base_nxp).map(Some)
            }
            Self::ResolvedTargets(report) => {
                let context = mpas_context_from_resolved_targets(mesh, report, base_nxp)?;
                if context.is_none() {
                    eprintln!("earthmesh_cli: MPAS nominal context unavailable: resolved regions do not cover every parent W site; no background width was supplied");
                }
                Ok(context)
            }
        }
    }
}

// The three nominal widths, each derived from the demand it names. They live
// with the demand rather than with the MPAS context, so the context -- a
// gridfile record -- depends on nothing upstream of the mesh.

/// Nominal generation demand evaluated at final W sites, not realized
/// geometry, birth levels or the legacy Spring U-edge targets. The complete
/// field defines the reference even when no W site reaches its finest level.
pub fn mpas_context_from_hfield(
    mesh: &crate::UnstructuredMesh,
    hfield: &HfieldGridfileContext,
    base_nxp: usize,
) -> io::Result<MpasGridfileContext> {
    hfield.validate()?;
    let finest = hfield
        .field
        .level_map(hfield.base_m, hfield.max_level)?
        .into_iter()
        .max()
        .ok_or_else(|| invalid("HField demand is empty"))?;
    let width_km = |level: u8| hfield.base_m / 2.0_f64.powi(i32::from(level)) / 1000.0;
    let reference = width_km(finest);
    let first =
        crate::unstructured_mesh_support::unstructured_w_row_layout(mesh).first_physical_row;
    if first >= mesh.w_points.len() {
        return Err(invalid("HField MPAS demand requires physical W cells"));
    }
    let mut cellwidth_km = vec![reference; mesh.w_points.len()];
    for (width, point) in cellwidth_km[first..]
        .iter_mut()
        .zip(&mesh.w_points[first..])
    {
        *width = width_km(hfield.field.try_level_at(
            point.lon,
            point.lat,
            hfield.base_m,
            hfield.max_level,
        )?);
    }
    let context = MpasGridfileContext {
        cellwidth_km,
        base_nxp,
        step: usize::from(finest) + 1,
        density_reference_width_km: reference,
        source: crate::mpas_gridfile_context::HFIELD_QUANTIZED_DEMAND_V1.to_string(),
    };
    context.validate(mesh.w_points.len())?;
    Ok(context)
}

/// Nominal emitted-pass demand, not achieved resolution or unexecuted requests.
/// The shared region predicate is sampled at final W sites; closure/transition
/// geometry does not change the nominal field. Missing reports never call this.
pub fn mpas_context_from_region_passes(
    mesh: &crate::UnstructuredMesh,
    report: &AdaptiveNestReport,
    base_m: f64,
    base_nxp: usize,
) -> io::Result<MpasGridfileContext> {
    if !base_m.is_finite()
        || base_m <= 0.0
        || report.deepest_level != report.passes.len()
        || report.passes.len() > 5
    {
        return Err(invalid(
            "adaptive MPAS demand requires a finite positive base and consistent depth in 0..=5",
        ));
    }
    let width_m = |level: usize| base_m / 2.0_f64.powi(level as i32);
    let mut indices = Vec::with_capacity(report.passes.len());
    for (index, pass) in report.passes.iter().enumerate() {
        if pass.level != index + 1
            || pass.cell_meters != width_m(index)
            || !pass
                .regions
                .iter()
                .any(|region| region.level() >= pass.level)
        {
            return Err(invalid("adaptive MPAS demand requires contiguous active passes and the actual judging-generation scale"));
        }
        for region in &pass.regions {
            region.validate()?;
        }
        indices.push(earthmesh_mesh::RefinementRegionIndex::new(&pass.regions));
    }
    let first =
        crate::unstructured_mesh_support::unstructured_w_row_layout(mesh).first_physical_row;
    if first >= mesh.w_points.len() {
        return Err(invalid("adaptive MPAS demand requires physical W cells"));
    }
    let reference = width_m(report.deepest_level) / 1000.0;
    let mut cellwidth_km = vec![reference; mesh.w_points.len()];
    for (width, point) in cellwidth_km[first..]
        .iter_mut()
        .zip(&mesh.w_points[first..])
    {
        if !point.lon.is_finite() || !point.lat.is_finite() || !(-90.0..=90.0).contains(&point.lat)
        {
            return Err(invalid(
                "adaptive MPAS demand requires finite geographic W sites",
            ));
        }
        let site = earthmesh_mesh::LonLatDegrees::new(point.lon, point.lat);
        let level = indices
            .iter()
            .enumerate()
            .rev()
            .find(|(index, regions)| regions.contains_lonlat_canonical(site, index + 1))
            .map_or(0, |(index, _)| index + 1);
        *width = width_m(level) / 1000.0;
    }
    let context = MpasGridfileContext {
        cellwidth_km,
        base_nxp,
        step: report.deepest_level + 1,
        density_reference_width_km: reference,
        source: crate::mpas_gridfile_context::ADAPTIVE_REGION_PASS_DEMAND_V1.to_string(),
    };
    context.validate(mesh.w_points.len())?;
    Ok(context)
}

/// Resolved nominal region demand only. No target outside its regions
/// means no complete MPAS context, never a guessed background width.
pub fn mpas_context_from_resolved_targets(
    mesh: &crate::UnstructuredMesh,
    report: &dyn ResolvedTargetWidths,
    base_nxp: usize,
) -> io::Result<Option<MpasGridfileContext>> {
    let first =
        crate::unstructured_mesh_support::unstructured_w_row_layout(mesh).first_physical_row;
    if first >= mesh.w_points.len() || base_nxp == 0 || i32::try_from(base_nxp).is_err() {
        return Err(invalid(
            "resolved-target MPAS demand requires physical W cells and positive i32 NXP",
        ));
    }
    let sites = mesh.w_points[first..]
        .iter()
        .map(|point| earthmesh_mesh::LonLatDegrees::new(point.lon, point.lat))
        .collect::<Vec<_>>();
    let targets = report.nominal_target_edges_m_at(&sites)?;
    let reference = report
        .target_edges_m()
        .into_iter()
        .map(|edge_m| edge_m / 1000.0)
        .reduce(f64::min);
    if reference.is_some_and(|value| !value.is_finite() || value <= 0.0) {
        return Err(invalid(
            "resolved-target MPAS scale cannot be represented in km",
        ));
    }
    let Some(widths) = targets.into_iter().collect::<Option<Vec<_>>>() else {
        return Ok(None);
    };
    let reference =
        reference.ok_or_else(|| invalid("resolved-target covered demand has no targets"))?;
    let mut cellwidth_km = vec![reference; first];
    cellwidth_km.extend(widths.into_iter().map(|width| width / 1000.0));
    let context = MpasGridfileContext {
        cellwidth_km,
        base_nxp,
        step: report.deepest_target_level() + 1,
        density_reference_width_km: reference,
        source: crate::mpas_gridfile_context::LEPP_RESOLVED_REGION_DEMAND_V1.to_string(),
    };
    context.validate(mesh.w_points.len())?;
    Ok(Some(context))
}
