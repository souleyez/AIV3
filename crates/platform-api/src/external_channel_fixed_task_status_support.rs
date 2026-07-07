use domain_model::{AssistantRun, AssistantRunEvent};
use serde_json::Value;

use crate::assistant_run_codex_fixed_task_support::codex_host_fixed_task_public_artifact_url_allowed;

pub(crate) fn external_channel_fixed_task_template_id(event: &AssistantRunEvent) -> Option<String> {
    event
        .payload
        .get("template_id")
        .and_then(Value::as_str)
        .or_else(|| event.payload.get("capability").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn external_channel_fixed_task_workflow_execution_id(
    event: &AssistantRunEvent,
) -> Value {
    event
        .payload
        .get("workflow_execution_id")
        .or_else(|| event.payload.get("codex_host_workflow_execution_id"))
        .cloned()
        .unwrap_or(Value::Null)
}

pub(crate) fn external_channel_fixed_task_card_type(template_id: &str) -> &'static str {
    match template_id {
        "data_ingestion_analysis" => "v3_data_ingestion_analysis",
        "static_page_image2_data_publish" => "v3_static_page_image2_publish_status",
        "answer_quality_autofix" => "v3_answer_quality_autofix",
        _ => "v3_codex_fixed_task",
    }
}

pub(crate) fn external_channel_fixed_task_status_prefix(template_id: &str) -> &'static str {
    match template_id {
        "data_ingestion_analysis" => "data_ingestion_analysis",
        "static_page_image2_data_publish" => "static_page_publish",
        "answer_quality_autofix" => "answer_quality_autofix",
        _ => "codex_fixed_task",
    }
}

pub(crate) fn external_channel_fixed_task_processing_text(
    template_id: &str,
    state: &str,
) -> &'static str {
    match (template_id, state) {
        ("data_ingestion_analysis", "retrying") => {
            "DataMax 数据接入分析仍在执行，后台任务超时后已自动续轮询。"
        }
        ("data_ingestion_analysis", "cancelled") => {
            "DataMax 数据接入分析任务已取消，未写入生产库，也未继续修改数据集。"
        }
        ("data_ingestion_analysis", "failed") => {
            "DataMax 数据接入分析未完成，已记录失败原因，需重试或人工处理。"
        }
        ("data_ingestion_analysis", _) => "DataMax 数据接入分析正在执行，请稍后查询结果。",
        ("static_page_image2_data_publish", "retrying") => {
            "DataMax 静态页发布仍在执行，后台任务超时后已自动续轮询。"
        }
        ("static_page_image2_data_publish", "cancelled") => {
            "DataMax 静态页发布任务已取消，未生成新的最终发布链接。"
        }
        ("static_page_image2_data_publish", "failed") => {
            "DataMax 静态页发布未完成，需重试或人工处理。"
        }
        ("static_page_image2_data_publish", _) => "DataMax 已完成页面规划，正在生成最终静态页。",
        ("answer_quality_autofix", "cancelled") => {
            "DataMax 回答质量修复诊断任务已取消，未应用任何代码或配置变更。"
        }
        ("answer_quality_autofix", "retrying") => {
            "DataMax 回答质量修复诊断仍在执行，已自动续轮询。"
        }
        ("answer_quality_autofix", "failed") => "DataMax 回答质量修复诊断未完成。",
        ("answer_quality_autofix", _) => "DataMax 回答质量修复诊断正在执行。",
        (_, "cancelled") => "DataMax 后台任务已取消。",
        (_, "retrying") => "DataMax 后台任务仍在执行，已自动续轮询。",
        (_, "failed") => "DataMax 后台任务未完成，需重试或人工处理。",
        _ => "DataMax 后台任务正在执行。",
    }
}

pub(crate) fn external_channel_fixed_task_processing_poll_after_seconds(state: &str) -> Value {
    match state {
        "retrying" => Value::from(30),
        "cancelled" | "failed" => Value::Null,
        _ => Value::from(15),
    }
}

pub(crate) fn external_channel_fixed_task_terminal_state(event_name: &str) -> &'static str {
    match event_name {
        "codex_host.fixed_task.completed" => "completed",
        "codex_host.fixed_task.needs_human" => "needs_human",
        "codex_host.fixed_task.rejected" => "failed",
        _ => "queued",
    }
}

