use std::collections::BTreeSet;

use serde_json::{Map, Value};

use crate::semantic_understanding::{
    stable_semantic_id, SemanticEvidenceRef, SemanticObservation, SemanticSourceIdentity,
    SemanticStatus, MAX_EVIDENCE_REFS, MAX_EXAMPLES,
};

#[derive(Clone, Debug)]
pub struct SemanticProfileInput {
    pub source_id: String,
    pub source_kind: String,
    pub title: String,
    pub metadata: Value,
    pub facts: Vec<Value>,
    pub evidence_labels: Vec<String>,
}

pub fn adapt_semantic_profile(input: &SemanticProfileInput) -> Vec<SemanticObservation> {
    let source_kind = input.source_kind.trim().to_ascii_lowercase();
    let mut observations = match source_kind.as_str() {
        "database" | "database_table" => adapt_database(input),
        "spreadsheet" | "excel" | "csv" | "spreadsheet_table" => adapt_spreadsheet(input),
        "document" | "pdf" | "word" | "text" | "document_section" => adapt_document(input),
        "asset" | "image" | "asset_profile" => adapt_asset(input),
        "media" | "audio" | "video" | "presentation" | "ppt" => adapt_media(input),
        "web" | "api" | "web_api" | "api_resource" => adapt_web_api(input),
        _ => Vec::new(),
    };

    if observations.is_empty() {
        observations.push(observation(
            input,
            "unresolved",
            "unknown_source",
            &input.source_id,
            &input.title,
            None,
            "unresolved_raw",
            SemanticStatus::Unresolved,
            0.0,
            Value::Object(Map::new()),
        ));
    }
    observations.sort_by(|left, right| left.id.cmp(&right.id));
    observations.dedup_by(|left, right| left.id == right.id);
    observations
}

fn adapt_database(input: &SemanticProfileInput) -> Vec<SemanticObservation> {
    let parse = input
        .metadata
        .get("parse_metadata")
        .or_else(|| input.metadata.get("external_metadata"))
        .unwrap_or(&input.metadata);
    let table = string_at(parse, "source_table").unwrap_or_else(|| input.title.clone());
    let table_comment = string_at(parse, "table_comment");
    let (label_source, status, confidence) = if table_comment.is_some() {
        ("source_comment", SemanticStatus::Confirmed, 1.0)
    } else {
        ("unresolved_source_object", SemanticStatus::Unresolved, 0.0)
    };
    let mut output = vec![observation(
        input,
        "object",
        "database_table",
        &table,
        &table,
        table_comment,
        label_source,
        status,
        confidence,
        json_subset(parse, &["schema", "table_comment"]),
    )];

    let primary_keys = string_array_at(parse, "primary_key");
    let comments = object_at(parse, "field_comments");
    if let Some(fields) = object_at(parse, "fields") {
        for key in sorted_keys(fields) {
            let comment = comments
                .and_then(|values| values.get(key))
                .and_then(value_string);
            let mut attributes = Map::new();
            attributes.insert(
                "primary_key".to_string(),
                Value::Bool(primary_keys.iter().any(|item| item == key)),
            );
            add_field_observation(
                &mut output,
                input,
                "database_table",
                &table,
                key,
                comment,
                fields.get(key),
                attributes,
            );
        }
    }
    for (index, foreign_key) in array_at(parse, "foreign_keys").iter().enumerate() {
        let technical_name =
            string_at(foreign_key, "field").unwrap_or_else(|| format!("foreign_key_{index}"));
        output.push(observation(
            input,
            "constraint",
            "database_table",
            &table,
            &technical_name,
            string_at(foreign_key, "references"),
            "source_constraint",
            SemanticStatus::Confirmed,
            1.0,
            foreign_key.clone(),
        ));
    }
    output
}

