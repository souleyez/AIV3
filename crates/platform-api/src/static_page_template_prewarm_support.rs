use chrono::{DateTime, Duration, Utc};
use contracts::ExternalBotMessageView;
use domain_model::AssistantRun;
use serde_json::{json, Map, Value};

use crate::{
    ensure_json_object, external_answer_policy_value, external_channel_platform_wire_value,
    external_message_type_wire_value, external_requested_skills_summary, sha256_hex,
    static_page_dataset_artifact_key, static_page_default_prompt_from_scope_or_refs,
    static_page_default_prompt_template_token, static_page_template_match_tokens,
    static_page_template_stability_key_with_default_prompt,
};

pub(crate) const STATIC_PAGE_TEMPLATE_PREWARM_TASK_KEY: &str = "prewarm_static_page_template";
pub(crate) const STATIC_PAGE_TEMPLATE_PREWARM_SOURCE: &str =
    "external_channel_static_page_template_prewarm_candidate";

#[derive(Debug, Clone)]
pub(crate) struct StaticPageTemplatePrewarmCandidate {
    pub(crate) prewarm_key: String,
    pub(crate) scope_tokens: Vec<String>,
    pub(crate) source_refs: Value,
    pub(crate) template_stability_key: String,
    pub(crate) dataset_artifact_key: Option<String>,
}

pub(crate) fn static_page_template_prewarm_source_refs(
    connection_id: &str,
    message: &ExternalBotMessageView,
) -> Value {
    json!({
        "source": STATIC_PAGE_TEMPLATE_PREWARM_SOURCE,
        "auto_publish_generated_artifact": true,
        "effect_image_confirmation_required": false,
        "continue_to_publish_after_effect_image": true,
        "prewarm": {
            "mode": "silent_low_load_template_prewarm",
            "customer_visible": false,
            "trigger": "external_channel_conversation_with_authorized_scope",
        },
        "channel_connection_id": connection_id,
        "platform": external_channel_platform_wire_value(&message.platform),
        "tenant_external_id": message.tenant_external_id,
        "bot_external_id": message.bot_external_id,
        "conversation_external_id": message.conversation_external_id,
        "thread_external_id": message.thread_external_id,
        "sender_external_id": message.sender_external_id,
        "message_external_id": message.message_external_id,
        "message_type": external_message_type_wire_value(&message.message_type),
        "artifact_type": "static_page",
        "output_format": message.output_format,
        "render_mode": "artifact",
        "requested_skills": external_requested_skills_summary(&message.requested_skills),
        "answer_policy": external_answer_policy_value(message),
    })
}

pub(crate) fn static_page_template_prewarm_candidate(
    connection_id: &str,
    selected_scope: &Value,
    message: &ExternalBotMessageView,
) -> Option<StaticPageTemplatePrewarmCandidate> {
    let mut source_refs = static_page_template_prewarm_source_refs(connection_id, message);
    let tokens = static_page_template_match_tokens(selected_scope, &source_refs);
    if tokens.is_empty() {
        return None;
    }
    let scope_tokens = tokens.into_iter().collect::<Vec<_>>();
    let default_prompt_token =
        static_page_default_prompt_template_token(selected_scope, &source_refs);
    let joined_tokens = scope_tokens.join("|");
    let prewarm_hash = sha256_hex([
        connection_id.as_bytes(),
        b":",
        joined_tokens.as_bytes(),
        b":",
        default_prompt_token.as_bytes(),
    ]);
    let prewarm_key = format!("static-page-template-prewarm:{}", &prewarm_hash[..24]);
    source_refs = static_page_template_prewarm_source_refs_with_key(source_refs, &prewarm_key);
    let template_stability_key = static_page_template_stability_key_with_default_prompt(
        "template:default",
        static_page_default_prompt_from_scope_or_refs(selected_scope, &source_refs),
    );
    let dataset_artifact_key = static_page_dataset_artifact_key(
        selected_scope,
        &source_refs,
        &template_stability_key,
        Some(connection_id),
    );
    Some(StaticPageTemplatePrewarmCandidate {
        prewarm_key,
        scope_tokens,
        source_refs,
        template_stability_key,
        dataset_artifact_key,
    })
}

