use std::fs;
use std::io;
use std::path::Path;

use earthmesh_mesh::{BoundaryConnection, BoundaryOrders};

use crate::{netcdf_to_io_error, require_len, usize_values_to_i32};

use super::reports::{ObcBoundaryWriteReport, Obcv2BoundaryWriteReport};

/// Write the `obc.nc4`/`obc_patch.nc4` schema produced by
/// `MOD_mask_postproc.F90:bdy_calculation`.
pub fn write_obc_boundary_netcdf(
    output: impl AsRef<Path>,
    orders: &BoundaryOrders,
) -> io::Result<ObcBoundaryWriteReport> {
    let bdy_num = orders.bdy_order.len();
    require_len("obc_order", orders.obc_order.len(), bdy_num)?;
    require_len("ibc_order", orders.ibc_order.len(), bdy_num)?;

    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("bdy_num", bdy_num)
        .map_err(netcdf_to_io_error)?;
    write_usize_1d(&mut file, "bdy_order", "bdy_num", &orders.bdy_order)?;
    write_usize_1d(&mut file, "obc_order", "bdy_num", &orders.obc_order)?;
    write_usize_1d(&mut file, "ibc_order", "bdy_num", &orders.ibc_order)?;

    Ok(ObcBoundaryWriteReport {
        output: output.to_path_buf(),
        boundary_points: bdy_num,
    })
}

/// Write the `obcv2.nc4`/`obcv2_patch.nc4` schema produced by
/// `MOD_mask_postproc.F90:bdy_connection`.
pub fn write_obcv2_boundary_netcdf(
    output: impl AsRef<Path>,
    connection: &BoundaryConnection,
) -> io::Result<Obcv2BoundaryWriteReport> {
    let num1 = connection.curves.num_bdy_long[0];
    let num2 = connection.curves.num_closed_curve;
    if connection.curves.close_curves.len() < num2 + 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "close_curves must include the placeholder plus num_closed_curve records",
        ));
    }
    if connection.curves.n_close_curve.len() < num2 + 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "n_close_curve must include the placeholder plus num_closed_curve records",
        ));
    }

    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let close_curve_values =
        flatten_close_curves_for_netcdf(&connection.curves.close_curves, num1, num2)?;
    let n_close_curve_values =
        usize_values_to_i32("n_close_curve", &connection.curves.n_close_curve[1..=num2])?;

    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("num1", num1)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("num2", num2)
        .map_err(netcdf_to_io_error)?;
    {
        let mut var = file
            .add_variable::<i32>("close_curve", &["num2", "num1"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&close_curve_values, (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<i32>("n_close_curve", &["num2"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&n_close_curve_values, ..)
            .map_err(netcdf_to_io_error)?;
    }

    Ok(Obcv2BoundaryWriteReport {
        output: output.to_path_buf(),
        longest_curve_slots: num1,
        closed_curves: num2,
    })
}

fn write_usize_1d(
    file: &mut netcdf::FileMut,
    name: &str,
    dim: &str,
    values: &[usize],
) -> io::Result<()> {
    let mut var = file
        .add_variable::<i32>(name, &[dim])
        .map_err(netcdf_to_io_error)?;
    var.put_values(&usize_values_to_i32(name, values)?, ..)
        .map_err(netcdf_to_io_error)
}

fn flatten_close_curves_for_netcdf(
    close_curves: &[Vec<usize>],
    longest_curve_slots: usize,
    closed_curves: usize,
) -> io::Result<Vec<i32>> {
    let mut values = Vec::with_capacity(longest_curve_slots * closed_curves);
    for curve_id in 1..=closed_curves {
        let curve = &close_curves[curve_id];
        if curve.len() > longest_curve_slots {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "close_curve {curve_id} length {} exceeds num1 {longest_curve_slots}",
                    curve.len()
                ),
            ));
        }
        values.extend(usize_values_to_i32("close_curve", curve)?);
        values.resize(values.len() + longest_curve_slots - curve.len(), 1);
    }
    Ok(values)
}