fn adapt_spreadsheet(input: &SemanticProfileInput) -> Vec<SemanticObservation> {
    let sheet = string_at(&input.metadata, "sheet_name")
        .unwrap_or_else(|| spreadsheet_business_title(&input.title));
    let mut output = vec![observation(
        input,
        "object",
        "spreadsheet_table",
        &sheet,
        &sheet,
        Some(sheet.clone()),
        "source_metadata",
        SemanticStatus::Observed,
        0.9,
        json_subset(&input.metadata, &["workbook", "sheet_name"]),
    )];
    let value_types = object_at(&input.metadata, "value_types");
    let formulas = object_at(&input.metadata, "formulas");
    let candidates = string_array_at(&input.metadata, "candidate_keys");
    for header in string_array_at(&input.metadata, "headers") {
        let mut attributes = Map::new();
        if let Some(value_type) = value_types.and_then(|values| values.get(&header)).cloned() {
            attributes.insert("value_type".to_string(), value_type);
        }
        if let Some(formula) = formulas.and_then(|values| values.get(&header)).cloned() {
            attributes.insert("formula".to_string(), formula);
        }
        attributes.insert(
            "candidate_key".to_string(),
            Value::Bool(candidates.contains(&header)),
        );
        add_field_observation(
            &mut output,
            input,
            "spreadsheet_table",
            &sheet,
            &header,
            None,
            None,
            attributes,
        );
    }
    output
}

fn adapt_document(input: &SemanticProfileInput) -> Vec<SemanticObservation> {
    let object_key = input.source_id.clone();
    let display_title = business_source_title(&input.title);
    let mut output = vec![observation(
        input,
        "object",
        "document_section",
        &object_key,
        &input.title,
        Some(display_title),
        "source_title",
        SemanticStatus::Observed,
        0.9,
        Value::Object(Map::new()),
    )];
    for (kind, key) in [
        ("section", "sections"),
        ("table", "tables"),
        ("entity", "entities"),
    ] {
        for value in string_array_at(&input.metadata, key) {
            output.push(observation(
                input,
                kind,
                "document_section",
                &object_key,
                &value,
                Some(value.clone()),
                "parsed_structure",
                SemanticStatus::Observed,
                0.85,
                Value::Object(Map::new()),
            ));
        }
    }
    for (index, fact) in input.facts.iter().enumerate() {
        let name = string_at(fact, "name").unwrap_or_else(|| format!("fact_{index}"));
        output.push(observation(
            input,
            "fact",
            "document_section",
            &object_key,
            &name,
            Some(name.clone()),
            "document_fact",
            SemanticStatus::Confirmed,
            1.0,
            fact.clone(),
        ));
    }
    output
}

fn adapt_asset(input: &SemanticProfileInput) -> Vec<SemanticObservation> {
    let object_key =
        string_at(&input.metadata, "profile_kind").unwrap_or_else(|| input.source_id.clone());
    let mut output = vec![observation(
        input,
        "object",
        "asset_profile",
        &object_key,
        &input.title,
        Some(business_source_title(&input.title)),
        "asset_profile",
        SemanticStatus::Observed,
        0.9,
        json_subset(&input.metadata, &["profile_kind"]),
    )];
    if let Some(profile) = input.metadata.get("safe_profile") {
        for (kind, key) in [("ocr", "ocr"), ("label", "labels")] {
            for value in string_array_at(profile, key) {
                output.push(observation(
                    input,
                    kind,
                    "asset_profile",
                    &object_key,
                    &value,
                    Some(value.clone()),
                    "asset_profile",
                    SemanticStatus::Observed,
                    0.85,
                    Value::Object(Map::new()),
                ));
            }
        }
    }
    if let Some(collection) = string_at(&input.metadata, "collection") {
        output.push(observation(
            input,
            "collection",
            "asset_profile",
            &object_key,
            &collection,
            Some(collection.clone()),
            "collection_membership",
            SemanticStatus::Observed,
            1.0,
            Value::Object(Map::new()),
        ));
    }
    output
}

