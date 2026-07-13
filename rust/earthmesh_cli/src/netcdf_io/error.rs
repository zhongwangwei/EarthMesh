use std::io;
use std::path::Path;

pub(crate) fn netcdf_to_io_error(err: netcdf::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, err.to_string())
}

pub(crate) fn open_netcdf(path: impl AsRef<Path>) -> Result<netcdf::File, netcdf::Error> {
    let _guard = hdf5_sys::LOCK.lock();
    suppress_hdf5_error_stack();
    netcdf::open(path)
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
