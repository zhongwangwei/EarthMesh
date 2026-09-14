use crate::read_unstructured_mesh_netcdf;
use crate::unstructured_mesh_support::{
    mesh_canonical_id_for_row, mesh_points_have_two_placeholder_rows, mesh_row_for_canonical_id,
};
use crate::validate_unstructured_mesh;
use crate::write_fvcom_mesh_2dm;
use crate::write_fvcom_ns_records;
use crate::FvcomMesh2dmWriteReport;
use crate::LonLatPoint;
use crate::UnstructuredMesh;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Write an SMS/FVCOM `.2dm` from a carved (`mask_postproc`) ocean mesh, which
/// uses two leading placeholder rows and a `(0,0)` boundary marker. Real nodes
/// are renumbered 1-based; triangles touching a placeholder/marker are dropped;
/// the open boundary (`obc_order`, in carved-id space) is re-mapped and written
/// as NS records so the `.2dm` carries its open-boundary specification.
pub(crate) fn write_fvcom_2dm_from_carved(
    mesh: &UnstructuredMesh,
    obc_order: &[usize],
    output: &Path,
) -> io::Result<FvcomMesh2dmWriteReport> {
    crate::ensure_parent_dir(output)?;
    validate_unstructured_mesh(mesh)?;
    let w_has_two_placeholders = mesh_points_have_two_placeholder_rows(&mesh.w_points);
    let m_has_two_placeholders = mesh_points_have_two_placeholder_rows(&mesh.m_points);
    let mut new_id = vec![0usize; mesh.w_points.len() + 2];
    let mut nodes: Vec<(usize, LonLatPoint)> = Vec::new();
    let mut next = 1usize;
    for (idx, p) in mesh.w_points.iter().enumerate() {
        if idx == 0 {
            continue;
        }
        let Some(canonical_id) = mesh_canonical_id_for_row(idx, w_has_two_placeholders) else {
            continue;
        };
        let canonical_id = usize::try_from(canonical_id).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "negative FVCOM canonical node id",
            )
        })?;
        new_id[canonical_id] = next;
        nodes.push((next, *p));
        next += 1;
    }
    let mut file = fs::File::create(output)?;
    writeln!(file, "MESH2D")?;
    writeln!(file, "MESHNAME \"FVCOM Mesh\"")?;
    let mut elements = 0usize;
    for (m_row, tri) in mesh.m_to_w.iter().enumerate() {
        if m_row == 0 {
            continue;
        }
        if mesh_canonical_id_for_row(m_row, m_has_two_placeholders).is_none() {
            continue;
        }
        let ids = [tri[0], tri[1], tri[2]];
        if ids.iter().any(|&v| {
            mesh_row_for_canonical_id(v, mesh.w_points.len(), w_has_two_placeholders).is_none()
                || (v as usize) >= new_id.len()
                || new_id[v as usize] == 0
        }) {
            continue;
        }
        elements += 1;
        writeln!(
            file,
            "E3T {} {} {} {} 1",
            elements, new_id[ids[0] as usize], new_id[ids[1] as usize], new_id[ids[2] as usize]
        )?;
    }
    for (id, p) in &nodes {
        writeln!(file, "ND {} {:.6} {:.6} {:.6}", id, p.lon, p.lat, 0.0)?;
    }
    if !obc_order.is_empty() {
        let remapped: Vec<usize> = obc_order
            .iter()
            .map(|&id| {
                if id == 1 || id >= new_id.len() || new_id[id] == 0 {
                    1
                } else {
                    new_id[id] + 1
                }
            })
            .collect();
        let boundary_segments = write_fvcom_ns_records(&mut file, &remapped)?;
        return Ok(FvcomMesh2dmWriteReport {
            output: output.to_path_buf(),
            triangles: elements,
            nodes: nodes.len(),
            boundary_segments,
        });
    }
    Ok(FvcomMesh2dmWriteReport {
        output: output.to_path_buf(),
        triangles: elements,
        nodes: nodes.len(),
        boundary_segments: 0,
    })
}

