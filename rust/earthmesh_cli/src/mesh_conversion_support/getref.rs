use std::io;

use super::netcdf_rows::f64_matrix_width;

pub(crate) fn require_getref_two_layer_values(
    name: &str,
    layers: &[Vec<Vec<f64>>],
) -> io::Result<()> {
    if layers.len() != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must contain exactly two layers"),
        ));
    }
    let layer0_rows = layers[0].len();
    let layer0_width = f64_matrix_width(&format!("{name}[0]"), &layers[0])?;
    for (index, layer) in layers.iter().enumerate().skip(1) {
        let width = f64_matrix_width(&format!("{name}[{index}]"), layer)?;
        if layer.len() != layer0_rows || width != layer0_width {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{name} layers must have identical row and column counts"),
            ));
        }
    }
    Ok(())
}

pub(crate) fn copy_getref_threshold_column(
    source: &[Vec<i32>],
    source_col: usize,
    target: &mut [Vec<i32>],
    target_col: usize,
) -> io::Result<()> {
    if source.len() != target.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "source threshold rows {} must match target rows {}",
                source.len(),
                target.len()
            ),
        ));
    }
    for (row_index, (source_row, target_row)) in source.iter().zip(target.iter_mut()).enumerate() {
        let value = *source_row.get(source_col).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("source threshold missing column {source_col} at row {row_index}"),
            )
        })?;
        let slot = target_row.get_mut(target_col).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("target threshold missing column {target_col} at row {row_index}"),
            )
        })?;
        *slot = value;
    }
    Ok(())
}

pub(crate) fn aggregate_getref_ref_sjx(
    ref_th: &[Vec<i32>],
    num_vertex: usize,
    ref_colnum: usize,
) -> io::Result<Vec<i32>> {
    let sjx_points = ref_th.len().checked_sub(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "threshold matrix must include Fortran placeholder row",
        )
    })?;
    if num_vertex > sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_vertex {num_vertex} exceeds sjx_points {sjx_points}"),
        ));
    }
    let mut ref_sjx = vec![0; sjx_points + 1];
    if ref_colnum == 0 {
        return Ok(ref_sjx);
    }
    for sjx_index in num_vertex + 1..=sjx_points {
        let row = ref_th.get(sjx_index).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("threshold matrix missing row {sjx_index}"),
            )
        })?;
        if row.len() <= ref_colnum {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "threshold row {sjx_index} width {} must cover ref_colnum {ref_colnum}",
                    row.len()
                ),
            ));
        }
        if row[1..=ref_colnum].iter().any(|flag| *flag != 0) {
            ref_sjx[sjx_index] = 1;
        }
    }
    Ok(ref_sjx)
}

pub(crate) fn get_getref_layer_value(
    layers: &[Vec<Vec<f64>>],
    layer: usize,
    row: usize,
    col: usize,
) -> io::Result<f64> {
    layers
        .get(layer)
        .and_then(|layer_values| layer_values.get(row))
        .and_then(|row_values| row_values.get(col))
        .copied()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("var3d missing layer {layer} value ({row},{col})"),
            )
        })
}
