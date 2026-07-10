use contracts::FashionDesignImageProfileV1;
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub(crate) const FASHION_DESIGN_IMAGE_PROFILE_KIND: &str =
    contracts::FASHION_DESIGN_IMAGE_PROFILE_SCHEMA;
pub(crate) const FASHION_DESIGN_IMAGE_PARSER_TASK_KIND: &str = "fashion_design_image_ocr_vlm";
pub(crate) const FASHION_DESIGN_IMAGE_PARSER_NAME: &str = "datamax-fashion-image-parser";
pub(crate) const FASHION_DESIGN_IMAGE_PARSER_VERSION: &str = "2026-06-17";

const LIST_LIMIT: usize = 24;
const TEXT_LIMIT: usize = 160;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FashionDesignImageParserWorkerInput {
    pub source_ref_present: bool,
    pub source_kind: Option<&'static str>,
    pub content_type: Option<String>,
    pub profile_seeded: bool,
}

pub(crate) fn fashion_design_image_parser_worker_input_contract(
    input: &FashionDesignImageParserWorkerInput,
) -> Value {
    json!({
        "task_kind": FASHION_DESIGN_IMAGE_PARSER_TASK_KIND,
        "parser_name": FASHION_DESIGN_IMAGE_PARSER_NAME,
        "parser_version": FASHION_DESIGN_IMAGE_PARSER_VERSION,
        "profile_schema": FASHION_DESIGN_IMAGE_PROFILE_KIND,
        "source_ref_present": input.source_ref_present,
        "source_kind": input.source_kind,
        "content_type": input.content_type.as_deref(),
        "profile_seeded": input.profile_seeded,
        "requested_outputs": [
            "fashion_design_image_v1",
            "retrieval_evidence_text"
        ],
    })
}

pub(crate) fn fashion_design_image_parser_worker_output_contract(payload: &Value) -> Value {
    let attributes = fashion_postchain_profile_attributes(payload);
    let profile_ready = attributes
        .as_object()
        .is_some_and(|object| !object.is_empty());
    json!({
        "status": if profile_ready { "completed" } else { "partial_completed" },
        "parser_name": FASHION_DESIGN_IMAGE_PARSER_NAME,
        "parser_version": FASHION_DESIGN_IMAGE_PARSER_VERSION,
        "profile_schema": FASHION_DESIGN_IMAGE_PROFILE_KIND,
        "profile_payload": attributes,
        "retrieval_evidence": {
            "source_kind": "asset_profile",
            "materialization": "profile_to_text",
            "ready": profile_ready,
        },
    })
}

pub(crate) fn fashion_design_image_parser_status_transition_dry_run(
    current_status: &str,
    worker_output_contract: &Value,
) -> Value {
    let current_status = normalize_parser_status(current_status);
    let retrieval_ready = worker_output_contract
        .pointer("/retrieval_evidence/ready")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let worker_status = worker_output_contract
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("partial_completed");
    let next_status = if current_status == "completed" {
        "completed"
    } else if retrieval_ready {
        "completed"
    } else {
        "retrying"
    };
    let action = if current_status == "completed" {
        "keep_completed"
    } else if retrieval_ready {
        "mark_completed_and_materialize_retrieval_evidence"
    } else {
        "mark_retrying_wait_for_profile"
    };

    json!({
        "task_kind": FASHION_DESIGN_IMAGE_PARSER_TASK_KIND,
        "parser_name": FASHION_DESIGN_IMAGE_PARSER_NAME,
        "parser_version": FASHION_DESIGN_IMAGE_PARSER_VERSION,
        "profile_schema": FASHION_DESIGN_IMAGE_PROFILE_KIND,
        "current_status": current_status,
        "worker_status": normalize_parser_status(worker_status),
        "next_status": next_status,
        "transition_action": action,
        "should_write_profile": retrieval_ready,
        "should_materialize_retrieval_evidence": retrieval_ready,
        "dry_run_only": true,
    })
}

