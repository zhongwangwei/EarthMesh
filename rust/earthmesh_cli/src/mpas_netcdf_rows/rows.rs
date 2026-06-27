use std::io;

fn zero_based_id(name: &str, value: usize) -> io::Result<i32> {
    if value == 0 {
        return Ok(0);
    }
    i32::try_from(value - 1).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} contains value {value} that does not fit NetCDF INT"),
        )
    })
}

pub(crate) fn zero_based_padded_rows(
    name: &str,
    rows: &[Vec<usize>],
    width: usize,
) -> io::Result<Vec<Vec<i32>>> {
    rows.iter()
        .enumerate()
        .map(|(row_idx, row)| {
            if row.len() > width {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{name} row {row_idx} width {} exceeds {width}", row.len()),
                ));
            }
            let mut output = row
                .iter()
                .copied()
                .map(|value| zero_based_id(name, value))
                .collect::<io::Result<Vec<_>>>()?;
            output.resize(width, 0);
            Ok(output)
        })
        .collect()
}

pub(crate) fn zero_based_triplet_rows(
    name: &str,
    rows: &[[usize; 3]],
) -> io::Result<Vec<Vec<i32>>> {
    rows.iter()
        .map(|row| {
            row.iter()
                .copied()
                .map(|value| zero_based_id(name, value))
                .collect()
        })
        .collect()
}

pub(crate) fn zero_based_pair_rows(name: &str, rows: &[[usize; 2]]) -> io::Result<Vec<[i32; 2]>> {
    rows.iter()
        .map(|row| Ok([zero_based_id(name, row[0])?, zero_based_id(name, row[1])?]))
        .collect()
}

pub(crate) fn pad_f64_rows(rows: &[Vec<f64>], width: usize) -> Vec<Vec<f64>> {
    rows.iter()
        .map(|row| {
            let mut output = row.clone();
            output.resize(width, 0.0);
            output
        })
        .collect()
}
