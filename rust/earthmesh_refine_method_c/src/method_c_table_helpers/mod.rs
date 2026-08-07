use std::io;

use crate::MethodCNestWd;
use earthmesh_mesh::{repairable_error, IcosahedronUEdge, RepairableKind};

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

/// The two open edges of the patch's second-order neighbour, given its
/// `nest_wd` row.
///
/// The row is taken whole rather than just its `iu` triple so the failure can
/// say *why* there is nothing to split. A face only gets edge ids when it is
/// subdivided, so an all-zero triple is not corruption -- it means the patch
/// reached a face this pass is not subdividing, and the flag separates the two
/// ways that happens: `< 0` is the face suppressed by another perimeter triple
/// (two triples' suppression zones met, so the mask has a neck there), `0` is a
/// face the mask never selected (the mask is one face thick there). They call
/// for opposite repairs, and the old message named neither.
pub(crate) fn method_c_split_outer_edges(
    neighbour: MethodCNestWd,
    u_edges: &[IcosahedronUEdge],
    label: &str,
    m_point: usize,
) -> io::Result<[usize; 2]> {
    let [ku1, ku2, ku3] = neighbour.iu;
    if !neighbour.is_subdivided() {
        let cause = if neighbour.is_suppressed() {
            "it is the face suppressed by another perimeter triple, so the mask necks down here"
        } else {
            "the mask does not select it, so the mask is one face thick here"
        };
        return Err(repairable_error(
            RepairableKind::TransitionPatch,
            Some(m_point),
            format!(
                "Method-C {label} transition patch neighbour is not subdivided ({cause}; nest_wd flag {})",
                neighbour.flag()
            ),
        ));
    }
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
    Err(repairable_error(
        RepairableKind::TransitionPatch,
        Some(m_point),
        format!("Method-C {label} transition patch has no solid split edge ({edge_summary})"),
    ))
}
