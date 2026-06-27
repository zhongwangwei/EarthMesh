use std::io;

pub fn compact_source_state_selected_matrix_fortran_order(
    matrix: &[Vec<i32>],
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<Vec<Vec<i32>>> {
    if matrix.len() < nlons_source + 1 || matrix.iter().any(|row| row.len() < nlats_source + 1) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "source-state selected matrix is smaller than source dimensions",
        ));
    }
    Ok((1..=nlons_source)
        .map(|lon| (1..=nlats_source).map(|lat| matrix[lon][lat]).collect())
        .collect())
}
