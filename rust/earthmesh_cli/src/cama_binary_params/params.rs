use std::fs;
use std::io;
use std::path::Path;

use crate::cama_binary_io::CamaBinaryGridSpec;

/// Parse a CaMa `params.txt` file into a Rust grid spec.
pub fn read_cama_grid_spec_from_params_file(
    path: impl AsRef<Path>,
) -> io::Result<CamaBinaryGridSpec> {
    parse_cama_params_text(&fs::read_to_string(path)?)
}

fn parse_cama_params_text(text: &str) -> io::Result<CamaBinaryGridSpec> {
    let mut values = Vec::new();
    for line in text.lines() {
        let payload = line.split("!!").next().unwrap_or_default().trim();
        if payload.is_empty() {
            continue;
        }
        let Some(first) = payload.split_whitespace().next() else {
            continue;
        };
        let value = first.parse::<f64>().map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid numeric record in CaMa params.txt: {err}"),
            )
        })?;
        values.push(value);
    }
    if values.len() < 8 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "CaMa params text must contain at least 8 numeric records",
        ));
    }
    Ok(CamaBinaryGridSpec {
        nx: values[0] as usize,
        ny: values[1] as usize,
        west: values[4],
        south: values[6],
        grid_size_deg: values[3],
        little_endian: true,
        y_reversed_storage: false,
    })
}
