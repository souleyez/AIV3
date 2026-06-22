use serde_json::{Map, Value};

pub(crate) fn value_array(value: Value) -> Vec<Value> {
    match value {
        Value::Array(items) => items,
        _ => Vec::new(),
    }
}

pub(crate) fn value_at_any_key<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| value.get(*key))
}

pub(crate) fn copy_json_fields(source: &Value, target: &mut Map<String, Value>, keys: &[&str]) {
    for key in keys {
        if let Some(value) = value_at_any_key(source, &[*key])
            .filter(|value| !value.is_null())
            .cloned()
        {
            target.insert((*key).to_string(), value);
        }
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

    #[test]
    fn value_at_any_key_returns_first_existing_alias() {
        let value = json!({
            "snake_case": "snake",
            "camelCase": "camel"
        });

        assert_eq!(
            value_at_any_key(&value, &["missing", "camelCase"]).and_then(Value::as_str),
            Some("camel")
        );
    }

    #[test]
    fn copy_json_fields_copies_non_null_selected_fields() {
        let source = json!({
            "method": "ocr",
            "text_chars": 128,
            "last_error": null,
            "ignored": true
        });
        let mut target = Map::new();

        copy_json_fields(
            &source,
            &mut target,
            &["method", "text_chars", "last_error"],
        );

        assert_eq!(target.get("method"), Some(&json!("ocr")));
        assert_eq!(target.get("text_chars"), Some(&json!(128)));
        assert!(!target.contains_key("last_error"));
        assert!(!target.contains_key("ignored"));
    }
}
