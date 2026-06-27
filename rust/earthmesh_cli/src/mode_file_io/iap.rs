use std::{io, path::Path};

use earthmesh_mesh::{
    centroid_spherical_mesh_fortran_indexed, circumcenter_spherical_mesh_fortran_indexed,
    lonlat_points_to_unit_xyz, xyz_to_lonlat_degrees, LonLatDegrees,
};

use crate::{
    derive_iap_w_to_m_fortran_indexed, gridfile_output_path, netcdf_to_io_error, normalize_degrees,
    rad_to_deg, require_len, required_dimension_len, required_values_f64,
    required_values_i32_matrix, scale_cartesian_points_by_earth_radius,
    usize_from_i32_connectivity, write_unstructured_mesh_netcdf, IapMeshReadPayload, LonLatPoint,
    UnstructuredMesh, UnstructuredMeshWriteReport,
};

pub fn read_iap_mesh_netcdf(inputfile: impl AsRef<Path>) -> io::Result<IapMeshReadPayload> {
    let file = netcdf::open(inputfile.as_ref()).map_err(netcdf_to_io_error)?;
    let source_triangles = required_dimension_len(&file, "sjx_points")?;
    let source_vertices = required_dimension_len(&file, "lbx_points")?;

    let glonw = required_values_f64(&file, "GLONW")?;
    let glatw = required_values_f64(&file, "GLATW")?;
    let source_neighbors = required_values_i32_matrix(
        &file,
        "itab_m%im",
        "sjx_points",
        "dimb",
        source_triangles,
        3,
    )?;
    let source_vertices_on_triangle = required_values_i32_matrix(
        &file,
        "itab_m%iw",
        "sjx_points",
        "dimb",
        source_triangles,
        3,
    )?;

    require_len("GLONW", glonw.len(), source_vertices)?;
    require_len("GLATW", glatw.len(), source_vertices)?;

    let mut w_points = Vec::with_capacity(source_vertices + 1);
    w_points.push(LonLatPoint { lon: 0.0, lat: 0.0 });
    for idx in 0..source_vertices {
        w_points.push(LonLatPoint {
            lon: normalize_degrees(rad_to_deg(glonw[idx])),
            lat: rad_to_deg(glatw[idx]),
        });
    }

    let mut triangle_neighbors = Vec::with_capacity(source_triangles + 1);
    let mut triangle_vertices = Vec::with_capacity(source_triangles + 1);
    triangle_neighbors.push([1, 1, 1]);
    triangle_vertices.push([1, 1, 1]);
    for idx in 0..source_triangles {
        let base = idx * 3;
        triangle_neighbors.push([
            source_neighbors[base] + 1,
            source_neighbors[base + 1] + 1,
            source_neighbors[base + 2] + 1,
        ]);
        triangle_vertices.push([
            source_vertices_on_triangle[base] + 1,
            source_vertices_on_triangle[base + 1] + 1,
            source_vertices_on_triangle[base + 2] + 1,
        ]);
    }

    Ok(IapMeshReadPayload {
        w_points,
        triangle_neighbors,
        triangle_vertices,
    })
}

