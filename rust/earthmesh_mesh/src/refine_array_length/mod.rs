use std::io;

use crate::{refine_boundary_connection_make_one_based, BoundaryConnection};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefineArrayLengthHalo {
    pub expanded_mrl: Vec<i32>,
    pub initial_boundary_mask: Vec<i32>,
    pub transition_boundary_mask: Vec<i32>,
    pub boundary_refine: Vec<usize>,
    pub boundary_refine_transition: Vec<usize>,
    pub num_transition_row_triangles: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RefineArrayLengthCalculation {
    pub halo: RefineArrayLengthHalo,
    pub boundary: BoundaryConnection,
}

/// Pure halo-sizing core of `MOD_refine.F90:Array_length_calculation`.
///
/// This excludes `bdy_connection_make` close-curve generation and NetCDF side
/// effects, but preserves the Canonical boundary criterion and outward halo
/// expansion that updates `num_tranrow_sjx`, `isbdy_refine`, `bdy_refine`, and
/// `bdy_refine_tran`.
pub fn refine_array_length_halo_one_based(
    set_dis_in: usize,
    num_center: usize,
    _sjx_points: usize,
    lbx_points: usize,
    mrl_new: &[i32],
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    initial_num_transition_row_triangles: usize,
) -> io::Result<RefineArrayLengthHalo> {
    if lbx_points >= triangles_on_cell.len() || lbx_points >= edge_counts.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "lbx_points must address triangles_on_cell and edge_counts",
        ));
    }
    if num_center > lbx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "num_center must not exceed lbx_points",
        ));
    }
    let mut expanded_mrl = mrl_new.to_vec();
    let mut boundary_mask = refine_boundary_mask_from_mrl(
        num_center,
        lbx_points,
        &expanded_mrl,
        triangles_on_cell,
        edge_counts,
    )?;
    let initial_boundary_mask = boundary_mask.clone();
    let mut num_transition_row_triangles = initial_num_transition_row_triangles;

    for _ in 0..set_dis_in {
        for cell in (num_center + 1)..=lbx_points {
            if boundary_mask[cell] != 1 {
                continue;
            }
            let num_edges = edge_counts[cell];
            for &triangle in triangles_on_cell[cell].iter().take(num_edges) {
                if triangle == 0 || triangle >= expanded_mrl.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("cell {cell} canonicals invalid triangle {triangle}"),
                    ));
                }
                if expanded_mrl[triangle] == 4 {
                    continue;
                }
                expanded_mrl[triangle] = 4;
                num_transition_row_triangles += 1;
            }
        }
        boundary_mask = refine_boundary_mask_from_mrl(
            num_center,
            lbx_points,
            &expanded_mrl,
            triangles_on_cell,
            edge_counts,
        )?;
    }

    let boundary_refine = ((num_center + 1)..=lbx_points)
        .filter(|&cell| initial_boundary_mask[cell] == 1)
        .collect::<Vec<_>>();
    let boundary_refine_transition = ((num_center + 1)..=lbx_points)
        .filter(|&cell| boundary_mask[cell] == 1)
        .collect::<Vec<_>>();

    Ok(RefineArrayLengthHalo {
        expanded_mrl,
        initial_boundary_mask,
        transition_boundary_mask: boundary_mask,
        boundary_refine,
        boundary_refine_transition,
        num_transition_row_triangles,
    })
}

/// File-I/O-free wrapper for `MOD_refine.F90:Array_length_calculation`.
///
/// This composes the already current halo sizing with
/// `bdy_connection_make` close-curve construction.  The Canonical
/// `close_Mesh_Save` NetCDF writes remain an adapter concern; callers can use
/// `boundary.curves.close_curves` plus their coordinate table to write the same
/// files.
pub fn refine_array_length_calculation_one_based(
    set_dis_in: usize,
    num_vertex: usize,
    num_center: usize,
    sjx_points: usize,
    lbx_points: usize,
    mrl_new: &[i32],
    triangle_neighbors: &[Vec<usize>],
    cells_on_triangle: &[[usize; 3]],
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    initial_num_transition_row_triangles: usize,
) -> io::Result<RefineArrayLengthCalculation> {
    let halo = refine_array_length_halo_one_based(
        set_dis_in,
        num_center,
        sjx_points,
        lbx_points,
        mrl_new,
        triangles_on_cell,
        edge_counts,
        initial_num_transition_row_triangles,
    )?;
    let boundary = refine_boundary_connection_make_one_based(
        num_vertex,
        sjx_points,
        lbx_points,
        mrl_new,
        triangle_neighbors,
        cells_on_triangle,
    )?;
    Ok(RefineArrayLengthCalculation { halo, boundary })
}

fn refine_boundary_mask_from_mrl(
    num_center: usize,
    lbx_points: usize,
    mrl: &[i32],
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
) -> io::Result<Vec<i32>> {
    let mut mask = vec![0_i32; lbx_points + 1];
    for cell in (num_center + 1)..=lbx_points {
        let num_edges = edge_counts[cell];
        if triangles_on_cell[cell].len() < num_edges {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("cell {cell} edge count {num_edges} exceeds triangles_on_cell row"),
            ));
        }
        let mut state_sum = 0_i32;
        for &triangle in triangles_on_cell[cell].iter().take(num_edges) {
            if triangle == 0 || triangle >= mrl.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("cell {cell} canonicals invalid triangle {triangle}"),
                ));
            }
            state_sum += mrl[triangle];
        }
        if state_sum == num_edges as i32 || state_sum == (num_edges as i32) * 4 {
            continue;
        }
        mask[cell] = 1;
    }
    Ok(mask)
}
