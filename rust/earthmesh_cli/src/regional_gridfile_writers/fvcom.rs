use crate::read_unstructured_mesh_netcdf;
use crate::unstructured_mesh_support::{mesh_canonical_id_for_row, mesh_row_for_canonical_id};
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
    let has_two_placeholders = |points: &[LonLatPoint]| {
        points.len() > 2
            && points[0].lon == 0.0
            && points[0].lat == 0.0
            && points[1].lon == 0.0
            && points[1].lat == 0.0
    };
    let w_has_two_placeholders = has_two_placeholders(&mesh.w_points);
    let m_has_two_placeholders = has_two_placeholders(&mesh.m_points);
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
