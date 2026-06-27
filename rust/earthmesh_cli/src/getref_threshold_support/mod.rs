use std::io;

pub(crate) fn require_getref_lookup_width(
    name: &str,
    rows: &[Vec<i32>],
    min_width: usize,
) -> io::Result<()> {
    if rows.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must include a Fortran placeholder row"),
        ));
    }
    if rows.iter().any(|row| row.len() < min_width) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} rows must have at least {min_width} columns"),
        ));
    }
    Ok(())
}
