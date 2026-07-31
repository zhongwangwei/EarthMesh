use std::{io, path::Path};

use crate::{
    gridfile_output_path, netcdf_to_io_error, normalize_degrees, require_len,
    required_dimension_len, required_values_f64, required_values_i32, required_values_i32_matrix,
    write_unstructured_mesh_netcdf, LonLatPoint, UnstructuredMesh, UnstructuredMeshWriteReport,
};

use super::{
    detect_connectivity_base, earthmesh_canonical_connectivity_id, validate_connectivity_base,
};

pub fn convert_fvcom_mode_file_to_earthmesh(
    mode_file: impl AsRef<Path>,
    file_dir: impl AsRef<Path>,
    nxp: usize,
    mode_grid: &str,
) -> io::Result<UnstructuredMeshWriteReport> {
    let mode_file = mode_file.as_ref();
    let file = crate::open_netcdf(mode_file).map_err(netcdf_to_io_error)?;
    let maxelem = required_dimension_len(&file, "maxelem")?;
    let n_nodes = required_dimension_len(&file, "node")?;
    let n_elements = required_dimension_len(&file, "nele")?;
    if maxelem == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "FVCOM maxelem must be positive",
        ));
    }

    let lonc = required_values_f64(&file, "lonc")?;
    let latc = required_values_f64(&file, "latc")?;
    let lon = required_values_f64(&file, "lon")?;
    let lat = required_values_f64(&file, "lat")?;
    let nv = required_values_i32_matrix(&file, "nv", "nele", "three", n_elements, 3)?;
    let nbve = required_values_i32_matrix(&file, "nbve", "node", "maxelem", n_nodes, maxelem)?;
    let ntve = required_values_i32(&file, "ntve")?;

    require_len("lonc", lonc.len(), n_elements)?;
    require_len("latc", latc.len(), n_elements)?;
    require_len("lon", lon.len(), n_nodes)?;
    require_len("lat", lat.len(), n_nodes)?;
    require_len("ntve", ntve.len(), n_nodes)?;
    for (node, &count) in ntve.iter().enumerate() {
        if count < 0 || count as usize > maxelem {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("FVCOM ntve[{node}]={count} must be in 0..={maxelem}"),
            ));
        }
    }
    // Every nv entry is active, so it unambiguously distinguishes standard
    // FVCOM's 1-based ids from EarthMesh's historical 0-based dialect. Reuse
    // that decision for nbve, whose zero padding is otherwise ambiguous.
    let connectivity_base = detect_connectivity_base("FVCOM nv", &nv, n_nodes)?;
    validate_connectivity_base(
        "FVCOM nbve",
        &nbve,
        n_elements,
        connectivity_base,
        connectivity_base == super::ConnectivityBase::One,
    )?;

    let mut m_points = Vec::with_capacity(n_elements + 1);
    m_points.push(LonLatPoint { lon: 0.0, lat: 0.0 });
    for idx in 0..n_elements {
        m_points.push(LonLatPoint {
            lon: normalize_degrees(lonc[idx]),
            lat: latc[idx],
        });
    }

    let mut w_points = Vec::with_capacity(n_nodes + 1);
    w_points.push(LonLatPoint { lon: 0.0, lat: 0.0 });
    for idx in 0..n_nodes {
        w_points.push(LonLatPoint {
            lon: normalize_degrees(lon[idx]),
            lat: lat[idx],
        });
    }

    let mut m_to_w = Vec::with_capacity(n_elements + 1);
    m_to_w.push([1, 1, 1]);
    for element in 0..n_elements {
        let base = element * 3;
        let mut triangle = [
            earthmesh_canonical_connectivity_id(nv[base], connectivity_base),
            earthmesh_canonical_connectivity_id(nv[base + 1], connectivity_base),
            earthmesh_canonical_connectivity_id(nv[base + 2], connectivity_base),
        ];
        let ring = triangle.map(|node| {
            let point = w_points[(node - 1) as usize];
            earthmesh_geometry::Point::new(point.lon, point.lat)
        });
        if earthmesh_geometry::signed_spherical_polygon_area_km2(&ring) < 0.0 {
            triangle.swap(1, 2);
        }
        m_to_w.push(triangle);
    }

    let mut w_to_m = Vec::with_capacity(n_nodes + 1);
    w_to_m.push(vec![1; maxelem]);
    for node in 0..n_nodes {
        let base = node * maxelem;
        w_to_m.push(
            nbve[base..base + maxelem]
                .iter()
                .map(|&value| earthmesh_canonical_connectivity_id(value, connectivity_base))
                .collect(),
        );
    }

    let mut n_w_to_m = Vec::with_capacity(n_nodes + 1);
    n_w_to_m.push(0);
    n_w_to_m.extend(ntve);

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
