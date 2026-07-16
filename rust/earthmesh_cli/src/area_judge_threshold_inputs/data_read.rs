use std::io;
use std::path::Path;

use earthmesh_mesh::AreaJudgeSourceBounds;
use netcdf::AttributeValue;

use crate::{netcdf_to_io_error, require_len, AreaJudgeThreshold2D, AreaJudgeThreshold2Layer};

/// Read one 2-D threshold window like `MOD_data_preprocess.F90:data_read_onelayer`.
pub fn data_read_onelayer_one_based(
    inputfile: impl AsRef<Path>,
    var_name: &str,
    bounds: AreaJudgeSourceBounds,
) -> io::Result<AreaJudgeThreshold2D> {
    let values = data_read_onelayer_values_one_based(inputfile, var_name, bounds)?;
    Ok(AreaJudgeThreshold2D {
        name: var_name.to_string(),
        values,
    })
}

/// Read two layer-specific threshold variables like `MOD_data_preprocess.F90:data_read_twolayer`.
pub fn data_read_twolayer_one_based(
    inputfile: impl AsRef<Path>,
    var_name: &str,
    bounds: AreaJudgeSourceBounds,
) -> io::Result<AreaJudgeThreshold2Layer> {
    let inputfile = inputfile.as_ref();
    let first = data_read_onelayer_values_one_based(inputfile, &format!("{var_name}_l1"), bounds)?;
    let second = data_read_onelayer_values_one_based(inputfile, &format!("{var_name}_l2"), bounds)?;
    Ok(AreaJudgeThreshold2Layer {
        name: var_name.to_string(),
        layers: vec![first, second],
    })
}

pub(super) fn data_read_onelayer_values_one_based(
    inputfile: impl AsRef<Path>,
    var_name: &str,
    bounds: AreaJudgeSourceBounds,
) -> io::Result<Vec<Vec<f64>>> {
    if bounds.minlon_source == 0
        || bounds.maxlat_source == 0
        || bounds.maxlon_source < bounds.minlon_source
        || bounds.minlat_source < bounds.maxlat_source
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "invalid one-based threshold bounds lon {}..{} lat {}..{}",
                bounds.minlon_source,
                bounds.maxlon_source,
                bounds.maxlat_source,
                bounds.minlat_source
            ),
        ));
    }
    let nlons_select = bounds.maxlon_source - bounds.minlon_source + 1;
    let nlats_select = bounds.minlat_source - bounds.maxlat_source + 1;
    let file = crate::open_netcdf(inputfile.as_ref()).map_err(netcdf_to_io_error)?;
    let variable = file.variable(var_name).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("missing {var_name} variable"),
        )
    })?;
    let start_lon = bounds.minlon_source.checked_sub(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "minlon_source must be one-based",
        )
    })?;
    let dimensions = variable.dimensions();
    if dimensions.len() != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{var_name} threshold variable must be two-dimensional"),
        ));
    }
    let dims = dimensions
        .iter()
        .map(|dimension| dimension.name().to_ascii_lowercase())
        .collect::<Vec<_>>();
    let lat_lon = is_lat_dim(&dims[0]) && is_lon_dim(&dims[1]);
    let lon_lat = is_lon_dim(&dims[0]) && is_lat_dim(&dims[1]);
    if !lat_lon && !lon_lat {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{var_name} threshold dimensions {:?} must identify longitude and latitude axes",
                dims
            ),
        ));
    }
    let lon_len = dimensions[usize::from(lat_lon)].len();
    let lat_position = usize::from(!lat_lon);
    let lat_len = dimensions[lat_position].len();
    if bounds.maxlon_source > lon_len || bounds.minlat_source > lat_len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "threshold bounds lon {}..{} lat {}..{} exceed {var_name} dimensions {lon_len}x{lat_len}",
                bounds.minlon_source,
                bounds.maxlon_source,
                bounds.maxlat_source,
                bounds.minlat_source
            ),
        ));
    }
    let latitude_order =
        threshold_latitude_order(&file, &dimensions[lat_position].name(), lat_len)?;
    let start_lat = match latitude_order {
        LatitudeOrder::NorthToSouth => bounds.maxlat_source.checked_sub(1).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "maxlat_source must be one-based",
            )
        })?,
        LatitudeOrder::SouthToNorth => lat_len - bounds.minlat_source,
    };
    let values = if lat_lon {
        variable
            .get_values::<f64, _>((
                start_lat..start_lat + nlats_select,
                start_lon..start_lon + nlons_select,
            ))
            .map_err(netcdf_to_io_error)?
    } else {
        variable
            .get_values::<f64, _>((
                start_lon..start_lon + nlons_select,
                start_lat..start_lat + nlats_select,
            ))
            .map_err(netcdf_to_io_error)?
    };
    let expected = nlons_select
        .checked_mul(nlats_select)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "threshold window overflows"))?;
    require_len(var_name, values.len(), expected)?;
    reject_invalid_threshold_values(&variable, var_name, &values)?;

    let mut selected = vec![vec![0.0; nlats_select + 1]; nlons_select + 1];
    for lon_offset in 0..nlons_select {
        for lat_offset in 0..nlats_select {
            let file_lat_offset = match latitude_order {
                LatitudeOrder::NorthToSouth => lat_offset,
                LatitudeOrder::SouthToNorth => nlats_select - 1 - lat_offset,
            };
            let index = if lat_lon {
                file_lat_offset * nlons_select + lon_offset
            } else {
                lon_offset * nlats_select + file_lat_offset
            };
            selected[lon_offset + 1][lat_offset + 1] = values[index];
        }
    }
    Ok(selected)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LatitudeOrder {
    NorthToSouth,
    SouthToNorth,
}

