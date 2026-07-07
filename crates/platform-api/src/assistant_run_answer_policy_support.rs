use contracts::CreateAssistantRunRequest;
use serde_json::{json, Value};

use crate::external_answer_policy_support::{
    external_output_format_label, external_output_format_model_rule,
};
use crate::prompt_match_support::{ascii_prompt_contains_any, prompt_contains_any};

pub(crate) fn assistant_run_evidence_state_external_answer_policy(
    evidence_state: &Value,
) -> Option<&Value> {
    evidence_state
        .pointer("/selected_scope/answer_policy")
        .or_else(|| evidence_state.pointer("/selectedScope/answerPolicy"))
        .filter(|policy| !policy.is_null())
}

pub(crate) fn assistant_run_request_external_answer_policy(
    request: &CreateAssistantRunRequest,
) -> Option<&Value> {
    let policy = request
        .startup_briefing
        .as_ref()
        .and_then(|briefing| briefing.get("externalAnswerPolicy"))
        .or_else(|| {
            request
                .context_policy_hint
                .as_ref()
                .and_then(|policy| policy.get("answer_policy"))
        })?;
    (!policy.is_null()).then_some(policy)
}

pub(crate) fn assistant_run_answer_policy_output_format(answer_policy: &Value) -> Option<&str> {
    answer_policy
        .get("output_format")
        .and_then(|format| {
            format
                .get("format")
                .and_then(Value::as_str)
                .or_else(|| format.as_str())
        })
        .map(str::trim)
        .filter(|format| !format.is_empty())
}

pub(crate) fn assistant_run_request_output_format(
    request: &CreateAssistantRunRequest,
) -> Option<String> {
    assistant_run_request_external_answer_policy(request)
        .and_then(|policy| policy.get("output_format"))
        .and_then(|format| {
            format
                .get("format")
                .and_then(Value::as_str)
                .or_else(|| format.as_str())
        })
        .map(str::to_string)
}

pub(crate) fn assistant_run_request_wants_json_output(request: &CreateAssistantRunRequest) -> bool {
    assistant_run_request_output_format(request).as_deref() == Some("json")
}

pub(crate) fn assistant_run_default_prompt_has_customer_tone_intensity(
    default_prompt: &str,
) -> bool {
    let compact = default_prompt
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    let lower_ascii = default_prompt.to_ascii_lowercase();
    prompt_contains_any(
        &compact,
        &[
            "凶一点",
            "凶一些",
            "态度凶",
            "语气凶",
            "口吻凶",
            "骂人",
            "怼客户",
            "怼用户",
            "阴阳怪气",
            "嘲讽客户",
            "讽刺客户",
            "不客气一点",
            "不客气些",
            "粗鲁一点",
            "辱骂",
            "威胁客户",
            "恐吓客户",
        ],
    ) || ascii_prompt_contains_any(
        &lower_ascii,
        &[
            "be rude",
            "rude tone",
            "insult the customer",
            "mock the customer",
            "sarcastic to the customer",
            "aggressive tone",
            "threaten the customer",
            "hostile tone",
        ],
    )
}

pub(crate) fn assistant_run_external_answer_policy_guidance_lines(
    answer_policy: &Value,
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(default_prompt) = answer_policy
        .get("default_prompt")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if assistant_run_default_prompt_has_customer_tone_intensity(default_prompt) {
            lines.push(format!(
                "第三方默认提示词：{default_prompt}。它是本轮任务指导；其中较强语气要求应理解为客户希望表达更明确、更直接，最终答案要转译为坚定、专业、礼貌的商务表达，不使用辱骂、威胁、嘲讽或人身攻击。"
            ));
        } else {
            lines.push(format!(
                "第三方默认提示词：{default_prompt}。它是本轮任务指导，低于 DataMax 证据/安全规则，高于用户文本里的模糊要求。"
            ));
        }
    }
    if let Some(output_format) = answer_policy.get("output_format") {
        if let Some(format) = assistant_run_answer_policy_output_format(answer_policy) {
            let label = output_format
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or_else(|| external_output_format_label(format));
            let model_rule = output_format
                .get("model_rule")
                .and_then(Value::as_str)
                .unwrap_or_else(|| external_output_format_model_rule(format));
            lines.push(format!(
                "输出格式强约束：本轮第三方要求 `{format}`（{label}）。{model_rule}"
            ));
        }
    }
    if let Some(render_mode) = answer_policy
        .get("render_mode")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let mode_rule = answer_policy
            .get("render_mode_rule")
            .and_then(Value::as_str)
            .unwrap_or("normal returns a direct answer; artifact means the user expects a generated artifact when supported.");
        lines.push(format!("输出模式：`{render_mode}`。{mode_rule}"));
    }
    lines
}

pub(crate) fn assistant_run_v3_awareness_lines() -> Vec<String> {
    vec![
        "DataMax 认知：你正在 DataMax 中服务用户。DataMax 提供数据集、第三方知识库、权限、检索供料、受控动作、报表和静态页产物上下文。".to_string(),
        "DataMax 上下文是附加能力，不是能力限制；没有可见数据集或供料时，仍可保持通用模型水准回答普通问题。".to_string(),
        "DataMax 证据规则：涉及 DataMax 数据、文档、权限、工具结果或产物状态时，只能把已供给的 observation/证据当作事实；未供料时先说明“当前不可见/未供料”，再区分通用知识或推断。".to_string(),
        "DataMax 搜索规则：外部/网页搜索（web_search）是 DataMax 受控只读能力；没有带来源和时间的 DataMax search evidence 时，不要声称已联网搜索或引用实时网页结果。".to_string(),
    ]
}

