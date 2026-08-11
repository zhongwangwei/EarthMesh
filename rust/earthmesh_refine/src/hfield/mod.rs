//! The gradient-limited cell-width field, as the refinement layer sees it.
//!
//! Re-exported from `earthmesh_hfield` rather than moved. The h-field is a
//! self-contained numeric kernel with its own tests and its own callers, and
//! folding it in would be a move commit tangled with an API commit. This is
//! where it belongs in the layering; the file move is a separate change with
//! nothing else in it.

// Was `pub use earthmesh_hfield::*;`. Nothing anywhere imports through this
// module -- callers reach `earthmesh_hfield` directly -- so the wildcard
// re-exported a whole crate's surface for no consumer, and the architecture
// gate forbids exactly that. The module stays for the note above it, which
// records where the h-field belongs in the layering once it is moved.
pub use earthmesh_hfield::{HField, EARTH_RADIUS_METERS};
