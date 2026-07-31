use std::io;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MethodCRepairableKind {
    TransitionPatch,
    Valence,
    NonTripletPerimeter,
}

#[derive(Clone, Debug)]
pub(crate) struct MethodCRepairableError {
    pub(crate) kind: MethodCRepairableKind,
    pub(crate) m_point: Option<usize>,
    pub(crate) parent_m_point: Option<usize>,
    pub(crate) parent_u_edge: Option<usize>,
    pub(crate) parent_m_valence_witnesses: Vec<usize>,
    pub(crate) perimeter_lengths: Vec<usize>,
    pub(crate) repair_attempts: usize,
    message: String,
}

impl std::fmt::Display for MethodCRepairableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for MethodCRepairableError {}

pub(crate) fn method_c_repairable_error(
    kind: MethodCRepairableKind,
    m_point: Option<usize>,
    message: impl Into<String>,
) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        MethodCRepairableError {
            kind,
            m_point,
            parent_m_point: None,
            parent_u_edge: None,
            parent_m_valence_witnesses: Vec::new(),
            perimeter_lengths: Vec::new(),
            repair_attempts: 0,
            message: message.into(),
        },
    )
}

pub(crate) fn method_c_repairable_perimeter_error(
    kind: MethodCRepairableKind,
    perimeter_lengths: Vec<usize>,
    repair_attempts: usize,
    message: impl Into<String>,
) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        MethodCRepairableError {
            kind,
            m_point: None,
            parent_m_point: None,
            parent_u_edge: None,
            parent_m_valence_witnesses: Vec::new(),
            perimeter_lengths,
            repair_attempts,
            message: message.into(),
        },
    )
}

pub(crate) fn method_c_repairable_payload(error: &io::Error) -> Option<&MethodCRepairableError> {
    error.get_ref()?.downcast_ref::<MethodCRepairableError>()
}

pub(crate) fn method_c_repairable_error_with_parent_origin(
    error: io::Error,
    parent_m_point: Option<usize>,
    parent_u_edge: Option<usize>,
) -> io::Error {
    let Some(payload) = method_c_repairable_payload(&error) else {
        return error;
    };
    let mut payload = payload.clone();
    payload.parent_m_point = parent_m_point;
    payload.parent_u_edge = parent_u_edge;
    io::Error::new(error.kind(), payload)
}

pub(crate) fn method_c_repairable_error_with_parent_m_valence_witnesses(
    error: io::Error,
    parent_m_valence_witnesses: Vec<usize>,
) -> io::Error {
    let Some(payload) = method_c_repairable_payload(&error) else {
        return error;
    };
    let mut payload = payload.clone();
    payload.parent_m_valence_witnesses = parent_m_valence_witnesses;
    io::Error::new(error.kind(), payload)
}

#[derive(Debug)]
pub(crate) struct MethodCParentLevelMismatchError {
    pub(crate) w_face: usize,
    pub(crate) actual_mrlw: usize,
    pub(crate) expected_mrlw: usize,
    message: String,
}

impl std::fmt::Display for MethodCParentLevelMismatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for MethodCParentLevelMismatchError {}

pub(crate) fn method_c_parent_level_mismatch_error(
    w_face: usize,
    actual_mrlw: usize,
    expected_mrlw: usize,
    message: impl Into<String>,
) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        MethodCParentLevelMismatchError {
            w_face,
            actual_mrlw,
            expected_mrlw,
            message: message.into(),
        },
    )
}

pub(crate) fn method_c_parent_level_mismatch_payload(
    error: &io::Error,
) -> Option<&MethodCParentLevelMismatchError> {
    error
        .get_ref()?
        .downcast_ref::<MethodCParentLevelMismatchError>()
}

use super::IcosahedronUEdge;

pub(crate) fn set_first_two(mut values: [usize; 6], first: usize, second: usize) -> [usize; 6] {
    values[0] = first;
    values[1] = second;
    values
}

pub(crate) fn other_edge_face(edge: IcosahedronUEdge, iw: usize) -> io::Result<usize> {
    if edge.iw[0] == iw {
        Ok(edge.iw[1])
    } else if edge.iw[1] == iw {
        Ok(edge.iw[0])
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Method-C edge does not touch W face {iw}"),
        ))
    }
}

pub(crate) fn canonical_other_endpoint_by_first(edge: IcosahedronUEdge, im: usize) -> usize {
    if edge.im[0] == im {
        edge.im[1]
    } else {
        edge.im[0]
    }
}

pub(crate) fn fill_missing_endpoint(edge: &mut IcosahedronUEdge, im: usize) {
    if edge.im[0] == 1 {
        edge.im[0] = im;
    } else {
        edge.im[1] = im;
    }
}

pub(crate) fn method_c_split_outer_edges(
    candidates: [usize; 3],
    u_edges: &[IcosahedronUEdge],
    label: &str,
    m_point: usize,
) -> io::Result<[usize; 2]> {
    let [ku1, ku2, ku3] = candidates;
    for (solid, first_open, second_open) in [(ku1, ku2, ku3), (ku2, ku3, ku1), (ku3, ku1, ku2)] {
        let edge = u_edges.get(solid).copied().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Method-C {label} candidate U edge {solid} is out of range"),
            )
        })?;
        if edge.im[0] > 1 && edge.im[1] > 1 {
            return Ok([first_open, second_open]);
        }
    }
    let edge_summary = [ku1, ku2, ku3]
        .map(|iu| {
            u_edges
                .get(iu)
                .map(|edge| format!("{iu}:{:?}", edge.im))
                .unwrap_or_else(|| format!("{iu}:<missing>"))
        })
        .join(", ");
    Err(method_c_repairable_error_with_parent_origin(
        method_c_repairable_error(
            MethodCRepairableKind::TransitionPatch,
            Some(m_point),
            format!("Method-C {label} transition patch has no solid split edge ({edge_summary})"),
        ),
        Some(m_point),
        None,
    ))
}
