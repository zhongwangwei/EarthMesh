use std::io;

use crate::{netcdf_to_io_error, require_len};

pub(crate) fn required_values_f64_any_matrix(
    file: &netcdf::File,
    name: &str,
    outer_len: usize,
    inner_len: usize,
) -> io::Result<Vec<f64>> {
    let variable = file.variable(name).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("missing {name} variable"),
        )
    })?;
    let dimensions = variable.dimensions();
    if dimensions.len() != 2 || dimensions[0].len() != outer_len || dimensions[1].len() != inner_len
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} dimensions must match longitude x latitude"),
        ));
    }
    if let Ok(values) = variable.get_values::<f64, _>((.., ..)) {
        require_len(name, values.len(), outer_len * inner_len)?;
        return Ok(values);
    }
    if let Ok(values) = variable.get_values::<f32, _>((.., ..)) {
        require_len(name, values.len(), outer_len * inner_len)?;
        return Ok(values.into_iter().map(f64::from).collect());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{name} variable must be readable as f64 or f32"),
    ))
}

pub(crate) fn required_values_i32_any_matrix(
    file: &netcdf::File,
    name: &str,
    outer_len: usize,
    inner_len: usize,
) -> io::Result<Vec<i32>> {
    let variable = file.variable(name).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("missing {name} variable"),
        )
    })?;
    let dimensions = variable.dimensions();
    if dimensions.len() != 2 || dimensions[0].len() != outer_len || dimensions[1].len() != inner_len
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} dimensions must match longitude x latitude"),
        ));
    }
    if let Ok(values) = variable.get_values::<i32, _>((.., ..)) {
        require_len(name, values.len(), outer_len * inner_len)?;
        return Ok(values);
    }
    if let Ok(values) = variable.get_values::<i16, _>((.., ..)) {
        require_len(name, values.len(), outer_len * inner_len)?;
        return Ok(values.into_iter().map(i32::from).collect());
    }
    if let Ok(values) = variable.get_values::<i8, _>((.., ..)) {
        require_len(name, values.len(), outer_len * inner_len)?;
        return Ok(values.into_iter().map(i32::from).collect());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{name} variable must be readable as i32, i16, or i8"),
    ))
}

pub(crate) fn required_values_i32_2d(file: &netcdf::File, name: &str) -> io::Result<Vec<i32>> {
    file.variable(name)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing {name} variable"),
            )
        })?
        .get_values::<i32, _>((.., ..))
        .map_err(netcdf_to_io_error)
}

pub(crate) fn required_values_i8_matrix(
    file: &netcdf::File,
    name: &str,
    outer_dim: &str,
    inner_dim: &str,
    outer_len: usize,
    inner_len: usize,
) -> io::Result<Vec<i8>> {
    let variable = file.variable(name).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("missing {name} variable"),
        )
    })?;
    let dimensions = variable.dimensions();
    if dimensions.len() != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} must be 2-D over {outer_dim}, {inner_dim}"),
        ));
    }
    let dimension_names = dimensions
        .iter()
        .map(|dimension| dimension.name())
        .collect::<Vec<_>>();
    let dimension_lengths = dimensions
        .iter()
        .map(|dimension| dimension.len())
        .collect::<Vec<_>>();
    let values = variable
        .get_values::<i8, _>((.., ..))
        .map_err(netcdf_to_io_error)?;
    require_len(name, values.len(), outer_len * inner_len)?;

    if dimension_names == [outer_dim, inner_dim] || dimension_lengths == [outer_len, inner_len] {
        return Ok(values);
    }
    if dimension_names == [inner_dim, outer_dim] || dimension_lengths == [inner_len, outer_len] {
        let mut transposed = vec![0; outer_len * inner_len];
        for inner in 0..inner_len {
            for outer in 0..outer_len {
                transposed[outer * inner_len + inner] = values[inner * outer_len + outer];
            }
        }
        return Ok(transposed);
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "{name} dimensions {:?} with lengths {:?} do not match expected ({outer_dim}, {inner_dim})",
            dimension_names, dimension_lengths
        ),
    ))
}

pub(crate) fn required_values_i32_matrix(
    file: &netcdf::File,
    name: &str,
    outer_dim: &str,
    inner_dim: &str,
    outer_len: usize,
    inner_len: usize,
) -> io::Result<Vec<i32>> {
    let variable = file.variable(name).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("missing {name} variable"),
        )
    })?;
    let dimensions = variable.dimensions();
    let dimension_names = dimensions
        .iter()
        .map(|dimension| dimension.name())
        .collect::<Vec<_>>();
    let dimension_lengths = dimensions
        .iter()
        .map(|dimension| dimension.len())
        .collect::<Vec<_>>();
    let values = variable
        .get_values::<i32, _>((.., ..))
        .map_err(netcdf_to_io_error)?;
    require_len(name, values.len(), outer_len * inner_len)?;

    if dimension_names == [outer_dim, inner_dim] || dimension_lengths == [outer_len, inner_len] {
        return Ok(values);
    }
    if dimension_names == [inner_dim, outer_dim] || dimension_lengths == [inner_len, outer_len] {
        let mut transposed = vec![0; outer_len * inner_len];
        for inner in 0..inner_len {
            for outer in 0..outer_len {
                transposed[outer * inner_len + inner] = values[inner * outer_len + outer];
            }
        }
        return Ok(transposed);
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "{name} dimensions {:?} with lengths {:?} do not match expected ({outer_dim}, {inner_dim})",
            dimension_names, dimension_lengths
        ),
    ))
}

pub(crate) fn optional_values_i32_2d(
    file: &netcdf::File,
    name: &str,
) -> io::Result<Option<Vec<i32>>> {
    let Some(variable) = file.variable(name) else {
        return Ok(None);
    };
    variable
        .get_values::<i32, _>((.., ..))
        .map(Some)
        .map_err(netcdf_to_io_error)
}
