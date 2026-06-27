use std::io;
use std::path::Path;

use earthmesh_mesh::{olam_gridinit_factorization_fortran, LonLatDegrees, OlamDelaunayMesh};

use crate::{
    read_unstructured_mesh_netcdf, unstructured_dimc, write_regional_gridfile,
    write_unstructured_mesh_netcdf, GridRegion, UnstructuredMesh, UnstructuredMeshWriteReport,
};

pub(crate) fn olam_delaunay_mesh_from_unstructured_gridfile(
    mesh: &UnstructuredMesh,
    nxp: usize,
    nspring: usize,
    beta: f64,
    spring_relax: f64,
    max_tris: usize,
) -> io::Result<OlamDelaunayMesh> {
    let m_point_lonlat = mesh
        .w_points
        .iter()
        .map(|point| LonLatDegrees::new(point.lon, point.lat))
        .collect::<Vec<_>>();
    let w_face_m_points = mesh
        .m_to_w
        .iter()
        .map(|row| {
            Ok([
                usize::try_from(row[0]).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("OLAM gridfile face contains negative M id {}", row[0]),
                    )
                })?,
                usize::try_from(row[1]).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("OLAM gridfile face contains negative M id {}", row[1]),
                    )
                })?,
                usize::try_from(row[2]).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("OLAM gridfile face contains negative M id {}", row[2]),
                    )
                })?,
            ])
        })
        .collect::<io::Result<Vec<_>>>()?;
    let m_face_counts = mesh
        .n_w_to_m
        .iter()
        .map(|&count| {
            usize::try_from(count).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("OLAM gridfile M-point valence is negative: {count}"),
                )
            })
        })
        .collect::<io::Result<Vec<_>>>()?;

    let pentagons = m_face_counts
        .iter()
        .enumerate()
        .filter(|&(row, &count)| row > 0 && count == 5)
        .count();
    if pentagons == 12 {
        return OlamDelaunayMesh::from_voronoi_gridfile_tables(
            &m_point_lonlat,
            &w_face_m_points,
            &m_face_counts,
        );
    }
    let factors = olam_gridinit_factorization_fortran(nxp).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid OLAM gridinit NXP {nxp}"),
        )
    })?;
    let mut mesh =
        OlamDelaunayMesh::from_icosahedron(factors.base_nxp, nspring, beta, spring_relax, max_tris)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "failed to build OLAM icosahedron fallback for NXP={}",
                        factors.base_nxp
                    ),
                )
            })?;
    if factors.expansion_factor > 1 {
        mesh = mesh.expand_by_factor(factors.expansion_factor)?;
    }
    Ok(mesh)
}

pub(crate) fn unstructured_mesh_write_report_from_file(
    output: impl AsRef<Path>,
) -> io::Result<UnstructuredMeshWriteReport> {
    let output = output.as_ref();
    let mesh = read_unstructured_mesh_netcdf(output)?;
    Ok(UnstructuredMeshWriteReport {
        output: output.to_path_buf(),
        sjx_points: mesh.m_points.len(),
        lbx_points: mesh.w_points.len(),
        dimc: unstructured_dimc(&mesh),
    })
}

pub(crate) fn write_olam_mesh_with_optional_domain(
    mesh: &UnstructuredMesh,
    raw_output_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    domain_region: Option<&GridRegion>,
    mode_grid: &str,
) -> io::Result<(
    Option<UnstructuredMeshWriteReport>,
    UnstructuredMeshWriteReport,
)> {
    let output_path = output_path.as_ref();
    match domain_region {
        Some(region) => {
            let raw_output = write_unstructured_mesh_netcdf(raw_output_path, mesh)?;
            let kept = write_regional_gridfile(&raw_output.output, output_path, region, mode_grid)?;
            if kept == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "OLAM domain mask kept no cells",
                ));
            }
            let output = unstructured_mesh_write_report_from_file(output_path)?;
            Ok((Some(raw_output), output))
        }
        None => {
            let output = write_unstructured_mesh_netcdf(output_path, mesh)?;
            Ok((None, output))
        }
    }
}
