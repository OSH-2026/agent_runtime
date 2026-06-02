use crate::error::DispatcherError;
use crate::plan::{Node, NodeId};
use base64::Engine;
use serde_json::Value;
use std::collections::HashMap;

pub fn resolve_node_payload(
    node: &Node,
    outputs: &HashMap<NodeId, Vec<u8>>,
    default_payload: &[u8],
) -> Result<Vec<u8>, DispatcherError> {
    match &node.inputs {
        Some(value) => {
            let resolved = resolve_value(value, outputs)?;
            serde_json::to_vec(&resolved)
                .map_err(|err| DispatcherError::Execution(err.to_string()))
        }
        None => Ok(default_payload.to_vec()),
    }
}

fn resolve_value(value: &Value, outputs: &HashMap<NodeId, Vec<u8>>) -> Result<Value, DispatcherError> {
    match value {
        Value::String(text) => Ok(Value::String(replace_refs(text, outputs)?)),
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(resolve_value(item, outputs)?);
            }
            Ok(Value::Array(out))
        }
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, value) in map {
                out.insert(key.clone(), resolve_value(value, outputs)?);
            }
            Ok(Value::Object(out))
        }
        _ => Ok(value.clone()),
    }
}

fn replace_refs(input: &str, outputs: &HashMap<NodeId, Vec<u8>>) -> Result<String, DispatcherError> {
    let mut result = String::new();
    let mut cursor = 0usize;
    while let Some(start) = input[cursor..].find("${") {
        let start_index = cursor + start;
        result.push_str(&input[cursor..start_index]);
        let ref_start = start_index + 2;
        let end_index = match input[ref_start..].find('}') {
            Some(offset) => ref_start + offset,
            None => {
                return Err(DispatcherError::Execution(format!(
                    "invalid reference: {}",
                    input
                )));
            }
        };
        let raw = input[ref_start..end_index].trim();
        let step_id = raw.split('.').next().unwrap_or("");
        if step_id.is_empty() {
            return Err(DispatcherError::Execution(format!(
                "invalid reference: {}",
                input
            )));
        }
        let output = outputs
            .get(step_id)
            .ok_or_else(|| DispatcherError::Execution(format!("missing output: {step_id}")))?;
        result.push_str(&bytes_to_string(output));
        cursor = end_index + 1;
    }
    result.push_str(&input[cursor..]);
    Ok(result)
}

fn bytes_to_string(bytes: &[u8]) -> String {
    match String::from_utf8(bytes.to_vec()) {
        Ok(text) => text,
        Err(_) => base64::engine::general_purpose::STANDARD.encode(bytes),
    }
}
