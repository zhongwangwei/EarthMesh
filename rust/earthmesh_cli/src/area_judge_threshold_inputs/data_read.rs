use std::io;
use std::path::Path;

use earthmesh_mesh::AreaJudgeSourceBounds;

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
    let start_lat = bounds.maxlat_source.checked_sub(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "maxlat_source must be one-based",
        )
    })?;
    let dims: Vec<String> = variable
        .dimensions()
        .iter()
        .map(|d| d.name().to_ascii_lowercase())
        .collect();
    let lat_lon = matches!(
        dims.as_slice(),
        [lat, lon, ..] if is_lat_dim(lat) && is_lon_dim(lon)
    );
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
    require_len(var_name, values.len(), nlons_select * nlats_select)?;

    let mut selected = vec![vec![0.0; nlats_select + 1]; nlons_select + 1];
    for lon_offset in 0..nlons_select {
        for lat_offset in 0..nlats_select {
            let index = if lat_lon {
                lat_offset * nlons_select + lon_offset
            } else {
                lon_offset * nlats_select + lat_offset
            };
            selected[lon_offset + 1][lat_offset + 1] = values[index];
        }
    }
    Ok(selected)
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
