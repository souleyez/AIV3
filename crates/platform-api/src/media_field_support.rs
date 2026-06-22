use serde_json::Value;

pub(crate) fn media_string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    })
}

pub(crate) fn media_numeric_field(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(|item| {
            item.as_f64()
                .or_else(|| item.as_str().and_then(|text| text.parse::<f64>().ok()))
        })
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn media_string_field_trims_first_non_empty_alias() {
        let value = json!({
            "summary": "  场景摘要  ",
            "label": "备用"
        });

        assert_eq!(
            media_string_field(&value, &["missing", "summary", "label"]),
            Some("场景摘要".to_string())
        );
    }

    #[test]
    fn media_string_field_skips_empty_values() {
        let value = json!({
            "summary": "   ",
            "label": "关键帧"
        });

        assert_eq!(
            media_string_field(&value, &["summary", "label"]),
            Some("关键帧".to_string())
        );
    }

    #[test]
    fn media_numeric_field_accepts_number_and_numeric_string() {
        let numeric = json!({ "start": 12.5 });
        let numeric_string = json!({ "timestamp": "7.25" });

        assert_eq!(media_numeric_field(&numeric, &["start"]), Some(12.5));
        assert_eq!(
            media_numeric_field(&numeric_string, &["timestamp"]),
            Some(7.25)
        );
    }

    #[test]
    fn media_numeric_field_returns_none_for_invalid_values() {
        let value = json!({
            "timestamp": "not-a-number",
            "end": null
        });

        assert_eq!(media_numeric_field(&value, &["timestamp", "end"]), None);
    }
}