pub fn convert_iap_ocean_mode_file_to_earthmesh(
    mode_file: impl AsRef<Path>,
    file_dir: impl AsRef<Path>,
    nxp: usize,
    mode_grid: &str,
) -> io::Result<UnstructuredMeshWriteReport> {
    let mode_file = mode_file.as_ref();
    let file = netcdf::open(mode_file).map_err(netcdf_to_io_error)?;
    let source_triangles = required_dimension_len(&file, "sjx_points")?;
    let source_vertices = required_dimension_len(&file, "lbx_points")?;
    let fortran_triangles = source_triangles + 1;
    let fortran_vertices = source_vertices + 1;

    let glonw = required_values_f64(&file, "GLONW")?;
    let glatw = required_values_f64(&file, "GLATW")?;
    let _triangles_on_triangle = required_values_i32_matrix(
        &file,
        "itab_m%im",
        "sjx_points",
        "dimb",
        source_triangles,
        3,
    )?;
    let source_m_to_w = required_values_i32_matrix(
        &file,
        "itab_m%iw",
        "sjx_points",
        "dimb",
        source_triangles,
        3,
    )?;

    require_len("GLONW", glonw.len(), source_vertices)?;
    require_len("GLATW", glatw.len(), source_vertices)?;

    let mut w_points_fortran = vec![LonLatDegrees::new(0.0, 0.0); fortran_vertices + 1];
    for source_idx in 0..source_vertices {
        let fortran_idx = source_idx + 2;
        w_points_fortran[fortran_idx] = LonLatDegrees::new(
            normalize_degrees(rad_to_deg(glonw[source_idx])),
            rad_to_deg(glatw[source_idx]),
        );
    }

    let mut m_to_w_fortran = vec![[1_usize, 1, 1]; fortran_triangles + 1];
    for source_idx in 0..source_triangles {
        let fortran_idx = source_idx + 2;
        let base = source_idx * 3;
        m_to_w_fortran[fortran_idx] = [
            usize_from_i32_connectivity(source_m_to_w[base], "itab_m%iw")? + 1,
            usize_from_i32_connectivity(source_m_to_w[base + 1], "itab_m%iw")? + 1,
            usize_from_i32_connectivity(source_m_to_w[base + 2], "itab_m%iw")? + 1,
        ];
    }

    let centroids = centroid_spherical_mesh_fortran_indexed(&w_points_fortran, &m_to_w_fortran)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "IAP-Ocean triangle connectivity references missing W points",
            )
        })?;
    let mut centroid_xyz = lonlat_points_to_unit_xyz(&centroids);
    let mut vertex_xyz = lonlat_points_to_unit_xyz(&w_points_fortran);
    scale_cartesian_points_by_earth_radius(&mut centroid_xyz);
    scale_cartesian_points_by_earth_radius(&mut vertex_xyz);
    let circumcenters =
        circumcenter_spherical_mesh_fortran_indexed(&centroid_xyz, &vertex_xyz, &m_to_w_fortran)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "IAP-Ocean spherical circumcenter calculation failed",
                )
            })?;

    let mut m_points_fortran = vec![LonLatDegrees::new(0.0, 0.0); fortran_triangles + 1];
    for fortran_idx in 2..=fortran_triangles {
        m_points_fortran[fortran_idx] = xyz_to_lonlat_degrees(circumcenters[fortran_idx]);
    }

    let mut m_points = Vec::with_capacity(fortran_triangles);
    for lonlat in m_points_fortran.iter().take(fortran_triangles + 1).skip(1) {
        m_points.push(LonLatPoint {
            lon: lonlat.lon_degrees,
            lat: lonlat.lat_degrees,
        });
    }

    let mut w_points = Vec::with_capacity(fortran_vertices);
    for point in w_points_fortran.iter().take(fortran_vertices + 1).skip(1) {
        w_points.push(LonLatPoint {
            lon: point.lon_degrees,
            lat: point.lat_degrees,
        });
    }

    let m_to_w = (1..=fortran_triangles)
        .map(|idx| {
            [
                m_to_w_fortran[idx][0] as i32,
                m_to_w_fortran[idx][1] as i32,
                m_to_w_fortran[idx][2] as i32,
            ]
        })
        .collect::<Vec<_>>();
    let (w_to_m, n_w_to_m) =
        derive_iap_w_to_m_fortran_indexed(fortran_vertices, &m_to_w_fortran, &m_points_fortran)?;

    let mesh = UnstructuredMesh {
        m_points,
        w_points,
        m_to_w,
        w_to_m,
        n_w_to_m,
    };
    let output = gridfile_output_path(file_dir, nxp, 1, mode_grid);
    write_unstructured_mesh_netcdf(output, &mesh)
}
