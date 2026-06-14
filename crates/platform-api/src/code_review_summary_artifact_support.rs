use std::collections::HashSet;

use chrono::{DateTime, Utc};
use contracts::{self, HtmlArtifactInteractionModeView, HtmlArtifactManifestView};
use domain_model::{AssistantRun, AssistantRunEvent};
use serde_json::{json, Value};

use crate::html_artifact_collection_support::sort_and_dedupe_html_artifacts;
use crate::html_artifact_summary_support::html_artifact_safe_summary_text;
use crate::{sha256_hex, value_array};

struct CodeReviewSummaryArtifactCandidate {
    payload: Value,
    created_at: DateTime<Utc>,
    source_kind: &'static str,
    source_event_name: Option<String>,
    sequence_no: Option<i32>,
    source_index: usize,
}

pub(crate) fn code_review_summary_artifacts_from_run_values(
    run: &AssistantRun,
    events: &[AssistantRunEvent],
    limit: usize,
) -> Vec<HtmlArtifactManifestView> {
    let mut candidates = Vec::<CodeReviewSummaryArtifactCandidate>::new();
    for (source_index, artifact) in value_array(run.output_artifacts.clone())
        .into_iter()
        .enumerate()
    {
        assistant_run_collect_code_review_payloads(
            &artifact,
            false,
            &mut candidates,
            limit,
            run.updated_at,
            "assistant_run_output_artifact",
            None,
            None,
            source_index,
        );
        if candidates.len() >= limit {
            break;
        }
    }

    for event in events {
        if candidates.len() >= limit {
            break;
        }
        let force_marker = assistant_run_code_review_marker_text(&event.event_name);
        assistant_run_collect_code_review_payloads(
            &event.payload,
            force_marker,
            &mut candidates,
            limit,
            event.created_at,
            "assistant_run_event",
            Some(event.event_name.clone()),
            Some(event.sequence_no),
            event.sequence_no.max(0) as usize,
        );
    }

    let mut seen = HashSet::<String>::new();
    let mut artifacts = Vec::new();
    for candidate in candidates {
        if artifacts.len() >= limit {
            break;
        }
        let signature = code_review_summary_payload_signature(&candidate.payload);
        if !seen.insert(signature.clone()) {
            continue;
        }
        if let Some(artifact) =
            code_review_summary_artifact_from_candidate(run, candidate, &signature)
        {
            artifacts.push(artifact);
        }
    }
    sort_and_dedupe_html_artifacts(&mut artifacts);
    artifacts.truncate(limit);
    artifacts
}

#[allow(clippy::too_many_arguments)]
fn assistant_run_collect_code_review_payloads(
    value: &Value,
    force_marker: bool,
    candidates: &mut Vec<CodeReviewSummaryArtifactCandidate>,
    limit: usize,
    created_at: DateTime<Utc>,
    source_kind: &'static str,
    source_event_name: Option<String>,
    sequence_no: Option<i32>,
    source_index: usize,
) {
    if candidates.len() >= limit {
        return;
    }

    match value {
        Value::Object(object) => {
            if assistant_run_code_review_payload_candidate(value, force_marker) {
                candidates.push(CodeReviewSummaryArtifactCandidate {
                    payload: value.clone(),
                    created_at,
                    source_kind,
                    source_event_name,
                    sequence_no,
                    source_index,
                });
                return;
            }
            for (child_index, (key, child)) in object.iter().enumerate() {
                if key == "html_artifacts" {
                    continue;
                }
                assistant_run_collect_code_review_payloads(
                    child,
                    force_marker,
                    candidates,
                    limit,
                    created_at,
                    source_kind,
                    source_event_name.clone(),
                    sequence_no,
                    source_index.saturating_mul(100).saturating_add(child_index),
                );
                if candidates.len() >= limit {
                    break;
                }
            }
        }
        Value::Array(entries) => {
            for (child_index, entry) in entries.iter().enumerate() {
                assistant_run_collect_code_review_payloads(
                    entry,
                    force_marker,
                    candidates,
                    limit,
                    created_at,
                    source_kind,
                    source_event_name.clone(),
                    sequence_no,
                    source_index.saturating_mul(100).saturating_add(child_index),
                );
                if candidates.len() >= limit {
                    break;
                }
            }
        }
        _ => {}
    }
}

fn assistant_run_code_review_payload_candidate(value: &Value, force_marker: bool) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    let marker = [
        "type",
        "kind",
        "category",
        "artifact_type",
        "artifactType",
        "template_id",
        "templateId",
        "name",
    ]
    .iter()
    .filter_map(|key| object.get(*key).and_then(Value::as_str))
    .any(assistant_run_code_review_marker_text);
    let has_findings = ["findings", "issues", "items"].iter().any(|key| {
        object
            .get(*key)
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
    });
    let has_summary = code_review_payload_string(
        value,
        &["summary", "message", "description", "body", "conclusion"],
    )
    .is_some();
    marker || (force_marker && (has_findings || has_summary))
}