pub(crate) fn fashion_design_image_parser_live_adapter_readiness_dry_run(
    worker_input_contract: &Value,
    worker_output_contract: &Value,
) -> Value {
    let input_contract_ok = fashion_parser_worker_input_contract_ok(worker_input_contract);
    let output_contract_ok = fashion_parser_worker_output_contract_ok(worker_output_contract);
    let worker_status = worker_output_contract
        .get("status")
        .and_then(Value::as_str)
        .map(normalize_parser_status)
        .unwrap_or("unknown");
    let worker_output_accepted = matches!(worker_status, "completed" | "partial_completed");
    let retrieval_ready = worker_output_contract
        .pointer("/retrieval_evidence/ready")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let profile_payload_ready = worker_output_contract
        .get("profile_payload")
        .and_then(Value::as_object)
        .is_some_and(|payload| !payload.is_empty());
    let sensitive_material_excluded = !contains_forbidden_contract_material(worker_input_contract)
        && !contains_forbidden_contract_material(worker_output_contract);
    let adapter_ready = input_contract_ok
        && output_contract_ok
        && worker_output_accepted
        && sensitive_material_excluded;
    let adapter_status = if !input_contract_ok {
        "blocked_worker_input_contract"
    } else if !output_contract_ok {
        "blocked_worker_output_contract"
    } else if !worker_output_accepted {
        "blocked_worker_status"
    } else if !sensitive_material_excluded {
        "blocked_sensitive_material"
    } else if retrieval_ready {
        "ready_for_profile_and_retrieval_commit"
    } else {
        "ready_for_partial_retry_adapter"
    };
    let next_adapter_action = if !adapter_ready {
        "review_blocked_adapter_contract"
    } else if retrieval_ready {
        "controlled_live_adapter_commit_review"
    } else {
        "controlled_live_adapter_retry_review"
    };

    json!({
        "contract": "fashion_design_image_parser_live_adapter_readiness_dry_run_v1",
        "mode": "dry_run",
        "task_kind": FASHION_DESIGN_IMAGE_PARSER_TASK_KIND,
        "parser_name": FASHION_DESIGN_IMAGE_PARSER_NAME,
        "parser_version": FASHION_DESIGN_IMAGE_PARSER_VERSION,
        "profile_schema": FASHION_DESIGN_IMAGE_PROFILE_KIND,
        "adapter_status": adapter_status,
        "adapter_ready": adapter_ready,
        "no_network": true,
        "no_model_call": true,
        "no_write": true,
        "no_callback": true,
        "production_write_allowed": false,
        "live_execute_still_requires_worker_endpoint": true,
        "worker_endpoint_value_included": false,
        "worker_request_payload_included": false,
        "worker_response_payload_included": false,
        "validation": {
            "worker_input_contract_ok": input_contract_ok,
            "worker_output_contract_ok": output_contract_ok,
            "worker_output_accepted": worker_output_accepted,
            "worker_status": worker_status,
            "profile_payload_ready": profile_payload_ready,
            "retrieval_evidence_ready": retrieval_ready,
            "sensitive_material_excluded": sensitive_material_excluded,
        },
        "operator_next_step": {
            "action": next_adapter_action,
            "worker_endpoint_required": true,
            "worker_endpoint_value_included": false,
            "operator_review_required": true,
        },
        "redaction": {
            "provider_raw_material_excluded": true,
            "provider_payload_excluded": true,
            "raw_object_locator_excluded": true,
            "source_url_value_included": false,
            "local_path_value_included": false,
            "secret_material_included": false,
        },
    })
}

pub(crate) fn fashion_postchain_profile_attributes(payload: &Value) -> Value {
    let profile = fashion_postchain_profile_from_payload(payload);
    serde_json::to_value(profile).unwrap_or_else(|_| json!({}))
}

