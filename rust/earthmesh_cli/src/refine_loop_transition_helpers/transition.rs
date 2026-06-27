use std::io;

use crate::*;

pub(crate) fn apply_transition_onedivide_two(
    state: &mut RefineLoopWorkingState,
    num_ref: usize,
    is_reverse: bool,
) -> io::Result<()> {
    state.iter += 1;
    if state.num_mp.len() <= state.iter {
        state.num_mp.resize(state.iter + 1, 0);
    }
    if state.num_wp.len() <= state.iter {
        state.num_wp.resize(state.iter + 1, 0);
    }
    state.num_mp[state.iter] = state.num_mp[state.iter - 1] + 2 * num_ref;
    state.num_wp[state.iter] = state.num_wp[state.iter - 1] + num_ref;
    state.mp_new.resize(
        state.num_mp[state.iter] + 1,
        LonLatPoint { lon: 0.0, lat: 0.0 },
    );
    state.wp_new.resize(
        state.num_wp[state.iter] + 1,
        LonLatPoint { lon: 0.0, lat: 0.0 },
    );
    if state.ngrmw_new.len() <= 3 {
        state.ngrmw_new.resize_with(4, Vec::new);
    }
    for row in &mut state.ngrmw_new[1..=3] {
        row.resize(state.num_mp[state.iter] + 1, 1);
    }

    state.apply_onedivide_two(is_reverse)?;
    for triangle in 1..state.ref_sjx.len() {
        if state.ref_sjx[triangle] != 0 {
            state.mrl_new[triangle] = 4;
        }
    }
    Ok(())
}

pub(crate) fn remove_isolated_one_into_four_markers(
    num_vertex: usize,
    old_mp: usize,
    triangle_neighbors: &[Vec<usize>],
    ref_sjx: &mut [i32],
) -> io::Result<()> {
    if triangle_neighbors.len() <= old_mp || ref_sjx.len() <= old_mp {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("old_mp {old_mp} requires one-based triangle neighbor and ref_sjx storage"),
        ));
    }
    for triangle in (num_vertex + 1)..=old_mp {
        if ref_sjx[triangle] != 1 {
            continue;
        }
        let neighbors = &triangle_neighbors[triangle];
        if neighbors.len() != 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("triangle neighbor row {triangle} must contain exactly three neighbors"),
            ));
        }
        let mut neighbor_marker_sum = 0_i32;
        for &neighbor in neighbors {
            if neighbor == 0 {
                continue;
            }
            if neighbor > old_mp {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle {triangle} references invalid neighbor {neighbor}"),
                ));
            }
            neighbor_marker_sum += ref_sjx[neighbor];
        }
        if neighbor_marker_sum <= 1 {
            ref_sjx[triangle] = 0;
        }
    }
    Ok(())
}

pub(crate) fn marked_triangles_have_valid_neighbors(
    num_vertex: usize,
    old_mp: usize,
    triangle_neighbors: &[Vec<usize>],
    ref_sjx: &[i32],
) -> bool {
    if triangle_neighbors.len() <= old_mp || ref_sjx.len() <= old_mp {
        return false;
    }
    if !((num_vertex + 1)..=old_mp).any(|triangle| ref_sjx[triangle] != 0) {
        return false;
    }
    ((num_vertex + 1)..=old_mp)
        .filter(|&triangle| ref_sjx[triangle] != 0)
        .all(|triangle| {
            triangle_neighbors[triangle]
                .iter()
                .all(|&neighbor| neighbor != 0 && neighbor <= old_mp)
        })
}

pub(crate) fn fortran_index_segments(segments: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut indexed = Vec::with_capacity(segments.len() + 1);
    indexed.push(Vec::new());
    for segment in segments {
        let mut row = Vec::with_capacity(segment.len() + 1);
        row.push(1);
        row.extend(segment.iter().copied());
        indexed.push(row);
    }
    indexed
}