fn assistant_run_code_review_marker_text(value: &str) -> bool {
    let normalized = value
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' ', '.'], "_");
    normalized.contains("code_review")
        || normalized.contains("review_summary")
        || normalized.contains("code_review_summary")
}

fn code_review_summary_artifact_from_candidate(
    run: &AssistantRun,
    candidate: CodeReviewSummaryArtifactCandidate,
    signature: &str,
) -> Option<HtmlArtifactManifestView> {
    let findings = code_review_summary_findings(&candidate.payload);
    let summary = code_review_payload_string(
        &candidate.payload,
        &["summary", "message", "description", "body", "conclusion"],
    )
    .map(|value| html_artifact_safe_summary_text(&value, 900))
    .filter(|value| !value.is_empty())
    .unwrap_or_else(|| {
        if findings.is_empty() {
            String::new()
        } else {
            format!("本次代码审查归纳出 {} 个发现。", findings.len())
        }
    });
    if summary.is_empty() && findings.is_empty() {
        return None;
    }

    let title = code_review_payload_string(&candidate.payload, &["title", "name"])
        .map(|value| html_artifact_safe_summary_text(&value, 120))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "代码审查摘要".to_string());
    let signature_digest = sha256_hex([
        run.id.to_string().as_bytes(),
        b":",
        signature.as_bytes(),
        b":",
        candidate.source_kind.as_bytes(),
        b":",
        candidate.source_index.to_string().as_bytes(),
    ]);
    let artifact_id = format!("html-code-review-{}", &signature_digest[..16]);
    let run_id = run.id.to_string();
    let finding_count = findings.len();

    Some(HtmlArtifactManifestView {
        kind: "html_artifact".to_string(),
        version: 1,
        id: artifact_id,
        title,
        source_type: contracts::HtmlArtifactSourceTypeView::CodeReview,
        template_id: contracts::HtmlArtifactTemplateIdView::CodeReviewSummary,
        owner_scope: contracts::HtmlArtifactOwnerScopeView {
            scope_type: "assistant_run".to_string(),
            id: run_id.clone(),
        },
        data_refs: vec![contracts::HtmlArtifactDataRefView {
            kind: "assistant_run".to_string(),
            id: run_id.clone(),
            label: "Assistant Run".to_string(),
        }],
        provenance: contracts::HtmlArtifactProvenanceView {
            producer: "v3-platform-api".to_string(),
            reason: "code review summary synthesis".to_string(),
            source_run_id: Some(run_id),
        },
        interaction_mode: HtmlArtifactInteractionModeView::ReadOnly,
        created_at: candidate.created_at,
        payload: json!({
            "summary": summary,
            "findings": findings,
            "findingCount": finding_count,
            "sourceKind": candidate.source_kind,
            "sourceEventName": candidate
                .source_event_name
                .as_deref()
                .map(|value| html_artifact_safe_summary_text(value, 120))
                .unwrap_or_default(),
            "sourceSequenceNo": candidate.sequence_no,
            "modelGuidance": "这是 DataMax 从结构化代码审查输出中合成的只读摘要；如需执行修改，应重新进入受控 AssistantRun/Codex action 流程。"
        }),
    })
}

fn code_review_summary_findings(payload: &Value) -> Vec<Value> {
    ["findings", "issues", "items"]
        .iter()
        .find_map(|key| payload.get(*key).and_then(Value::as_array))
        .map(|items| {
            items
                .iter()
                .take(30)
                .filter_map(code_review_summary_finding_payload)
                .collect()
        })
        .unwrap_or_default()
}

fn code_review_summary_finding_payload(finding: &Value) -> Option<Value> {
    if let Some(text) = finding.as_str() {
        let title = html_artifact_safe_summary_text(text, 180);
        return (!title.is_empty()).then(|| {
            json!({
                "severity": "P?",
                "title": title,
                "detail": "",
                "file": "",
                "line": Value::Null,
            })
        });
    }

    let object = finding.as_object()?;
    let severity = code_review_payload_string(finding, &["severity", "priority", "level"])
        .map(|value| html_artifact_safe_summary_text(&value, 24))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "P?".to_string());
    let title = code_review_payload_string(finding, &["title", "message", "summary", "rule"])
        .map(|value| html_artifact_safe_summary_text(&value, 180))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            code_review_payload_string(finding, &["file", "path"])
                .map(|value| html_artifact_safe_summary_text(&value, 180))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "问题".to_string())
        });
    let detail = code_review_payload_string(finding, &["detail", "body", "description"])
        .map(|value| html_artifact_safe_summary_text(&value, 700))
        .unwrap_or_default();
    let file = code_review_payload_string(finding, &["file", "path", "filename"])
        .map(|value| html_artifact_safe_summary_text(&value, 180))
        .unwrap_or_default();
    let line = [
        "line",
        "startLine",
        "start_line",
        "lineNumber",
        "line_number",
    ]
    .iter()
    .find_map(|key| object.get(*key).and_then(Value::as_u64));
    if title.is_empty() && detail.is_empty() && file.is_empty() {
        return None;
    }
    Some(json!({
        "severity": severity,
        "title": title,
        "detail": detail,
        "file": file,
        "line": line,
    }))
}