pub(crate) fn threshold_latitude_order(
    file: &netcdf::File,
    dimension_name: &str,
    expected_len: usize,
) -> io::Result<LatitudeOrder> {
    let coordinate = [dimension_name, "lat", "latitude", "nav_lat"]
        .into_iter()
        .filter_map(|name| file.variable(name))
        .find(|variable| {
            let dims = variable.dimensions();
            dims.len() == 1
                && dims[0].name().eq_ignore_ascii_case(dimension_name)
                && dims[0].len() == expected_len
        });
    let Some(coordinate) = coordinate else {
        // Legacy Canonical threshold files often expose dimensions only. Their
        // documented storage order is north-to-south.
        return Ok(LatitudeOrder::NorthToSouth);
    };
    let values = if let Ok(values) = coordinate.get_values::<f64, _>(..) {
        values
    } else {
        coordinate
            .get_values::<f32, _>(..)
            .map_err(netcdf_to_io_error)?
            .into_iter()
            .map(f64::from)
            .collect()
    };
    require_len("latitude coordinate", values.len(), expected_len)?;
    if values.iter().any(|value| !value.is_finite()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "threshold latitude coordinate must contain finite values",
        ));
    }
    let ascending = values.windows(2).all(|pair| pair[0] < pair[1]);
    let descending = values.windows(2).all(|pair| pair[0] > pair[1]);
    match (ascending, descending) {
        (true, false) => Ok(LatitudeOrder::SouthToNorth),
        (false, true) => Ok(LatitudeOrder::NorthToSouth),
        _ if values.len() <= 1 => Ok(LatitudeOrder::NorthToSouth),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "threshold latitude coordinate must be strictly monotonic",
        )),
    }
}

pub(crate) fn threshold_longitude_coordinates(
    file: &netcdf::File,
    dimension_name: &str,
    expected_len: usize,
) -> io::Result<Option<Vec<f64>>> {
    let coordinate = [dimension_name, "lon", "longitude", "nav_lon"]
        .into_iter()
        .filter_map(|name| file.variable(name))
        .find(|variable| {
            let dims = variable.dimensions();
            dims.len() == 1
                && dims[0].name().eq_ignore_ascii_case(dimension_name)
                && dims[0].len() == expected_len
        });
    let Some(coordinate) = coordinate else {
        return Ok(None);
    };
    let values = if let Ok(values) = coordinate.get_values::<f64, _>(..) {
        values
    } else {
        coordinate
            .get_values::<f32, _>(..)
            .map_err(netcdf_to_io_error)?
            .into_iter()
            .map(f64::from)
            .collect()
    };
    require_len("longitude coordinate", values.len(), expected_len)?;
    if values
        .iter()
        .any(|value| !value.is_finite() || !(-360.0..=360.0).contains(value))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "threshold longitude coordinate must contain finite degree values in -360..=360",
        ));
    }
    let ascending = values.windows(2).all(|pair| pair[0] < pair[1]);
    let descending = values.windows(2).all(|pair| pair[0] > pair[1]);
    if values.len() > 1 && !ascending && !descending {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "threshold longitude coordinate must be strictly monotonic",
        ));
    }
    if values.len() > 1 && (values[values.len() - 1] - values[0]).abs() >= 360.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "threshold longitude coordinate must not duplicate the periodic seam",
        ));
    }
    Ok(Some(values))
}

