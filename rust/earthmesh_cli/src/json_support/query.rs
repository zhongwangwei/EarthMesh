use std::collections::BTreeMap;
use std::io;

use super::JsonNode;

pub(crate) fn geojson_feature_nodes(root: &JsonNode) -> Vec<&JsonNode> {
    match root
        .as_object()
        .and_then(|object| object.get("type"))
        .and_then(JsonNode::as_str)
    {
        Some("FeatureCollection") => root
            .as_object()
            .and_then(|object| object.get("features"))
            .and_then(JsonNode::as_array)
            .map(|features| {
                features
                    .iter()
                    .filter(|feature| {
                        feature
                            .as_object()
                            .and_then(|object| object.get("type"))
                            .and_then(JsonNode::as_str)
                            == Some("Feature")
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Some("Feature") => vec![root],
        _ => Vec::new(),
    }
}

pub(crate) fn json_string_usize_map(
    value: Option<&JsonNode>,
    default: Option<&BTreeMap<String, usize>>,
) -> io::Result<BTreeMap<String, usize>> {
    let Some(value) = value else {
        return Ok(default.cloned().unwrap_or_default());
    };
    let object = value.as_object().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected a JSON object mapping string keys to integer values",
        )
    })?;
    let mut output = BTreeMap::new();
    for (key, value) in object {
        output.insert(key.clone(), json_node_to_usize(value)?);
    }
    Ok(output)
}

pub(crate) fn json_usize_f64_map_node(
    value: Option<&JsonNode>,
    default: Option<&BTreeMap<usize, f64>>,
) -> io::Result<BTreeMap<usize, f64>> {
    let Some(value) = value else {
        return Ok(default.cloned().unwrap_or_default());
    };
    let object = value.as_object().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected a JSON object mapping integer keys to numeric values",
        )
    })?;
    let mut output = BTreeMap::new();
    for (key, value) in object {
        let degree = key.parse::<usize>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "expected JSON object keys to be non-negative integers",
            )
        })?;
        output.insert(degree, json_node_to_f64(value)?);
    }
    Ok(output)
}

pub(crate) fn json_node_to_usize(value: &JsonNode) -> io::Result<usize> {
    let value = json_node_to_f64(value)?;
    if value < 0.0 || value.fract() != 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected a non-negative integer JSON number",
        ));
    }
    Ok(value as usize)
}

pub(crate) fn json_node_to_f64(value: &JsonNode) -> io::Result<f64> {
    let value = value
        .as_f64()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "expected a JSON number"))?;
    if !value.is_finite() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected a finite JSON number",
        ));
    }
    Ok(value)
}
