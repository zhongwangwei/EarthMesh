use earthmesh_mesh::{
    BoundaryClosedCurves, BoundaryConnection, RefineArrayLengthCalculation, RefineArrayLengthHalo,
};

use crate::*;

pub(crate) fn empty_refine_array_length_report() -> RefineArrayLengthCalculationRunReport {
    RefineArrayLengthCalculationRunReport {
        calculation: RefineArrayLengthCalculation {
            halo: RefineArrayLengthHalo {
                expanded_mrl: Vec::new(),
                initial_boundary_mask: Vec::new(),
                transition_boundary_mask: Vec::new(),
                boundary_refine: Vec::new(),
                boundary_refine_transition: Vec::new(),
                num_transition_row_triangles: 0,
            },
            boundary: BoundaryConnection {
                bdy_num_in: 0,
                boundary_order: Vec::new(),
                boundary_neighbors: Vec::new(),
                curves: BoundaryClosedCurves {
                    num_closed_curve: 0,
                    num_bdy_long: [0; 3],
                    close_curves: vec![Vec::new()],
                    n_close_curve: vec![0],
                },
            },
        },
        close_meshes: RefineArrayLengthCloseMeshesWriteReport {
            mask_patch_ndm: 0,
            outputs: Vec::new(),
        },
    }
}

pub(crate) fn identity_ngr_renew_report(
    old_mp: usize,
    old_wp: usize,
    state: &RefineLoopWorkingState,
) -> NgrRenewReport {
    NgrRenewReport {
        num_sjx: old_mp,
        num_dbx: old_wp,
        vertex_mapping: (0..=old_wp).collect(),
        adjacency_capacity: state.ngrwm.len().saturating_sub(1).max(7),
        boundary_refine: Vec::new(),
        boundary_refine_transition: Vec::new(),
    }
}
