pub(crate) fn strip_fortran_comment(line: &str) -> &str {
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

pub(crate) fn fortran_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

pub(crate) fn parse_fortran_string(value: &str) -> String {
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

pub(crate) fn parse_f32(field: &str, value: &str) -> Result<f32, String> {
    value
        .trim()
        .trim_end_matches(',')
        .parse()
        .map_err(|err| format!("invalid real for {field}: {value} ({err})"))
}

pub(crate) fn parse_f64(field: &str, value: &str) -> Result<f64, String> {
    value
        .trim()
        .trim_end_matches(',')
        .parse()
        .map_err(|err| format!("invalid real for {field}: {value} ({err})"))
}

pub(crate) fn parse_fortran_bool(field: &str, value: &str) -> Result<bool, String> {
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

pub(crate) fn parse_i32_fortran_1_based_array<const N: usize>(
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