pub(crate) fn external_channel_fixed_task_terminal_task_status(
    template_id: &str,
    terminal_state: &str,
) -> String {
    match (template_id, terminal_state) {
        ("static_page_image2_data_publish", "completed") => "static_page_published".to_string(),
        _ => format!(
            "{}_{}",
            external_channel_fixed_task_status_prefix(template_id),
            terminal_state
        ),
    }
}

pub(crate) fn external_channel_fixed_task_terminal_text(
    template_id: &str,
    terminal_state: &str,
) -> &'static str {
    match (template_id, terminal_state) {
        ("data_ingestion_analysis", "completed") => {
            "DataMax 已完成数据接入分析，已生成只读质量报告、字段映射和后续动作建议。"
        }
        ("data_ingestion_analysis", "needs_human") => {
            "DataMax 数据接入分析需要人工确认后继续，当前不会自动写库或修改 schema。"
        }
        ("data_ingestion_analysis", "failed") => {
            "DataMax 数据接入分析未完成，已记录失败原因，需重试或人工处理。"
        }
        ("data_ingestion_analysis", _) => "DataMax 已提交数据接入分析任务。",
        ("static_page_image2_data_publish", "completed") => "DataMax 静态页已生成并发布。",
        ("static_page_image2_data_publish", "needs_human") => {
            "DataMax 静态页发布需要人工确认或处理。"
        }
        ("static_page_image2_data_publish", "failed") => {
            "DataMax 静态页发布未完成，需重试或人工处理。"
        }
        ("static_page_image2_data_publish", _) => "DataMax 已提交静态页发布任务。",
        ("answer_quality_autofix", "completed") => "DataMax 已完成回答质量修复诊断。",
        ("answer_quality_autofix", "needs_human") => "DataMax 回答质量修复诊断需要人工审查后继续。",
        ("answer_quality_autofix", "failed") => "DataMax 回答质量修复诊断未完成。",
        ("answer_quality_autofix", _) => "DataMax 已提交回答质量修复诊断任务。",
        (_, "completed") => "DataMax 后台任务已完成。",
        (_, "needs_human") => "DataMax 后台任务需要人工处理。",
        (_, "failed") => "DataMax 后台任务未完成，需重试或人工处理。",
        _ => "DataMax 后台任务已提交。",
    }
}

pub(crate) fn external_channel_static_page_fixed_task_event_with_status_context(
    events: &[AssistantRunEvent],
    fixed_event: &AssistantRunEvent,
) -> AssistantRunEvent {
    let mut event = fixed_event.clone();
    let Some(context) = events.iter().rev().find(|candidate| {
        candidate.sequence_no <= fixed_event.sequence_no
            && matches!(
                candidate.event_name.as_str(),
                "assistant_run.external_channel_static_page_pipeline_queued"
                    | "assistant_run.external_channel_static_page_publish_queued"
            )
    }) else {
        return event;
    };
    let Some(target) = event.payload.as_object_mut() else {
        return event;
    };
    for key in [
        "status_url",
        "status_method",
        "draft_id",
        "image_job_id",
        "poll_after_seconds",
        "recipient_delivery",
        "permission_review_status",
        "editable_after_publish",
    ] {
        if target.get(key).is_none_or(Value::is_null) {
            if let Some(value) = context.payload.get(key) {
                target.insert(key.to_string(), value.clone());
            }
        }
    }
    event
}

pub(crate) fn external_channel_static_page_latest_codex_heartbeat_after(
    events: &[AssistantRunEvent],
    sequence_no: i32,
) -> Option<&AssistantRunEvent> {
    events.iter().rev().find(|event| {
        event.sequence_no > sequence_no
            && matches!(
                event.event_name.as_str(),
                "codex_host_task.cloudflare_heartbeat" | "codex_host_task.exec_heartbeat"
            )
    })
}

pub(crate) fn external_channel_static_page_latest_named_event<'a>(
    events: &'a [AssistantRunEvent],
    event_name: &str,
) -> Option<&'a AssistantRunEvent> {
    events
        .iter()
        .rev()
        .find(|event| event.event_name == event_name)
}

