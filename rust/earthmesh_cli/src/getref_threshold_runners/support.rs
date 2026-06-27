use std::io;
use std::path::Path;

use crate::matrix_width;

pub(super) fn contain_rows_to_fortran_indexed(
    name: &str,
    rows: &[Vec<i32>],
) -> io::Result<Vec<Vec<i32>>> {
    let width = matrix_width(name, rows)?;
    let mut indexed = Vec::with_capacity(rows.len() + 1);
    indexed.push(vec![0; width]);
    indexed.extend(rows.iter().cloned());
    Ok(indexed)
}

pub(super) fn contain_rows_to_fortran_indexed_with_empty_width(
    name: &str,
    rows: &[Vec<i32>],
    empty_width: usize,
) -> io::Result<Vec<Vec<i32>>> {
    if rows.is_empty() {
        return Ok(vec![vec![0; empty_width]]);
    }
    contain_rows_to_fortran_indexed(name, rows)
}

pub(super) fn getref_ustr_ii_width_for_mesh_type(mesh_type: &str) -> io::Result<usize> {
    match mesh_type {
        "landmesh" | "oceanmesh" => Ok(2),
        "atmos" | "atmosmesh" | "LOCmesh" | "earthmesh" => Ok(3),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported GetRef mesh_type {other}"),
        )),
    }
}

pub(super) fn required_getref_output_path<'a>(
    path: Option<&'a Path>,
    component: &str,
) -> io::Result<&'a Path> {
    path.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("missing GetRef {component} threshold output path"),
        )
    })
}
