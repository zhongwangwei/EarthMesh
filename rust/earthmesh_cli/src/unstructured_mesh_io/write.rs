use std::io;
use std::path::Path;

use crate::{
    lat_values, lon_values, netcdf_to_io_error, unstructured_dimc, validate_unstructured_mesh,
    MethodCGridfileMetadataSlices, UnstructuredMesh, UnstructuredMeshWriteReport,
};

use super::rows::{flatten_m_to_w, flatten_w_to_m};

pub fn write_unstructured_mesh_netcdf(
    output: impl AsRef<Path>,
    mesh: &UnstructuredMesh,
) -> io::Result<UnstructuredMeshWriteReport> {
    write_unstructured_mesh_netcdf_with_method_c_metadata(output, mesh, Default::default())
}

pub fn write_unstructured_mesh_netcdf_with_refine_levels(
    output: impl AsRef<Path>,
    mesh: &UnstructuredMesh,
    m_refine_level: Option<&[i32]>,
    w_refine_level: Option<&[i32]>,
) -> io::Result<UnstructuredMeshWriteReport> {
    write_unstructured_mesh_netcdf_with_method_c_metadata(
        output,
        mesh,
        MethodCGridfileMetadataSlices {
            m_refine_level,
            w_refine_level,
            ..Default::default()
        },
    )
}

pub fn write_unstructured_mesh_netcdf_with_method_c_metadata(
    output: impl AsRef<Path>,
    mesh: &UnstructuredMesh,
    metadata: MethodCGridfileMetadataSlices<'_>,
) -> io::Result<UnstructuredMeshWriteReport> {
    validate_unstructured_mesh(mesh)?;
    for (name, values) in [
        ("earthmesh_m_refine_level", metadata.m_refine_level),
        (
            "earthmesh_m_refine_level_orig",
            metadata.m_refine_level_orig,
        ),
        ("earthmesh_m_ngr", metadata.m_ngr),
    ] {
        validate_metadata_len(name, values, mesh.m_points.len())?;
    }
    for (name, values) in [
        ("earthmesh_w_refine_level", metadata.w_refine_level),
        (
            "earthmesh_w_refine_level_orig",
            metadata.w_refine_level_orig,
        ),
        ("earthmesh_w_ngr", metadata.w_ngr),
    ] {
        validate_metadata_len(name, values, mesh.w_points.len())?;
    }
    validate_metadata_len(
        "earthmesh_m_lineage",
        metadata.m_lineage,
        mesh.m_points.len(),
    )?;
    validate_metadata_len(
        "earthmesh_w_lineage",
        metadata.w_lineage,
        mesh.w_points.len(),
    )?;
    let output = output.as_ref();
    crate::ensure_parent_dir(output)?;

    let dimc = unstructured_dimc(mesh);
    let mut file = crate::create_netcdf(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("sjx_points", mesh.m_points.len())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("lbx_points", mesh.w_points.len())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("dimb", 3).map_err(netcdf_to_io_error)?;
    file.add_dimension("dimc", dimc)
        .map_err(netcdf_to_io_error)?;

    {
        let mut var = file
            .add_variable::<f64>("GLONM", &["sjx_points"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&lon_values(&mesh.m_points), ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<f64>("GLATM", &["sjx_points"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&lat_values(&mesh.m_points), ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<f64>("GLONW", &["lbx_points"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&lon_values(&mesh.w_points), ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<f64>("GLATW", &["lbx_points"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&lat_values(&mesh.w_points), ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<i32>("itab_m%iw", &["sjx_points", "dimb"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&flatten_m_to_w(&mesh.m_to_w), (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<i32>("itab_w%im", &["lbx_points", "dimc"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&flatten_w_to_m(&mesh.w_to_m, dimc), (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<i32>("n_ngrwm", &["lbx_points"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&mesh.n_w_to_m, ..)
            .map_err(netcdf_to_io_error)?;
    }
    for (name, dimension, values) in [
        (
            "earthmesh_m_refine_level",
            "sjx_points",
            metadata.m_refine_level,
        ),
        (
            "earthmesh_m_refine_level_orig",
            "sjx_points",
            metadata.m_refine_level_orig,
        ),
        ("earthmesh_m_ngr", "sjx_points", metadata.m_ngr),
        (
            "earthmesh_w_refine_level",
            "lbx_points",
            metadata.w_refine_level,
        ),
        (
            "earthmesh_w_refine_level_orig",
            "lbx_points",
            metadata.w_refine_level_orig,
        ),
        ("earthmesh_w_ngr", "lbx_points", metadata.w_ngr),
    ] {
        if let Some(values) = values {
            let mut var = file
                .add_variable::<i32>(name, &[dimension])
                .map_err(netcdf_to_io_error)?;
            var.put_values(values, ..).map_err(netcdf_to_io_error)?;
        }
    }
    for (name, dimension, values) in [
        ("earthmesh_m_lineage", "sjx_points", metadata.m_lineage),
        ("earthmesh_w_lineage", "lbx_points", metadata.w_lineage),
    ] {
        if let Some(values) = values {
            let mut var = file
                .add_variable::<i64>(name, &[dimension])
                .map_err(netcdf_to_io_error)?;
            var.put_values(values, ..).map_err(netcdf_to_io_error)?;
        }
    }

    Ok(UnstructuredMeshWriteReport {
        output: output.to_path_buf(),
        sjx_points: mesh.m_points.len(),
        lbx_points: mesh.w_points.len(),
        dimc,
    })
}

fn validate_metadata_len<T>(name: &str, levels: Option<&[T]>, expected: usize) -> io::Result<()> {
    let Some(levels) = levels else {
        return Ok(());
    };
    if levels.len() == expected {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("{name} length {} must equal {expected}", levels.len()),
    ))
}
