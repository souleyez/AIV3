use chrono::{DateTime, Utc};
use domain_model::{StaticPageDraft, StaticPageDraftId};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::static_page_existing_artifact_support::static_page_generated_template_public_url;
use crate::{
    codex_host_fixed_task_public_artifact_url_allowed, ensure_json_object,
    truncate_assistant_supply_text,
};

pub(crate) fn static_page_current_artifact_draft_id(
    current_artifact: &Value,
) -> Option<StaticPageDraftId> {
    [
        "backendDraftId",
        "backend_draft_id",
        "staticPageDraftId",
        "static_page_draft_id",
        "draft_id",
        "id",
    ]
    .iter()
    .find_map(|key| {
        current_artifact
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(StaticPageDraftId)
    })
}

pub(crate) fn static_page_revision_explicit_intent_present(value: &str) -> bool {
    let text = value.trim();
    if text.is_empty() {
        return false;
    }
    if text.contains("修改报表") {
        return true;
    }
    let lower = text.to_ascii_lowercase();
    let subject_terms = [
        "报表",
        "页面",
        "静态页",
        "图表",
        "看板",
        "模板",
        "模块",
        "产物",
        "链接",
        "html",
        "dashboard",
    ];
    let action_terms = [
        "修改",
        "调整",
        "更改",
        "改成",
        "换成",
        "重做",
        "重新做",
        "重新生成",
        "优化",
        "修复",
        "更正",
        "纠正",
        "去掉",
        "删除",
        "增加",
        "新增",
        "加上",
        "移动",
        "合并",
        "拆分",
        "前置",
        "置顶",
        "放到",
        "提到",
        "暗黑",
        "手机端",
        "风格",
        "布局",
        "字段",
        "颜色",
        "排序",
        "筛选",
        "联动",
        "刷新数据",
        "发布新链接",
        "不喜欢",
    ];
    let has_subject = subject_terms
        .iter()
        .any(|needle| text.contains(needle) || lower.contains(needle));
    let has_action = action_terms
        .iter()
        .any(|needle| text.contains(needle) || lower.contains(needle));
    has_subject && has_action
}

pub(crate) fn static_page_public_url_from_current_artifact(
    current_artifact: &Value,
) -> Option<String> {
    [
        "/finalPage/publicUrl",
        "/finalPage/public_url",
        "/finalPage/generatedArtifactUrl",
        "/finalPage/generated_artifact_url",
        "/final_page/publicUrl",
        "/final_page/public_url",
        "/final_page/generatedArtifactUrl",
        "/final_page/generated_artifact_url",
        "/artifactStability/publicUrl",
        "/artifactStability/public_url",
        "/artifact_stability/publicUrl",
        "/artifact_stability/public_url",
    ]
    .into_iter()
    .filter_map(|pointer| current_artifact.pointer(pointer).and_then(Value::as_str))
    .chain(
        [
            "publicUrl",
            "public_url",
            "generatedArtifactUrl",
            "generated_artifact_url",
        ]
        .into_iter()
        .filter_map(|key| current_artifact.get(key).and_then(Value::as_str)),
    )
    .map(str::trim)
    .filter(|value| codex_host_fixed_task_public_artifact_url_allowed(value))
    .map(ToOwned::to_owned)
    .next()
}