pub(crate) fn numeric_missing_values(variable: &netcdf::Variable<'_>) -> io::Result<Vec<f64>> {
    use netcdf::types::{FloatType, IntType, NcVariableType};

    let mut missing = numeric_attribute_values(variable, "_FillValue")?;
    missing.extend(numeric_attribute_values(variable, "missing_value")?);
    let default_fill = match variable.vartype() {
        NcVariableType::Float(FloatType::F32) => variable
            .fill_value::<f32>()
            .map_err(netcdf_to_io_error)?
            .map(f64::from),
        NcVariableType::Float(FloatType::F64) => {
            variable.fill_value::<f64>().map_err(netcdf_to_io_error)?
        }
        NcVariableType::Int(IntType::U8) => variable
            .fill_value::<u8>()
            .map_err(netcdf_to_io_error)?
            .map(f64::from),
        NcVariableType::Int(IntType::U16) => variable
            .fill_value::<u16>()
            .map_err(netcdf_to_io_error)?
            .map(f64::from),
        NcVariableType::Int(IntType::U32) => variable
            .fill_value::<u32>()
            .map_err(netcdf_to_io_error)?
            .map(f64::from),
        NcVariableType::Int(IntType::U64) => variable
            .fill_value::<u64>()
            .map_err(netcdf_to_io_error)?
            .map(|value| value as f64),
        NcVariableType::Int(IntType::I8) => variable
            .fill_value::<i8>()
            .map_err(netcdf_to_io_error)?
            .map(f64::from),
        NcVariableType::Int(IntType::I16) => variable
            .fill_value::<i16>()
            .map_err(netcdf_to_io_error)?
            .map(f64::from),
        NcVariableType::Int(IntType::I32) => variable
            .fill_value::<i32>()
            .map_err(netcdf_to_io_error)?
            .map(f64::from),
        NcVariableType::Int(IntType::I64) => variable
            .fill_value::<i64>()
            .map_err(netcdf_to_io_error)?
            .map(|value| value as f64),
        _ => None,
    };
    missing.extend(default_fill);
    Ok(missing)
}

pub(crate) fn reject_invalid_threshold_values(
    variable: &netcdf::Variable<'_>,
    var_name: &str,
    values: &[f64],
) -> io::Result<()> {
    let missing = numeric_missing_values(variable)?;
    if let Some((index, value)) = values
        .iter()
        .copied()
        .enumerate()
        .find(|(_, value)| !value.is_finite() || missing.contains(value))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{var_name} threshold window contains missing/non-finite value {value} at flat index {index}"
            ),
        ));
    }
    Ok(())
}

fn numeric_attribute_values(variable: &netcdf::Variable<'_>, name: &str) -> io::Result<Vec<f64>> {
    let Some(value) = variable.attribute_value(name) else {
        return Ok(Vec::new());
    };
    let value = value.map_err(netcdf_to_io_error)?;
    let values = match value {
        AttributeValue::Double(value) => vec![value],
        AttributeValue::Doubles(values) => values,
        AttributeValue::Float(value) => vec![f64::from(value)],
        AttributeValue::Floats(values) => values.into_iter().map(f64::from).collect(),
        AttributeValue::Int(value) => vec![f64::from(value)],
        AttributeValue::Ints(values) => values.into_iter().map(f64::from).collect(),
        AttributeValue::Short(value) => vec![f64::from(value)],
        AttributeValue::Shorts(values) => values.into_iter().map(f64::from).collect(),
        AttributeValue::Schar(value) => vec![f64::from(value)],
        AttributeValue::Schars(values) => values.into_iter().map(f64::from).collect(),
        AttributeValue::Uchar(value) => vec![f64::from(value)],
        AttributeValue::Uchars(values) => values.into_iter().map(f64::from).collect(),
        AttributeValue::Ushort(value) => vec![f64::from(value)],
        AttributeValue::Ushorts(values) => values.into_iter().map(f64::from).collect(),
        AttributeValue::Uint(value) => vec![f64::from(value)],
        AttributeValue::Uints(values) => values.into_iter().map(f64::from).collect(),
        AttributeValue::Longlong(value) => vec![value as f64],
        AttributeValue::Longlongs(values) => values.into_iter().map(|value| value as f64).collect(),
        AttributeValue::Ulonglong(value) => vec![value as f64],
        AttributeValue::Ulonglongs(values) => {
            values.into_iter().map(|value| value as f64).collect()
        }
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{name} attribute on threshold variable must be numeric, got {other:?}"),
            ))
        }
    };
    Ok(values)
}

fn is_lon_dim(name: &str) -> bool {
    is_axis_dim(name, &["lon", "longitude"], "x")
}

fn is_lat_dim(name: &str) -> bool {
    is_axis_dim(name, &["lat", "latitude"], "y")
}

fn is_axis_dim(name: &str, aliases: &[&str], short_axis: &str) -> bool {
    let normalized = name.to_ascii_lowercase();
    if normalized == short_axis || aliases.contains(&normalized.as_str()) {
        return true;
    }
    normalized
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .any(|token| aliases.contains(&token))
}

#[cfg(test)]
mod tests {
    use super::{is_lat_dim, is_lon_dim};

    #[test]
    fn threshold_data_axis_names_are_exact_or_tokenized() {
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
