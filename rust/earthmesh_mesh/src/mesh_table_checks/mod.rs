use std::io;

pub fn require_method_c_len(name: &str, actual: usize, required: usize) -> io::Result<()> {
    if actual < required {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} length {actual} is shorter than required {required}"),
        ));
    }
    Ok(())
}

pub fn require_method_c_id(label: &str, id: usize, max: usize) -> io::Result<()> {
    if id <= 1 || id > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} id {id} is outside active Method-C range 2..={max}"),
        ));
    }
    Ok(())
}

pub fn require_unique_active_triplet(
    label: &str,
    owner: usize,
    values: [usize; 3],
    max: usize,
) -> io::Result<()> {
    for &value in &values {
        require_method_c_id(label, value, max)?;
    }
    if values[0] == values[1] || values[0] == values[2] || values[1] == values[2] {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} for owner {owner} contains duplicates: {values:?}"),
        ));
    }
    Ok(())
}
