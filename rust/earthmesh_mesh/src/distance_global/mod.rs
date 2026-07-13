use crate::spring_global_debug;
use crate::{
    cellwidth_layers_one_based, distance_layers, dists_on_edge_layers_one_based,
    DistanceLayerSpacing,
};

/// One active or skipped refinement iteration for the Rust port of
/// `MOD_grid_preprocess:set_distsOnEdge_global`.
#[derive(Debug, Clone, Copy)]
pub struct GlobalDistanceStep<'a> {
    pub active: bool,
    pub halo: usize,
    pub refinement_flags: &'a [bool],
    pub num_vertex_in: usize,
    pub num_center_in: usize,
}

/// Borrowed inputs for the pure calculation side of
/// `MOD_grid_preprocess:set_distsOnEdge_global`.
#[derive(Debug, Clone, Copy)]
pub struct SetDistsOnEdgeGlobalInput<'a> {
    pub base_dists_on_edge: f64,
    pub base_cellwidth: Option<f64>,
    pub num_rc: usize,
    pub spacing: DistanceLayerSpacing,
    pub triangles_on_cell: &'a [Vec<usize>],
    pub cells_on_triangle: Option<&'a [[usize; 3]]>,
    pub edges_on_vertex: &'a [[usize; 3]],
    pub cells_on_edge: &'a [[usize; 2]],
    pub steps: &'a [GlobalDistanceStep<'a>],
}

/// Output from `set_distsOnEdge_global` calculation orchestration.
#[derive(Debug, Clone, PartialEq)]
pub struct SetDistsOnEdgeGlobalOutput {
    pub dists_on_edge: Vec<f64>,
    pub cellwidth: Option<Vec<f64>>,
}

/// Rust orchestration wrapper for `MOD_grid_preprocess:set_distsOnEdge_global`.
///
/// The Canonical routine derives refined-region flags through
/// `refine_sjx_regional_make` and reads global `halo`, `step`, and
/// `exit_loop_step` state. This pure Rust wrapper keeps the same distance
/// update sequence but accepts each iteration's refinement flags explicitly:
/// initialize background values, halve the selected edge/cellwidth scale after
/// each active iteration, build transition layers, then call the current
/// `distsOnEdge_layers_make` and optional `cellwidth_layers_make` kernels.
pub fn set_dists_on_edge_global_one_based(
    input: SetDistsOnEdgeGlobalInput<'_>,
) -> Option<SetDistsOnEdgeGlobalOutput> {
    let mut dists_on_edge = vec![input.base_dists_on_edge; input.cells_on_edge.len()];
    let mut cellwidth = input
        .base_cellwidth
        .map(|base| vec![base; input.triangles_on_cell.len()]);

    if cellwidth.is_some() && input.cells_on_triangle.is_none() {
        return None;
    }

    let mut edge_scale = input.base_dists_on_edge;
    let mut cellwidth_scale = input.base_cellwidth;

    for step in input.steps {
        if !step.active {
            continue;
        }
        spring_global_debug(&format!(
            "distance step halo={} num_vertex_in={} num_center_in={} flags={} active_after_vertex={}",
            step.halo,
            step.num_vertex_in,
            step.num_center_in,
            step.refinement_flags.len(),
            step.refinement_flags
                .iter()
                .enumerate()
                .filter(|(idx, flag)| **flag && *idx > step.num_vertex_in)
                .count()
        ));
        let dist_len = step.halo + input.num_rc;
        if dist_len == 0 {
            return None;
        }

        let current_edge_scale = edge_scale;
        edge_scale = current_edge_scale / 2.0;
        let edge_layers = distance_layers(2 * dist_len, current_edge_scale, input.spacing)?;
        let before_changed = dists_on_edge
            .iter()
            .filter(|value| (**value - input.base_dists_on_edge).abs() > 1.0e-12)
            .count();
        dists_on_edge = dists_on_edge_layers_one_based(
            step.num_vertex_in,
            step.num_center_in,
            input.num_rc,
            dist_len,
            input.triangles_on_cell,
            input.edges_on_vertex,
            input.cells_on_edge,
            &edge_layers,
            step.refinement_flags,
            &dists_on_edge,
        )?;
        let after_changed = dists_on_edge
            .iter()
            .filter(|value| (**value - input.base_dists_on_edge).abs() > 1.0e-12)
            .count();
        spring_global_debug(&format!(
            "distance step changed_edges before={before_changed} after={after_changed}"
        ));

        if let (Some(current_cellwidth), Some(cells_on_triangle), Some(widths)) =
            (cellwidth_scale, input.cells_on_triangle, cellwidth.as_ref())
        {
            let next_cellwidth_scale = current_cellwidth / 2.0;
            let cellwidth_layers = distance_layers(dist_len, current_cellwidth, input.spacing)?;
            let updated = cellwidth_layers_one_based(
                step.num_vertex_in,
                step.num_center_in,
                input.num_rc,
                dist_len,
                cells_on_triangle,
                input.triangles_on_cell,
                &cellwidth_layers,
                step.refinement_flags,
                widths,
            )?;
            cellwidth = Some(updated);
            cellwidth_scale = Some(next_cellwidth_scale);
        }
    }

    Some(SetDistsOnEdgeGlobalOutput {
        dists_on_edge,
        cellwidth,
    })
}
