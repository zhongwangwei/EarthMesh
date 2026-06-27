use crate::{format_coupling_number, JsonNode};

pub(crate) fn json_node_to_string(node: &JsonNode) -> String {
    match node {
        JsonNode::Null => "null".into(),
        JsonNode::Bool(b) => b.to_string(),
        JsonNode::Number(n) => format_coupling_number(*n),
        JsonNode::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
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
                .map(|(k, v)| format!(
                    "\"{}\": {}",
                    k.replace('\\', "\\\\").replace('"', "\\\""),
                    json_node_to_string(v)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}
