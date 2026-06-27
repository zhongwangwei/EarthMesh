use std::io;
use std::path::{Path, PathBuf};

pub(super) fn getref_is_in_refine_from_contain(is_in_area_ustr: &[i32]) -> Vec<i32> {
    let mut values = Vec::with_capacity(is_in_area_ustr.len() + 1);
    values.push(0);
    values.extend_from_slice(is_in_area_ustr);
    values
}

pub(super) fn mkgrd_calculated_getref_output_refs<'a>(
    mesh_type: &str,
    outputs: &'a [PathBuf],
) -> io::Result<(Option<&'a Path>, Option<&'a Path>, Option<&'a Path>)> {
    match mesh_type {
        "landmesh" => Ok((Some(required_output_at(outputs, 0, "land")?), None, None)),
        "oceanmesh" => Ok((None, Some(required_output_at(outputs, 0, "ocean")?), None)),
        "atmos" | "atmosmesh" => Ok((None, None, Some(required_output_at(outputs, 0, "atmos")?))),
        "LOCmesh" | "earthmesh" => Ok((
            Some(required_output_at(outputs, 0, "land")?),
            Some(required_output_at(outputs, 1, "ocean")?),
            Some(required_output_at(outputs, 2, "atmos")?),
        )),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported calculated GetRef mesh_type {other}"),
        )),
    }
}

fn required_output_at<'a>(
    outputs: &'a [PathBuf],
    index: usize,
    component: &str,
) -> io::Result<&'a Path> {
    outputs.get(index).map(PathBuf::as_path).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("missing calculated GetRef {component} threshold output path"),
        )
    })
}
