use chrono::{DateTime, Utc};
use contracts::{
    CreateAssistantRunRequest, HtmlArtifactDataRefView, HtmlArtifactInteractionModeView,
    HtmlArtifactManifestView, HtmlArtifactOwnerScopeView, HtmlArtifactProvenanceView,
    HtmlArtifactSourceTypeView, HtmlArtifactTemplateIdView,
};
use domain_model::AssistantRunId;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    assistant_run_compact_dataset_entity_scan_payloads_for_prompt,
    assistant_run_resume_profile_support::{
        resume_profile_array_string, resume_profile_array_values, resume_profile_candidate_name,
        value_string,
    },
    escape_markdown_table_cell,
    html_artifact_summary_support::html_artifact_safe_summary_text,
    prompt_requests_resume_project_delivery_listing,
    ASSISTANT_RUN_HTML_GENERATION_ROUTE_RAPID_ARTIFACT,
    ASSISTANT_RUN_RESUME_PROJECT_DELIVERY_ARTIFACT_MIN_DOCUMENTS,
    ASSISTANT_RUN_RESUME_PROJECT_DELIVERY_ARTIFACT_MIN_ROWS,
    ASSISTANT_RUN_RESUME_PROJECT_DELIVERY_ROW_LIMIT,
};

pub(crate) fn assistant_run_resume_project_delivery_direct_answer(
    scans: &[Value],
    profile_rows: &[Value],
) -> Option<String> {
    let delivery_rows = scans
        .iter()
        .flat_map(|scan| {
            scan.get("resume_project_delivery_rows")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    if delivery_rows.is_empty() && profile_rows.is_empty() {
        return None;
    }

    let scanned_document_count = scans
        .iter()
        .filter_map(|scan| scan.get("scanned_document_count").and_then(Value::as_u64))
        .max()
        .unwrap_or(profile_rows.len() as u64);
    let mut rows_by_document: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for row in delivery_rows {
        let document_id = value_string(&row, "document_id");
        rows_by_document.entry(document_id).or_default().push(row);
    }

    let mut lines = vec![
        format!(
            "已按可见简历逐份扫描项目交付经历：覆盖 {scanned_document_count} 份文档，识别到 {} 条项目交付记录。",
            rows_by_document.values().map(Vec::len).sum::<usize>()
        ),
        "未识别到项目交付段的候选人会单独标记，避免只展示检索命中的个别人。".to_string(),
        String::new(),
        "| 候选人 | 项目/状态 | 交付职责或成果 | 技术栈 | 来源文档 |".to_string(),
        "| --- | --- | --- | --- | --- |".to_string(),
    ];

    let mut emitted_documents = BTreeSet::new();
    for profile in profile_rows {
        let document_id = value_string(profile, "document_id");
        if !document_id.is_empty() && document_id != "-" {
            emitted_documents.insert(document_id.clone());
        }
        let candidate_name = resume_profile_candidate_name(profile);
        let document_title = value_string(profile, "document_title");
        if let Some(rows) = rows_by_document.get(&document_id) {
            for row in rows {
                lines.push(format!(
                    "| {} | {} | {} | {} | {} |",
                    escape_markdown_table_cell(&candidate_name),
                    escape_markdown_table_cell(&value_string(row, "project_name")),
                    escape_markdown_table_cell(&value_string(row, "delivery_summary")),
                    escape_markdown_table_cell(&resume_profile_array_string(row, "tech_stack", 6)),
                    escape_markdown_table_cell(&document_title),
                ));
            }
        } else {
            lines.push(format!(
                "| {} | 未识别到项目交付段 | 当前轻量扫描未抽到明确项目名称或职责句 | - | {} |",
                escape_markdown_table_cell(&candidate_name),
                escape_markdown_table_cell(&document_title),
            ));
        }
    }

    for (document_id, rows) in rows_by_document {
        if emitted_documents.contains(&document_id) {
            continue;
        }
        for row in rows {
            lines.push(format!(
                "| {} | {} | {} | {} | {} |",
                escape_markdown_table_cell(&value_string(&row, "candidate_name")),
                escape_markdown_table_cell(&value_string(&row, "project_name")),
                escape_markdown_table_cell(&value_string(&row, "delivery_summary")),
                escape_markdown_table_cell(&resume_profile_array_string(&row, "tech_stack", 6)),
                escape_markdown_table_cell(&value_string(&row, "document_title")),
            ));
        }
    }

    Some(lines.join("\n"))
}

pub(crate) fn assistant_run_resume_project_delivery_controlled_answer(
    evidence_state: &Value,
    request: &CreateAssistantRunRequest,
) -> Option<String> {
    if !prompt_requests_resume_project_delivery_listing(&request.prompt) {
        return None;
    }
    let scans = assistant_run_compact_dataset_entity_scan_payloads_for_prompt(
        evidence_state,
        &request.prompt,
    );
    if scans.is_empty() {
        return None;
    }
    let profile_rows = scans
        .iter()
        .flat_map(|scan| {
            scan.get("resume_profile_rows")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    assistant_run_resume_project_delivery_direct_answer(&scans, &profile_rows)
}

pub(crate) fn assistant_run_resume_project_delivery_html_artifact(
    run_id: AssistantRunId,
    request: &CreateAssistantRunRequest,
    evidence_state: &Value,
    created_at: DateTime<Utc>,
) -> Option<HtmlArtifactManifestView> {
    if !prompt_requests_resume_project_delivery_listing(&request.prompt) {
        return None;
    }
    let scans = assistant_run_compact_dataset_entity_scan_payloads_for_prompt(
        evidence_state,
        &request.prompt,
    );
    if scans.is_empty() {
        return None;
    }
    let payload = assistant_run_resume_project_delivery_artifact_payload(&scans)?;
    let scanned_document_count = payload
        .get("summary")
        .and_then(|summary| summary.get("scannedDocumentCount"))
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let rendered_row_count = payload
        .get("summary")
        .and_then(|summary| summary.get("renderedRowCount"))
        .and_then(Value::as_u64)
        .unwrap_or_default() as usize;
    if scanned_document_count < ASSISTANT_RUN_RESUME_PROJECT_DELIVERY_ARTIFACT_MIN_DOCUMENTS
        && rendered_row_count < ASSISTANT_RUN_RESUME_PROJECT_DELIVERY_ARTIFACT_MIN_ROWS
    {
        return None;
    }

    Some(HtmlArtifactManifestView {
        kind: "html_artifact".to_string(),
        version: 1,
        id: format!("html-artifact-resume-project-delivery-{run_id}"),
        title: "简历项目交付明细表".to_string(),
        source_type: HtmlArtifactSourceTypeView::Report,
        template_id: HtmlArtifactTemplateIdView::ResumeProjectDeliveryMatrix,
        owner_scope: HtmlArtifactOwnerScopeView {
            scope_type: "assistant_run".to_string(),
            id: run_id.to_string(),
        },
        data_refs: assistant_run_resume_project_delivery_data_refs(&scans),
        provenance: HtmlArtifactProvenanceView {
            producer: "v3-assistant-run".to_string(),
            reason: "multi_resume_project_delivery_answer".to_string(),
            source_run_id: Some(run_id.to_string()),
        },
        interaction_mode: HtmlArtifactInteractionModeView::ReadOnly,
        created_at,
        payload,
    })
}

fn assistant_run_resume_project_delivery_artifact_payload(scans: &[Value]) -> Option<Value> {
    let profile_rows = scans
        .iter()
        .flat_map(|scan| {
            scan.get("resume_profile_rows")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let delivery_rows = scans
        .iter()
        .flat_map(|scan| {
            scan.get("resume_project_delivery_rows")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    if profile_rows.is_empty() && delivery_rows.is_empty() {
        return None;
    }
    let scanned_document_count = scans
        .iter()
        .filter_map(|scan| scan.get("scanned_document_count").and_then(Value::as_u64))
        .max()
        .unwrap_or(profile_rows.len() as u64);
    let mut rows_by_document: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for row in &delivery_rows {
        let document_id = value_string(row, "document_id");
        rows_by_document
            .entry(document_id)
            .or_default()
            .push(row.clone());
    }

    let mut rendered_rows = Vec::new();
    let mut emitted_documents = BTreeSet::new();
    let mut missing_project_delivery_count = 0usize;
    for profile in &profile_rows {
        let document_id = value_string(profile, "document_id");
        if !document_id.is_empty() && document_id != "-" {
            emitted_documents.insert(document_id.clone());
        }
        let candidate_name = resume_profile_candidate_name(profile);
        let document_title = value_string(profile, "document_title");
        if let Some(rows) = rows_by_document.get(&document_id) {
            for row in rows {
                rendered_rows.push(assistant_run_resume_project_delivery_artifact_row(
                    &candidate_name,
                    &document_title,
                    row,
                    "recognized",
                ));
            }
        } else {
            missing_project_delivery_count += 1;
            rendered_rows.push(json!({
                "candidateName": html_artifact_safe_summary_text(&candidate_name, 80),
                "projectName": "未识别到项目交付段",
                "status": "missing_project_delivery_section",
                "deliverySummary": "当前轻量扫描未抽到明确项目名称或职责句",
                "techStack": [],
                "documentTitle": html_artifact_safe_summary_text(&document_title, 160),
                "confidence": null,
            }));
        }
    }

    for (document_id, rows) in rows_by_document {
        if emitted_documents.contains(&document_id) {
            continue;
        }
        for row in rows {
            rendered_rows.push(assistant_run_resume_project_delivery_artifact_row(
                &value_string(&row, "candidate_name"),
                &value_string(&row, "document_title"),
                &row,
                "recognized",
            ));
        }
    }
    if rendered_rows.is_empty() {
        return None;
    }
    rendered_rows.truncate(ASSISTANT_RUN_RESUME_PROJECT_DELIVERY_ROW_LIMIT);

    Some(json!({
        "generationPolicy": assistant_run_rapid_html_artifact_generation_policy(),
        "summary": {
            "scannedDocumentCount": scanned_document_count,
            "resumeProfileCount": profile_rows.len(),
            "projectDeliveryRowCount": delivery_rows.len(),
            "renderedRowCount": rendered_rows.len(),
            "missingProjectDeliveryCount": missing_project_delivery_count,
        },
        "rows": rendered_rows,
        "notes": [
            "本页来自 DataMax 结构化简历扫描结果，用于承载高信息量回答的完整明细。",
            "未识别到项目交付段的候选人会保留占位行，避免只展示检索命中的少数简历。"
        ],
    }))
}

fn assistant_run_rapid_html_artifact_generation_policy() -> Value {
    json!({
        "route": ASSISTANT_RUN_HTML_GENERATION_ROUTE_RAPID_ARTIFACT,
        "intendedUse": "chat_high_information_detail",
        "templateAuthority": "v3_safe_html_artifact_manifest",
        "htmlAnythingRole": "rapid_template_reference_only",
        "image2Required": false,
        "staticPagePipeline": false,
        "publishAsGeneratedArtifact": false,
    })
}

fn assistant_run_resume_project_delivery_artifact_row(
    fallback_candidate_name: &str,
    fallback_document_title: &str,
    row: &Value,
    status: &str,
) -> Value {
    let candidate_name = value_string(row, "candidate_name");
    let document_title = value_string(row, "document_title");
    let effective_candidate_name = if candidate_name.is_empty() {
        fallback_candidate_name
    } else {
        &candidate_name
    };
    let effective_document_title = if document_title.is_empty() {
        fallback_document_title
    } else {
        &document_title
    };
    json!({
        "candidateName": html_artifact_safe_summary_text(effective_candidate_name, 80),
        "projectName": html_artifact_safe_summary_text(&value_string(row, "project_name"), 120),
        "status": status,
        "deliverySummary": html_artifact_safe_summary_text(&value_string(row, "delivery_summary"), 320),
        "techStack": resume_profile_array_values(row, "tech_stack")
            .into_iter()
            .map(|value| html_artifact_safe_summary_text(&value, 80))
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>(),
        "documentTitle": html_artifact_safe_summary_text(effective_document_title, 160),
        "confidence": row.get("confidence").cloned().unwrap_or(Value::Null),
    })
}

fn assistant_run_resume_project_delivery_data_refs(
    scans: &[Value],
) -> Vec<HtmlArtifactDataRefView> {
    let mut seen = BTreeSet::new();
    scans
        .iter()
        .filter_map(|scan| scan.get("dataset_id").and_then(Value::as_str))
        .map(str::trim)
        .filter(|dataset_id| !dataset_id.is_empty())
        .filter(|dataset_id| seen.insert((*dataset_id).to_string()))
        .take(8)
        .map(|dataset_id| HtmlArtifactDataRefView {
            kind: "dataset".to_string(),
            id: dataset_id.to_string(),
            label: format!("数据集 {dataset_id}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn direct_answer_marks_missing_project_delivery_per_profile() {
        let scans = vec![json!({
            "scanned_document_count": 2,
            "resume_project_delivery_rows": [{
                "document_id": "doc-1",
                "candidate_name": "张三",
                "project_name": "知识库平台",
                "delivery_summary": "负责接口交付",
                "tech_stack": ["Rust", "React"],
                "document_title": "张三简历.pdf"
            }]
        })];
        let profile_rows = vec![
            json!({"document_id": "doc-1", "candidate_name": "张三", "document_title": "张三简历.pdf"}),
            json!({"document_id": "doc-2", "candidate_name": "李四", "document_title": "李四简历.pdf"}),
        ];

        let answer = assistant_run_resume_project_delivery_direct_answer(&scans, &profile_rows)
            .expect("project delivery answer");

        assert!(answer.contains("覆盖 2 份文档，识别到 1 条项目交付记录"));
        assert!(
            answer.contains("| 张三 | 知识库平台 | 负责接口交付 | Rust；React | 张三简历.pdf |")
        );
        assert!(answer.contains("| 李四 | 未识别到项目交付段 | 当前轻量扫描未抽到明确项目名称或职责句 | - | 李四简历.pdf |"));
    }

    #[test]
    fn artifact_payload_preserves_summary_rows_and_safe_policy() {
        let scans = vec![json!({
            "dataset_id": "dataset-1",
            "scanned_document_count": 7,
            "resume_profile_rows": [{
                "document_id": "doc-1",
                "candidate_name": "张三",
                "document_title": "张三简历.pdf"
            }],
            "resume_project_delivery_rows": [{
                "document_id": "doc-1",
                "candidate_name": "张三",
                "project_name": "知识库平台",
                "delivery_summary": "负责接口交付",
                "tech_stack": ["Rust"],
                "document_title": "张三简历.pdf",
                "confidence": 0.8
            }]
        })];

        let payload = assistant_run_resume_project_delivery_artifact_payload(&scans)
            .expect("project delivery payload");

        assert_eq!(payload["summary"]["scannedDocumentCount"], json!(7));
        assert_eq!(payload["summary"]["resumeProfileCount"], json!(1));
        assert_eq!(payload["summary"]["projectDeliveryRowCount"], json!(1));
        assert_eq!(payload["rows"][0]["status"], json!("recognized"));
        assert_eq!(payload["rows"][0]["techStack"][0], json!("Rust"));
        assert_eq!(
            payload["generationPolicy"]["route"],
            json!(ASSISTANT_RUN_HTML_GENERATION_ROUTE_RAPID_ARTIFACT)
        );
        assert_eq!(payload["generationPolicy"]["image2Required"], json!(false));
    }

    #[test]
    fn data_refs_dedupe_trim_and_limit_dataset_ids() {
        let scans = (0..10)
            .map(|index| {
                let dataset_id = if index == 1 {
                    " dataset-0 ".to_string()
                } else {
                    format!("dataset-{index}")
                };
                json!({"dataset_id": dataset_id})
            })
            .collect::<Vec<_>>();

        let refs = assistant_run_resume_project_delivery_data_refs(&scans);

        assert_eq!(refs.len(), 8);
        assert_eq!(refs[0].id, "dataset-0");
        assert_eq!(refs[1].id, "dataset-2");
        assert!(refs.iter().all(|item| item.kind == "dataset"));
    }
}
