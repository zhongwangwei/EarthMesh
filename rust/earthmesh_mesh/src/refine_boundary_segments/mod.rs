use std::io;

pub(crate) fn refine_boundary_segments_fortran_indexed(
    set_dis_in: usize,
    closed_curves: &[Vec<usize>],
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    mrl_new: &[i32],
) -> io::Result<Vec<Vec<usize>>> {
    let mut segments = Vec::new();
    for curve in closed_curves {
        if curve.len() < 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "closed curve must contain at least three cells",
            ));
        }
        let mut closed = curve.clone();
        closed.push(curve[0]);

        let mut segment_start_end = vec![(0_usize, 0_usize); closed.len()];
        if set_dis_in == 1 {
            for j in 0..(closed.len() - 1) {
                segment_start_end[j] = (j, j + 1);
            }
        } else {
            let first_turn = (0..(closed.len() - 1))
                .find(|&idx| {
                    refined_count_around_cell(closed[idx], triangles_on_cell, edge_counts, mrl_new)
                        != 3
                })
                .unwrap_or(0);
            if first_turn != 0 {
                let unique_len = closed.len() - 1;
                let mut rotated = Vec::with_capacity(unique_len + 1);
                rotated.extend_from_slice(&closed[first_turn..unique_len]);
                rotated.extend_from_slice(&closed[..first_turn]);
                rotated.push(rotated[0]);
                closed = rotated;
            }

            let mut start = 0_usize;
            segment_start_end[start].0 = 0;
            for j in 1..(closed.len() - 1) {
                let refined_count =
                    refined_count_around_cell(closed[j], triangles_on_cell, edge_counts, mrl_new);
                if refined_count == 3 {
                    continue;
                }
                segment_start_end[start].1 = j;
                segment_start_end[j].0 = j;
                start = j;
            }
            segment_start_end[start].1 = closed.len() - 1;

            let original_ranges: Vec<(usize, usize)> = segment_start_end
                .iter()
                .copied()
                .filter(|(range_start, range_end)| range_end > range_start)
                .collect();
            segment_start_end.fill((0, 0));
            for (range_start, range_end) in original_ranges {
                let num = range_end - range_start;
                if num <= set_dis_in {
                    segment_start_end[range_start] = (range_start, range_end);
                    continue;
                }
                let mut num_segment = ((num + 1) as f64 / set_dis_in as f64).floor() as usize;
                if (num + 1) % set_dis_in != 0 {
                    num_segment += 1;
                }
                if num % set_dis_in == 0 {
                    num_segment = num_segment.saturating_sub(1);
                }
                num_segment = num_segment.max(1);
                let mut subranges = Vec::with_capacity(num_segment);
                let mut sub_start = range_start;
                for _ in 0..(num_segment - 1) {
                    let sub_end = sub_start + set_dis_in;
                    subranges.push((sub_start, sub_end));
                    sub_start = sub_end;
                }
                subranges.push((sub_start, range_end));
                if set_dis_in >= 3 && subranges.len() >= 2 {
                    let min_len = set_dis_in.div_ceil(2);
                    let last_idx = subranges.len() - 1;
                    let (last_start, last_end) = subranges[last_idx];
                    if last_end - last_start < min_len {
                        let adjusted_start = last_end - min_len;
                        subranges[last_idx].0 = adjusted_start;
                        subranges[last_idx - 1].1 = adjusted_start;
                    }
                }
                for (sub_start, sub_end) in subranges {
                    segment_start_end[sub_start] = (sub_start, sub_end);
                }
            }
        }

        let total_edges: usize = segment_start_end
            .iter()
            .filter_map(|&(start, end)| (end > start).then_some(end - start))
            .sum();
        if total_edges != closed.len() - 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "boundary segment edge total {total_edges} differs from closed curve edge count {}",
                    closed.len() - 1
                ),
            ));
        }

        for (start, end) in segment_start_end {
            if end <= start {
                continue;
            }
            let mut segment = Vec::with_capacity(end - start);
            for k in start..end {
                let cell_a = closed[k];
                let cell_b = closed[k + 1];
                segment.push(common_unrefined_triangle_between_cells(
                    cell_a,
                    cell_b,
                    triangles_on_cell,
                    edge_counts,
                    mrl_new,
                )?);
            }
            segments.push(segment);
        }
    }
    Ok(segments)
}

fn refined_count_around_cell(
    cell: usize,
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    mrl_new: &[i32],
) -> i32 {
    let num_edges = edge_counts[cell];
    let state_sum: i32 = triangles_on_cell[cell][..num_edges]
        .iter()
        .map(|&triangle| mrl_new[triangle])
        .sum();
    (state_sum - num_edges as i32) / 3
}

fn common_unrefined_triangle_between_cells(
    cell_a: usize,
    cell_b: usize,
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    mrl_new: &[i32],
) -> io::Result<usize> {
    for &triangle_a in &triangles_on_cell[cell_a][..edge_counts[cell_a]] {
        if mrl_new[triangle_a] == 4 {
            continue;
        }
        for &triangle_b in &triangles_on_cell[cell_b][..edge_counts[cell_b]] {
            if mrl_new[triangle_b] == 4 {
                continue;
            }
            if triangle_a == triangle_b {
                return Ok(triangle_a);
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("cells {cell_a} and {cell_b} do not share an unrefined boundary triangle"),
    ))
}
