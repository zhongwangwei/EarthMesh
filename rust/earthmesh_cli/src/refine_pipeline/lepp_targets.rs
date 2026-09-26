//! LEPP's resolved region targets, as the request layer's MPAS width reads them.

use std::io;

use crate::refinement_demand::width::ResolvedTargetWidths;

/// LEPP's report, as the request layer's `ResolvedTargetWidths`. A wrapper
/// because neither the trait (earthmesh_inputs) nor the report
/// (earthmesh_refine_method_c) belongs to this crate.
pub struct LeppResolvedTargets<'a>(pub &'a earthmesh_refine_method_c::AdaptiveHybridReport);

impl ResolvedTargetWidths for LeppResolvedTargets<'_> {
    fn nominal_target_edges_m_at(
        &self,
        sites: &[earthmesh_mesh::LonLatDegrees],
    ) -> io::Result<Vec<Option<f64>>> {
        self.0
            .nominal_target_edges_at(sites)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))
    }

    fn target_edges_m(&self) -> Vec<f64> {
        self.0
            .resolved_targets
            .iter()
            .map(|target| target.target_edge_m)
            .collect()
    }

    fn deepest_target_level(&self) -> usize {
        self.0
            .resolved_targets
            .iter()
            .map(|target| target.demand.region.level())
            .max()
            .unwrap_or(0)
    }
}
