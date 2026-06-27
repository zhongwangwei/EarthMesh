use earthmesh_mesh::GlobalDistanceStep;

use crate::*;

pub(super) fn final_quality_global_distance_steps(
    plan: &MkgrdFinalQualityGlobalSpringIoPlan,
) -> Vec<GlobalDistanceStep<'_>> {
    plan.distance_steps
        .iter()
        .map(|step| GlobalDistanceStep {
            active: step.active,
            halo: step.halo,
            refinement_flags: &step.refinement_flags,
            num_vertex_in: step.num_vertex_in,
            num_center_in: step.num_center_in,
        })
        .collect()
}

pub(super) fn final_quality_global_spring_options<'a>(
    plan: &'a MkgrdFinalQualityGlobalSpringIoPlan,
    distance_steps: &'a [GlobalDistanceStep<'a>],
) -> SpringjustmentGlobalRunOptions<'a> {
    SpringjustmentGlobalRunOptions {
        base_dists_on_edge: plan.base_dists_on_edge,
        base_cellwidth: plan.base_cellwidth,
        distance_num_rc: plan.distance_num_rc,
        distance_spacing: plan.distance_spacing,
        distance_steps,
        niter_refine: plan.niter_refine,
        relax: plan.relax,
        radius: plan.radius,
        diagnostic_every: 100,
    }
}
