use serde_json::{json, Value};

use crate::{
    assistant_run_static_page_section_title_hints, collect_string_list, push_string_hint,
    static_page_artifact_string, static_page_binding_string,
};

pub(crate) fn build_static_page_structure_signals(
    field_candidates: &Value,
    module_bindings: &[Value],
) -> Value {
    const HINT_LIMIT: usize = 12;
    const FIELD_LIMIT: usize = 4;
    const MODULE_LIMIT: usize = 8;

    let mut section_title_hints = Vec::<String>::new();
    let field_candidate_briefs = field_candidates
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter(|candidate| {
                    static_page_artifact_string(candidate, &["fieldPath", "field_path", "field"])
                        .as_deref()
                        == Some("retrieval.section_title_hints")
                })
                .take(FIELD_LIMIT)
                .map(|candidate| {
                    let hints =
                        assistant_run_static_page_section_title_hints(candidate, HINT_LIMIT);
                    for hint in &hints {
                        push_string_hint(&mut section_title_hints, hint);
                    }
                    json!({
                        "sourceId": static_page_artifact_string(candidate, &["sourceId", "source_id"]).unwrap_or_default(),
                        "fieldPath": "retrieval.section_title_hints",
                        "label": static_page_artifact_string(candidate, &["label"]).unwrap_or_default(),
                        "kind": static_page_artifact_string(candidate, &["kind"]).unwrap_or_default(),
                        "confidence": candidate.get("confidence").cloned().unwrap_or(Value::Null),
                        "evidenceIds": candidate
                            .get("evidenceIds")
                            .or_else(|| candidate.get("evidence_ids"))
                            .cloned()
                            .unwrap_or_else(|| json!([])),
                        "sectionTitleHints": hints,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let bound_modules = module_bindings
        .iter()
        .filter_map(|module| {
            let binding = module
                .get("binding")
                .or_else(|| module.get("dataBinding"))
                .or_else(|| module.get("data_binding"))
                .unwrap_or(&Value::Null);
            let binding_quality = module
                .get("bindingQuality")
                .or_else(|| module.get("binding_quality"))
                .unwrap_or(&Value::Null);
            let matched_candidate = binding_quality
                .get("matchedFieldCandidate")
                .or_else(|| binding_quality.get("matched_field_candidate"))
                .unwrap_or(&Value::Null);
            let field_path =
                static_page_artifact_string(binding, &["fieldPath", "field_path", "field"])
                    .or_else(|| {
                        static_page_artifact_string(
                            binding_quality,
                            &["fieldPath", "field_path", "field"],
                        )
                    })
                    .or_else(|| {
                        static_page_artifact_string(
                            matched_candidate,
                            &["fieldPath", "field_path", "field"],
                        )
                    });
            if field_path.as_deref() != Some("retrieval.section_title_hints") {
                return None;
            }
            let hints =
                assistant_run_static_page_section_title_hints(matched_candidate, HINT_LIMIT);
            for hint in &hints {
                push_string_hint(&mut section_title_hints, hint);
            }
            Some(json!({
                "moduleId": static_page_artifact_string(module, &["moduleId", "module_id"]).unwrap_or_default(),
                "title": static_page_artifact_string(module, &["title"]).unwrap_or_default(),
                "fieldPath": "retrieval.section_title_hints",
                "bindingQualityStatus": static_page_artifact_string(module, &["bindingQualityStatus", "binding_quality_status"])
                    .or_else(|| static_page_artifact_string(binding_quality, &["status"]))
                    .unwrap_or_default(),
                "sectionTitleHints": hints,
            }))
        })
        .take(MODULE_LIMIT)
        .collect::<Vec<_>>();

    section_title_hints.truncate(HINT_LIMIT);
    json!({
        "version": 1,
        "status": if section_title_hints.is_empty() { "none" } else { "available" },
        "policy": "source_structure_only_no_body_no_sample_rows",
        "sectionTitleHints": section_title_hints,
        "fieldCandidates": field_candidate_briefs,
        "boundModules": bound_modules,
    })
}

pub(crate) fn static_page_heading_field_candidate(field_candidates: &Value) -> Option<&Value> {
    field_candidates.as_array()?.iter().find(|candidate| {
        candidate
            .get("sourceId")
            .or_else(|| candidate.get("source_id"))
            .and_then(Value::as_str)
            == Some("evidence")
            && candidate
                .get("fieldPath")
                .or_else(|| candidate.get("field_path"))
                .and_then(Value::as_str)
                == Some("retrieval.section_title_hints")
            && !static_page_field_candidate_section_title_hints(candidate).is_empty()
    })
}

fn static_page_field_candidate_section_title_hints(candidate: &Value) -> Vec<String> {
    let mut hints = Vec::new();
    for key in ["sectionTitleHints", "section_title_hints"] {
        if let Some(value) = candidate.get(key) {
            collect_string_list(value, &mut hints);
        }
    }
    hints.truncate(6);
    hints
}

pub(crate) fn enrich_docs_page_heading_binding(
    module: &Value,
    binding: Value,
    heading_candidate: Option<&Value>,
) -> Value {
    let Some(heading_candidate) = heading_candidate else {
        return binding;
    };
    if !docs_page_structure_module_uses_heading_hints(module) {
        return binding;
    }
    if let Some(field_path) = static_page_binding_string(&binding, &["fieldPath", "field_path"]) {
        if field_path != "retrieval.section_title_hints" {
            return binding;
        }
    }

    let mut next = binding.as_object().cloned().unwrap_or_default();
    next.insert("type".to_string(), json!("retrieval_evidence"));
    next.insert("sourceId".to_string(), json!("evidence"));
    next.insert(
        "fieldPath".to_string(),
        json!("retrieval.section_title_hints"),
    );
    next.insert(
        "label".to_string(),
        heading_candidate
            .get("label")
            .and_then(Value::as_str)
            .map(|label| json!(label))
            .unwrap_or_else(|| json!("文档段落标题线索")),
    );
    if let Some(evidence_ids) = heading_candidate.get("evidenceIds") {
        next.insert("evidenceIds".to_string(), evidence_ids.clone());
    }
    Value::Object(next)
}

fn docs_page_structure_module_uses_heading_hints(module: &Value) -> bool {
    for key in ["id", "role"] {
        match module.get(key).and_then(Value::as_str) {
            Some("scope" | "steps" | "interfaces" | "checks") => return true,
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_signals_include_field_candidates_and_bound_modules() {
        let field_candidates = json!([
            {
                "sourceId": "evidence",
                "fieldPath": "retrieval.section_title_hints",
                "label": "段落标题：接口与数据 / 校验与交付",
                "kind": "structure_signal",
                "confidence": 0.9,
                "evidenceIds": ["ev-1"],
                "sectionTitleHints": ["接口与数据", "校验与交付"]
            }
        ]);
        let module_bindings = vec![json!({
            "moduleId": "interfaces",
            "title": "接口与数据",
            "binding": {"fieldPath": "retrieval.section_title_hints"},
            "bindingQuality": {
                "status": "confirmed",
                "matchedFieldCandidate": {
                    "fieldPath": "retrieval.section_title_hints",
                    "sectionTitleHints": ["接口与数据"]
                }
            }
        })];

        let signals = build_static_page_structure_signals(&field_candidates, &module_bindings);

        assert_eq!(signals["status"], json!("available"));
        assert_eq!(
            signals["policy"],
            json!("source_structure_only_no_body_no_sample_rows")
        );
        assert_eq!(signals["sectionTitleHints"][0], json!("接口与数据"));
        assert_eq!(
            signals["fieldCandidates"][0]["fieldPath"],
            json!("retrieval.section_title_hints")
        );
        assert_eq!(signals["boundModules"][0]["moduleId"], json!("interfaces"));
        assert_eq!(
            signals["boundModules"][0]["bindingQualityStatus"],
            json!("confirmed")
        );
    }

    #[test]
    fn heading_candidate_requires_evidence_source_and_hints() {
        let candidates = json!([
            {
                "sourceId": "model",
                "fieldPath": "retrieval.section_title_hints",
                "sectionTitleHints": ["忽略"]
            },
            {
                "sourceId": "evidence",
                "fieldPath": "retrieval.section_title_hints",
                "section_title_hints": ["范围与边界"]
            }
        ]);

        let candidate =
            static_page_heading_field_candidate(&candidates).expect("candidate should match");

        assert_eq!(candidate["section_title_hints"][0], json!("范围与边界"));
    }

    #[test]
    fn enrich_docs_page_heading_binding_only_for_structure_modules() {
        let heading_candidate = json!({
            "label": "段落标题：接口与数据",
            "evidenceIds": ["ev-1"]
        });
        let binding = json!({"fieldPath": "retrieval.section_title_hints"});

        let enriched = enrich_docs_page_heading_binding(
            &json!({"id": "interfaces", "title": "接口"}),
            binding.clone(),
            Some(&heading_candidate),
        );
        let unchanged = enrich_docs_page_heading_binding(
            &json!({"id": "hero", "title": "文档概览"}),
            binding,
            Some(&heading_candidate),
        );

        assert_eq!(enriched["type"], json!("retrieval_evidence"));
        assert_eq!(enriched["sourceId"], json!("evidence"));
        assert_eq!(enriched["label"], json!("段落标题：接口与数据"));
        assert_eq!(enriched["evidenceIds"][0], json!("ev-1"));
        assert!(unchanged.get("sourceId").is_none());
    }
}
