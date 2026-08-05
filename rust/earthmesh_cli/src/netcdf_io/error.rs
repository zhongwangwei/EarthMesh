use std::io;
use std::path::Path;

pub(crate) fn netcdf_to_io_error(err: netcdf::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, err.to_string())
}

/// Open a NetCDF file, naming it if that fails.
///
/// The library's own error is the C status and nothing else, so a missing input
/// surfaces as `netcdf error(2): No such file or directory` with no path in it.
/// A run reads many rasters; that message says a file is missing without saying
/// which, and the one the user has to fix is the only thing they need to know.
/// Reported through `Error::Str` so the signature is unchanged and every caller
/// keeps its own context on top.
pub(crate) fn open_netcdf(path: impl AsRef<Path>) -> Result<netcdf::File, netcdf::Error> {
    let path = path.as_ref();
    let _guard = hdf5_sys::LOCK.lock();
    suppress_hdf5_error_stack();
    netcdf::open(path).map_err(|err| {
        // "does not exist" is worth separating from "exists but would not
        // open": the first is a path to correct, the second a file to inspect.
        let detail = if path.exists() {
            "could not be opened as NetCDF"
        } else {
            "does not exist"
        };
        netcdf::Error::Str(format!("{} {detail}: {err}", path.display()))
    })
}

pub(crate) fn create_netcdf(path: impl AsRef<Path>) -> Result<netcdf::FileMut, netcdf::Error> {
    let _guard = hdf5_sys::LOCK.lock();
    suppress_hdf5_error_stack();
    if !path.as_ref().exists() {
        let _ = std::fs::File::create(path.as_ref());
    }
    netcdf::create(path)
}

fn suppress_hdf5_error_stack() {
    unsafe {
        hdf5_sys::h5e::H5Eset_auto2(hdf5_sys::h5e::H5E_DEFAULT, None, std::ptr::null_mut());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_input_is_named_in_the_error() {
        // The library reports only the C status, so a run reading several
        // rasters said "No such file or directory" without saying which one --
        // and the file to fix is the only thing the message needed to carry.
        let missing = std::env::temp_dir().join("earthmesh_netcdf_absent_probe.nc");
        let _ = std::fs::remove_file(&missing);

        let error = open_netcdf(&missing).expect_err("a missing file must not open");
        let message = error.to_string();
        assert!(
            message.contains(&missing.display().to_string()),
            "the path must be in the message: {message}"
        );
        assert!(message.contains("does not exist"), "{message}");
    }

    #[test]
    fn a_file_that_is_not_netcdf_is_reported_as_unopenable_not_absent() {
        // Two different problems: a path to correct, and a file to inspect.
        // Collapsing them sends the reader looking in the wrong place.
        let path = std::env::temp_dir().join("earthmesh_netcdf_not_netcdf_probe.nc");
        std::fs::write(&path, b"this is not a NetCDF file").expect("write probe");

        let error = open_netcdf(&path).expect_err("plain text must not open as NetCDF");
        let message = error.to_string();
        assert!(message.contains(&path.display().to_string()), "{message}");
        assert!(
            message.contains("could not be opened as NetCDF"),
            "{message}"
        );
        let _ = std::fs::remove_file(path);
    }
}
