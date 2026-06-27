use std::io;
use std::path::Path;

use crate::*;

pub(crate) fn apply_previous_refine_region_prefilter(
    file_dir: &Path,
    previous_step: usize,
    dist_len: usize,
    old_mp: usize,
    old_wp: usize,
    state: &mut RefineLoopWorkingState,
) -> io::Result<()> {
    let mut close_curves = Vec::new();
    for curve_id in 1.. {
        let path = refine_array_length_close_mesh_output_path(file_dir, previous_step, curve_id);
        if !path.exists() {
            break;
        }
        close_curves.push(read_close_mesh_netcdf(path)?);
    }
    if close_curves.is_empty() {
        return Ok(());
    }
    if state.mp_new.len() <= old_mp || state.n_ngrwm.len() <= old_wp {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "previous refine-region prefilter requires current mesh arrays",
        ));
    }
    if state.ngrwm.iter().any(|row| row.len() <= old_wp) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "previous refine-region prefilter requires vertex-to-triangle rows",
        ));
    }

    let mut mrl_bk = vec![0_i32; old_mp + 1];
    for triangle in 1..=old_mp {
        let point = state.mp_new[triangle];
        if close_curves
            .iter()
            .any(|curve| point_in_close_curve_fortran(point, curve))
        {
            mrl_bk[triangle] = 1;
        }
    }

    for _ in 0..dist_len {
        let mut boundary = vec![false; old_wp + 1];
        for cell in 1..=old_wp {
            let num_edges = state.n_ngrwm[cell];
            if num_edges == 0 || state.ngrwm.len() <= num_edges {
                continue;
            }
            let mut marked = 0_i32;
            for row in 1..=num_edges {
                let triangle = state.ngrwm[row][cell];
                if triangle <= old_mp {
                    marked += mrl_bk[triangle];
                }
            }
            if marked != 0 && marked != num_edges as i32 {
                boundary[cell] = true;
            }
        }
        for cell in 1..=old_wp {
            if !boundary[cell] {
                continue;
            }
            let num_edges = state.n_ngrwm[cell];
            if state.ngrwm.len() <= num_edges {
                continue;
            }
            for row in 1..=num_edges {
                let triangle = state.ngrwm[row][cell];
                if triangle <= old_mp && mrl_bk[triangle] != 0 {
                    mrl_bk[triangle] = 0;
                }
            }
        }
    }

    for triangle in 1..=old_mp {
        if state.ref_sjx[triangle] != 0 && mrl_bk[triangle] != 1 {
            state.ref_sjx[triangle] = 0;
        }
    }
    Ok(())
}

fn point_in_close_curve_fortran(point: LonLatPoint, curve: &[LonLatPoint]) -> bool {
    if curve.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut previous = curve.len() - 1;
    for current in 0..curve.len() {
        let current_point = curve[current];
        let previous_point = curve[previous];
        if (current_point.lat > point.lat) != (previous_point.lat > point.lat) {
            let denominator = previous_point.lat - current_point.lat;
            if denominator != 0.0 {
                let lon_intersect = (previous_point.lon - current_point.lon)
                    * (point.lat - current_point.lat)
                    / denominator
                    + current_point.lon;
                if point.lon < lon_intersect {
                    inside = !inside;
                }
            }
        }
        previous = current;
    }
    inside
}
