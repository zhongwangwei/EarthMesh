use std::io;

use crate::{netcdf_to_io_error, require_len};

fn transpose<T: Copy + Default>(values: &[T], outer_len: usize, inner_len: usize) -> Vec<T> {
    let mut transposed = vec![T::default(); outer_len * inner_len];
    for inner in 0..inner_len {
        for outer in 0..outer_len {
            transposed[outer * inner_len + inner] = values[inner * outer_len + outer];
        }
    }
    transposed
}

#[cfg(test)]
fn is_lon_dim(name: &str) -> bool {
    is_axis_dim(name, &["lon", "longitude"], "x")
}

#[cfg(test)]
fn is_lat_dim(name: &str) -> bool {
    is_axis_dim(name, &["lat", "latitude"], "y")
}

#[cfg(test)]
fn is_axis_dim(name: &str, aliases: &[&str], short_axis: &str) -> bool {
    let normalized = name.to_ascii_lowercase();
    normalized == short_axis
        || aliases.contains(&normalized.as_str())
        || normalized
            .split(|ch: char| !ch.is_ascii_alphanumeric())
            .any(|token| aliases.contains(&token))
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

    if dimension_names == [outer_dim, inner_dim] {
        if dimension_lengths != [outer_len, inner_len] {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{name} dimensions {:?} have wrong lengths {:?}",
                    dimension_names, dimension_lengths
                ),
            ));
        }
        return Ok(values);
    }
    if dimension_names == [inner_dim, outer_dim] {
        if dimension_lengths != [inner_len, outer_len] {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{name} dimensions {:?} have wrong lengths {:?}",
                    dimension_names, dimension_lengths
                ),
            ));
        }
        return Ok(transpose(&values, outer_len, inner_len));
    }
    if dimension_lengths == [outer_len, inner_len] {
        return Ok(values);
    }
    if dimension_lengths == [inner_len, outer_len] {
        return Ok(transpose(&values, outer_len, inner_len));
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

    if dimension_names == [outer_dim, inner_dim] {
        if dimension_lengths != [outer_len, inner_len] {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{name} dimensions {:?} have wrong lengths {:?}",
                    dimension_names, dimension_lengths
                ),
            ));
        }
        return Ok(values);
    }
    if dimension_names == [inner_dim, outer_dim] {
        if dimension_lengths != [inner_len, outer_len] {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{name} dimensions {:?} have wrong lengths {:?}",
                    dimension_names, dimension_lengths
                ),
            ));
        }
        return Ok(transpose(&values, outer_len, inner_len));
    }
    if dimension_lengths == [outer_len, inner_len] {
        return Ok(values);
    }
    if dimension_lengths == [inner_len, outer_len] {
        return Ok(transpose(&values, outer_len, inner_len));
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

#[cfg(test)]
mod tests {
    use super::{is_lat_dim, is_lon_dim};

    #[test]
    fn axis_dimension_names_are_exact_or_tokenized() {
        assert!(is_lon_dim("lon"));
        assert!(is_lon_dim("longitude"));
        assert!(is_lon_dim("nav_lon"));
        assert!(is_lon_dim("x"));
        assert!(is_lat_dim("lat"));
        assert!(is_lat_dim("latitude"));
        assert!(is_lat_dim("nav_lat"));
        assert!(is_lat_dim("y"));

        assert!(!is_lon_dim("pixel"));
        assert!(!is_lon_dim("x_index"));
        assert!(!is_lat_dim("quality"));
        assert!(!is_lat_dim("y_index"));
    }
}
