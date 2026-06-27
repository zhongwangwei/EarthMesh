use std::fs;
use std::io;
use std::path::Path;

use crate::{
    netcdf_to_io_error, write_f64_1d, write_f64_matrix_rows, write_f64_scalar, write_i32_1d,
};

use super::types::{GlobalQualityMesh, GlobalQualityWriteReport, QualityClassMetrics};
use super::validation::validate_global_quality_mesh;

/// Write the `quality_save_global` schema produced by
/// `MOD_file_preprocess.F90:quality_save_global`.
pub fn write_quality_global_netcdf(
    output: impl AsRef<Path>,
    quality: &GlobalQualityMesh,
) -> io::Result<GlobalQualityWriteReport> {
    validate_global_quality_mesh(quality)?;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let num_sjx = quality.sjx.length.len();
    let num_wbx = quality.wbx.length.len();
    let num_lbx = quality.lbx.length.len();
    let num_qbx = quality.qbx.as_ref().map_or(0, |qbx| qbx.length.len());

    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("num_sjx", num_sjx)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("num_wbx", num_wbx)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("num_lbx", num_lbx)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("two", 2).map_err(netcdf_to_io_error)?;
    file.add_dimension("thr", 3).map_err(netcdf_to_io_error)?;
    file.add_dimension("fiv", 5).map_err(netcdf_to_io_error)?;
    file.add_dimension("six", 6).map_err(netcdf_to_io_error)?;

    write_quality_class(&mut file, "sjx", "num_sjx", "thr", &quality.sjx)?;
    write_quality_class(&mut file, "wbx", "num_wbx", "fiv", &quality.wbx)?;
    write_quality_class(&mut file, "lbx", "num_lbx", "six", &quality.lbx)?;
    if let Some(qbx) = &quality.qbx {
        file.add_dimension("num_qbx", num_qbx)
            .map_err(netcdf_to_io_error)?;
        file.add_dimension("sev", 7).map_err(netcdf_to_io_error)?;
        write_quality_class(&mut file, "qbx", "num_qbx", "sev", qbx)?;
    }

    Ok(GlobalQualityWriteReport {
        output: output.to_path_buf(),
        num_sjx,
        num_wbx,
        num_lbx,
        num_qbx,
    })
}

fn write_quality_class(
    file: &mut netcdf::FileMut,
    suffix: &str,
    row_dim: &str,
    width_dim: &str,
    metrics: &QualityClassMetrics,
) -> io::Result<()> {
    write_f64_matrix_rows(
        file,
        &format!("length_{suffix}"),
        &[row_dim, width_dim],
        &metrics.length,
    )?;
    write_f64_matrix_rows(
        file,
        &format!("angle_{suffix}"),
        &[row_dim, width_dim],
        &metrics.angle,
    )?;
    write_f64_1d(file, &format!("Extr_{suffix}"), "two", &metrics.extr)?;
    write_f64_1d(file, &format!("Eavg_{suffix}"), "two", &metrics.eavg)?;
    write_f64_scalar(file, &format!("Savg_{suffix}"), metrics.savg)?;
    write_i32_1d(file, &format!("less_{suffix}"), row_dim, &metrics.less)?;
    write_i32_1d(file, &format!("more_{suffix}"), row_dim, &metrics.more)?;
    Ok(())
}
