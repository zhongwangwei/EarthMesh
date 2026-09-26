use std::io;

pub fn required_dimension_len(file: &netcdf::File, name: &str) -> io::Result<usize> {
    file.dimension(name)
        .map(|dimension| dimension.len())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing {name} dimension"),
            )
        })
}

pub fn first_existing_dimension_len(file: &netcdf::File, names: &[&str]) -> io::Result<usize> {
    for name in names {
        if let Some(dimension) = file.dimension(name) {
            return Ok(dimension.len());
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        format!("missing dimension; expected one of {}", names.join(", ")),
    ))
}

pub fn require_len(name: &str, actual: usize, required: usize) -> io::Result<()> {
    if actual < required {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} length {actual} is shorter than required {required}"),
        ));
    }
    Ok(())
}
