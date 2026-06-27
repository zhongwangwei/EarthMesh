use std::fs;
use std::io;
use std::path::Path;

use crate::parse_value_after_equals;

use super::types::{BBoxMask, BBoxPoint};

/// Parse the text `.nml` branch of `mkgrd.F90:bbox_mask_make`.
///
/// Returns `Ok(None)` when `refine_degree > max_iter_spc`, matching the Fortran
/// early return before any output/count update.
pub fn parse_bbox_mask_nml(
    inputfile: impl AsRef<Path>,
    max_iter_spc: usize,
) -> io::Result<Option<BBoxMask>> {
    let content = fs::read_to_string(inputfile)?;
    let mut lines = content.lines();
    let bbox_num_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing bbox_num line"))?;
    let refine_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing bbox_refine line"))?;
    let bbox_num = parse_value_after_equals::<usize>(bbox_num_line, "bbox_num")?;
    let refine_degree = parse_value_after_equals::<usize>(refine_line, "bbox_refine")?;
    if refine_degree > max_iter_spc {
        return Ok(None);
    }

    let mut points = Vec::with_capacity(bbox_num);
    for index in 0..bbox_num {
        let line = lines.next().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing bbox point row {}", index + 1),
            )
        })?;
        let values = line
            .split_whitespace()
            .map(|value| {
                value.parse::<f64>().map_err(|err| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("invalid bbox coordinate {value}: {err}"),
                    )
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        if values.len() != 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("bbox point row {} must contain 4 values", index + 1),
            ));
        }
        let point = BBoxPoint {
            west: values[0],
            east: values[1],
            north: values[2],
            south: values[3],
        };
        if point.west > point.east {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bbox west must be <= east like bbox_mask_make",
            ));
        }
        if point.north < point.south {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bbox north must be >= south like bbox_mask_make",
            ));
        }
        points.push(point);
    }

    Ok(Some(BBoxMask {
        refine_degree,
        points,
    }))
}
