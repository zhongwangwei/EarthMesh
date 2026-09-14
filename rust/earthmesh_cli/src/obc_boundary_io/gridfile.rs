use std::{io, path::Path};

use crate::{netcdf_to_io_error, open_netcdf};

const OBC_ATTRIBUTE: &str = "earthmesh_fvcom_obc_order";

/// Persist the same-run canonical OBC sequence in a newly produced gridfile.
/// An existing zero-length integer attribute is distinct from absent metadata.
pub(crate) fn write_gridfile_obc_order(gridfile: &Path, order: &[usize]) -> io::Result<()> {
    let values = order
        .iter()
        .map(|&id| {
            i32::try_from(id)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "FVCOM OBC id exceeds i32"))
        })
        .collect::<io::Result<Vec<_>>>()?;
    let mut file = netcdf::append(gridfile).map_err(netcdf_to_io_error)?;
    file.add_attribute(OBC_ATTRIBUTE, values)
        .map_err(netcdf_to_io_error)?;
    file.close().map_err(netcdf_to_io_error)
}

/// Read boundary context owned by this gridfile, never a neighboring run's sidecar.
pub fn read_gridfile_obc_order(gridfile: &Path) -> io::Result<Option<Vec<usize>>> {
    let file = open_netcdf(gridfile).map_err(netcdf_to_io_error)?;
    let Some(attribute) = file.attribute(OBC_ATTRIBUTE) else {
        return Ok(None);
    };
    let values = match attribute.value().map_err(netcdf_to_io_error)? {
        netcdf::AttributeValue::Ints(values) => values,
        netcdf::AttributeValue::Int(value) => vec![value],
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "FVCOM OBC context must be an integer attribute",
            ))
        }
    };
    values
        .into_iter()
        .map(|id| {
            usize::try_from(id).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "FVCOM OBC context contains a negative id",
                )
            })
        })
        .collect::<io::Result<Vec<_>>>()
        .map(Some)
}
