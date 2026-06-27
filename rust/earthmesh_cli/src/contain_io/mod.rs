mod read;
mod types;
mod validation;
mod write;

pub use read::read_contain_netcdf;
pub use types::{ContainMesh, ContainWriteReport, FlatContainMesh};
pub(crate) use validation::validate_contain_mesh;
pub use write::{write_contain_netcdf, write_flat_contain_netcdf};
