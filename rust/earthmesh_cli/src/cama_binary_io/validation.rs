use std::io;

use super::geometry::{CamaBinaryGridSpec, CamaBinaryWindow};

pub(crate) fn validate_cama_binary_window(
    grid: CamaBinaryGridSpec,
    window: CamaBinaryWindow,
) -> io::Result<()> {
    grid.validate()?;
    if window.width == 0 || window.height == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "CaMa window dimensions must be positive",
        ));
    }
    let x_end = window
        .x_start
        .checked_add(window.width)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "CaMa window x overflow"))?;
    let y_end = window
        .y_start
        .checked_add(window.height)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "CaMa window y overflow"))?;
    if x_end > grid.nx || y_end > grid.ny {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "requested window is outside grid",
        ));
    }
    Ok(())
}
