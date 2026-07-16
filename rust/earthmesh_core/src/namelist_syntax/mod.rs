#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamelistAssignment {
    pub field: String,
    pub indices: Vec<usize>,
    pub value: String,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct NamelistGroupSpan<'a> {
    pub(crate) name: &'a str,
    pub(crate) text: &'a str,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

/// Locate complete namelist groups without treating `/`, `&`, or `!` inside a
/// quoted value as syntax. Spans include the opening `&name` and closing `/`.
pub(crate) fn namelist_group_spans(contents: &str) -> Result<Vec<NamelistGroupSpan<'_>>, String> {
    let bytes = contents.as_bytes();
    let mut spans = Vec::new();
    let mut index = 0usize;
    let mut quote = None;
    while index < bytes.len() {
        match bytes[index] {
            b'!' if quote.is_none() => {
                index = contents[index..]
                    .find('\n')
                    .map_or(bytes.len(), |offset| index + offset + 1);
            }
            current @ (b'\'' | b'"') => {
                if quote == Some(current) {
                    if bytes.get(index + 1) == Some(&current) {
                        index += 2;
                        continue;
                    }
                    quote = None;
                } else if quote.is_none() {
                    quote = Some(current);
                }
                index += 1;
            }
            b'&' if quote.is_none() => {
                let start = index;
                let name_start = start + 1;
                let mut name_end = name_start;
                while bytes
                    .get(name_end)
                    .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
                {
                    name_end += 1;
                }
                if name_end == name_start {
                    index += 1;
                    continue;
                }
                let mut cursor = name_end;
                let mut group_quote = None;
                let mut group_end = None;
                while cursor < bytes.len() {
                    match bytes[cursor] {
                        b'!' if group_quote.is_none() => {
                            cursor = contents[cursor..]
                                .find('\n')
                                .map_or(bytes.len(), |offset| cursor + offset + 1);
                        }
                        current @ (b'\'' | b'"') => {
                            if group_quote == Some(current) {
                                if bytes.get(cursor + 1) == Some(&current) {
                                    cursor += 2;
                                    continue;
                                }
                                group_quote = None;
                            } else if group_quote.is_none() {
                                group_quote = Some(current);
                            }
                            cursor += 1;
                        }
                        b'/' if group_quote.is_none() => {
                            group_end = Some(cursor + 1);
                            break;
                        }
                        _ => cursor += 1,
                    }
                }
                let end = group_end.ok_or_else(|| {
                    format!(
                        "namelist group &{} has no unquoted '/' terminator",
                        &contents[name_start..name_end]
                    )
                })?;
                spans.push(NamelistGroupSpan {
                    name: &contents[name_start..name_end],
                    text: &contents[start..end],
                    start,
                    end,
                });
                index = end;
            }
            _ => index += 1,
        }
    }
    Ok(spans)
}

/// Rewrite scalar fields in exactly one namelist group and normalize that group
/// to one assignment per line. Other text and groups remain byte-for-byte.
pub fn rewrite_namelist_group_fields(
    contents: &str,
    section: &str,
    variable_prefix: &str,
    replacements: &[(&str, &str)],
) -> Result<String, String> {
    let spans = namelist_group_spans(contents)?;
    let matching = spans
        .iter()
        .filter(|span| span.name.eq_ignore_ascii_case(section))
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(format!(
            "expected exactly one &{section} group, found {}",
            matching.len()
        ));
    }
    let span = matching[0];
    let assignments = namelist_assignments(span.text, section)?;
    let replacement_names = replacements
        .iter()
        .map(|(field, _)| field.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let mut emitted_replacements = vec![false; replacements.len()];
    let mut rewritten_group = format!("&{section}\n");
    for assignment in assignments {
        let replacement_index = replacement_names
            .iter()
            .position(|field| field.eq_ignore_ascii_case(&assignment.field));
        let value = if let Some(index) = replacement_index {
            if emitted_replacements[index] {
                continue;
            }
            emitted_replacements[index] = true;
            replacements[index].1
        } else {
            assignment.value.as_str()
        };
        push_namelist_assignment(
            &mut rewritten_group,
            variable_prefix,
            &assignment.field,
            &assignment.indices,
            value,
        );
    }
    for (index, (field, value)) in replacements.iter().enumerate() {
        if !emitted_replacements[index] {
            push_namelist_assignment(&mut rewritten_group, variable_prefix, field, &[], value);
        }
    }
    rewritten_group.push('/');

    let mut rewritten = String::with_capacity(contents.len() + rewritten_group.len());
    rewritten.push_str(&contents[..span.start]);
    rewritten.push_str(&rewritten_group);
    rewritten.push_str(&contents[span.end..]);
    Ok(rewritten)
}

