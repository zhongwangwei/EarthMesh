use std::fs;
use std::io;
use std::path::Path;

use super::shared::parse_float_row;
use super::types::{CircleMask, CloseMask};
use super::{validate_circle_mask, validate_close_mask};
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
    let mask = CircleMask {
        refine_degree,
        points,
        radius_km,
    };
    validate_circle_mask(&mask).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid circle mask namelist: {err}"),
        )
    })?;
    Ok(Some(mask))
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
    let mask = CloseMask {
        refine_degree,
        points,
    };
    validate_close_mask(&mask).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid close mask namelist: {err}"),
        )
    })?;
    Ok(Some(mask))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_case(name: &str, contents: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "earthmesh_{name}_{}_{}.nml",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn circle_namelist_preserves_cartesian_y_for_contextual_validation() {
        let path = write_case(
            "circle_invalid_lat",
            "circle_num = 1\ncircle_refine = 1\n0 91 10\n",
        );
        let mask = parse_circle_mask_nml(&path, 5).unwrap().unwrap();
        assert_eq!(mask.points[0].lat, 91.0);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn close_namelist_preserves_cartesian_y_for_contextual_validation() {
        let path = write_case(
            "close_invalid_lat",
            "close_num = 3\nclose_refine = 1\n0 0\n1 0\n0 -91\n",
        );
        let mask = parse_close_mask_nml(&path, 5).unwrap().unwrap();
        assert_eq!(mask.points[2].lat, -91.0);
        let _ = fs::remove_file(path);
    }
}
