mod dimensions;
mod error;
mod matrices;
mod scalars;
mod values;

pub(crate) use dimensions::{first_existing_dimension_len, required_dimension_len};
pub(crate) use error::netcdf_to_io_error;
pub(crate) use matrices::{
    optional_values_i32_2d, required_values_f64_any_matrix, required_values_i32_2d,
    required_values_i32_any_matrix, required_values_i32_matrix, required_values_i8_matrix,
};
pub(crate) use scalars::{required_scalar_usize_i32, write_f64_scalar, write_i32_scalar};
pub(crate) use values::{
    required_values_f64, required_values_f64_any, required_values_i32, required_values_i8,
};
