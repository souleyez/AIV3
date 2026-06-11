use serde_json::Value;

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
