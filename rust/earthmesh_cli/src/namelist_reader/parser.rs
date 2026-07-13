use std::io;

#[derive(Debug, Clone)]
pub(crate) struct NamelistAssignment {
    pub(crate) field: String,
    pub(crate) indices: Vec<usize>,
    pub(crate) value: String,
}

pub(crate) fn namelist_assignments(
    contents: &str,
    section: &str,
) -> io::Result<Vec<NamelistAssignment>> {
    let mut assignments = Vec::new();
    let mut in_section = false;
    for line in contents.lines() {
        let mut uncommented = strip_unquoted_comment(line).trim();
        let lower = uncommented.to_ascii_lowercase();
        if namelist_line_starts_section(&lower, section) {
            in_section = true;
            uncommented = uncommented
                .split_once(char::is_whitespace)
                .map(|(_, rest)| rest.trim())
                .unwrap_or("");
            if uncommented.is_empty() {
                continue;
            }
        }
        let section_end = if in_section {
            unquoted_slash_index(uncommented)
        } else {
            None
        };
        let closes_section = section_end.is_some();
        if closes_section {
            uncommented = uncommented[..section_end.unwrap()].trim();
        }
        if in_section && uncommented.is_empty() && closes_section {
            break;
        }
        if in_section && uncommented == "/" {
            break;
        }
        if !in_section || uncommented.is_empty() {
            if closes_section {
                break;
            }
            continue;
        }
        for assignment in split_namelist_assignment_items(uncommented) {
            let Some((lhs, rhs)) = assignment.split_once('=') else {
                continue;
            };
            let Some((field, indices)) = parse_namelist_lhs(lhs)? else {
                continue;
            };
            assignments.push(NamelistAssignment {
                field,
                indices,
                value: parse_namelist_string(rhs.trim_end_matches(',')),
            });
        }
        if closes_section {
            break;
        }
    }
    Ok(assignments)
}

fn unquoted_slash_index(line: &str) -> Option<usize> {
    unquoted_char_index(line, '/')
}

fn strip_unquoted_comment(line: &str) -> &str {
    if let Some(idx) = unquoted_char_index(line, '!') {
        &line[..idx]
    } else {
        line
    }
}

fn unquoted_char_index(line: &str, needle: char) -> Option<usize> {
    let mut quote = None;
    for (idx, ch) in line.char_indices() {
        match ch {
            '\'' | '"' => {
                if quote == Some(ch) {
                    quote = None;
                } else if quote.is_none() {
                    quote = Some(ch);
                }
            }
            _ if ch == needle && quote.is_none() => return Some(idx),
            _ => {}
        }
    }
    None
}

fn split_namelist_assignment_items(line: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut start = 0usize;
    let mut quote = None;
    let mut paren_depth = 0usize;
    for (idx, ch) in line.char_indices() {
        match ch {
            '\'' | '"' => {
                if quote == Some(ch) {
                    quote = None;
                } else if quote.is_none() {
                    quote = Some(ch);
                }
            }
            '(' if quote.is_none() => paren_depth += 1,
            ')' if quote.is_none() && paren_depth > 0 => paren_depth -= 1,
            ',' if quote.is_none()
                && paren_depth == 0
                && rest_starts_assignment(&line[idx + ch.len_utf8()..]) =>
            {
                let item = line[start..idx].trim();
                if !item.is_empty() {
                    items.push(item);
                }
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }
    let item = line[start..].trim();
    if !item.is_empty() {
        items.push(item);
    }
    items
}

fn rest_starts_assignment(rest: &str) -> bool {
    let rest = rest.trim_start();
    let Some(eq) = rest.find('=') else {
        return false;
    };
    let lhs = rest[..eq].trim();
    let Some(first) = lhs.chars().next() else {
        return false;
    };
    first.is_ascii_alphabetic()
        && lhs
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '%' | '(' | ')' | ',' | ' '))
}

pub(crate) fn namelist_has_section(contents: &str, section: &str) -> bool {
    contents.lines().any(|line| {
        let uncommented = strip_unquoted_comment(line).trim();
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

pub(crate) fn parse_namelist_lhs(lhs: &str) -> io::Result<Option<(String, Vec<usize>)>> {
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
            format!("invalid Method-C native namelist field index syntax: {lhs}"),
        ));
    };
    let close = open + 1 + close;
    let name = field[..open].trim().to_string();
    let mut indices = Vec::new();
    for raw_index in field[open + 1..close].split(',') {
        let index = raw_index.trim().parse::<usize>().map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid Method-C native namelist index in {lhs}: {err}"),
            )
        })?;
        indices.push(index);
    }
    Ok(Some((name, indices)))
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