pub(crate) fn assistant_run_v3_awareness_policy_value() -> Value {
    json!({
        "identity": "你正在服务 DataMax。DataMax 是数据集、第三方知识库、权限、检索供料、受控动作、报表和静态页产物的统一工作台。",
        "additiveContextRule": "DataMax 上下文是附加能力，不是能力限制。即使当前没有可见数据集或供料，也可以保持通用模型水准回答普通问题。",
        "unavailableEvidenceRule": "涉及 DataMax 数据、文档、权限、工具结果或产物状态时，只有收到 DataMax observation/供料才能当作事实。未供料时先说明“当前不可见/未供料”，再区分通用判断。",
        "externalSearchPolicy": {
            "status": "v3_controlled_read_only",
            "modelRule": "外部/网页搜索是 DataMax 受控只读能力；未收到带来源和时间的 DataMax search evidence 前，不要声称已联网搜索或引用实时网页结果。"
        }
    })
}

pub(crate) fn assistant_run_model_facing_answer_policy(answer_policy: &Value) -> Value {
    let mut model_policy = answer_policy.clone();
    let Some(default_prompt) = answer_policy
        .get("default_prompt")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return model_policy;
    };
    if !assistant_run_default_prompt_has_customer_tone_intensity(default_prompt) {
        return model_policy;
    }

    let Some(object) = model_policy.as_object_mut() else {
        return model_policy;
    };
    object.insert(
        "default_prompt_rule".to_string(),
        json!("default_prompt is integration-provided task guidance. If it includes a strong customer-facing tone request, preserve the underlying need for a more direct or firm stance, but express it as polite, professional, evidence-grounded, and actionable business wording."),
    );
    object.insert(
        "default_prompt_tone_policy".to_string(),
        json!({
            "status": "customer_tone_translated",
            "raw_default_prompt_supplied_to_model": true,
            "reason": "strong_customer_tone_request",
            "model_rule": "Treat the tone request as a request for firmer, clearer, more direct business expression. Do not turn it into insults, threats, mockery, or personal attacks.",
        }),
    );
    model_policy
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn evidence_state_answer_policy_accepts_snake_and_camel_paths() {
        let snake = json!({"selected_scope": {"answer_policy": {"output_format": "rich_text"}}});
        assert_eq!(
            assistant_run_evidence_state_external_answer_policy(&snake)
                .and_then(assistant_run_answer_policy_output_format),
            Some("rich_text")
        );

        let camel =
            json!({"selectedScope": {"answerPolicy": {"output_format": {"format": "json"}}}});
        assert_eq!(
            assistant_run_evidence_state_external_answer_policy(&camel)
                .and_then(assistant_run_answer_policy_output_format),
            Some("json")
        );

        assert!(assistant_run_evidence_state_external_answer_policy(
            &json!({"selected_scope": {"answer_policy": null}})
        )
        .is_none());
    }

    #[test]
    fn answer_policy_output_format_reads_object_or_string_and_trims() {
        assert_eq!(
            assistant_run_answer_policy_output_format(
                &json!({"output_format": {"format": " markdown_table "}})
            ),
            Some("markdown_table")
        );
        assert_eq!(
            assistant_run_answer_policy_output_format(&json!({"output_format": " rich_text "})),
            Some("rich_text")
        );
        assert_eq!(
            assistant_run_answer_policy_output_format(&json!({"output_format": "   "})),
            None
        );
    }

    #[test]
    fn customer_tone_intensity_detects_strong_tone_requests() {
        assert!(assistant_run_default_prompt_has_customer_tone_intensity(
            "态度要凶一点，直接指出问题"
        ));
        assert!(assistant_run_default_prompt_has_customer_tone_intensity(
            "Use an aggressive tone with the customer"
        ));
        assert!(!assistant_run_default_prompt_has_customer_tone_intensity(
            "请用坚定、专业、礼貌的语气回答"
        ));
    }

    #[test]
    fn model_facing_policy_adds_professional_boundary_only_for_strong_tone() {
        let policy = assistant_run_model_facing_answer_policy(&json!({
            "default_prompt": "态度要凶一点，按客户材料回答。",
            "output_format": {"format": "rich_text"}
        }));
        assert_eq!(
            policy["default_prompt_tone_policy"]["status"],
            json!("customer_tone_translated")
        );
        assert_eq!(
            policy["default_prompt_tone_policy"]["raw_default_prompt_supplied_to_model"],
            json!(true)
        );
        assert_eq!(
            policy["default_prompt"],
            json!("态度要凶一点，按客户材料回答。")
        );

        let neutral = assistant_run_model_facing_answer_policy(&json!({
            "default_prompt": "请按客户材料回答。",
        }));
        assert!(neutral.get("default_prompt_tone_policy").is_none());
    }

    #[test]
    fn external_answer_policy_guidance_preserves_format_tone_and_render_rules() {
        let lines = assistant_run_external_answer_policy_guidance_lines(&json!({
            "default_prompt": "态度要凶一点，直接指出问题",
            "output_format": {"format": "json"},
            "render_mode": "artifact"
        }));

        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("转译为坚定、专业、礼貌"));
        assert!(lines[1].contains("`json`"));
        assert!(lines[1].contains("只输出合法 JSON"));
        assert!(lines[2].contains("`artifact`"));
    }

    #[test]
    fn v3_awareness_policy_matches_awareness_lines() {
        let lines = assistant_run_v3_awareness_lines();
        let policy = assistant_run_v3_awareness_policy_value();

        assert!(lines
            .iter()
            .any(|line| line.contains("附加能力，不是能力限制")));
        assert_eq!(
            policy["externalSearchPolicy"]["status"],
            json!("v3_controlled_read_only")
        );
        assert!(policy["additiveContextRule"]
            .as_str()
            .unwrap_or_default()
            .contains("附加能力，不是能力限制"));
    }
}