fn adapt_media(input: &SemanticProfileInput) -> Vec<SemanticObservation> {
    let object_key = input.source_id.clone();
    let display_title = business_source_title(&input.title);
    let mut output = vec![observation(
        input,
        "object",
        "media_segment",
        &object_key,
        &input.title,
        Some(display_title),
        "source_title",
        SemanticStatus::Observed,
        0.9,
        Value::Object(Map::new()),
    )];
    for (index, page) in array_at(&input.metadata, "pages").iter().enumerate() {
        let title = string_at(page, "title").unwrap_or_else(|| format!("page_{}", index + 1));
        output.push(observation(
            input,
            "page",
            "media_segment",
            &object_key,
            &title,
            Some(title.clone()),
            "parsed_structure",
            SemanticStatus::Observed,
            0.85,
            page.clone(),
        ));
    }
    for (index, segment) in array_at(&input.metadata, "segments").iter().enumerate() {
        let name =
            string_at(segment, "speaker").unwrap_or_else(|| format!("segment_{}", index + 1));
        output.push(observation(
            input,
            "segment",
            "media_segment",
            &object_key,
            &name,
            Some(name.clone()),
            "parsed_structure",
            SemanticStatus::Observed,
            0.85,
            segment.clone(),
        ));
    }
    for label in string_array_at(&input.metadata, "visual_labels") {
        output.push(observation(
            input,
            "visual_label",
            "media_segment",
            &object_key,
            &label,
            Some(label.clone()),
            "parsed_structure",
            SemanticStatus::Observed,
            0.8,
            Value::Object(Map::new()),
        ));
    }
    output
}

fn business_source_title(title: &str) -> String {
    let trimmed = title.trim();
    let lower = trimmed.to_ascii_lowercase();
    for suffix in [
        ".tar.gz", ".docx", ".xlsx", ".pptx", ".pdf", ".doc", ".xls", ".ppt", ".csv", ".json",
        ".html", ".htm", ".txt", ".md", ".zip", ".mp4", ".mp3", ".wav",
    ] {
        if lower.ends_with(suffix) {
            let candidate = trimmed[..trimmed.len() - suffix.len()].trim();
            if !candidate.is_empty() {
                return candidate.to_string();
            }
        }
    }
    trimmed.to_string()
}

fn spreadsheet_business_title(title: &str) -> String {
    let trimmed = title.trim();
    let delimiter_count = trimmed
        .chars()
        .filter(|character| matches!(character, ',' | '\t' | '|'))
        .count();
    if trimmed.chars().count() > 80 || delimiter_count >= 4 {
        "表格数据".to_string()
    } else {
        business_source_title(trimmed)
    }
}

fn adapt_web_api(input: &SemanticProfileInput) -> Vec<SemanticObservation> {
    let resource =
        string_at(&input.metadata, "resource_type").unwrap_or_else(|| input.source_id.clone());
    let label = string_at(&input.metadata, "title").unwrap_or_else(|| input.title.clone());
    let mut output = vec![observation(
        input,
        "object",
        "api_resource",
        &resource,
        &resource,
        Some(label),
        "source_metadata",
        SemanticStatus::Observed,
        0.9,
        json_subset(&input.metadata, &["parent"]),
    )];
    for field in string_array_at(&input.metadata, "fields") {
        add_field_observation(
            &mut output,
            input,
            "api_resource",
            &resource,
            &field,
            None,
            None,
            Map::new(),
        );
    }
    for link in string_array_at(&input.metadata, "links") {
        if let Some(safe_link) = safe_web_reference(&link) {
            output.push(observation(
                input,
                "reference",
                "api_resource",
                &resource,
                &safe_link,
                Some("资源引用".to_string()),
                "source_link",
                SemanticStatus::Confirmed,
                1.0,
                Value::Object(Map::new()),
            ));
        }
    }
    output
}