pub(crate) fn static_page_template_prewarm_source_refs_with_key(
    mut source_refs: Value,
    prewarm_key: &str,
) -> Value {
    ensure_json_object(&mut source_refs);
    if let Some(object) = source_refs.as_object_mut() {
        object.insert("prewarm_key".to_string(), json!(prewarm_key));
        let prewarm = object
            .entry("prewarm".to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(prewarm_object) = prewarm.as_object_mut() {
            prewarm_object.insert("key".to_string(), json!(prewarm_key));
        }
    }
    source_refs
}

pub(crate) fn static_page_template_prewarm_key_from_source_refs(
    source_refs: &Value,
) -> Option<String> {
    source_refs
        .pointer("/prewarm/key")
        .or_else(|| source_refs.get("prewarm_key"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn static_page_template_prewarm_delay(now: DateTime<Utc>) -> DateTime<Utc> {
    let delay_minutes = std::env::var("STATIC_PAGE_TEMPLATE_PREWARM_DELAY_MINUTES")
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .unwrap_or(30)
        .clamp(1, 24 * 60);
    now + Duration::minutes(delay_minutes)
}

pub(crate) fn static_page_template_prewarm_low_load_policy() -> Value {
    json!({
        "mode": "low_load_only",
        "customer_visible": false,
        "execution_priority": "background",
        "worker_must_recheck_before_image2": true,
        "skip_if_any_scope_template_exists": true,
        "max_parallel_image2_html": 1,
        "cloudflare_fallback_parallelism": 1,
        "load_checks": [
            "explicit_customer_static_page_queue_empty_or_low",
            "codex_host_queue_below_threshold",
            "model_gateway_assistant_chat_below_threshold",
            "no_demo_freeze_window"
        ],
    })
}

pub(crate) fn static_page_template_prewarm_prompt(message: &ExternalBotMessageView) -> String {
    let prompt_hint = message
        .default_prompt
        .as_deref()
        .or(message.text.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(400).collect::<String>())
        .unwrap_or_else(|| "按当前授权数据集组合生成通用经营可视化页面模板。".to_string());
    format!(
        "为当前授权数据集组合预热一套客户不可见、可复用的 DataMax 动态可视化页面模板。\
默认使用现有数据源和文档范围，保留时间范围、区域/门店/分区等筛选能力；页面完成后仅作为同一数据集组合和相近 default_prompt 的默认模板，客户未明确要求时不要发送说明。\
当前主题/默认提示：{prompt_hint}"
    )
}

pub(crate) fn static_page_template_prewarm_task_payload(
    connection_id: &str,
    run: &AssistantRun,
    message: &ExternalBotMessageView,
    selected_scope: &Value,
    candidate: &StaticPageTemplatePrewarmCandidate,
    now: DateTime<Utc>,
) -> Value {
    json!({
        "type": "static_page_template_prewarm_candidate",
        "logical_queue": "static_page_template_prewarm",
        "logical_task_key": "prepare_template_when_low_load",
        "prewarm_key": candidate.prewarm_key,
        "template_id": "static_page_image2_data_publish",
        "fixed_task_template_id": "static_page_image2_data_publish",
        "channel_connection_id": connection_id,
        "platform": external_channel_platform_wire_value(&message.platform),
        "conversation_external_id": message.conversation_external_id,
        "message_external_id": message.message_external_id,
        "assistant_run_id": run.id.to_string(),
        "local_thread_id": run.local_thread_id,
        "scope_tokens": candidate.scope_tokens,
        "selected_scope": selected_scope,
        "source_refs": candidate.source_refs,
        "template_stability_key": candidate.template_stability_key,
        "dataset_artifact_key": candidate.dataset_artifact_key,
        "default_prompt_match_policy": "same_default_prompt_required",
        "low_load_policy": static_page_template_prewarm_low_load_policy(),
        "execution_contract": {
            "next_action": "create_image2_visual_then_static_page_template_only_when_low_load",
            "customer_reply_policy": "silent_unless_user_requests_static_page",
            "permission_scope": "selected_external_channel_scope_only",
            "public_api_change_allowed": false,
            "auth_change_allowed": false,
            "request_response_field_change_allowed": false
        },
        "created_at": now.to_rfc3339(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain_model::{AssistantRunId, TenantId};

    fn sample_message() -> ExternalBotMessageView {
        serde_json::from_value(json!({
            "platform": "generic_chat",
            "tenant_external_id": "tenant-ext-001",
            "bot_external_id": "bot-v3",
            "conversation_external_id": "chat-prewarm-room",
            "sender_external_id": "user-ext-001",
            "message_external_id": "msg-001",
            "message_type": "text",
            "text": "看看整体经营情况",
            "default_prompt": "新世界经营月报",
            "output_format": "rich_text",
            "idempotency_key": "generic:tenant-ext-001:msg-001",
            "received_at": Utc::now()
        }))
        .expect("sample external bot message")
    }

    #[test]
    fn prewarm_candidate_uses_scope_and_stays_customer_invisible() {
        let message = sample_message();
        let selected_scope = json!({
            "type": "external_channel",
            "requested_dataset_external_ids": ["dataset-b", "dataset-a"]
        });

        let candidate =
            static_page_template_prewarm_candidate("generic-chat-main", &selected_scope, &message)
                .expect("scope should create a candidate");

        assert!(candidate
            .scope_tokens
            .contains(&"dataset_external:dataset-a".to_string()));
        assert!(candidate
            .prewarm_key
            .starts_with("static-page-template-prewarm:"));
        assert_eq!(
            candidate.source_refs["source"],
            json!(STATIC_PAGE_TEMPLATE_PREWARM_SOURCE)
        );
        assert_eq!(
            candidate.source_refs["prewarm"]["customer_visible"],
            json!(false)
        );
        assert_eq!(
            static_page_template_prewarm_key_from_source_refs(&candidate.source_refs).as_deref(),
            Some(candidate.prewarm_key.as_str())
        );
        assert!(candidate
            .dataset_artifact_key
            .as_deref()
            .is_some_and(|value| value.contains("dataset_external_id:dataset-a")));
    }

    #[test]
    fn prewarm_task_payload_declares_low_load_silent_contract() {
        let message = sample_message();
        let selected_scope = json!({
            "type": "external_channel",
            "requested_dataset_external_ids": ["dataset-a"]
        });
        let candidate =
            static_page_template_prewarm_candidate("generic-chat-main", &selected_scope, &message)
                .expect("scope should create a candidate");
        let now = Utc::now();
        let run = AssistantRun {
            id: AssistantRunId::new(),
            tenant_id: TenantId::new(),
            user_id: None,
            local_thread_id: Some("external:chat-prewarm-room".to_string()),
            user_prompt: "看看整体经营情况".to_string(),
            startup_briefing: json!({}),
            selected_scope: selected_scope.clone(),
            scope_candidates: json!([]),
            context_policy: json!({}),
            evidence_state: json!({}),
            service_lane: "external_channel".to_string(),
            execution_trail: json!([]),
            output_artifacts: json!([]),
            runtime_manifest: json!({}),
            created_at: now,
            updated_at: now,
        };

        let payload = static_page_template_prewarm_task_payload(
            "generic-chat-main",
            &run,
            &message,
            &selected_scope,
            &candidate,
            now,
        );

        assert_eq!(
            payload["logical_queue"],
            json!("static_page_template_prewarm")
        );
        assert_eq!(payload["low_load_policy"]["mode"], json!("low_load_only"));
        assert_eq!(
            payload["low_load_policy"]["max_parallel_image2_html"],
            json!(1)
        );
        assert_eq!(
            payload["execution_contract"]["customer_reply_policy"],
            json!("silent_unless_user_requests_static_page")
        );
        assert_eq!(
            payload["execution_contract"]["request_response_field_change_allowed"],
            json!(false)
        );
        assert_eq!(
            payload["source_refs"]["prewarm"]["customer_visible"],
            json!(false)
        );
    }
}
