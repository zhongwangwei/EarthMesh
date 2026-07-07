use std::io;

#[derive(Debug, Clone)]
pub(crate) struct OlamNamelistAssignment {
    pub(crate) field: String,
    pub(crate) indices: Vec<usize>,
    pub(crate) value: String,
}

pub(crate) fn olam_namelist_assignments(
    contents: &str,
    section: &str,
) -> io::Result<Vec<OlamNamelistAssignment>> {
    let mut assignments = Vec::new();
    let mut in_section = false;
    for line in contents.lines() {
        let uncommented = line.split('!').next().unwrap_or("").trim();
        let lower = uncommented.to_ascii_lowercase();
        if namelist_line_starts_section(&lower, section) {
            in_section = true;
            continue;
        }
        if in_section && uncommented == "/" {
            break;
        }
        if !in_section || uncommented.is_empty() {
            continue;
        }
        let Some((lhs, rhs)) = uncommented.split_once('=') else {
            continue;
        };
        let Some((field, indices)) = parse_olam_native_lhs(lhs)? else {
            continue;
        };
        assignments.push(OlamNamelistAssignment {
            field,
            indices,
            value: parse_olam_native_string(rhs.trim_end_matches(',')),
        });
    }
    Ok(assignments)
}

pub(crate) fn olam_namelist_has_section(contents: &str, section: &str) -> bool {
    contents.lines().any(|line| {
        let uncommented = line.split('!').next().unwrap_or("").trim();
        namelist_line_starts_section(&uncommented.to_ascii_lowercase(), section)
    })
}

fn namelist_line_starts_section(line: &str, section: &str) -> bool {
    let section_header = format!("&{}", section.to_ascii_lowercase());
    if !line.starts_with(&section_header) {
        return false;
    }
    line[section_header.len()..]
        .chars()
        .next()
        .is_none_or(|ch| ch.is_whitespace() || ch == '/')
}

pub(crate) fn parse_olam_native_lhs(lhs: &str) -> io::Result<Option<(String, Vec<usize>)>> {
    let raw = lhs.trim().to_ascii_lowercase();
    let field = raw.rsplit('%').next().unwrap_or(&raw).trim();
    if field.is_empty() {
        return Ok(None);
    }
    let Some(open) = field.find('(') else {
        return Ok(Some((field.to_string(), Vec::new())));
    };
    let Some(close) = field[open + 1..].find(')') else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid OLAM native namelist field index syntax: {lhs}"),
        ));
    };
    let close = open + 1 + close;
    let name = field[..open].trim().to_string();
    let mut indices = Vec::new();
    for raw_index in field[open + 1..close].split(',') {
        let index = raw_index.trim().parse::<usize>().map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid OLAM native namelist index in {lhs}: {err}"),
            )
        })?;
        indices.push(index);
    }
    Ok(Some((name, indices)))
}

pub(crate) fn olam_native_index(
    assignment: &OlamNamelistAssignment,
    offset: usize,
) -> io::Result<usize> {
    assignment.indices.get(offset).copied().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "native OLAM field {} requires index {}",
                assignment.field,
                offset + 1
            ),
        )
    })
}

pub(crate) fn parse_olam_native_string(value: &str) -> String {
    value
        .trim()
        .trim_end_matches(',')
        .trim()
        .trim_matches('\'')
        .trim_matches('"')
        .trim()
        .to_string()
}

pub(crate) fn parse_olam_native_bool(field: &str, value: &str) -> io::Result<bool> {
    match value.trim().trim_matches('.').to_ascii_lowercase().as_str() {
        "true" | "t" => Ok(true),
        "false" | "f" => Ok(false),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid OLAM native boolean {field}={value}"),
        )),
    }
}

pub(crate) fn parse_olam_native_i32(field: &str, value: &str) -> io::Result<i32> {
    value.trim().parse::<i32>().map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid OLAM native integer {field}={value}: {err}"),
        )
    })
}

pub(crate) fn parse_olam_native_usize(field: &str, value: &str) -> io::Result<usize> {
    value.trim().parse::<usize>().map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid OLAM native integer {field}={value}: {err}"),
        )
    })
}

pub(crate) fn parse_olam_native_f64(field: &str, value: &str) -> io::Result<f64> {
    value
        .trim()
        .replace('D', "E")
        .replace('d', "e")
        .parse::<f64>()
        .map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid OLAM native real {field}={value}: {err}"),
            )
        })
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namelist_section_match_is_token_exact() {
        assert!(olam_namelist_has_section("&mkgrd\n/", "mkgrd"));
        assert!(olam_namelist_has_section("&mkgrd /\n", "mkgrd"));
        assert!(!olam_namelist_has_section("&mkgrd_extra\n/", "mkgrd"));
        assert!(!olam_namelist_has_section("&hfield_debug\n/", "hfield"));
    }
}
