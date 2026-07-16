use crate::JsonNode;

pub(crate) fn json_node_to_string(node: &JsonNode) -> String {
    match node {
        JsonNode::Null => "null".into(),
        JsonNode::Bool(value) => value.to_string(),
        JsonNode::Number(value) => crate::json_number(*value),
        JsonNode::String(value) => {
            serde_json::to_string(value).expect("strings are always valid JSON")
        }
        JsonNode::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(json_node_to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        JsonNode::Object(map) => format!(
            "{{{}}}",
            map.iter()
                .map(|(key, value)| format!(
                    "{}: {}",
                    serde_json::to_string(key).expect("object keys are always valid JSON"),
                    json_node_to_string(value)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn node_serialization_preserves_legacy_numbers_and_stable_object_order() {
        let node = JsonNode::Object(BTreeMap::from([
            ("z".to_string(), JsonNode::Number(-0.0)),
            ("a".to_string(), JsonNode::Number(1.0)),
            ("nan".to_string(), JsonNode::Number(f64::NAN)),
        ]));
        assert_eq!(
            json_node_to_string(&node),
            r#"{"a": 1, "nan": null, "z": 0}"#
        );
    }
}