/// Write the standard FVCOM `.2dm` mesh straight from a base gridfile, in pure
/// Rust. Open-boundary segments are omitted (none for a from-scratch mesh).
pub fn write_standard_fvcom_from_gridfile(
    gridfile: impl AsRef<Path>,
    output_2dm: impl AsRef<Path>,
) -> io::Result<FvcomMesh2dmWriteReport> {
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    write_fvcom_mesh_2dm(output_2dm, &mesh, &[])
}

/// Model-format adapter for an admitted final TRI gridfile. Boundary context
/// belongs to the file; an absent context is allowed only for a closed mesh.
/// This exports mesh geometry/OBC, not bathymetry or model forcing.
pub fn write_fvcom_from_final_gridfile(
    gridfile: &Path,
    output: &Path,
) -> io::Result<FvcomMesh2dmWriteReport> {
    use crate::grid_quality_pipeline::{quality_input_from_gridfile, read_gridfile_mesh_points};
    use std::collections::{HashMap, HashSet};
    let invalid = |message: String| io::Error::new(io::ErrorKind::InvalidData, message);
    let points = read_gridfile_mesh_points(gridfile)?;
    let input = quality_input_from_gridfile(&points)?;
    let layout = crate::gridfile_w_row_layout(&points);
    let mut edge_counts = HashMap::new();
    let key = |a: usize, b: usize| (a.min(b), a.max(b));
    for cell in &input.cells {
        for (a, b) in cell
            .vertices
            .iter()
            .zip(cell.vertices.iter().cycle().skip(1))
        {
            *edge_counts.entry(key(*a, *b)).or_insert(0_usize) += 1;
        }
    }
    let boundary_edges = edge_counts
        .iter()
        .filter_map(|(&edge, &count)| (count == 1).then_some(edge))
        .collect::<HashSet<_>>();
    let boundary_vertices = boundary_edges
        .iter()
        .flat_map(|&(a, b)| [a, b])
        .collect::<HashSet<_>>();
    let obc = match crate::obc_boundary_io::read_gridfile_obc_order(gridfile)? {
        Some(order) => order,
        None if boundary_edges.is_empty() => Vec::new(),
        None => return Err(invalid("FVCOM final delivery requires embedded OBC context for a mesh with boundary edges; cannot substitute an empty boundary".to_string())),
    };
    if !obc.is_empty() && obc[0] != 1 {
        return Err(invalid(
            "FVCOM OBC order must start with the canonical placeholder 1".to_string(),
        ));
    }
    let mut previous = None;
    for &id in &obc {
        if id == 1 {
            previous = None;
            continue;
        }
        let row = i32::try_from(id)
            .ok()
            .and_then(|id| layout.physical_row_for_canonical_id(id, points.w_lon.len()))
            .filter(|row| boundary_vertices.contains(row))
            .ok_or_else(|| {
                invalid(format!(
                    "FVCOM OBC id {id} is not a physical boundary vertex"
                ))
            })?;
        if let Some(prev) = previous {
            if !boundary_edges.contains(&key(prev, row)) {
                return Err(invalid(format!(
                    "FVCOM OBC consecutive nodes are not a boundary edge: rows {prev}, {row}"
                )));
            }
        }
        previous = Some(row);
    }
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    crate::atomic_output::validate_output_path(gridfile, output)?;
    let mut report = None;
    crate::atomic_output::atomic_write(output, |temporary| {
        let mut written = write_fvcom_2dm_from_carved(&mesh, &obc, temporary)?;
        let expected_nodes = input
            .cells
            .iter()
            .flat_map(|cell| cell.vertices.iter().copied())
            .collect::<HashSet<_>>()
            .len();
        if written.triangles != input.cells.len() || written.nodes != expected_nodes {
            return Err(invalid(format!(
                "FVCOM adapter changed physical counts: triangles {} -> {}, nodes {} -> {}",
                input.cells.len(),
                written.triangles,
                expected_nodes,
                written.nodes
            )));
        }
        written.output = output.to_path_buf();
        report = Some(written);
        Ok(())
    })?;
    report.ok_or_else(|| invalid("FVCOM publication produced no report".to_string()))
}
