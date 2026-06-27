mod crossings;
mod spans;

pub(crate) use crossings::append_non_rectilinear_hole_edge_crossing_xs;
pub(crate) use spans::{ring_y_spans_at_slab_boundary, ring_y_spans_at_x, triangle_y_span_at_x};
