//! A refinement failure the caller can do something about, and the taxonomy
//! that says which.
//!
//! Not every bad mesh is a bug. A valence a lattice phase cannot carry, or a
//! seed too near a pentagon, is a legality gate doing its job -- and the caller
//! can retry at another phase or another scale. An error that says only "the
//! topology is wrong" leaves it no move to make.
//!
//! Shared rather than Method-C's: `icosahedron_m_neighbors` raises these while
//! deriving neighbours, which every backend does.

use std::io;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairableKind {
    TransitionPatch,
    Valence,
    NonTripletPerimeter,
}

#[derive(Debug)]
pub struct MethodCRepairableError {
    pub kind: RepairableKind,
    pub m_point: Option<usize>,
    message: String,
}

impl std::fmt::Display for MethodCRepairableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for MethodCRepairableError {}

pub fn repairable_error(
    kind: RepairableKind,
    m_point: Option<usize>,
    message: impl Into<String>,
) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        MethodCRepairableError {
            kind,
            m_point,
            message: message.into(),
        },
    )
}

pub fn method_c_repairable_payload(error: &io::Error) -> Option<&MethodCRepairableError> {
    error.get_ref()?.downcast_ref::<MethodCRepairableError>()
}

pub fn set_first_two(mut values: [usize; 6], first: usize, second: usize) -> [usize; 6] {
    values[0] = first;
    values[1] = second;
    values
}
