use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Evidence report from writing `MOD_file_preprocess.F90:MPAS_info_Save` graph.info.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpasGraphInfoWriteReport {
    pub output: PathBuf,
    pub n_cells_written: usize,
    pub interior_edges: usize,
    pub cells_with_boundary_edges: usize,
}

/// Write the METIS-style `graph.info` text produced by
/// `MOD_file_preprocess.F90:MPAS_info_Save`.
///
/// Inputs keep the legacy placeholder row at Rust index `0`; only rows/edges
/// from index `1` onward are written or counted, matching Fortran `2:nCells`
/// and `2:nEdges` loops after internal placeholder-row removal.
pub fn write_mpas_graph_info(
    output: impl AsRef<Path>,
    max_edges: usize,
    cells_on_cell: &[Vec<i32>],
    cells_on_edge: &[[i32; 2]],
    n_edges_on_cell: &[i32],
) -> io::Result<MpasGraphInfoWriteReport> {
    if max_edges == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "max_edges must be positive",
        ));
    }
    if cells_on_cell.is_empty() || cells_on_edge.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "MPAS graph info inputs must include the legacy placeholder row",
        ));
    }
    if cells_on_cell.len() != n_edges_on_cell.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "n_edges_on_cell length {} must match cells_on_cell length {}",
                n_edges_on_cell.len(),
                cells_on_cell.len()
            ),
        ));
    }
    for (idx, row) in cells_on_cell.iter().enumerate() {
        if row.len() < max_edges {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "cells_on_cell row {idx} width {} must be at least max_edges {max_edges}",
                    row.len()
                ),
            ));
        }
    }
    if n_edges_on_cell.iter().any(|&value| value < 0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "n_edges_on_cell values must be non-negative",
        ));
    }

    let mut n_edges_on_cell_usize = Vec::with_capacity(n_edges_on_cell.len());
    for (idx, &value) in n_edges_on_cell.iter().enumerate() {
        let count = usize::try_from(value).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("n_edges_on_cell row {idx} is out of range"),
            )
        })?;
        if count > max_edges {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("n_edges_on_cell row {idx} exceeds max_edges {max_edges}"),
            ));
        }
        n_edges_on_cell_usize.push(count);
    }

    let interior_edges = cells_on_edge
        .iter()
        .skip(1)
        .filter(|edge| edge[0] != 0 && edge[1] != 0)
        .count();
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(output)?;
    writeln!(file, "{:10}{:10}", cells_on_cell.len() - 1, interior_edges)?;

    let mut cells_with_boundary_edges = 0;
    for cell_id in 1..cells_on_cell.len() {
        let expected_edges = n_edges_on_cell_usize[cell_id];
        let mut neighbors = Vec::new();
        for &neighbor in cells_on_cell[cell_id].iter().take(expected_edges) {
            if neighbor > 0 {
                neighbors.push(neighbor);
            }
        }
        for neighbor in &neighbors {
            write!(file, "{:10}", neighbor)?;
        }
        writeln!(file)?;
        if neighbors.len() < expected_edges {
            cells_with_boundary_edges += 1;
        }
    }

    Ok(MpasGraphInfoWriteReport {
        output: output.to_path_buf(),
        n_cells_written: cells_on_cell.len() - 1,
        interior_edges,
        cells_with_boundary_edges,
    })
}