#[allow(clippy::too_many_arguments)]
fn observation(
    input: &SemanticProfileInput,
    observation_kind: &str,
    object_kind: &str,
    object_key: &str,
    technical_name: &str,
    label_hint: Option<String>,
    label_source: &str,
    status: SemanticStatus,
    confidence: f64,
    attributes: Value,
) -> SemanticObservation {
    SemanticObservation {
        id: stable_semantic_id(
            "observation",
            &[
                &input.source_id,
                observation_kind,
                object_kind,
                object_key,
                technical_name,
            ],
        ),
        source_kind: input.source_kind.clone(),
        source_id: input.source_id.clone(),
        source_identity: semantic_source_identity(input, object_kind, object_key),
        observation_kind: observation_kind.to_string(),
        object_kind: object_kind.to_string(),
        object_key: object_key.to_string(),
        technical_name: technical_name.to_string(),
        label_hint,
        label_source: label_source.to_string(),
        value_type: None,
        semantic_role_hint: None,
        observed_values: Vec::new(),
        attributes,
        status,
        confidence,
        evidence_refs: evidence_refs(input),
    }
}

fn semantic_source_identity(
    input: &SemanticProfileInput,
    object_kind: &str,
    object_key: &str,
) -> Option<SemanticSourceIdentity> {
    if object_kind != "database_table" {
        return None;
    }
    let parse = input
        .metadata
        .get("parse_metadata")
        .or_else(|| input.metadata.get("external_metadata"))
        .unwrap_or(&input.metadata);
    let source_system_key = string_at_any(
        parse,
        &["source_system_key", "source_system", "connection_key"],
    )
    .or_else(|| {
        input
            .metadata
            .get("external_source")
            .and_then(|value| string_at_any(value, &["source_id", "source_system_key"]))
    })
    .unwrap_or_else(|| input.source_id.trim().to_string());
    let mut source_schema_key =
        string_at_any(parse, &["source_schema", "schema"]).unwrap_or_default();
    let mut source_object_key = object_key.trim().to_string();
    if source_schema_key.is_empty() {
        if let Some((schema, object)) = source_object_key.rsplit_once('.') {
            if !schema.trim().is_empty() && !object.trim().is_empty() {
                source_schema_key = schema.trim().to_string();
                source_object_key = object.trim().to_string();
            }
        }
    } else if source_object_key
        .to_ascii_lowercase()
        .starts_with(&format!("{}.", source_schema_key.to_ascii_lowercase()))
    {
        source_object_key = source_object_key[source_schema_key.len() + 1..]
            .trim()
            .to_string();
    }
    Some(SemanticSourceIdentity {
        source_system_key,
        source_schema_key,
        source_object_key,
    })
}

#[allow(clippy::too_many_arguments)]
fn add_field_observation(
    output: &mut Vec<SemanticObservation>,
    input: &SemanticProfileInput,
    object_kind: &str,
    object_key: &str,
    technical_name: &str,
    label_hint: Option<String>,
    observed_value: Option<&Value>,
    attributes: Map<String, Value>,
) {
    let label_source = if label_hint.is_some() {
        "source_comment"
    } else {
        "parsed_structure"
    };
    let mut item = observation(
        input,
        "field",
        object_kind,
        object_key,
        technical_name,
        label_hint,
        label_source,
        SemanticStatus::Observed,
        0.85,
        Value::Object(attributes),
    );
    item.observed_values = observed_value
        .and_then(value_string)
        .into_iter()
        .take(MAX_EXAMPLES)
        .collect();
    item.value_type = observed_value.map(value_type_name).map(str::to_string);
    output.push(item);
}

fn evidence_refs(input: &SemanticProfileInput) -> Vec<SemanticEvidenceRef> {
    let mut labels = input
        .evidence_labels
        .iter()
        .map(|label| label.trim())
        .filter(|label| !label.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    if labels.is_empty() {
        labels.insert("来源结构".to_string());
    }
    labels
        .into_iter()
        .take(MAX_EVIDENCE_REFS)
        .map(|label| SemanticEvidenceRef {
            source_kind: input.source_kind.clone(),
            source_id: input.source_id.clone(),
            label,
        })
        .collect()
}

fn object_at<'a>(value: &'a Value, key: &str) -> Option<&'a Map<String, Value>> {
    value.get(key)?.as_object()
}

