use crate::require_len;
use crate::LonLatPoint;
use crate::UnstructuredMesh;
use earthmesh_mesh::LonLatDegrees;
use earthmesh_mesh::SpringjustmentGlobalCoreOutput;
use earthmesh_mesh::SpringjustmentRegionalCoreOutput;
use std::io;

pub(super) fn lonlat_degrees_to_lonlat_point(point: LonLatDegrees) -> LonLatPoint {
    LonLatPoint {
        lon: point.lon_degrees,
        lat: point.lat_degrees,
    }
}

pub(super) fn unstructured_mesh_from_springjustment_global(
    source: &UnstructuredMesh,
    output: &SpringjustmentGlobalCoreOutput,
) -> io::Result<UnstructuredMesh> {
    require_len(
        "Springjustment_global updated_triangle_lonlat",
        output.updated_triangle_lonlat.len(),
        source.m_points.len(),
    )?;
    require_len(
        "Springjustment_global updated_cell_lonlat",
        output.updated_cell_lonlat.len(),
        source.w_points.len(),
    )?;

    Ok(UnstructuredMesh {
        m_points: output
            .updated_triangle_lonlat
            .iter()
            .take(source.m_points.len())
            .copied()
            .map(lonlat_degrees_to_lonlat_point)
            .collect(),
        w_points: output
            .updated_cell_lonlat
            .iter()
            .take(source.w_points.len())
            .copied()
            .map(lonlat_degrees_to_lonlat_point)
            .collect(),
        m_to_w: source.m_to_w.clone(),
        w_to_m: source.w_to_m.clone(),
        n_w_to_m: source.n_w_to_m.clone(),
    })
}

pub(super) fn unstructured_mesh_from_springjustment_regional(
    source: &UnstructuredMesh,
    output: &SpringjustmentRegionalCoreOutput,
) -> io::Result<UnstructuredMesh> {
    require_len(
        "Springjustment_regional_step updated_triangle_lonlat",
        output.updated_triangle_lonlat.len(),
        source.m_points.len(),
    )?;
    require_len(
        "Springjustment_regional_step updated_cell_lonlat",
        output.updated_cell_lonlat.len(),
        source.w_points.len(),
    )?;

    Ok(UnstructuredMesh {
        m_points: output
            .updated_triangle_lonlat
            .iter()
            .take(source.m_points.len())
            .copied()
            .map(lonlat_degrees_to_lonlat_point)
            .collect(),
        w_points: output
            .updated_cell_lonlat
            .iter()
            .take(source.w_points.len())
            .copied()
            .map(lonlat_degrees_to_lonlat_point)
            .collect(),
        m_to_w: source.m_to_w.clone(),
        w_to_m: source.w_to_m.clone(),
        n_w_to_m: source.n_w_to_m.clone(),
    })
}