pub(crate) fn static_page_revision_source_refs(
    mut source_refs: Value,
    source_draft: &StaticPageDraft,
    existing_artifact: &Value,
    instruction: &str,
    public_url: &str,
    now: DateTime<Utc>,
) -> Value {
    ensure_json_object(&mut source_refs);
    let generated_reference = json!({
        "templateId": format!("generated-static-page:{}", source_draft.id),
        "source": "v3-static-page-template-library",
        "templateKind": "generated_static_page",
        "label": source_draft.title,
        "publicUrl": public_url,
        "revisionInstruction": truncate_assistant_supply_text(instruction, 1200),
        "createdAt": now,
    });
    if let Some(object) = source_refs.as_object_mut() {
        let mut template_references = object
            .get("template_references")
            .or_else(|| object.get("templateReferences"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|reference| {
                static_page_generated_template_public_url(reference).as_deref() != Some(public_url)
            })
            .collect::<Vec<_>>();
        template_references.insert(0, generated_reference);
        object.insert(
            "template_references".to_string(),
            Value::Array(template_references.clone()),
        );
        object.insert(
            "templateReferences".to_string(),
            Value::Array(template_references),
        );
        object.insert(
            "revision_source".to_string(),
            json!("main_chat_current_static_page_artifact"),
        );
        object.insert(
            "source_draft_id".to_string(),
            json!(source_draft.id.to_string()),
        );
        object.insert("existing_artifact".to_string(), existing_artifact.clone());
        object.insert(
            "revision_instruction".to_string(),
            json!(truncate_assistant_supply_text(instruction, 1200)),
        );
        object.insert(
            "template_match_policy".to_string(),
            json!("current_artifact_existing_revision"),
        );
        object.insert(
            "style_reuse_policy".to_string(),
            json!("preserve_existing_artifact_style_unless_explicit_redesign"),
        );
        object.insert(
            "data_refresh_policy".to_string(),
            json!("refresh_current_authorized_data_against_existing_artifact"),
        );
        object.insert(
            "publish_mode".to_string(),
            json!("new_generated_artifact_only"),
        );
    }
    source_refs
}

pub(crate) fn static_page_revision_draft_title(source_title: &str) -> String {
    let trimmed = source_title.trim();
    if trimmed.is_empty() {
        "静态页修订版".to_string()
    } else if trimmed.contains("修订") {
        truncate_assistant_supply_text(trimmed, 80)
    } else {
        truncate_assistant_supply_text(&format!("{trimmed}（修订）"), 80)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain_model::{AssistantRunId, StaticPageDraftStatus, TenantId, UserId};

    const PUBLIC_URL: &str =
        "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/report/index.html";

    fn test_draft(title: &str) -> StaticPageDraft {
        StaticPageDraft {
            id: StaticPageDraftId::new(),
            tenant_id: TenantId::new(),
            assistant_run_id: AssistantRunId::new(),
            owner_user_id: Some(UserId::new()),
            title: title.to_string(),
            status: StaticPageDraftStatus::Rendered,
            selected_scope: json!({}),
            visibility_snapshot: json!({}),
            source_refs: json!({}),
            draft_payload: json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn current_artifact_draft_id_reads_supported_keys() {
        let draft_id = StaticPageDraftId::new();

        assert_eq!(
            static_page_current_artifact_draft_id(&json!({
                "backendDraftId": format!(" {draft_id} ")
            })),
            Some(draft_id)
        );
        assert_eq!(
            static_page_current_artifact_draft_id(&json!({
                "backendDraftId": "not-a-uuid",
                "static_page_draft_id": draft_id.to_string()
            })),
            Some(draft_id)
        );
        assert_eq!(
            static_page_current_artifact_draft_id(&json!({
                "draft_id": " "
            })),
            None
        );
    }

    #[test]
    fn revision_intent_accepts_report_edits_and_rejects_view_only_prompts() {
        for prompt in [
            "修改报表：把门店取高模块放到最前面",
            "我不喜欢这个风格，报表改成暗黑一点并适合手机端展示",
            "修复这个页面，切换门店后近7日销售要联动刷新",
            "去掉风险百分比模块，增加租售比健康度图表",
            "把门店取高风险模块提到最前面，刷新当前报表数据后给我新链接",
        ] {
            assert!(
                static_page_revision_explicit_intent_present(prompt),
                "prompt should be accepted: {prompt}"
            );
        }

        for prompt in [
            "",
            "看一下这个报表现在是否正常",
            "取高是什么意思？",
            "风险识别系统有哪些项目经历？",
            "这个链接还能打开吗？",
        ] {
            assert!(
                !static_page_revision_explicit_intent_present(prompt),
                "prompt should not be accepted: {prompt}"
            );
        }
    }

    #[test]
    fn current_artifact_public_url_prefers_allowed_nested_fields() {
        let current_artifact = json!({
            "finalPage": {
                "publicUrl": " https://example.com/private.html ",
                "generatedArtifactUrl": format!(" {PUBLIC_URL} ")
            },
            "publicUrl": "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/root/index.html"
        });

        assert_eq!(
            static_page_public_url_from_current_artifact(&current_artifact).as_deref(),
            Some(PUBLIC_URL)
        );
        assert_eq!(
            static_page_public_url_from_current_artifact(&json!({
                "publicUrl": "https://example.com/private.html"
            })),
            None
        );
    }

    #[test]
    fn revision_source_refs_insert_generated_template_and_deduplicate_public_url() {
        let source_draft = test_draft("静态页：新百经营月报");
        let now = Utc::now();
        let long_instruction = format!("{}{}", "修复近7日销售联动。", "x".repeat(1400));
        let existing_artifact = json!({
            "kind": "v3_generated_static_page",
            "public_url": PUBLIC_URL
        });
        let source_refs = json!({
            "template_references": [
                {
                    "templateId": "generated-static-page:old",
                    "source": "v3-static-page-template-library",
                    "publicUrl": PUBLIC_URL
                },
                {
                    "templateId": "keep",
                    "source": "v3-static-page-template-library",
                    "publicUrl": "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/keep/index.html"
                }
            ]
        });

        let updated = static_page_revision_source_refs(
            source_refs,
            &source_draft,
            &existing_artifact,
            &long_instruction,
            PUBLIC_URL,
            now,
        );
        let references = updated["template_references"]
            .as_array()
            .expect("template references should be an array");

        assert_eq!(
            references[0]["templateId"],
            json!(format!("generated-static-page:{}", source_draft.id))
        );
        assert_eq!(references[0]["publicUrl"], json!(PUBLIC_URL));
        assert_eq!(references[0]["createdAt"], json!(now));
        assert!(references[0]["revisionInstruction"]
            .as_str()
            .is_some_and(|value| value.chars().count() <= 1200));
        assert_eq!(references.len(), 2);
        assert_eq!(references[1]["templateId"], json!("keep"));
        assert_eq!(
            updated["templateReferences"],
            updated["template_references"]
        );
        assert_eq!(
            updated["revision_source"],
            json!("main_chat_current_static_page_artifact")
        );
        assert_eq!(
            updated["source_draft_id"],
            json!(source_draft.id.to_string())
        );
        assert_eq!(updated["existing_artifact"], existing_artifact);
        assert_eq!(
            updated["template_match_policy"],
            json!("current_artifact_existing_revision")
        );
        assert_eq!(
            updated["publish_mode"],
            json!("new_generated_artifact_only")
        );
    }

    #[test]
    fn revision_draft_title_appends_revision_once_and_falls_back_for_blank() {
        assert_eq!(static_page_revision_draft_title(""), "静态页修订版");
        assert_eq!(
            static_page_revision_draft_title(" 静态页：新百经营月报 "),
            "静态页：新百经营月报（修订）"
        );
        assert_eq!(
            static_page_revision_draft_title("静态页：新百经营月报（修订）"),
            "静态页：新百经营月报（修订）"
        );
        assert!(
            static_page_revision_draft_title(&format!("{}{}", "报表", "很长".repeat(80)))
                .chars()
                .count()
                <= 80
        );
    }
}