pub(crate) fn fashion_postchain_profile_from_payload(
    payload: &Value,
) -> FashionDesignImageProfileV1 {
    FashionDesignImageProfileV1 {
        category: first_text(payload, &["category", "品类"]),
        audience: collect_texts(
            payload,
            &["audience", "gender", "target_audience", "受众", "性别"],
        ),
        seasons: collect_texts(payload, &["seasons", "season", "季节"]),
        styles: collect_texts(payload, &["styles", "style", "风格"]),
        silhouettes: collect_texts(
            payload,
            &["silhouettes", "silhouette", "shape", "版型", "廓形"],
        ),
        collars: collect_texts(payload, &["collars", "collar", "领型"]),
        sleeves: collect_texts(payload, &["sleeves", "sleeve", "袖型"]),
        waists: collect_texts(payload, &["waists", "waist", "腰型", "腰线"]),
        hems: collect_texts(payload, &["hems", "hem", "length", "下摆", "衣长"]),
        materials: collect_texts(
            payload,
            &["materials", "material", "fabric", "fabrics", "面料", "材质"],
        ),
        crafts: collect_texts(
            payload,
            &["crafts", "craft", "process", "processes", "工艺"],
        ),
        colors: collect_texts(payload, &["colors", "color", "色彩", "颜色"]),
        patterns: collect_texts(
            payload,
            &["patterns", "pattern", "prints", "print", "图案", "印花"],
        ),
        scenes: collect_texts(
            payload,
            &["scenes", "scene", "occasion", "occasions", "场景"],
        ),
        sku_text_marks: collect_texts(
            payload,
            &[
                "sku_text_marks",
                "skuTextMarks",
                "sku",
                "skus",
                "text_marks",
                "文字标记",
            ],
        ),
        visible_text: collect_texts(
            payload,
            &[
                "visible_text",
                "visibleText",
                "ocr_text",
                "ocrText",
                "transcribedText",
                "可见文字",
            ],
        ),
        tags: collect_texts(payload, &["tags", "keywords", "labels", "标签", "关键词"]),
        noun_terms: collect_texts(
            payload,
            &["noun_terms", "nounTermHints", "objects", "entities", "名词"],
        ),
        caption: first_text(
            payload,
            &[
                "caption",
                "summary",
                "description",
                "visualSummary",
                "视觉摘要",
            ],
        ),
        confidence: confidence(payload),
    }
}

fn first_text(payload: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(text) = normalized_text_values(payload.get(*key)).into_iter().next() {
            return Some(text);
        }
    }
    None
}

fn collect_texts(payload: &Value, keys: &[&str]) -> Vec<String> {
    let mut values = BTreeSet::new();
    for key in keys {
        for text in normalized_text_values(payload.get(*key)) {
            values.insert(text);
        }
    }
    values.into_iter().take(LIST_LIMIT).collect()
}

fn normalized_text_values(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::String(text)) => normalize_text(text).into_iter().collect(),
        Some(Value::Array(values)) => values
            .iter()
            .flat_map(|value| match value {
                Value::String(text) => normalize_text(text).into_iter().collect::<Vec<_>>(),
                Value::Number(number) => normalize_text(&number.to_string())
                    .into_iter()
                    .collect::<Vec<_>>(),
                Value::Object(object) => object
                    .get("name")
                    .or_else(|| object.get("label"))
                    .and_then(Value::as_str)
                    .and_then(normalize_text)
                    .into_iter()
                    .collect(),
                _ => Vec::new(),
            })
            .take(LIST_LIMIT)
            .collect(),
        Some(Value::Object(object)) => object
            .get("name")
            .or_else(|| object.get("label"))
            .and_then(Value::as_str)
            .and_then(normalize_text)
            .into_iter()
            .collect(),
        Some(Value::Number(number)) => normalize_text(&number.to_string()).into_iter().collect(),
        _ => Vec::new(),
    }
}

fn normalize_text(value: &str) -> Option<String> {
    let text = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let text = text.trim_matches(|c: char| c == ',' || c == '，' || c == ';' || c == '；');
    if text.is_empty() {
        None
    } else {
        Some(text.chars().take(TEXT_LIMIT).collect())
    }
}

fn normalize_parser_status(status: &str) -> &'static str {
    match status.trim().to_ascii_lowercase().as_str() {
        "queued" | "pending" => "pending",
        "running" | "processing" | "parsing" => "parsing",
        "retry" | "retrying" => "retrying",
        "done" | "succeeded" | "success" | "completed" => "completed",
        "partial" | "partial_completed" => "partial_completed",
        "failed" | "error" => "failed",
        _ => "unknown",
    }
}

fn fashion_parser_worker_input_contract_ok(contract: &Value) -> bool {
    contract_text_eq(contract, "task_kind", FASHION_DESIGN_IMAGE_PARSER_TASK_KIND)
        && contract_text_eq(contract, "parser_name", FASHION_DESIGN_IMAGE_PARSER_NAME)
        && contract_text_eq(
            contract,
            "parser_version",
            FASHION_DESIGN_IMAGE_PARSER_VERSION,
        )
        && contract_text_eq(
            contract,
            "profile_schema",
            FASHION_DESIGN_IMAGE_PROFILE_KIND,
        )
        && contract
            .get("source_ref_present")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && contract
            .get("source_kind")
            .and_then(Value::as_str)
            .is_some_and(|kind| matches!(kind, "external_id" | "object_key" | "image_url"))
        && contract
            .get("requested_outputs")
            .and_then(Value::as_array)
            .is_some_and(|outputs| {
                value_array_contains(outputs, "fashion_design_image_v1")
                    && value_array_contains(outputs, "retrieval_evidence_text")
            })
}

