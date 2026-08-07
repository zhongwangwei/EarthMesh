//! The gradient-limited cell-width field, as the refinement layer sees it.
//!
//! Re-exported from `earthmesh_hfield` rather than moved. The h-field is a
//! self-contained numeric kernel with its own tests and its own callers, and
//! folding it in would be a move commit tangled with an API commit. This is
//! where it belongs in the layering; the file move is a separate change with
//! nothing else in it.

pub use earthmesh_hfield::*;
