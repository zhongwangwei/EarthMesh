use std::fs;
use std::io::{self, Write};
use std::path::Path;

use crate::*;

/// Write an SMS/FVCOM `.2dm` from a carved (`mask_postproc`) ocean mesh, which
/// uses two leading placeholder rows and a `(0,0)` boundary marker. Real nodes
/// are renumbered 1-based; triangles touching a placeholder/marker are dropped;
/// the open boundary (`obc_order`, in carved-id space) is re-mapped and written
/// as NS records so the `.2dm` carries its open-boundary specification.
pub(super) fn write_fvcom_2dm_from_carved(
    mesh: &UnstructuredMesh,
    obc_order: &[usize],
    output: &Path,
) -> io::Result<usize> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let is_marker = |p: &LonLatPoint| p.lon == 0.0 && p.lat == 0.0;
    let mut new_id = vec![0usize; mesh.w_points.len() + 2];
    let mut nodes: Vec<(usize, LonLatPoint)> = Vec::new();
    let mut next = 1usize;
    for (idx, p) in mesh.w_points.iter().enumerate() {
        if idx == 0 || is_marker(p) {
            continue;
        }
        new_id[idx + 1] = next;
        nodes.push((next, *p));
        next += 1;
    }
    let mut file = fs::File::create(output)?;
    writeln!(file, "MESH2D")?;
    writeln!(file, "MESHNAME \"FVCOM Mesh\"")?;
    let mut elements = 0usize;
    for tri in mesh.m_to_w.iter().skip(1) {
        let ids = [tri[0], tri[1], tri[2]];
        if ids
            .iter()
            .any(|&v| v < 1 || (v as usize) >= new_id.len() || new_id[v as usize] == 0)
        {
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
        write_fvcom_ns_records(&mut file, &remapped)?;
    }
    Ok(elements)
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