fn fashion_parser_worker_output_contract_ok(contract: &Value) -> bool {
    contract_text_eq(contract, "parser_name", FASHION_DESIGN_IMAGE_PARSER_NAME)
        && contract_text_eq(
            contract,
            "parser_version",
            FASHION_DESIGN_IMAGE_PARSER_VERSION,
        )
        && contract_text_eq(
            contract,
            "profile_schema",
            FASHION_DESIGN_IMAGE_PROFILE_KIND,
        )
        && contract
            .get("status")
            .and_then(Value::as_str)
            .map(normalize_parser_status)
            .is_some_and(|status| matches!(status, "completed" | "partial_completed"))
        && contract
            .pointer("/retrieval_evidence/source_kind")
            .and_then(Value::as_str)
            == Some("asset_profile")
        && contract
            .pointer("/retrieval_evidence/materialization")
            .and_then(Value::as_str)
            == Some("profile_to_text")
}

fn contract_text_eq(contract: &Value, key: &str, expected: &str) -> bool {
    contract.get(key).and_then(Value::as_str) == Some(expected)
}

fn value_array_contains(values: &[Value], expected: &str) -> bool {
    values.iter().any(|value| value.as_str() == Some(expected))
}

fn contains_forbidden_contract_material(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, value)| {
            forbidden_contract_key(key) || contains_forbidden_contract_material(value)
        }),
        Value::Array(values) => values.iter().any(contains_forbidden_contract_material),
        Value::String(text) => forbidden_contract_value(text),
        _ => false,
    }
}

fn forbidden_contract_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    matches!(
        key.as_str(),
        "raw_provider_payload"
            | "provider_payload"
            | "unknown_provider_blob"
            | "object_key"
            | "local_path"
            | "image_url"
            | "source_url"
            | "url"
            | "bearer"
            | "authorization"
            | "cookie"
    )
}

fn forbidden_contract_value(value: &str) -> bool {
    let value = value.trim();
    let lower = value.to_ascii_lowercase();
    lower.contains("http://")
        || lower.contains("https://")
        || lower.contains("objects/private")
        || lower.contains("raw_provider_payload")
        || lower.contains("bearer ")
        || lower.contains("authorization:")
        || lower.contains("cookie=")
        || value.contains(":\\")
        || lower.contains("/users/")
}

