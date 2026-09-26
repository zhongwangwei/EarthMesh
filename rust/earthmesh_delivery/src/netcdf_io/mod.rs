mod dimensions;
mod error;
mod matrices;
mod scalars;
mod values;

pub use dimensions::{first_existing_dimension_len, require_len, required_dimension_len};
pub use error::{create_netcdf, netcdf_to_io_error, open_netcdf};
pub use matrices::{
    optional_values_i32_2d, required_values_i32_2d, required_values_i32_matrix,
    required_values_i32_matrix_named, required_values_i8_matrix,
};
pub use scalars::{required_scalar_usize_i32, write_f64_scalar, write_i32_scalar};
pub use values::{
    required_values_f64, required_values_f64_any, required_values_i32, required_values_i8,
};
