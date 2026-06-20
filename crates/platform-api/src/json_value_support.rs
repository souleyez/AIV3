use serde_json::Value;

pub(crate) fn value_array(value: Value) -> Vec<Value> {
    match value {
        Value::Array(items) => items,
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn value_array_returns_owned_array_items() {
        let items = value_array(json!(["alpha", { "kind": "beta" }]));

        assert_eq!(items.len(), 2);
        assert_eq!(items[0], json!("alpha"));
        assert_eq!(items[1], json!({ "kind": "beta" }));
    }

    #[test]
    fn value_array_returns_empty_for_non_arrays() {
        assert!(value_array(Value::Null).is_empty());
        assert!(value_array(json!({ "items": [] })).is_empty());
        assert!(value_array(json!("not-array")).is_empty());
    }
}
