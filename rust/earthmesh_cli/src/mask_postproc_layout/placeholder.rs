use crate::{LonLatPoint, MaskPostprocLayout};

pub(crate) fn ensure_leading_mask_postproc_placeholder(
    layout: MaskPostprocLayout,
) -> MaskPostprocLayout {
    if has_leading_mask_postproc_placeholder(&layout) {
        layout
    } else {
        add_leading_mask_postproc_placeholder(layout)
    }
}

fn has_leading_mask_postproc_placeholder(layout: &MaskPostprocLayout) -> bool {
    let is_zero_point = |point: &LonLatPoint| point.lon == 0.0 && point.lat == 0.0;
    layout.center_points.len() > 1
        && layout.vertex_points.len() > 1
        && is_zero_point(&layout.center_points[0])
        && is_zero_point(&layout.center_points[1])
        && is_zero_point(&layout.vertex_points[0])
        && is_zero_point(&layout.vertex_points[1])
        && layout
            .center_neighbor_counts
            .get(0..=1)
            .is_some_and(|counts| counts.iter().all(|&count| count == 0))
}

pub(super) fn add_leading_mask_postproc_placeholder(
    mut layout: MaskPostprocLayout,
) -> MaskPostprocLayout {
    layout.ustr_points += 1;
    layout.ustr_bounds += 1;
    layout
        .center_points
        .insert(0, LonLatPoint { lon: 0.0, lat: 0.0 });
    layout
        .vertex_points
        .insert(0, LonLatPoint { lon: 0.0, lat: 0.0 });
    let center_width = layout
        .center_neighbors
        .first()
        .map(|row| row.len())
        .unwrap_or(0);
    let vertex_width = layout
        .vertex_neighbors
        .first()
        .map(|row| row.len())
        .unwrap_or(0);
    layout.center_neighbors.insert(0, vec![1; center_width]);
    layout.vertex_neighbors.insert(0, vec![1; vertex_width]);
    layout.center_neighbor_counts.insert(0, 0);
    layout.vertex_neighbor_counts.insert(0, 0);
    layout
}
