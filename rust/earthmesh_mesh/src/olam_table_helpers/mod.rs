use std::io;

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
            format!("OLAM Method-C edge does not touch W face {iw}"),
        ))
    }
}

pub(crate) fn fortran_other_endpoint_by_first(edge: IcosahedronUEdge, im: usize) -> usize {
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
) -> io::Result<[usize; 2]> {
    let [ku1, ku2, ku3] = candidates;
    for (solid, first_open, second_open) in [(ku1, ku2, ku3), (ku2, ku3, ku1), (ku3, ku1, ku2)] {
        let edge = u_edges.get(solid).copied().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("OLAM Method-C {label} candidate U edge {solid} is out of range"),
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
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        format!("OLAM Method-C {label} transition patch has no solid split edge ({edge_summary})"),
    ))
}
