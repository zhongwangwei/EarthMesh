use std::io;
use std::path::Path;

use earthmesh_mesh::{
    method_c_gridinit_factorization_canonical, LonLatDegrees, MethodCGridfileMetadata,
    TriangularMesh,
};

use crate::{
    read_unstructured_mesh_netcdf, unstructured_dimc, write_regional_gridfile_with_refine_levels,
    write_unstructured_mesh_netcdf_with_method_c_metadata, GridRegion,
    MethodCGridfileMetadataSlices, UnstructuredMesh, UnstructuredMeshWriteReport,
};

pub(crate) fn method_c_delaunay_mesh_from_unstructured_gridfile(
    mesh: &UnstructuredMesh,
    metadata: MethodCGridfileMetadataSlices<'_>,
    nxp: usize,
    nspring: usize,
    beta: f64,
    spring_relax: f64,
    max_tris: usize,
) -> io::Result<TriangularMesh> {
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
                        format!("Method-C gridfile face contains negative M id {}", row[0]),
                    )
                })?,
                usize::try_from(row[1]).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Method-C gridfile face contains negative M id {}", row[1]),
                    )
                })?,
                usize::try_from(row[2]).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Method-C gridfile face contains negative M id {}", row[2]),
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
                    format!("Method-C gridfile M-point valence is negative: {count}"),
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
        return TriangularMesh::from_voronoi_gridfile_tables_with_metadata(
            &m_point_lonlat,
            &w_face_m_points,
            &m_face_counts,
            MethodCGridfileMetadata {
                m_refine_level: metadata.m_refine_level,
                m_refine_level_orig: metadata.m_refine_level_orig,
                m_ngr: metadata.m_ngr,
                w_refine_level: metadata.w_refine_level,
                w_refine_level_orig: metadata.w_refine_level_orig,
                w_ngr: metadata.w_ngr,
            },
        );
    }
    let factors = method_c_gridinit_factorization_canonical(nxp).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid Method-C gridinit NXP {nxp}"),
        )
    })?;
    let mut mesh =
        TriangularMesh::from_icosahedron(factors.base_nxp, nspring, beta, spring_relax, max_tris)
            .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "failed to build Method-C icosahedron fallback for NXP={}",
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

pub(crate) fn write_method_c_mesh_with_optional_domain(
    mesh: &UnstructuredMesh,
    raw_output_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    domain_region: Option<&GridRegion>,
    mode_grid: &str,
) -> io::Result<(
    Option<UnstructuredMeshWriteReport>,
    UnstructuredMeshWriteReport,
)> {
    write_method_c_mesh_with_optional_domain_and_refine_levels(
        mesh,
        raw_output_path,
        output_path,
        domain_region,
        mode_grid,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn write_method_c_mesh_with_optional_domain_and_refine_levels(
    mesh: &UnstructuredMesh,
    raw_output_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    domain_region: Option<&GridRegion>,
    mode_grid: &str,
    m_refine_level: Option<&[i32]>,
    w_refine_level: Option<&[i32]>,
) -> io::Result<(
    Option<UnstructuredMeshWriteReport>,
    UnstructuredMeshWriteReport,
)> {
    write_method_c_mesh_with_optional_domain_and_metadata(
        mesh,
        raw_output_path,
        output_path,
        domain_region,
        mode_grid,
        MethodCGridfileMetadataSlices {
            m_refine_level,
            w_refine_level,
            ..Default::default()
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn write_method_c_mesh_with_optional_domain_and_metadata(
    mesh: &UnstructuredMesh,
    raw_output_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    domain_region: Option<&GridRegion>,
    mode_grid: &str,
    metadata: MethodCGridfileMetadataSlices<'_>,
) -> io::Result<(
    Option<UnstructuredMeshWriteReport>,
    UnstructuredMeshWriteReport,
)> {
    let output_path = output_path.as_ref();
    match domain_region {
        Some(region) => {
            let raw_output = write_unstructured_mesh_netcdf_with_method_c_metadata(
                raw_output_path,
                mesh,
                metadata,
            )?;
            let kept = write_regional_gridfile_with_refine_levels(
                &raw_output.output,
                output_path,
                region,
                mode_grid,
                None,
                None,
            )?;
            if kept == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Method-C domain mask kept no cells",
                ));
            }
            let output = unstructured_mesh_write_report_from_file(output_path)?;
            Ok((Some(raw_output), output))
        }
        None => {
            let output =
                write_unstructured_mesh_netcdf_with_method_c_metadata(output_path, mesh, metadata)?;
            Ok((None, output))
        }
    }
}
