use std::io;

use crate::UnstructuredMesh;

pub(crate) fn cells_on_triangle_fortran_indexed_from_mesh(
    mesh: &UnstructuredMesh,
) -> io::Result<Vec<[usize; 3]>> {
    mesh.m_to_w
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let mut out = [0usize; 3];
            for (slot, value) in row.iter().copied().enumerate() {
                if value < 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("m_to_w row {row_idx} slot {slot} has negative cell id {value}"),
                    ));
                }
                let value = value as usize;
                if value >= mesh.w_points.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "m_to_w row {row_idx} references cell id {value}, but only {} cell rows exist",
                            mesh.w_points.len()
                        ),
                    ));
                }
                out[slot] = value;
            }
            Ok(out)
        })
        .collect()
}

pub(crate) fn triangles_on_cell_fortran_indexed_from_mesh(
    mesh: &UnstructuredMesh,
) -> io::Result<Vec<Vec<usize>>> {
    let mut rows = Vec::with_capacity(mesh.w_to_m.len());
    for (row_idx, row) in mesh.w_to_m.iter().enumerate() {
        let mut out = Vec::with_capacity(row.len());
        for (slot, value) in row.iter().copied().enumerate() {
            if value < 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("w_to_m row {row_idx} slot {slot} has negative triangle id {value}"),
                ));
            }
            let value = value as usize;
            if value >= mesh.m_points.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "w_to_m row {row_idx} references triangle id {value}, but only {} triangle rows exist",
                        mesh.m_points.len()
                    ),
                ));
            }
            out.push(value);
        }
        rows.push(out);
    }
    Ok(rows)
}

pub(crate) fn n_edges_on_cell_usize_from_mesh(mesh: &UnstructuredMesh) -> io::Result<Vec<usize>> {
    mesh.n_w_to_m
        .iter()
        .enumerate()
        .map(|(idx, value)| {
            if *value < 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("n_w_to_m row {idx} has negative edge count {value}"),
                ));
            }
            Ok(*value as usize)
        })
        .collect()
}

pub(crate) fn parse_value_after_equals<T>(line: &str, field: &str) -> io::Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let (_, value) = line.split_once('=').ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{field} line must contain '='"),
        )
    })?;
    value.trim().parse::<T>().map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid {field} value: {err}"),
        )
    })
}