fn push_namelist_assignment(
    output: &mut String,
    variable_prefix: &str,
    field: &str,
    indices: &[usize],
    value: &str,
) {
    output.push_str("  ");
    output.push_str(variable_prefix);
    output.push('%');
    output.push_str(field);
    if !indices.is_empty() {
        output.push('(');
        output.push_str(
            &indices
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(","),
        );
        output.push(')');
    }
    output.push_str(" = ");
    output.push_str(value);
    output.push('\n');
}

/// Parse one namelist group, including assignments placed on the group-header
/// line and multiple comma-separated assignments on one line.
pub fn namelist_assignments(
    contents: &str,
    section: &str,
) -> Result<Vec<NamelistAssignment>, String> {
    let mut assignments: Vec<NamelistAssignment> = Vec::new();
    let mut in_section = false;
    for line in contents.lines() {
        let mut uncommented = strip_canonical_comment(line).trim();
        if let Some(rest) = namelist_section_rest(uncommented, section) {
            in_section = true;
            uncommented = rest;
        }
        let section_end = in_section
            .then(|| unquoted_char_index(uncommented, '/'))
            .flatten();
        let closes_section = section_end.is_some();
        if let Some(end) = section_end {
            uncommented = uncommented[..end].trim();
        }
        if in_section && !uncommented.is_empty() {
            for assignment in split_namelist_assignment_items(uncommented) {
                let Some((lhs, rhs)) = assignment.split_once('=') else {
                    let continuation = assignment.trim().trim_end_matches(',').trim();
                    if !continuation.is_empty() {
                        if let Some(previous) = assignments.last_mut() {
                            previous.value.push_str(", ");
                            previous.value.push_str(continuation);
                        }
                    }
                    continue;
                };
                let Some((field, indices)) = parse_namelist_lhs(lhs)? else {
                    continue;
                };
                assignments.push(NamelistAssignment {
                    field,
                    indices,
                    value: rhs.trim().trim_end_matches(',').trim().to_string(),
                });
            }
        }
        if closes_section {
            break;
        }
    }
    Ok(assignments)
}

pub fn namelist_has_section(contents: &str, section: &str) -> bool {
    contents
        .lines()
        .any(|line| namelist_section_rest(strip_canonical_comment(line).trim(), section).is_some())
}

fn namelist_section_rest<'a>(line: &'a str, section: &str) -> Option<&'a str> {
    let section_header = format!("&{}", section.to_ascii_lowercase());
    if !line.to_ascii_lowercase().starts_with(&section_header) {
        return None;
    }
    let rest = &line[section_header.len()..];
    rest.chars()
        .next()
        .is_none_or(|ch| ch.is_whitespace() || matches!(ch, ',' | '/'))
        .then(|| rest.trim_start().trim_start_matches(',').trim_start())
}

