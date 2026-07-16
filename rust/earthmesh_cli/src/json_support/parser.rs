use std::io;

use super::JsonNode;

pub(crate) struct JsonParser<'a> {
    text: &'a str,
}

impl<'a> JsonParser<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        Self { text }
    }

    pub(crate) fn parse(self) -> io::Result<JsonNode> {
        serde_json::from_str(self.text).map_err(|error| {
            io::Error::new(io::ErrorKind::InvalidData, format!("invalid JSON: {error}"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_raw_utf8_and_unicode_surrogates() {
        let parsed = JsonParser::new(r#"{"name":"中文","emoji":"\uD83D\uDE00"}"#)
            .parse()
            .expect("parse unicode");
        let object = parsed.as_object().expect("object");
        assert_eq!(object.get("name").and_then(JsonNode::as_str), Some("中文"));
        assert_eq!(object.get("emoji").and_then(JsonNode::as_str), Some("😀"));
    }

    #[test]
    fn rejects_unpaired_unicode_surrogates() {
        assert!(JsonParser::new(r#""\uD83Dx""#).parse().is_err());
        assert!(JsonParser::new(r#""\uDE00""#).parse().is_err());
    }

    #[test]
    fn rejects_numbers_outside_the_json_grammar() {
        for invalid in ["-.1", "1.", "01", "-01", "1e", "1e+", "1e400"] {
            assert!(
                JsonParser::new(invalid).parse().is_err(),
                "{invalid} must not be accepted as a JSON number"
            );
        }
    }

    #[test]
    fn accepts_numbers_in_the_json_grammar() {
        for valid in ["0", "-0", "10", "-0.25", "1e3", "1E-3"] {
            assert!(
                JsonParser::new(valid).parse().is_ok(),
                "{valid} must be accepted as a JSON number"
            );
        }
    }

    #[test]
    fn preserves_domain_visible_node_variants_and_negative_zero() {
        let parsed = JsonParser::new(
            r#"{"array":[],"bool":false,"null":null,"number":-0,"object":{},"string":"x"}"#,
        )
        .parse()
        .expect("parse every JSON node kind");
        let object = parsed.as_object().expect("object");
        assert!(matches!(object.get("array"), Some(JsonNode::Array(_))));
        assert_eq!(object.get("bool").and_then(JsonNode::as_bool), Some(false));
        assert!(matches!(object.get("null"), Some(JsonNode::Null)));
        assert!(object
            .get("number")
            .and_then(JsonNode::as_f64)
            .is_some_and(f64::is_sign_negative));
        assert!(matches!(object.get("object"), Some(JsonNode::Object(_))));
        assert_eq!(object.get("string").and_then(JsonNode::as_str), Some("x"));
    }

    #[test]
    fn rejects_non_json_nan_literal() {
        assert!(JsonParser::new("NaN").parse().is_err());
    }
}
