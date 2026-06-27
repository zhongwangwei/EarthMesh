use std::io;

pub(crate) fn fortran_rows_to_triangle_major(
    rows: &[Vec<usize>],
    max_triangle: usize,
) -> io::Result<Vec<[usize; 3]>> {
    if rows.len() <= 3 || rows[1..=3].iter().any(|row| row.len() <= max_triangle) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("triangle count {max_triangle} requires one-based rows 1..=3"),
        ));
    }
    let mut out = vec![[0, 0, 0]; max_triangle + 1];
    for triangle in 1..=max_triangle {
        out[triangle] = [rows[1][triangle], rows[2][triangle], rows[3][triangle]];
    }
    Ok(out)
}