fn array_at<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn string_at(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(value_string)
}

fn string_at_any(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| string_at(value, key))
}

fn string_array_at(value: &Value, key: &str) -> Vec<String> {
    array_at(value, key)
        .iter()
        .filter_map(value_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn value_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.trim().to_string()).filter(|value| !value.is_empty()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "text",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn sorted_keys(values: &Map<String, Value>) -> Vec<&str> {
    let mut keys = values.keys().map(String::as_str).collect::<Vec<_>>();
    keys.sort_unstable();
    keys
}

fn json_subset(value: &Value, keys: &[&str]) -> Value {
    let mut subset = Map::new();
    for key in keys {
        if let Some(item) = value.get(*key) {
            subset.insert((*key).to_string(), item.clone());
        }
    }
    Value::Object(subset)
}

fn safe_web_reference(value: &str) -> Option<String> {
    let value = value.trim();
    if !(value.starts_with("https://") || value.starts_with("http://")) {
        return None;
    }
    Some(value.split(['?', '#']).next().unwrap_or(value).to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::semantic_understanding::SemanticStatus;

    fn input(source_kind: &str, metadata: serde_json::Value) -> SemanticProfileInput {
        SemanticProfileInput {
            source_id: format!("fixture:{source_kind}"),
            source_kind: source_kind.to_string(),
            title: format!("{source_kind} fixture"),
            metadata,
            facts: Vec::new(),
            evidence_labels: vec!["fixture evidence".to_string()],
        }
    }

    #[test]
    fn database_adapter_reads_table_fields_primary_key_and_constraints() {
        let observations = adapt_semantic_profile(&input(
            "database",
            json!({"parse_metadata": {
                "source_table": "lease_contract",
                "primary_key": ["contract_id"],
                "fields": {"contract_id": "C-001", "rent_amount": 1200},
                "field_comments": {"rent_amount": "租金金额"},
                "foreign_keys": [{"field": "store_id", "references": "store.id"}]
            }}),
        ));

        assert_observations(&observations, "database_table");
        assert!(observations
            .iter()
            .any(|item| item.technical_name == "contract_id"));
        assert!(observations
            .iter()
            .any(|item| item.observation_kind == "constraint"));
    }

    #[test]
    fn database_adapter_preserves_internal_source_identity_without_serializing_it() {
        let observations = adapt_semantic_profile(&input(
            "database",
            json!({"parse_metadata": {
                "source_system_key": "oracle-badw-internal",
                "schema": "finance_private",
                "source_table": "lease_contract",
                "fields": {"contract_id": "C-001"}
            }}),
        ));
        let object = observations
            .iter()
            .find(|item| item.observation_kind == "object")
            .expect("database object observation");
        let identity = object
            .source_identity
            .as_ref()
            .expect("database source identity");

        assert_eq!(identity.source_system_key, "oracle-badw-internal");
        assert_eq!(identity.source_schema_key, "finance_private");
        assert_eq!(identity.source_object_key, "lease_contract");

        let public_json = serde_json::to_string(object).expect("serializable observation");
        assert!(!public_json.contains("oracle-badw-internal"));
        assert!(!public_json.contains("source_identity"));
    }

    #[test]
    fn spreadsheet_adapter_reads_sheet_headers_formulas_and_key_candidates() {
        let observations = adapt_semantic_profile(&input(
            "spreadsheet",
            json!({
                "sheet_name": "经营分析",
                "headers": ["门店编号", "销售额"],
                "value_types": {"销售额": "number"},
                "formulas": {"销售额": "=SUM(B2:B9)"},
                "candidate_keys": ["门店编号"]
            }),
        ));

        assert_observations(&observations, "spreadsheet_table");
        assert!(observations
            .iter()
            .any(|item| item.technical_name == "销售额"));
    }

    #[test]
    fn spreadsheet_row_like_titles_do_not_become_business_object_labels() {
        let mut fixture = input("spreadsheet", json!({}));
        fixture.title =
            "2026-03-31,Douyin,华东仓,智能穿戴,旗舰手表X1,180,12,168,117,111,154.3".to_string();
        let observations = adapt_semantic_profile(&fixture);
        let object = observations
            .iter()
            .find(|item| item.observation_kind == "object")
            .expect("spreadsheet object");

        assert_eq!(object.label_hint.as_deref(), Some("表格数据"));
        assert_eq!(spreadsheet_business_title("经营分析.xlsx"), "经营分析");
    }

    #[test]
    fn document_adapter_reads_sections_tables_entities_facts_and_evidence() {
        let mut fixture = input(
            "document",
            json!({"sections": ["合同范围", "付款条款"], "tables": ["租金表"], "entities": ["新百"]}),
        );
        fixture.facts = vec![json!({"name": "合同编号", "value_type": "text"})];
        let observations = adapt_semantic_profile(&fixture);

        assert_observations(&observations, "document_section");
        assert!(observations
            .iter()
            .any(|item| item.observation_kind == "fact"));
    }

    #[test]
    fn source_file_extensions_are_removed_from_business_labels_only() {
        let mut fixture = input("document", json!({}));
        fixture.title = "新百项目.zip".to_string();
        let observations = adapt_semantic_profile(&fixture);
        let object = observations
            .iter()
            .find(|item| item.observation_kind == "object")
            .expect("document object");

        assert_eq!(object.label_hint.as_deref(), Some("新百项目"));
        assert_eq!(object.technical_name, "新百项目.zip");
        assert_eq!(business_source_title("周报.DOCX"), "周报");
        assert_eq!(business_source_title("无扩展名"), "无扩展名");
    }

    #[test]
    fn asset_adapter_reads_safe_profile_ocr_labels_and_collection() {
        let observations = adapt_semantic_profile(&input(
            "asset",
            json!({
                "profile_kind": "image_profile",
                "safe_profile": {"ocr": ["春季陈列"], "labels": ["服装", "门店"]},
                "collection": "商品图片"
            }),
        ));

        assert_observations(&observations, "asset_profile");
        assert!(observations
            .iter()
            .any(|item| item.observation_kind == "collection"));
    }

    #[test]
    fn media_adapter_reads_pages_segments_speakers_and_visual_labels() {
        let observations = adapt_semantic_profile(&input(
            "media",
            json!({
                "pages": [{"title": "经营摘要"}],
                "segments": [{"start_ms": 0, "speaker": "主持人", "transcript": "销售回顾"}],
                "visual_labels": ["趋势图"]
            }),
        ));

        assert_observations(&observations, "media_segment");
        assert!(observations
            .iter()
            .any(|item| item.observation_kind == "segment"));
    }

    #[test]
    fn web_api_adapter_reads_resource_hierarchy_fields_and_safe_links() {
        let observations = adapt_semantic_profile(&input(
            "web_api",
            json!({
                "resource_type": "store",
                "title": "门店资源",
                "parent": "retail",
                "fields": ["store_id", "name"],
                "links": ["https://example.test/stores"]
            }),
        ));

        assert_observations(&observations, "api_resource");
        assert!(observations
            .iter()
            .any(|item| item.observation_kind == "reference"));
    }

    #[test]
    fn unknown_or_incomplete_source_is_unresolved_instead_of_failing() {
        let observations = adapt_semantic_profile(&input("unknown", json!({})));

        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].status, SemanticStatus::Unresolved);
        assert!(!observations[0].evidence_refs.is_empty());
    }

    fn assert_observations(observations: &[SemanticObservation], expected_kind: &str) {
        assert!(!observations.is_empty());
        assert!(observations
            .iter()
            .any(|item| item.object_kind == expected_kind));
        assert!(observations.iter().all(|item| {
            !item.id.is_empty()
                && !item.label_source.is_empty()
                && item.confidence >= 0.0
                && !item.evidence_refs.is_empty()
        }));
    }
}
