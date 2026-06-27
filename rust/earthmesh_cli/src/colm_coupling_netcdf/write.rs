use std::fs;
use std::io;
use std::path::Path;

use crate::colm_coupling_csv::{
    coast_class_code, read_colm_coupling_csv_rows, river_class_code, surface_class_code,
    write_colm_f64_var, write_colm_i32_var, write_colm_i8_var,
};
use crate::{netcdf_to_io_error, ColmCouplingNetcdfWriteReport};

/// Write the CoLM package coupling metadata NetCDF schema from the package CSV.
///
/// This is a Rust-native equivalent of the numeric/string-code boundary in
/// `util.hydro_mesh.colm_coupling` so v3 hydro/coast package handoffs are not
/// Python-only.
pub fn write_colm_coupling_netcdf_from_csv(
    input_csv: impl AsRef<Path>,
    output_netcdf: impl AsRef<Path>,
    case_name: &str,
    delivery_manifest: impl AsRef<Path>,
) -> io::Result<ColmCouplingNetcdfWriteReport> {
    let rows = read_colm_coupling_csv_rows(input_csv)?;
    let output = output_netcdf.as_ref().to_path_buf();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = netcdf::create(&output).map_err(netcdf_to_io_error)?;
    file.add_dimension("cell", rows.len())
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("kind", "earthmesh_colm_coupling_netcdf")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("case_name", case_name)
        .map_err(netcdf_to_io_error)?;
    file.add_attribute(
        "delivery_manifest",
        delivery_manifest.as_ref().to_string_lossy().as_ref(),
    )
    .map_err(netcdf_to_io_error)?;
    file.add_attribute(
        "surface_class_code_meanings",
        "0=UNKNOWN 1=LAND 2=OCEAN 3=COAST",
    )
    .map_err(netcdf_to_io_error)?;
    file.add_attribute("river_class_code_meanings", "0=none/R0 1=R1 2=R2 3=R3")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute(
        "coast_class_code_meanings",
        "0=none 1=COAST 2=COAST_LAND 3=COAST_OCEAN",
    )
    .map_err(netcdf_to_io_error)?;

    {
        let mut var = file
            .add_string_variable("cell_id", &["cell"])
            .map_err(netcdf_to_io_error)?;
        for (index, row) in rows.iter().enumerate() {
            var.put_string(&row.cell_id, index)
                .map_err(netcdf_to_io_error)?;
        }
    }
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
    write_colm_i8_var(
        &mut file,
        "surface_class_code",
        &rows
            .iter()
            .map(|row| surface_class_code(&row.surface_class))
            .collect::<Vec<_>>(),
    )?;
    write_colm_i8_var(
        &mut file,
        "has_river",
        &rows
            .iter()
            .map(|row| i8::from(row.has_river))
            .collect::<Vec<_>>(),
    )?;
    write_colm_i8_var(
        &mut file,
        "river_class_code",
        &rows
            .iter()
            .map(|row| river_class_code(&row.river_class))
            .collect::<Vec<_>>(),
    )?;
    write_colm_f64_var(
        &mut file,
        "river_fraction",
        &rows
            .iter()
            .map(|row| row.river_fraction)
            .collect::<Vec<_>>(),
        Some("1"),
    )?;
    write_colm_f64_var(
        &mut file,
        "estimated_river_area_m2",
        &rows
            .iter()
            .map(|row| row.estimated_river_area_m2)
            .collect::<Vec<_>>(),
        Some("m2"),
    )?;
    write_colm_i8_var(
        &mut file,
        "has_coast",
        &rows
            .iter()
            .map(|row| i8::from(row.has_coast))
            .collect::<Vec<_>>(),
    )?;
    write_colm_i8_var(
        &mut file,
        "coast_class_code",
        &rows
            .iter()
            .map(|row| coast_class_code(&row.coast_class))
            .collect::<Vec<_>>(),
    )?;
    write_colm_f64_var(
        &mut file,
        "coastal_fraction",
        &rows
            .iter()
            .map(|row| row.coastal_fraction)
            .collect::<Vec<_>>(),
        Some("1"),
    )?;
    write_colm_f64_var(
        &mut file,
        "normalized_cell_area_m2",
        &rows
            .iter()
            .map(|row| row.normalized_cell_area_m2)
            .collect::<Vec<_>>(),
        Some("m2"),
    )?;
    write_colm_f64_var(
        &mut file,
        "source_areaCell",
        &rows
            .iter()
            .map(|row| row.source_area_cell)
            .collect::<Vec<_>>(),
        Some("source_areaCell_units from CSV when available"),
    )?;

    Ok(ColmCouplingNetcdfWriteReport {
        output,
        rows: rows.len(),
    })
}
