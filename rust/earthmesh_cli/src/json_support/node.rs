use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum JsonNode {
    Object(BTreeMap<String, JsonNode>),
    Array(Vec<JsonNode>),
    String(String),
    Number(f64),
    Bool(bool),
    Null,
}

impl JsonNode {
    pub(crate) fn as_object(&self) -> Option<&BTreeMap<String, JsonNode>> {
        match self {
            JsonNode::Object(value) => Some(value),
            _ => None,
        }
    }

    pub(crate) fn as_array(&self) -> Option<&Vec<JsonNode>> {
        match self {
            JsonNode::Array(value) => Some(value),
            _ => None,
        }
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            JsonNode::String(value) => Some(value),
            _ => None,
        }
    }

    pub(crate) fn as_f64(&self) -> Option<f64> {
        match self {
            JsonNode::Number(value) => Some(*value),
            _ => None,
        }
    }

    pub(crate) fn as_bool(&self) -> Option<bool> {
        match self {
            JsonNode::Bool(value) => Some(*value),
            _ => None,
        }
    }
}
