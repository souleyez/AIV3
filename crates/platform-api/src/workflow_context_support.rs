use serde_json::Value;
use uuid::Uuid;

pub(crate) fn workflow_context_uuid(context: &Value, key: &str) -> Option<Uuid> {
    context
        .get(key)
        .and_then(Value::as_str)
        .and_then(|raw| Uuid::parse_str(raw).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn workflow_context_uuid_parses_matching_string_uuid() {
        let id = Uuid::new_v4();
        let context = json!({
            "assistant_run_id": id.to_string(),
            "other_id": Uuid::new_v4().to_string()
        });

        assert_eq!(
            workflow_context_uuid(&context, "assistant_run_id"),
            Some(id)
        );
    }

    #[test]
    fn workflow_context_uuid_ignores_missing_non_string_and_invalid_values() {
        let context = json!({
            "numeric_id": 123,
            "object_id": {"id": Uuid::new_v4().to_string()},
            "invalid_id": "not-a-uuid"
        });

        assert_eq!(workflow_context_uuid(&context, "missing_id"), None);
        assert_eq!(workflow_context_uuid(&context, "numeric_id"), None);
        assert_eq!(workflow_context_uuid(&context, "object_id"), None);
        assert_eq!(workflow_context_uuid(&context, "invalid_id"), None);
    }
}
