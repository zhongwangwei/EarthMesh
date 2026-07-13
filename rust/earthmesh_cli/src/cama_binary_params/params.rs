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
    let mut little_endian = true;
    let mut y_reversed_storage = true;
    for line in text.lines() {
        let mut parts = line.splitn(2, "!!");
        let payload = parts.next().unwrap_or_default().trim();
        if let Some(comment) = parts.next() {
            apply_storage_directives(comment, &mut little_endian, &mut y_reversed_storage);
        }
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
        little_endian,
        y_reversed_storage,
    })
}

fn apply_storage_directives(text: &str, little_endian: &mut bool, y_reversed_storage: &mut bool) {
    let normalized = text.to_ascii_lowercase().replace('-', "_");
    let tokens = normalized
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    for (index, token) in tokens.iter().copied().enumerate() {
        let next = tokens.get(index + 1).copied().unwrap_or("");
        match (token, next) {
            ("little_endian", _) | ("endian=little", _) | ("endian", "little") => {
                *little_endian = true
            }
            ("big_endian", _) | ("endian=big", _) | ("endian", "big") => *little_endian = false,
            ("no_yrev", _)
            | ("y_reversed=false", _)
            | ("yrev=false", _)
            | ("y_reversed", "false")
            | ("yrev", "false") => *y_reversed_storage = false,
            ("y_reversed=true", _)
            | ("yrev=true", _)
            | ("y_reversed", "true")
            | ("yrev", "true") => *y_reversed_storage = true,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_cama_params_text;

    #[test]
    fn cama_params_default_to_documented_little_endian_with_big_override() {
        let default_spec = parse_cama_params_text("4\n3\n1\n0.5\n100\n102\n10\n11.5\n").unwrap();
        assert!(default_spec.little_endian);
        assert!(default_spec.y_reversed_storage);

        let big_spec =
            parse_cama_params_text("4 !! endian=big yrev=false\n3\n1\n0.5\n100\n102\n10\n11.5\n")
                .unwrap();
        assert!(!big_spec.little_endian);
        assert!(!big_spec.y_reversed_storage);
    }
}
