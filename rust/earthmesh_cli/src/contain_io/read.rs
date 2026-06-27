use std::io;
use std::path::Path;

use crate::{
    require_len, required_dimension_len, required_values_i32, required_values_i32_matrix,
    rows_from_flat_i32,
};

use super::types::ContainMesh;
use super::validation::validate_contain_mesh;

/// Read the `contain_*.nc4` schema produced by
/// `MOD_file_preprocess.F90:Contain_Save`.
pub fn read_contain_netcdf(input: impl AsRef<Path>) -> io::Result<ContainMesh> {
    let input = input.as_ref();
    let file = netcdf::open(input).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to open contain mesh {}: {err}", input.display()),
        )
    })?;
    let num_ustr = required_dimension_len(&file, "num_ustr")?;
    let num_ii = required_dimension_len(&file, "num_ii")?;
    let dim_a = required_dimension_len(&file, "dim_a")?;
    let dim_b = required_dimension_len(&file, "dim_b")?;
    let ustr_id_values =
        required_values_i32_matrix(&file, "ustr_id", "num_ustr", "dim_a", num_ustr, dim_a)?;
    let ustr_ii_values =
        required_values_i32_matrix(&file, "ustr_ii", "num_ii", "dim_b", num_ii, dim_b)?;
    let is_in_area_ustr = required_values_i32(&file, "IsInArea_ustr")?;
    require_len("IsInArea_ustr", is_in_area_ustr.len(), num_ustr)?;

    let contain = ContainMesh {
        ustr_id: rows_from_flat_i32(&ustr_id_values, dim_a),
        ustr_ii: rows_from_flat_i32(&ustr_ii_values, dim_b),
        is_in_area_ustr,
    };
    validate_contain_mesh(&contain)?;
    Ok(contain)
}
