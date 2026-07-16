use std::fs;
use std::io;
use std::path::Path;

use crate::parse_value_after_equals;

use super::types::{BBoxMask, BBoxPoint};
use super::validate_bbox_mask;

/// Parse the text `.nml` branch of `mkgrd.F90:bbox_mask_make`.
///
/// Returns `Ok(None)` when `refine_degree > max_iter_spc`, matching the Canonical
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
        points.push(point);
    }

    let mask = BBoxMask {
        refine_degree,
        points,
    };
    validate_bbox_mask(&mask).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid bbox mask namelist: {err}"),
        )
    })?;
    Ok(Some(mask))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bbox_namelist_preserves_cartesian_y_for_contextual_validation() {
        let path = std::env::temp_dir().join(format!(
            "earthmesh_bbox_invalid_lat_{}.nml",
            std::process::id()
        ));
        fs::write(&path, "bbox_num = 1\nbbox_refine = 1\n170 -170 91 -10\n").unwrap();
        let mask = parse_bbox_mask_nml(&path, 5).unwrap().unwrap();
        assert_eq!(mask.points[0].north, 91.0);
        let _ = fs::remove_file(path);
    }
}
