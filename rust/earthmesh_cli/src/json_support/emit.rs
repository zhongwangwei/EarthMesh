use std::collections::BTreeMap;

pub(crate) fn json_usize_map(values: &BTreeMap<String, usize>) -> String {
    let mut text = String::from("{");
    for (index, (key, value)) in values.iter().enumerate() {
        if index > 0 {
            text.push(',');
        }
        text.push('"');
        text.push_str(&json_escape_string(key));
        text.push_str("\":");
        text.push_str(&value.to_string());
    }
    text.push('}');
    text
}

pub(crate) fn json_usize_f64_map(values: &BTreeMap<usize, f64>) -> String {
    let mut text = String::from("{");
    for (index, (key, value)) in values.iter().enumerate() {
        if index > 0 {
            text.push(',');
        }
        text.push('"');
        text.push_str(&key.to_string());
        text.push_str("\":");
        text.push_str(&json_number(*value));
    }
    text.push('}');
    text
}

pub(crate) fn json_number(value: f64) -> String {
    if value.is_finite() {
        let mut text = value.to_string();
        if text == "-0" {
            text = "0".to_string();
        }
        text
    } else {
        "null".to_string()
    }
}

pub(crate) fn json_escape_string(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => escaped.push(ch),
        }
    }
    escaped
}

pub(crate) fn json_string_array(values: &[String]) -> String {
    let mut text = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            text.push(',');
        }
        text.push('"');
        text.push_str(&json_escape_string(value));
        text.push('"');
    }
    text.push(']');
    text
}