fn unquoted_char_index(line: &str, needle: char) -> Option<usize> {
    let mut quote = None;
    for (index, ch) in line.char_indices() {
        match ch {
            '\'' | '"' if quote == Some(ch) => quote = None,
            '\'' | '"' if quote.is_none() => quote = Some(ch),
            _ if ch == needle && quote.is_none() => return Some(index),
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
    for (index, ch) in line.char_indices() {
        match ch {
            '\'' | '"' if quote == Some(ch) => quote = None,
            '\'' | '"' if quote.is_none() => quote = Some(ch),
            '(' if quote.is_none() => paren_depth += 1,
            ')' if quote.is_none() && paren_depth > 0 => paren_depth -= 1,
            ',' if quote.is_none()
                && paren_depth == 0
                && rest_starts_assignment(&line[index + ch.len_utf8()..]) =>
            {
                let item = line[start..index].trim();
                if !item.is_empty() {
                    items.push(item);
                }
                start = index + ch.len_utf8();
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
    lhs.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic())
        && lhs
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '%' | '(' | ')' | ',' | ' '))
}

fn parse_namelist_lhs(lhs: &str) -> Result<Option<(String, Vec<usize>)>, String> {
    let raw = lhs.trim().to_ascii_lowercase();
    let field = raw.rsplit('%').next().unwrap_or(&raw).trim();
    if field.is_empty() {
        return Ok(None);
    }
    let Some(open) = field.find('(') else {
        return Ok(Some((field.to_string(), Vec::new())));
    };
    let Some(close) = field[open + 1..].find(')') else {
        return Err(format!("invalid namelist field index syntax: {lhs}"));
    };
    let close = open + 1 + close;
    let name = field[..open].trim().to_string();
    let indices = field[open + 1..close]
        .split(',')
        .map(|raw_index| {
            raw_index
                .trim()
                .parse::<usize>()
                .map_err(|err| format!("invalid namelist index in {lhs}: {err}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some((name, indices)))
}

pub(crate) fn strip_canonical_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut chars = line.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        match ch {
            '\'' | '"' if quote == Some(ch) => {
                if chars.peek().is_some_and(|(_, next)| *next == ch) {
                    chars.next();
                } else {
                    quote = None;
                }
            }
            '\'' | '"' if quote.is_none() => quote = Some(ch),
            '!' if quote.is_none() => return &line[..index],
            _ => {}
        }
    }
    line
}

pub(crate) fn canonical_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

pub(crate) fn parse_canonical_string(value: &str) -> String {
    let trimmed = value.trim().trim_end_matches(',').trim();
    let Some(quote) = trimmed
        .chars()
        .next()
        .filter(|ch| *ch == '\'' || *ch == '"')
    else {
        return trimmed.to_string();
    };
    let inner = trimmed
        .strip_prefix(quote)
        .and_then(|value| value.strip_suffix(quote))
        .unwrap_or(trimmed);
    match quote {
        '\'' => inner.replace("''", "'"),
        '"' => inner.replace("\"\"", "\""),
        _ => inner.to_string(),
    }
}

pub(crate) fn parse_i32(field: &str, value: &str) -> Result<i32, String> {
    value
        .trim()
        .trim_end_matches(',')
        .parse()
        .map_err(|err| format!("invalid integer for {field}: {value} ({err})"))
}

pub(crate) fn parse_f64(field: &str, value: &str) -> Result<f64, String> {
    value
        .trim()
        .trim_end_matches(',')
        .parse()
        .map_err(|err| format!("invalid real for {field}: {value} ({err})"))
}

pub(crate) fn parse_canonical_bool(field: &str, value: &str) -> Result<bool, String> {
    match value
        .trim()
        .trim_end_matches(',')
        .to_ascii_lowercase()
        .as_str()
    {
        ".true." | "true" | "t" => Ok(true),
        ".false." | "false" | "f" => Ok(false),
        other => Err(format!("invalid logical for {field}: {other}")),
    }
}

pub(crate) fn parse_i32_canonical_1_based_array<const N: usize>(
    field: &str,
    value: &str,
) -> Result<[i32; N], String> {
    let values = value
        .split(',')
        .map(|part| parse_i32(field, part.trim()))
        .collect::<Result<Vec<_>, _>>()?;
    if values.len() >= N {
        return Err(format!(
            "invalid integer array length for {field}: expected at most {}, got {}",
            N - 1,
            values.len()
        ));
    }
    let mut parsed = [0; N];
    for (index, value) in values.into_iter().enumerate() {
        parsed[index + 1] = value;
    }
    Ok(parsed)
}

pub(crate) fn parse_f64_array<const N: usize>(
    field: &str,
    value: &str,
    mut parsed: [f64; N],
) -> Result<[f64; N], String> {
    let values = value
        .split(',')
        .map(|part| parse_f64(field, part.trim()))
        .collect::<Result<Vec<_>, _>>()?;
    if values.len() > N {
        return Err(format!(
            "invalid real array length for {field}: expected at most {N}, got {}",
            values.len()
        ));
    }
    for (index, value) in values.into_iter().enumerate() {
        parsed[index] = value;
    }
    Ok(parsed)
}