fn code_review_payload_string(payload: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| {
            payload.get(*key).and_then(|value| match value {
                Value::String(text) => Some(text.trim().to_string()),
                Value::Number(number) => Some(number.to_string()),
                _ => None,
            })
        })
        .filter(|value| !value.is_empty())
}

fn code_review_summary_payload_signature(payload: &Value) -> String {
    let summary = code_review_payload_string(payload, &["summary", "message", "description"])
        .unwrap_or_default();
    let first_finding = ["findings", "issues", "items"]
        .iter()
        .find_map(|key| payload.get(*key).and_then(Value::as_array))
        .and_then(|items| items.first())
        .and_then(|finding| {
            code_review_payload_string(finding, &["title", "message", "summary", "file", "path"])
                .or_else(|| finding.as_str().map(ToOwned::to_owned))
        })
        .unwrap_or_default();
    format!("{summary}|{first_finding}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain_model::{AssistantRunEventId, AssistantRunId, TenantId};

    fn code_review_run(now: DateTime<Utc>) -> AssistantRun {
        AssistantRun {
            id: AssistantRunId::new(),
            tenant_id: TenantId::new(),
            user_id: None,
            local_thread_id: Some("thread-code-review".to_string()),
            user_prompt: "审查这次改动".to_string(),
            startup_briefing: json!({}),
            selected_scope: json!({}),
            scope_candidates: json!([]),
            context_policy: json!({}),
            evidence_state: json!({}),
            service_lane: "codex_review".to_string(),
            execution_trail: json!([]),
            output_artifacts: json!([]),
            runtime_manifest: json!({}),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn code_review_summary_artifact_exposes_safe_read_only_manifest() {
        let now = Utc::now();
        let mut run = code_review_run(now);
        run.output_artifacts = json!([{
            "type": "code_review_summary",
            "title": "平台接口审查",
            "summary": "发现一个权限边界问题。",
            "findings": [{
                "severity": "P1",
                "title": "缺少所有者校验",
                "detail": "更新接口在保存前需要校验 owner_user_id。",
                "file": "crates/platform-api/src/lib.rs",
                "line": 42
            }]
        }]);

        let artifacts = code_review_summary_artifacts_from_run_values(&run, &[], 10);

        assert_eq!(artifacts.len(), 1);
        let artifact = &artifacts[0];
        assert_eq!(
            artifact.source_type,
            contracts::HtmlArtifactSourceTypeView::CodeReview
        );
        assert_eq!(
            artifact.template_id,
            contracts::HtmlArtifactTemplateIdView::CodeReviewSummary
        );
        assert_eq!(
            artifact.interaction_mode,
            HtmlArtifactInteractionModeView::ReadOnly
        );
        assert_eq!(artifact.owner_scope.id, run.id.to_string());
        assert_eq!(artifact.payload["summary"], json!("发现一个权限边界问题。"));
        assert_eq!(artifact.payload["findingCount"], json!(1));
        assert_eq!(
            artifact.payload["findings"][0]["file"],
            json!("crates/platform-api/src/lib.rs")
        );
        assert_eq!(artifact.payload["findings"][0]["line"], json!(42));
    }

    #[test]
    fn code_review_summary_artifact_uses_event_marker_and_redacts_unsafe_strings() {
        let now = Utc::now();
        let run = code_review_run(now);
        let event = AssistantRunEvent {
            id: AssistantRunEventId::new(),
            tenant_id: run.tenant_id,
            run_id: run.id,
            sequence_no: 3,
            event_name: "codex.code_review.completed".to_string(),
            payload: json!({
                "summary": "结果见 https://example.invalid/raw-log",
                "findings": [{
                    "severity": "P2",
                    "title": "不要暴露 Authorization header",
                    "detail": "Bearer raw-token should not be visible",
                    "file": "src/auth.rs",
                    "line": 7
                }]
            }),
            created_at: now,
        };

        let artifacts = code_review_summary_artifacts_from_run_values(&run, &[event], 10);
        let serialized =
            serde_json::to_string(&artifacts).expect("code review artifact serializes safely");

        assert_eq!(artifacts.len(), 1);
        assert_eq!(
            artifacts[0].payload["sourceEventName"],
            json!("codex.code_review.completed")
        );
        assert_eq!(artifacts[0].payload["sourceSequenceNo"], json!(3));
        assert!(serialized.contains("已移除敏感或不安全内容"));
        assert!(!serialized.contains("https://example.invalid"));
        assert!(!serialized.to_ascii_lowercase().contains("bearer raw-token"));
    }
}
