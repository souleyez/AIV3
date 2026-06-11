use serde_json::{json, Map, Value};

pub(crate) fn redacted_summary(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .filter(|(key, _)| !sensitive_key(key))
                .map(|(key, value)| (key, redacted_summary(value)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(redacted_summary).collect()),
        Value::String(text) if text.starts_with("[redacted") => {
            Value::String("[redacted]".to_string())
        }
        other => other,
    }
}

pub(crate) fn sensitive_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower.contains("secret")
        || lower.contains("token")
        || lower.contains("authorization")
        || lower.contains("cookie")
        || lower.contains("password")
}

pub(crate) fn has_redacted_value(value: &Value) -> bool {
    match value {
        Value::String(text) => text.starts_with("[redacted"),
        Value::Array(items) => items.iter().any(has_redacted_value),
        Value::Object(map) => map.values().any(has_redacted_value),
        _ => false,
    }
}

pub(crate) fn action_result_payload_summary(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let sensitive_field_count = map.keys().filter(|key| sensitive_key(key)).count();
            json!({
                "kind": "object",
                "field_count": map.len(),
                "sensitive_field_count": sensitive_field_count,
            })
        }
        Value::Array(items) => json!({
            "kind": "array",
            "item_count": items.len(),
        }),
        Value::String(text) => json!({
            "kind": "string",
            "length_chars": text.chars().count(),
        }),
        Value::Number(_) => json!({
            "kind": "number",
        }),
        Value::Bool(_) => json!({
            "kind": "boolean",
        }),
        Value::Null => json!({
            "kind": "null",
        }),
    }
}

pub(crate) fn action_response_summary(response: Option<&Value>, response_text: &str) -> Value {
    if let Some(response) = response {
        return json!({
            "json": action_redacted_response_summary(response),
        });
    }
    json!({
        "text_chars": response_text.chars().count(),
        "body_redacted": true,
    })
}

fn action_redacted_response_summary(response: &Value) -> Value {
    match response {
        Value::Object(map) => {
            let mut summary = Map::new();
            for key in [
                "external_request_id",
                "externalRequestId",
                "request_id",
                "requestId",
                "status",
                "code",
            ] {
                if let Some(value) = map.get(key) {
                    summary.insert(key.to_string(), value.clone());
                }
            }
            if map.contains_key("message") {
                summary.insert("message_present".to_string(), json!(true));
            }
            Value::Object(summary)
        }
        _ => Value::Null,
    }
}