pub(crate) fn external_channel_static_page_run_has_published_artifact(run: &AssistantRun) -> bool {
    run.output_artifacts
        .as_array()
        .map(|artifacts| {
            artifacts.iter().any(|artifact| {
                artifact.get("type").and_then(Value::as_str)
                    == Some("external_channel_static_page_artifact")
                    && artifact
                        .get("public_url")
                        .and_then(Value::as_str)
                        .map(codex_host_fixed_task_public_artifact_url_allowed)
                        .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

pub(crate) fn external_channel_static_page_recovery_workflow_execution_id(
    events: &[AssistantRunEvent],
    exec_event: &AssistantRunEvent,
) -> Option<String> {
    for pointer in [
        "/codex_host_workflow_execution_id",
        "/workflow_execution_id",
        "/execution_id",
    ] {
        if let Some(value) = exec_event
            .payload
            .pointer(pointer)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(value.to_string());
        }
    }
    external_channel_static_page_recovery_publish_queued_payload(events, exec_event)
        .and_then(|payload| {
            payload
                .get("codex_host_workflow_execution_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
        .map(str::to_string)
}

pub(crate) fn external_channel_static_page_recovery_publish_queued_payload<'a>(
    events: &'a [AssistantRunEvent],
    exec_event: &AssistantRunEvent,
) -> Option<&'a Value> {
    let exec_workflow_id = exec_event
        .payload
        .get("codex_host_workflow_execution_id")
        .or_else(|| exec_event.payload.get("workflow_execution_id"))
        .or_else(|| exec_event.payload.get("execution_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    events
        .iter()
        .rev()
        .filter(|event| event.sequence_no <= exec_event.sequence_no)
        .find(|event| {
            if event.event_name != "assistant_run.external_channel_static_page_publish_queued" {
                return false;
            }
            if let Some(exec_workflow_id) = exec_workflow_id {
                return event
                    .payload
                    .get("codex_host_workflow_execution_id")
                    .and_then(Value::as_str)
                    == Some(exec_workflow_id);
            }
            true
        })
        .map(|event| &event.payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{AssistantRunEventId, AssistantRunId, TenantId};
    use serde_json::json;

    fn assistant_event(sequence_no: i32, event_name: &str, payload: Value) -> AssistantRunEvent {
        AssistantRunEvent {
            id: AssistantRunEventId::new(),
            tenant_id: TenantId::new(),
            run_id: AssistantRunId::new(),
            sequence_no,
            event_name: event_name.to_string(),
            payload,
            created_at: Utc::now(),
        }
    }

    fn fixed_task_event(payload: Value) -> AssistantRunEvent {
        assistant_event(1, "codex_host.fixed_task.queued", payload)
    }

    fn assistant_run_with_output_artifacts(output_artifacts: Value) -> AssistantRun {
        let now = Utc::now();
        AssistantRun {
            id: AssistantRunId::new(),
            tenant_id: TenantId::new(),
            user_id: None,
            local_thread_id: None,
            user_prompt: "生成报表".to_string(),
            startup_briefing: json!({}),
            selected_scope: json!({}),
            scope_candidates: json!([]),
            context_policy: json!({}),
            evidence_state: json!({}),
            service_lane: "external_channel".to_string(),
            execution_trail: json!([]),
            output_artifacts,
            runtime_manifest: json!({}),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn template_id_prefers_template_id_and_trims_blank() {
        let event = fixed_task_event(json!({
            "template_id": " static_page_image2_data_publish ",
            "capability": "data_ingestion_analysis"
        }));

        assert_eq!(
            external_channel_fixed_task_template_id(&event).as_deref(),
            Some("static_page_image2_data_publish")
        );
    }

    #[test]
    fn template_id_falls_back_to_capability_and_ignores_empty_values() {
        let event = fixed_task_event(json!({
            "capability": " answer_quality_autofix "
        }));
        assert_eq!(
            external_channel_fixed_task_template_id(&event).as_deref(),
            Some("answer_quality_autofix")
        );

        let empty = fixed_task_event(json!({
            "template_id": " ",
            "capability": " "
        }));
        assert_eq!(external_channel_fixed_task_template_id(&empty), None);
    }

    #[test]
    fn workflow_execution_id_prefers_workflow_execution_id() {
        let event = fixed_task_event(json!({
            "workflow_execution_id": "workflow-a",
            "codex_host_workflow_execution_id": "workflow-b"
        }));
        assert_eq!(
            external_channel_fixed_task_workflow_execution_id(&event),
            json!("workflow-a")
        );

        let fallback = fixed_task_event(json!({
            "codex_host_workflow_execution_id": "workflow-b"
        }));
        assert_eq!(
            external_channel_fixed_task_workflow_execution_id(&fallback),
            json!("workflow-b")
        );

        let missing = fixed_task_event(json!({}));
        assert_eq!(
            external_channel_fixed_task_workflow_execution_id(&missing),
            Value::Null
        );
    }

    #[test]
    fn card_type_maps_known_templates_and_unknown_default() {
        assert_eq!(
            external_channel_fixed_task_card_type("data_ingestion_analysis"),
            "v3_data_ingestion_analysis"
        );
        assert_eq!(
            external_channel_fixed_task_card_type("static_page_image2_data_publish"),
            "v3_static_page_image2_publish_status"
        );
        assert_eq!(
            external_channel_fixed_task_card_type("answer_quality_autofix"),
            "v3_answer_quality_autofix"
        );
        assert_eq!(
            external_channel_fixed_task_card_type("unknown"),
            "v3_codex_fixed_task"
        );
    }

    #[test]
    fn status_prefix_maps_known_templates_and_unknown_default() {
        assert_eq!(
            external_channel_fixed_task_status_prefix("data_ingestion_analysis"),
            "data_ingestion_analysis"
        );
        assert_eq!(
            external_channel_fixed_task_status_prefix("static_page_image2_data_publish"),
            "static_page_publish"
        );
        assert_eq!(
            external_channel_fixed_task_status_prefix("answer_quality_autofix"),
            "answer_quality_autofix"
        );
        assert_eq!(
            external_channel_fixed_task_status_prefix("unknown"),
            "codex_fixed_task"
        );
    }

    #[test]
    fn processing_text_maps_template_and_state() {
        assert_eq!(
            external_channel_fixed_task_processing_text("data_ingestion_analysis", "retrying"),
            "DataMax 数据接入分析仍在执行，后台任务超时后已自动续轮询。"
        );
        assert_eq!(
            external_channel_fixed_task_processing_text(
                "static_page_image2_data_publish",
                "failed"
            ),
            "DataMax 静态页发布未完成，需重试或人工处理。"
        );
        assert_eq!(
            external_channel_fixed_task_processing_text("answer_quality_autofix", "cancelled"),
            "DataMax 回答质量修复诊断任务已取消，未应用任何代码或配置变更。"
        );
        assert_eq!(
            external_channel_fixed_task_processing_text("unknown", "running"),
            "DataMax 后台任务正在执行。"
        );
    }

    #[test]
    fn processing_poll_after_seconds_maps_terminal_and_retry_states() {
        assert_eq!(
            external_channel_fixed_task_processing_poll_after_seconds("retrying"),
            Value::from(30)
        );
        assert_eq!(
            external_channel_fixed_task_processing_poll_after_seconds("running"),
            Value::from(15)
        );
        assert_eq!(
            external_channel_fixed_task_processing_poll_after_seconds("failed"),
            Value::Null
        );
        assert_eq!(
            external_channel_fixed_task_processing_poll_after_seconds("cancelled"),
            Value::Null
        );
    }

    #[test]
    fn terminal_state_maps_fixed_task_event_names() {
        assert_eq!(
            external_channel_fixed_task_terminal_state("codex_host.fixed_task.completed"),
            "completed"
        );
        assert_eq!(
            external_channel_fixed_task_terminal_state("codex_host.fixed_task.needs_human"),
            "needs_human"
        );
        assert_eq!(
            external_channel_fixed_task_terminal_state("codex_host.fixed_task.rejected"),
            "failed"
        );
        assert_eq!(
            external_channel_fixed_task_terminal_state("codex_host.fixed_task.queued"),
            "queued"
        );
    }

    #[test]
    fn terminal_task_status_preserves_static_page_publish_alias() {
        assert_eq!(
            external_channel_fixed_task_terminal_task_status(
                "static_page_image2_data_publish",
                "completed"
            ),
            "static_page_published"
        );
        assert_eq!(
            external_channel_fixed_task_terminal_task_status(
                "data_ingestion_analysis",
                "needs_human"
            ),
            "data_ingestion_analysis_needs_human"
        );
        assert_eq!(
            external_channel_fixed_task_terminal_task_status("unknown", "failed"),
            "codex_fixed_task_failed"
        );
    }

    #[test]
    fn terminal_text_maps_template_and_state() {
        assert_eq!(
            external_channel_fixed_task_terminal_text("data_ingestion_analysis", "completed"),
            "DataMax 已完成数据接入分析，已生成只读质量报告、字段映射和后续动作建议。"
        );
        assert_eq!(
            external_channel_fixed_task_terminal_text(
                "static_page_image2_data_publish",
                "needs_human"
            ),
            "DataMax 静态页发布需要人工确认或处理。"
        );
        assert_eq!(
            external_channel_fixed_task_terminal_text("answer_quality_autofix", "failed"),
            "DataMax 回答质量修复诊断未完成。"
        );
        assert_eq!(
            external_channel_fixed_task_terminal_text("unknown", "queued"),
            "DataMax 后台任务已提交。"
        );
    }

    #[test]
    fn static_page_fixed_task_status_context_copies_missing_fields_from_latest_context() {
        let fixed_event = assistant_event(
            8,
            "codex_host.fixed_task.queued",
            json!({
                "status_url": null,
                "status_method": "POST",
                "template_id": "static_page_image2_data_publish"
            }),
        );
        let events = vec![
            assistant_event(
                3,
                "assistant_run.external_channel_static_page_pipeline_queued",
                json!({
                    "status_url": "https://old.example/status",
                    "draft_id": "draft-old",
                    "recipient_delivery": {"enabled": false}
                }),
            ),
            assistant_event(
                7,
                "assistant_run.external_channel_static_page_publish_queued",
                json!({
                    "status_url": "https://new.example/status",
                    "status_method": "GET",
                    "draft_id": "draft-new",
                    "image_job_id": "image-job-new",
                    "poll_after_seconds": 15,
                    "recipient_delivery": {"enabled": true},
                    "permission_review_status": "ready",
                    "editable_after_publish": true
                }),
            ),
        ];

        let event = external_channel_static_page_fixed_task_event_with_status_context(
            &events,
            &fixed_event,
        );

        assert_eq!(
            event.payload["status_url"],
            json!("https://new.example/status")
        );
        assert_eq!(event.payload["status_method"], json!("POST"));
        assert_eq!(event.payload["draft_id"], json!("draft-new"));
        assert_eq!(event.payload["image_job_id"], json!("image-job-new"));
        assert_eq!(event.payload["poll_after_seconds"], json!(15));
        assert_eq!(
            event.payload["recipient_delivery"],
            json!({"enabled": true})
        );
        assert_eq!(event.payload["permission_review_status"], json!("ready"));
        assert_eq!(event.payload["editable_after_publish"], json!(true));
    }

    #[test]
    fn static_page_fixed_task_status_context_ignores_future_and_non_context_events() {
        let fixed_event = assistant_event(
            5,
            "codex_host.fixed_task.queued",
            json!({"template_id": "static_page_image2_data_publish"}),
        );
        let events = vec![
            assistant_event(
                4,
                "assistant_run.external_channel_static_page_started",
                json!({"status_url": "https://wrong.example/status"}),
            ),
            assistant_event(
                6,
                "assistant_run.external_channel_static_page_publish_queued",
                json!({"status_url": "https://future.example/status"}),
            ),
        ];

        let event = external_channel_static_page_fixed_task_event_with_status_context(
            &events,
            &fixed_event,
        );

        assert_eq!(event.payload.get("status_url"), None);
    }

    #[test]
    fn static_page_fixed_task_status_context_requires_object_payload() {
        let fixed_event = assistant_event(5, "codex_host.fixed_task.queued", json!("raw"));
        let events = vec![assistant_event(
            4,
            "assistant_run.external_channel_static_page_publish_queued",
            json!({"status_url": "https://new.example/status"}),
        )];

        let event = external_channel_static_page_fixed_task_event_with_status_context(
            &events,
            &fixed_event,
        );

        assert_eq!(event.payload, json!("raw"));
    }

    #[test]
    fn latest_codex_heartbeat_after_finds_latest_runtime_heartbeat_after_sequence() {
        let events = vec![
            assistant_event(
                3,
                "codex_host_task.exec_heartbeat",
                json!({"heartbeat_count": 1}),
            ),
            assistant_event(6, "codex_host_task.poll_retry", json!({"attempt": 1})),
            assistant_event(
                7,
                "codex_host_task.cloudflare_heartbeat",
                json!({"heartbeat_count": 2}),
            ),
            assistant_event(
                9,
                "codex_host_task.exec_heartbeat",
                json!({"heartbeat_count": 3}),
            ),
        ];

        let heartbeat = external_channel_static_page_latest_codex_heartbeat_after(&events, 5)
            .expect("heartbeat");

        assert_eq!(heartbeat.sequence_no, 9);
        assert_eq!(heartbeat.payload["heartbeat_count"], json!(3));
        assert!(external_channel_static_page_latest_codex_heartbeat_after(&events, 9).is_none());
    }

    #[test]
    fn latest_named_event_finds_latest_matching_event() {
        let events = vec![
            assistant_event(
                1,
                "static_page_image_job.created",
                json!({"image_job_id": "old"}),
            ),
            assistant_event(
                2,
                "static_page_image_job.running",
                json!({"image_job_id": "running"}),
            ),
            assistant_event(
                3,
                "static_page_image_job.created",
                json!({"image_job_id": "new"}),
            ),
        ];

        let event = external_channel_static_page_latest_named_event(
            &events,
            "static_page_image_job.created",
        )
        .expect("created event");

        assert_eq!(event.sequence_no, 3);
        assert_eq!(event.payload["image_job_id"], json!("new"));
        assert!(external_channel_static_page_latest_named_event(&events, "missing").is_none());
    }

    #[test]
    fn static_page_run_has_published_artifact_requires_generated_artifact_url() {
        let valid = assistant_run_with_output_artifacts(json!([
            {
                "type": "external_channel_static_page_artifact",
                "public_url": "https://v3.elepcloud.com/generated-artifacts/report/index.html"
            }
        ]));
        assert!(external_channel_static_page_run_has_published_artifact(
            &valid
        ));

        let invalid_url = assistant_run_with_output_artifacts(json!([
            {
                "type": "external_channel_static_page_artifact",
                "public_url": "https://example.com/report/index.html"
            }
        ]));
        assert!(!external_channel_static_page_run_has_published_artifact(
            &invalid_url
        ));

        let wrong_type = assistant_run_with_output_artifacts(json!([
            {
                "type": "assistant_message",
                "public_url": "https://v3.elepcloud.com/generated-artifacts/report/index.html"
            }
        ]));
        assert!(!external_channel_static_page_run_has_published_artifact(
            &wrong_type
        ));
    }

    #[test]
    fn recovery_publish_queued_payload_matches_latest_prior_event_and_workflow() {
        let exec_event = assistant_event(
            7,
            "codex_host_task.exec_completed",
            json!({"workflow_execution_id": "workflow-b"}),
        );
        let events = vec![
            assistant_event(
                3,
                "assistant_run.external_channel_static_page_publish_queued",
                json!({
                    "codex_host_workflow_execution_id": "workflow-a",
                    "draft_id": "draft-a"
                }),
            ),
            assistant_event(
                6,
                "assistant_run.external_channel_static_page_publish_queued",
                json!({
                    "codex_host_workflow_execution_id": "workflow-b",
                    "draft_id": "draft-b"
                }),
            ),
            assistant_event(
                8,
                "assistant_run.external_channel_static_page_publish_queued",
                json!({
                    "codex_host_workflow_execution_id": "workflow-b",
                    "draft_id": "future"
                }),
            ),
        ];

        let payload =
            external_channel_static_page_recovery_publish_queued_payload(&events, &exec_event)
                .expect("queued payload");

        assert_eq!(payload["draft_id"], json!("draft-b"));
    }

    #[test]
    fn recovery_workflow_execution_id_prefers_exec_event_then_publish_context() {
        let direct = assistant_event(
            5,
            "codex_host_task.exec_completed",
            json!({"execution_id": " workflow-direct "}),
        );
        assert_eq!(
            external_channel_static_page_recovery_workflow_execution_id(&[], &direct).as_deref(),
            Some("workflow-direct")
        );

        let fallback = assistant_event(5, "codex_host_task.exec_completed", json!({}));
        let events = vec![assistant_event(
            4,
            "assistant_run.external_channel_static_page_publish_queued",
            json!({"codex_host_workflow_execution_id": "workflow-fallback"}),
        )];
        assert_eq!(
            external_channel_static_page_recovery_workflow_execution_id(&events, &fallback)
                .as_deref(),
            Some("workflow-fallback")
        );
    }
}
