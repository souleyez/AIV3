use serde_json::{Map, Value};

pub(crate) fn external_channel_static_page_report_title_is_generic(title: &str) -> bool {
    matches!(
        title.trim(),
        "" | "static_page_image2_data_publish" | "DataMax 静态页" | "DataMax 经营分析报表"
    )
}

pub(crate) fn external_channel_static_page_card_value_missing(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => true,
        Some(Value::String(value)) => value.trim().is_empty(),
        Some(Value::Array(items)) => items.is_empty(),
        _ => false,
    }
}

pub(crate) fn external_channel_static_page_card_title_missing_or_generic(
    value: Option<&Value>,
) -> bool {
    value
        .and_then(Value::as_str)
        .map(external_channel_static_page_report_title_is_generic)
        .unwrap_or(true)
}

pub(crate) fn external_channel_static_page_card_insert_if_missing(
    object: &mut Map<String, Value>,
    key: &str,
    value: Value,
) {
    if external_channel_static_page_card_value_missing(object.get(key)) {
        object.insert(key.to_string(), value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn report_title_generic_detection_matches_existing_defaults() {
        for title in [
            "",
            "   ",
            "static_page_image2_data_publish",
            "DataMax 静态页",
            "DataMax 经营分析报表",
        ] {
            assert!(external_channel_static_page_report_title_is_generic(title));
        }
        assert!(!external_channel_static_page_report_title_is_generic(
            "新世界百货经营管理月报表"
        ));
    }

    #[test]
    fn card_missing_checks_match_null_empty_string_and_empty_array_only() {
        assert!(external_channel_static_page_card_value_missing(None));
        assert!(external_channel_static_page_card_value_missing(Some(
            &Value::Null
        )));
        assert!(external_channel_static_page_card_value_missing(Some(
            &json!("   ")
        )));
        assert!(external_channel_static_page_card_value_missing(Some(
            &json!([])
        )));
        assert!(!external_channel_static_page_card_value_missing(Some(
            &json!("value")
        )));
        assert!(!external_channel_static_page_card_value_missing(Some(
            &json!([null])
        )));
        assert!(!external_channel_static_page_card_value_missing(Some(
            &json!(0)
        )));
    }

    #[test]
    fn card_insert_overwrites_missing_but_keeps_existing_values() {
        let mut object = Map::new();
        external_channel_static_page_card_insert_if_missing(
            &mut object,
            "data_url",
            json!("https://v3.elepcloud.com/generated-artifacts/a/data.json"),
        );
        assert_eq!(
            object["data_url"],
            json!("https://v3.elepcloud.com/generated-artifacts/a/data.json")
        );

        external_channel_static_page_card_insert_if_missing(
            &mut object,
            "data_url",
            json!("https://v3.elepcloud.com/generated-artifacts/a/other.json"),
        );
        assert_eq!(
            object["data_url"],
            json!("https://v3.elepcloud.com/generated-artifacts/a/data.json")
        );

        object.insert("empty".to_string(), json!(""));
        external_channel_static_page_card_insert_if_missing(&mut object, "empty", json!("filled"));
        assert_eq!(object["empty"], json!("filled"));
    }

    #[test]
    fn card_title_missing_includes_generic_titles() {
        assert!(external_channel_static_page_card_title_missing_or_generic(
            None
        ));
        assert!(external_channel_static_page_card_title_missing_or_generic(
            Some(&json!("DataMax 经营分析报表"))
        ));
        assert!(!external_channel_static_page_card_title_missing_or_generic(
            Some(&json!("新世界百货经营管理月报表"))
        ));
        assert!(external_channel_static_page_card_title_missing_or_generic(
            Some(&json!(123))
        ));
    }
}
