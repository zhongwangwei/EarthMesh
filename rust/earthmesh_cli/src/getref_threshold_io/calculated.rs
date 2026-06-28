use std::io;
use std::path::PathBuf;

use crate::netcdf_to_io_error;

/// Read calculated GetRef component threshold files and aggregate their
/// `ref_th_*` matrices into the top-level one-based `ref_sjx` marker vector.
pub fn read_getref_calculated_ref_sjx_netcdf(
    inputs: &[PathBuf],
    num_vertex: usize,
) -> io::Result<Vec<i32>> {
    if inputs.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "calculated threshold marker reader requires at least one file",
        ));
    }
    let mut markers: Option<Vec<i32>> = None;
    for input in inputs {
        let file = crate::open_netcdf(input).map_err(netcdf_to_io_error)?;
        let (var_name, variable) = ["ref_th_Lnd", "ref_th_Ocn", "ref_th_Atmos"]
            .into_iter()
            .find_map(|name| file.variable(name).map(|variable| (name, variable)))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "calculated threshold file {} is missing ref_th_Lnd/ref_th_Ocn/ref_th_Atmos",
                        input.display()
                    ),
                )
            })?;
        let sjx_points = file
            .dimension("sjx_points")
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{var_name} threshold file is missing sjx_points dimension"),
                )
            })?
            .len();
        let ref_colnum = file
            .dimension("ref_colnum")
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{var_name} threshold file is missing ref_colnum dimension"),
                )
            })?
            .len();
        if num_vertex > sjx_points {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("num_vertex {num_vertex} exceeds {var_name} sjx_points {sjx_points}"),
            ));
        }
        let values = variable
            .get_values::<i32, _>((.., ..))
            .map_err(netcdf_to_io_error)?;
        if values.len() != sjx_points * ref_colnum {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{var_name} has {} values but sjx_points*ref_colnum is {}",
                    values.len(),
                    sjx_points * ref_colnum
                ),
            ));
        }
        let target = markers.get_or_insert_with(|| vec![0; sjx_points + 1]);
        if target.len() != sjx_points + 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "calculated threshold file {} has sjx_points {sjx_points}, expected {}",
                    input.display(),
                    target.len() - 1
                ),
            ));
        }
        for sjx_index in (num_vertex + 1)..=sjx_points {
            let start = (sjx_index - 1) * ref_colnum;
            if values[start..start + ref_colnum]
                .iter()
                .any(|&marker| marker != 0)
            {
                target[sjx_index] = 1;
            }
        }
    }
    markers.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "calculated threshold marker reader received no files",
        )
    })
}
