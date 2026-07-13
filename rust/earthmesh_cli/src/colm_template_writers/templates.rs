use std::io;
use std::path::Path;

use crate::colm_coupling_csv::{
    colm_land_fraction, finite_or_fill, finite_or_zero, fraction_or_fill, fraction_or_zero,
    read_colm_coupling_csv_rows, write_colm_f64_var, write_colm_i32_var,
};
use crate::{
    netcdf_to_io_error, ColmForcingTemplateNetcdfWriteReport, ColmRestartTemplateNetcdfWriteReport,
};

/// Write a small CoLM restart-template handoff from the same package CSV used
/// by the coupling metadata exporter.
///
/// The template keeps EarthMesh hydro/coast fractions in model-ready NetCDF
/// columns so CoLM2024/CoLM20XX adapters can consume a Rust-written restart
/// seed without depending on Python-only postprocessing.
pub fn write_colm_restart_template_netcdf_from_csv(
    input_csv: impl AsRef<Path>,
    output_netcdf: impl AsRef<Path>,
    case_name: &str,
) -> io::Result<ColmRestartTemplateNetcdfWriteReport> {
    let rows = read_colm_coupling_csv_rows(input_csv)?;
    let output = output_netcdf.as_ref().to_path_buf();
    crate::ensure_parent_dir(&output)?;
    let mut file = crate::create_netcdf(&output).map_err(netcdf_to_io_error)?;
    file.add_dimension("cell", rows.len())
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("kind", "earthmesh_colm_restart_template_netcdf")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("case_name", case_name)
        .map_err(netcdf_to_io_error)?;
    file.add_attribute(
        "land_fraction_definition",
        "LAND=1 OCEAN=0 COAST=1-coastal_fraction",
    )
    .map_err(netcdf_to_io_error)?;

    write_colm_i32_var(
        &mut file,
        "cell_index",
        &rows.iter().map(|row| row.cell_index).collect::<Vec<_>>(),
    )?;
    write_colm_f64_var(
        &mut file,
        "center_lon",
        &rows.iter().map(|row| row.center_lon).collect::<Vec<_>>(),
        Some("degrees_east"),
    )?;
    write_colm_f64_var(
        &mut file,
        "center_lat",
        &rows.iter().map(|row| row.center_lat).collect::<Vec<_>>(),
        Some("degrees_north"),
    )?;
    write_colm_f64_var(
        &mut file,
        "land_fraction",
        &rows.iter().map(colm_land_fraction).collect::<Vec<_>>(),
        Some("1"),
    )?;
    write_colm_f64_var(
        &mut file,
        "river_fraction",
        &rows
            .iter()
            .map(|row| fraction_or_fill(row.river_fraction))
            .collect::<Vec<_>>(),
        Some("1"),
    )?;
    write_colm_f64_var(
        &mut file,
        "coastal_fraction",
        &rows
            .iter()
            .map(|row| fraction_or_fill(row.coastal_fraction))
            .collect::<Vec<_>>(),
        Some("1"),
    )?;
    write_colm_f64_var(
        &mut file,
        "normalized_cell_area_m2",
        &rows
            .iter()
            .map(|row| finite_or_fill(row.normalized_cell_area_m2))
            .collect::<Vec<_>>(),
        Some("m2"),
    )?;

    Ok(ColmRestartTemplateNetcdfWriteReport {
        output,
        rows: rows.len(),
    })
}

/// Write a CoLM forcing-template handoff from the package CSV.
///
/// This converts the EarthMesh surface/hydro/coast fractions into area-weighted
/// model forcing columns so coupled-model adapters have a Rust-owned seed file
/// before full model-specific forcing generation is completed.
pub fn write_colm_forcing_template_netcdf_from_csv(
    input_csv: impl AsRef<Path>,
    output_netcdf: impl AsRef<Path>,
    case_name: &str,
) -> io::Result<ColmForcingTemplateNetcdfWriteReport> {
    let rows = read_colm_coupling_csv_rows(input_csv)?;
    let output = output_netcdf.as_ref().to_path_buf();
    crate::ensure_parent_dir(&output)?;
    let mut file = crate::create_netcdf(&output).map_err(netcdf_to_io_error)?;
    file.add_dimension("cell", rows.len())
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("kind", "earthmesh_colm_forcing_template_netcdf")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("case_name", case_name)
        .map_err(netcdf_to_io_error)?;
    file.add_attribute(
        "area_definition",
        "land_area=normalized_area*land_fraction river_area=estimated_river_area coastal_area=normalized_area*coastal_fraction",
    )
    .map_err(netcdf_to_io_error)?;

    write_colm_i32_var(
        &mut file,
        "cell_index",
        &rows.iter().map(|row| row.cell_index).collect::<Vec<_>>(),
    )?;
    write_colm_f64_var(
        &mut file,
        "center_lon",
        &rows.iter().map(|row| row.center_lon).collect::<Vec<_>>(),
        Some("degrees_east"),
    )?;
    write_colm_f64_var(
        &mut file,
        "center_lat",
        &rows.iter().map(|row| row.center_lat).collect::<Vec<_>>(),
        Some("degrees_north"),
    )?;
    write_colm_f64_var(
        &mut file,
        "land_forcing_area_m2",
        &rows
            .iter()
            .map(|row| row.normalized_cell_area_m2 * colm_land_fraction(row))
            .collect::<Vec<_>>(),
        Some("m2"),
    )?;
    write_colm_f64_var(
        &mut file,
        "river_forcing_area_m2",
        &rows
            .iter()
            .map(|row| finite_or_zero(row.estimated_river_area_m2))
            .collect::<Vec<_>>(),
        Some("m2"),
    )?;
    write_colm_f64_var(
        &mut file,
        "coastal_forcing_area_m2",
        &rows
            .iter()
            .map(|row| row.normalized_cell_area_m2 * fraction_or_zero(row.coastal_fraction))
            .collect::<Vec<_>>(),
        Some("m2"),
    )?;

    Ok(ColmForcingTemplateNetcdfWriteReport {
        output,
        rows: rows.len(),
    })
}
