use std::io;
use std::path::{Path, PathBuf};

use crate::MkgrdFinalQualityCheckIoPlan;

pub(super) fn required_final_quality_path<'a>(
    path: Option<&'a Path>,
    label: &str,
) -> io::Result<&'a Path> {
    path.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Final_Grid_Quality_Check requires {label} path"),
        )
    })
}

pub(super) fn final_quality_file_dir_and_nxp(
    plan: &MkgrdFinalQualityCheckIoPlan,
) -> io::Result<(PathBuf, usize)> {
    let gridfile_dir = plan.input_gridfile.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Final_Grid_Quality_Check input_gridfile must have a parent directory",
        )
    })?;
    let file_dir = gridfile_dir.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Final_Grid_Quality_Check input_gridfile must live under <file_dir>/gridfile",
        )
    })?;
    let filename = plan
        .input_gridfile
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Final_Grid_Quality_Check input_gridfile must have a UTF-8 file name",
            )
        })?;
    Ok((
        file_dir.to_path_buf(),
        parse_nxp_from_gridfile_name(filename)?,
    ))
}

fn parse_nxp_from_gridfile_name(filename: &str) -> io::Result<usize> {
    let start = filename.find("NXP").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("gridfile name {filename} does not contain NXP"),
        )
    })? + 3;
    let digits = filename[start..]
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("gridfile name {filename} does not contain NXP digits"),
        ));
    }
    digits.parse::<usize>().map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("failed to parse NXP from gridfile name {filename}: {err}"),
        )
    })
}
