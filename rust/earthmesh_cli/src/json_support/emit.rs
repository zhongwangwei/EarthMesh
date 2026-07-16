use std::collections::BTreeMap;

pub(crate) fn json_usize_map(values: &BTreeMap<String, usize>) -> String {
    serde_json::to_string(values).expect("string-to-usize maps are always valid JSON")
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
    let quoted = serde_json::to_string(value).expect("strings are always valid JSON");
    quoted[1..quoted.len() - 1].to_string()
}

pub(crate) fn json_string_array(values: &[String]) -> String {
    serde_json::to_string(values).expect("string arrays are always valid JSON")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_helpers_preserve_legacy_numeric_and_escape_contracts() {
        assert_eq!(json_number(1.0), "1");
        assert_eq!(json_number(-0.0), "0");
        assert_eq!(json_number(f64::NAN), "null");
        assert_eq!(json_escape_string("a\n\"b"), "a\\n\\\"b");
        assert_eq!(
            json_string_array(&["中文".to_string(), "a\nb".to_string()]),
            r#"["中文","a\nb"]"#
        );

        let values = BTreeMap::from([(2usize, -0.0), (1usize, 1.0)]);
        assert_eq!(json_usize_f64_map(&values), r#"{"1":1,"2":0}"#);
    }

    #[test]
    fn map_emitters_keep_btree_key_order() {
        let values = BTreeMap::from([("z".to_string(), 2usize), ("a".to_string(), 1usize)]);
        assert_eq!(json_usize_map(&values), r#"{"a":1,"z":2}"#);
    }
}
