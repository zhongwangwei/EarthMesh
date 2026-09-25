//! LEPP's resolved region targets, as the request layer's MPAS width reads them.

use std::io;

use crate::refinement_demand::width::ResolvedTargetWidths;

impl ResolvedTargetWidths for earthmesh_refine_method_c::AdaptiveHybridReport {
    fn nominal_target_edges_m_at(
        &self,
        sites: &[earthmesh_mesh::LonLatDegrees],
    ) -> io::Result<Vec<Option<f64>>> {
        self.nominal_target_edges_at(sites)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))
    }

    fn target_edges_m(&self) -> Vec<f64> {
        self.resolved_targets
            .iter()
            .map(|target| target.target_edge_m)
            .collect()
    }

    fn deepest_target_level(&self) -> usize {
        self.resolved_targets
            .iter()
            .map(|target| target.demand.region.level())
            .max()
            .unwrap_or(0)
    }
}
