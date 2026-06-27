use std::fs;
use std::io;
use std::path::Path;

use super::shared::parse_float_row;
use super::types::{CircleMask, CloseMask};
use crate::{parse_value_after_equals, LonLatPoint};

/// Parse the text `.nml` branch of `mkgrd.F90:circle_mask_make`.
pub fn parse_circle_mask_nml(
    inputfile: impl AsRef<Path>,
    max_iter_spc: usize,
) -> io::Result<Option<CircleMask>> {
    let content = fs::read_to_string(inputfile)?;
    let mut lines = content.lines();
    let count_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing circle_num line"))?;
    let refine_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing circle_refine line"))?;
    let circle_num = parse_value_after_equals::<usize>(count_line, "circle_num")?;
    let refine_degree = parse_value_after_equals::<usize>(refine_line, "circle_refine")?;
    if refine_degree > max_iter_spc {
        return Ok(None);
    }

    let mut points = Vec::with_capacity(circle_num);
    let mut radius_km = Vec::with_capacity(circle_num);
    for index in 0..circle_num {
        let values = parse_float_row(
            lines.next().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("missing circle point row {}", index + 1),
                )
            })?,
            3,
            "circle point",
            index + 1,
        )?;
        points.push(LonLatPoint {
            lon: values[0],
            lat: values[1],
        });
        radius_km.push(values[2]);
    }
    Ok(Some(CircleMask {
        refine_degree,
        points,
        radius_km,
    }))
}

/// Parse the text `.nml` branch of `mkgrd.F90:close_mask_make`.
pub fn parse_close_mask_nml(
    inputfile: impl AsRef<Path>,
    max_iter_spc: usize,
) -> io::Result<Option<CloseMask>> {
    let content = fs::read_to_string(inputfile)?;
    let mut lines = content.lines();
    let count_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing close_num line"))?;
    let refine_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing close_refine line"))?;
    let close_num = parse_value_after_equals::<usize>(count_line, "close_num")?;
    let refine_degree = parse_value_after_equals::<usize>(refine_line, "close_refine")?;
    if refine_degree > max_iter_spc {
        return Ok(None);
    }

    let mut points = Vec::with_capacity(close_num);
    for index in 0..close_num {
        let values = parse_float_row(
            lines.next().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("missing close point row {}", index + 1),
                )
            })?,
            2,
            "close point",
            index + 1,
        )?;
        points.push(LonLatPoint {
            lon: values[0],
            lat: values[1],
        });
    }
    Ok(Some(CloseMask {
        refine_degree,
        points,
    }))
}