fn confidence(payload: &Value) -> Option<f32> {
    payload
        .get("confidence")
        .or_else(|| payload.get("score"))
        .and_then(Value::as_f64)
        .filter(|value| (0.0..=1.0).contains(value))
        .map(|value| value as f32)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn fashion_postchain_adapter_normalizes_worker_response_to_profile_contract() {
        let payload = json!({
            "category": "dress",
            "gender": ["women"],
            "season": "spring_summer",
            "style": ["commute", "resort"],
            "silhouette": "a-line",
            "collar": "round_neck",
            "sleeve": "short_sleeve",
            "waist": "high_waist",
            "hem": "midi",
            "material": ["cotton", "linen"],
            "craft": "pleated",
            "color": ["green", "white"],
            "pattern": "floral",
            "scene": ["xiaohongshu_launch"],
            "visible_text": ["新款"],
            "tags": ["连衣裙", "泡泡袖"],
            "caption": "春夏连衣裙灵感图",
            "confidence": 0.87
        });

        let profile = fashion_postchain_profile_from_payload(&payload);

        assert_eq!(FASHION_DESIGN_IMAGE_PROFILE_KIND, "fashion_design_image_v1");
        assert_eq!(profile.category.as_deref(), Some("dress"));
        assert_eq!(profile.audience, vec!["women"]);
        assert_eq!(profile.seasons, vec!["spring_summer"]);
        assert_eq!(profile.styles, vec!["commute", "resort"]);
        assert_eq!(profile.collars, vec!["round_neck"]);
        assert_eq!(profile.materials, vec!["cotton", "linen"]);
        assert_eq!(profile.visible_text, vec!["新款"]);
        assert_eq!(profile.caption.as_deref(), Some("春夏连衣裙灵感图"));
        assert_eq!(profile.confidence, Some(0.87));
    }

    #[test]
    fn fashion_postchain_adapter_allows_partial_payloads_without_failing_import() {
        let attributes = fashion_postchain_profile_attributes(&json!({
            "summary": "仅识别到一张无文字服装图",
            "颜色": "黑色",
            "unknown_provider_blob": {"raw": true}
        }));

        assert_eq!(attributes["caption"], json!("仅识别到一张无文字服装图"));
        assert_eq!(attributes["colors"], json!(["黑色"]));
        assert!(attributes.get("unknown_provider_blob").is_none());
    }

    #[test]
    fn fashion_parser_input_contract_is_safe_and_explicit() {
        let contract = fashion_design_image_parser_worker_input_contract(
            &FashionDesignImageParserWorkerInput {
                source_ref_present: true,
                source_kind: Some("object_key"),
                content_type: Some("image/png".to_string()),
                profile_seeded: true,
            },
        );

        assert_eq!(contract["task_kind"], json!("fashion_design_image_ocr_vlm"));
        assert_eq!(
            contract["parser_name"],
            json!("datamax-fashion-image-parser")
        );
        assert_eq!(contract["profile_schema"], json!("fashion_design_image_v1"));
        assert_eq!(contract["source_ref_present"], json!(true));
        assert_eq!(contract["source_kind"], json!("object_key"));
        assert_eq!(
            contract["requested_outputs"][1],
            json!("retrieval_evidence_text")
        );
        let serialized = serde_json::to_string(&contract).expect("contract should serialize");
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("raw_provider_payload"));
        assert!(!serialized.contains("https://example.com"));
    }

    #[test]
    fn fashion_parser_output_contract_normalizes_profile_and_retrieval_state() {
        let contract = fashion_design_image_parser_worker_output_contract(&json!({
            "category": "dress",
            "season": "spring_summer",
            "collar": "round_neck",
            "sleeve": "puff_sleeve",
            "caption": "春夏连衣裙灵感图",
            "raw_provider_payload": {"should_not_surface": true},
            "object_key": "objects/private/look.png"
        }));

        assert_eq!(contract["status"], json!("completed"));
        assert_eq!(contract["profile_schema"], json!("fashion_design_image_v1"));
        assert_eq!(contract["profile_payload"]["category"], json!("dress"));
        assert_eq!(
            contract["profile_payload"]["seasons"],
            json!(["spring_summer"])
        );
        assert_eq!(contract["retrieval_evidence"]["ready"], json!(true));
        assert_eq!(
            contract["retrieval_evidence"]["materialization"],
            json!("profile_to_text")
        );
        let serialized = serde_json::to_string(&contract).expect("contract should serialize");
        assert!(!serialized.contains("raw_provider_payload"));
        assert!(!serialized.contains("objects/private"));
    }

    #[test]
    fn fashion_parser_status_transition_dry_run_marks_completed_or_retrying() {
        let completed_output = fashion_design_image_parser_worker_output_contract(&json!({
            "category": "dress",
            "caption": "春夏连衣裙灵感图"
        }));
        let completed_transition =
            fashion_design_image_parser_status_transition_dry_run("pending", &completed_output);

        assert_eq!(completed_transition["current_status"], json!("pending"));
        assert_eq!(completed_transition["next_status"], json!("completed"));
        assert_eq!(
            completed_transition["transition_action"],
            json!("mark_completed_and_materialize_retrieval_evidence")
        );
        assert_eq!(
            completed_transition["should_materialize_retrieval_evidence"],
            json!(true)
        );
        assert_eq!(completed_transition["dry_run_only"], json!(true));

        let partial_output = fashion_design_image_parser_worker_output_contract(&json!({}));
        let retry_transition =
            fashion_design_image_parser_status_transition_dry_run("parsing", &partial_output);
        assert_eq!(retry_transition["current_status"], json!("parsing"));
        assert_eq!(retry_transition["next_status"], json!("retrying"));
        assert_eq!(
            retry_transition["transition_action"],
            json!("mark_retrying_wait_for_profile")
        );
        assert_eq!(
            retry_transition["should_materialize_retrieval_evidence"],
            json!(false)
        );
        let serialized =
            serde_json::to_string(&completed_transition).expect("transition should serialize");
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("raw_provider_payload"));
    }

    #[test]
    fn fashion_parser_live_adapter_readiness_dry_run_accepts_safe_contracts() {
        let input = fashion_design_image_parser_worker_input_contract(
            &FashionDesignImageParserWorkerInput {
                source_ref_present: true,
                source_kind: Some("object_key"),
                content_type: Some("image/png".to_string()),
                profile_seeded: false,
            },
        );
        let output = fashion_design_image_parser_worker_output_contract(&json!({
            "category": "dress",
            "caption": "春夏连衣裙灵感图",
            "color": ["green", "white"]
        }));

        let readiness = fashion_design_image_parser_live_adapter_readiness_dry_run(&input, &output);

        assert_eq!(
            readiness["contract"],
            json!("fashion_design_image_parser_live_adapter_readiness_dry_run_v1")
        );
        assert_eq!(
            readiness["adapter_status"],
            json!("ready_for_profile_and_retrieval_commit")
        );
        assert_eq!(readiness["adapter_ready"], json!(true));
        assert_eq!(readiness["no_network"], json!(true));
        assert_eq!(readiness["no_model_call"], json!(true));
        assert_eq!(readiness["no_write"], json!(true));
        assert_eq!(readiness["production_write_allowed"], json!(false));
        assert_eq!(
            readiness["live_execute_still_requires_worker_endpoint"],
            json!(true)
        );
        assert_eq!(readiness["worker_endpoint_value_included"], json!(false));
        assert_eq!(
            readiness["validation"]["worker_input_contract_ok"],
            json!(true)
        );
        assert_eq!(
            readiness["validation"]["worker_output_contract_ok"],
            json!(true)
        );
        assert_eq!(
            readiness["validation"]["retrieval_evidence_ready"],
            json!(true)
        );
        let serialized = serde_json::to_string(&readiness).expect("readiness should serialize");
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("raw_provider_payload"));
        assert!(!serialized.contains("https://example.com"));
    }

    #[test]
    fn fashion_parser_live_adapter_readiness_dry_run_blocks_bad_input_contract() {
        let mut input = fashion_design_image_parser_worker_input_contract(
            &FashionDesignImageParserWorkerInput {
                source_ref_present: true,
                source_kind: Some("object_key"),
                content_type: Some("image/png".to_string()),
                profile_seeded: false,
            },
        );
        input["task_kind"] = json!("unexpected_worker_task");
        let output = fashion_design_image_parser_worker_output_contract(&json!({
            "category": "dress",
            "caption": "春夏连衣裙灵感图"
        }));

        let readiness = fashion_design_image_parser_live_adapter_readiness_dry_run(&input, &output);

        assert_eq!(
            readiness["adapter_status"],
            json!("blocked_worker_input_contract")
        );
        assert_eq!(readiness["adapter_ready"], json!(false));
        assert_eq!(
            readiness["validation"]["worker_input_contract_ok"],
            json!(false)
        );
        assert_eq!(readiness["production_write_allowed"], json!(false));
    }

    #[test]
    fn fashion_parser_live_adapter_readiness_dry_run_blocks_sensitive_output_material() {
        let input = fashion_design_image_parser_worker_input_contract(
            &FashionDesignImageParserWorkerInput {
                source_ref_present: true,
                source_kind: Some("object_key"),
                content_type: Some("image/png".to_string()),
                profile_seeded: false,
            },
        );
        let output = json!({
            "status": "completed",
            "parser_name": FASHION_DESIGN_IMAGE_PARSER_NAME,
            "parser_version": FASHION_DESIGN_IMAGE_PARSER_VERSION,
            "profile_schema": FASHION_DESIGN_IMAGE_PROFILE_KIND,
            "profile_payload": {
                "category": "dress",
                "raw_provider_payload": {"should_not_surface": true}
            },
            "retrieval_evidence": {
                "source_kind": "asset_profile",
                "materialization": "profile_to_text",
                "ready": true
            }
        });

        let readiness = fashion_design_image_parser_live_adapter_readiness_dry_run(&input, &output);

        assert_eq!(
            readiness["adapter_status"],
            json!("blocked_sensitive_material")
        );
        assert_eq!(readiness["adapter_ready"], json!(false));
        assert_eq!(
            readiness["validation"]["sensitive_material_excluded"],
            json!(false)
        );
        assert_eq!(readiness["no_network"], json!(true));
        assert_eq!(readiness["no_write"], json!(true));
    }
}
