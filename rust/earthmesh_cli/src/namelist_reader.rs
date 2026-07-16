use std::io;

pub(crate) use earthmesh_core::NamelistAssignment;

pub(crate) fn namelist_assignments(
    contents: &str,
    section: &str,
) -> io::Result<Vec<NamelistAssignment>> {
    let mut assignments = earthmesh_core::namelist_assignments(contents, section)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    for assignment in &mut assignments {
        assignment.value = parse_namelist_string(&assignment.value);
    }
    Ok(assignments)
}

pub(crate) fn namelist_has_section(contents: &str, section: &str) -> bool {
    earthmesh_core::namelist_has_section(contents, section)
}

pub(crate) fn native_grid_index(
    assignment: &NamelistAssignment,
    offset: usize,
) -> io::Result<usize> {
    assignment.indices.get(offset).copied().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "native Method-C field {} requires index {}",
                assignment.field,
                offset + 1
            ),
        )
    })
}

pub(crate) fn parse_namelist_string(value: &str) -> String {
    value
        .trim()
        .trim_end_matches(',')
        .trim()
        .trim_matches('\'')
        .trim_matches('"')
        .trim()
        .to_string()
}

fn first_namelist_value(value: &str) -> &str {
    let value = value.trim();
    let value = value.split(',').next().unwrap_or(value).trim();
    if let Some((count, repeated)) = value.split_once('*') {
        if count.trim().parse::<usize>().is_ok() {
            return repeated.trim();
        }
    }
    value
}

pub(crate) fn parse_namelist_bool(field: &str, value: &str) -> io::Result<bool> {
    match first_namelist_value(value)
        .trim_matches('.')
        .to_ascii_lowercase()
        .as_str()
    {
        "true" | "t" => Ok(true),
        "false" | "f" => Ok(false),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid Method-C native boolean {field}={value}"),
        )),
    }
}

pub(crate) fn parse_namelist_i32(field: &str, value: &str) -> io::Result<i32> {
    first_namelist_value(value).parse::<i32>().map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid Method-C native integer {field}={value}: {err}"),
        )
    })
}

pub(crate) fn parse_namelist_usize(field: &str, value: &str) -> io::Result<usize> {
    first_namelist_value(value).parse::<usize>().map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid Method-C native integer {field}={value}: {err}"),
        )
    })
}

pub(crate) fn parse_namelist_f64(field: &str, value: &str) -> io::Result<f64> {
    first_namelist_value(value)
        .replace('D', "E")
        .replace('d', "e")
        .parse::<f64>()
        .map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid Method-C native real {field}={value}: {err}"),
            )
        })
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namelist_section_match_is_token_exact() {
        assert!(namelist_has_section("&mkgrd\n/", "mkgrd"));
        assert!(namelist_has_section("&mkgrd /\n", "mkgrd"));
        assert!(!namelist_has_section("&mkgrd_extra\n/", "mkgrd"));
        assert!(!namelist_has_section("&hfield_debug\n/", "hfield"));
    }

    #[test]
    fn namelist_parser_handles_inline_assignments_and_section_end() {
        let assignments =
            namelist_assignments("&mkgrd ngrids=3, halo=4,4,3, deltax=2*1.5d0 /\n", "mkgrd")
                .expect("parse");
        assert_eq!(assignments.len(), 3);
        assert_eq!(assignments[0].field, "ngrids");
        assert_eq!(
            parse_namelist_usize("ngrids", &assignments[0].value).unwrap(),
            3
        );
        assert_eq!(assignments[1].value, "4,4,3");
        assert_eq!(
            parse_namelist_usize("halo", &assignments[1].value).unwrap(),
            4
        );
        assert_eq!(
            parse_namelist_f64("deltax", &assignments[2].value).unwrap(),
            1.5
        );
    }

    #[test]
    fn namelist_parser_does_not_close_on_slash_inside_quotes() {
        let assignments = namelist_assignments(
            "&mkgrd\n  NL%base_dir='/tmp/earthmesh/'\n  NL%mdomain=5\n/\n",
            "mkgrd",
        )
        .expect("parse");
        assert_eq!(
            assignments
                .iter()
                .map(|a| a.field.as_str())
                .collect::<Vec<_>>(),
            ["base_dir", "mdomain"]
        );
    }

    #[test]
    fn namelist_parser_does_not_comment_on_exclamation_inside_quotes() {
        let assignments = namelist_assignments(
            "&mkgrd\n  NL%base_dir='/tmp/earth!mesh/' ! real comment\n  NL%mdomain=5\n/\n",
            "mkgrd",
        )
        .expect("parse");
        assert_eq!(assignments[0].field, "base_dir");
        assert_eq!(assignments[0].value, "/tmp/earth!mesh/");
        assert_eq!(assignments[1].field, "mdomain");
    }
}
