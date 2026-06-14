use chrono::{DateTime, Utc};
use contracts::{self, HtmlArtifactInteractionModeView, HtmlArtifactManifestView};
use domain_model::AssistantRunId;
use serde_json::{json, Value};

pub(crate) fn wechat_video_login_handoff_answer_text() -> String {
    "当前不能自动从微信视频号链接拿到视频文件。请上传视频文件、提供可匿名下载的直接视频 URL，或申请授权录屏处理；拿到视频文件后再提取 PPT。".to_string()
}

pub(crate) fn assistant_run_wechat_video_handoff_runtime_manifest(lane: &str) -> Value {
    json!({
        "mode": "direct_answer",
        "provider": "platform_direct_answer",
        "model": "wechat-video-login-handoff-v1",
        "lane": lane,
        "reason": "login_gated_video_source_not_supported",
    })
}

pub(crate) fn wechat_video_login_handoff_artifact_from_prompt(
    run_id: AssistantRunId,
    prompt: &str,
    created_at: DateTime<Utc>,
) -> Option<HtmlArtifactManifestView> {
    let lower = prompt.to_ascii_lowercase();
    let mentions_wechat_video = lower.contains("weixin.qq.com/sph/")
        || lower.contains("channels.weixin.qq.com/sph/")
        || prompt.contains("视频号");
    let wants_slide_output = lower.contains("ppt")
        || lower.contains("powerpoint")
        || prompt.contains("课件")
        || prompt.contains("幻灯片")
        || (prompt.contains("提取") && prompt.contains("视频"));
    // Login-gated acquisition, QR login, cookies, and recording bypasses are out of scope.
    // This path only supports uploaded video files or directly/publicly resolvable video URLs.
    if mentions_wechat_video && wants_slide_output {
        let run_id_text = run_id.to_string();
        let short_code =
            wechat_video_short_code_from_prompt(prompt).unwrap_or_else(|| "未识别".to_string());
        return Some(HtmlArtifactManifestView {
            kind: "html_artifact".to_string(),
            version: 1,
            id: format!("html-artifact-wechat-video-login-handoff-{run_id_text}"),
            title: "视频来源受限 · 微信视频号".to_string(),
            source_type: contracts::HtmlArtifactSourceTypeView::VideoExtraction,
            template_id: contracts::HtmlArtifactTemplateIdView::WechatVideoLoginHandoff,
            owner_scope: contracts::HtmlArtifactOwnerScopeView {
                scope_type: "assistant_run".to_string(),
                id: run_id_text.clone(),
            },
            data_refs: Vec::new(),
            provenance: contracts::HtmlArtifactProvenanceView {
                producer: "v3-platform-api".to_string(),
                reason: "wechat_video_login_handoff_required".to_string(),
                source_run_id: Some(run_id_text),
            },
            interaction_mode: HtmlArtifactInteractionModeView::ActionIntent,
            created_at,
            payload: json!({
                "status": "unsupported_source",
                "failure_reason": "login_gated_video_source_not_supported",
                "sourcePlatform": "微信视频号",
                "source_platform": "微信视频号",
                "shortCode": short_code.clone(),
                "short_code": short_code,
                "targetArtifact": "视频 PPT 提取",
                "target_artifact": "视频 PPT 提取",
                "blockedReason": "当前没有拿到可处理的视频文件，不能声称已经看过视频或已经生成 PPT。",
                "blocked_reason": "当前没有拿到可处理的视频文件，不能声称已经看过视频或已经生成 PPT。",
                "supportedNextSteps": [
                    {
                        "title": "上传视频文件",
                        "detail": "用户或第三方系统上传原始视频文件后，再明确触发提取视频里的 PPT。"
                    },
                    {
                        "title": "提供匿名直连视频 URL",
                        "detail": "第三方系统可先把视频保存到自己的对象存储，再把可下载的视频地址交给 DataMax。"
                    },
                    {
                        "title": "申请授权录屏处理",
                        "detail": "确有授权但拿不到视频文件时，进入 operator 审批的短时录屏兜底流程。"
                    }
                ],
                "supported_next_steps": [
                    "upload_video_file",
                    "provide_direct_video_url",
                    "request_authorized_capture"
                ]
            }),
        });
    }
    None
}

fn wechat_video_short_code_from_prompt(prompt: &str) -> Option<String> {
    let lower = prompt.to_ascii_lowercase();
    for marker in ["weixin.qq.com/sph/", "channels.weixin.qq.com/sph/"] {
        let Some(index) = lower.find(marker) else {
            continue;
        };
        let start = index + marker.len();
        let code = prompt
            .get(start..)
            .unwrap_or_default()
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
            .take(80)
            .collect::<String>();
        if !code.is_empty() {
            return Some(code);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handoff_artifact_returns_actionable_payload_without_raw_source_url() {
        let run_id = AssistantRunId::new();
        let artifact = wechat_video_login_handoff_artifact_from_prompt(
            run_id,
            "https://weixin.qq.com/sph/ActLMg4yTD 试试用智能助手提取这个视频的PPT",
            Utc::now(),
        )
        .expect("login-gated video PPT request should produce a handoff artifact");

        assert_eq!(
            artifact.id,
            format!("html-artifact-wechat-video-login-handoff-{run_id}")
        );
        assert_eq!(
            artifact.source_type,
            contracts::HtmlArtifactSourceTypeView::VideoExtraction
        );
        assert_eq!(
            artifact.template_id,
            contracts::HtmlArtifactTemplateIdView::WechatVideoLoginHandoff
        );
        assert_eq!(
            artifact.interaction_mode,
            HtmlArtifactInteractionModeView::ActionIntent
        );
        assert_eq!(
            artifact.payload["failure_reason"],
            json!("login_gated_video_source_not_supported")
        );
        assert_eq!(artifact.payload["shortCode"], json!("ActLMg4yTD"));
        assert_eq!(
            artifact.payload["supported_next_steps"],
            json!([
                "upload_video_file",
                "provide_direct_video_url",
                "request_authorized_capture"
            ])
        );
        let serialized = serde_json::to_string(&artifact).expect("artifact should serialize");
        assert!(!serialized.contains("weixin.qq.com"));
        assert!(!serialized.contains("channels.weixin.qq.com"));
        assert!(!serialized.contains("扫码"));
        assert!(!serialized.to_ascii_lowercase().contains("cookie"));
    }

    #[test]
    fn handoff_artifact_supports_channels_domain_and_runtime_manifest_lane() {
        let artifact = wechat_video_login_handoff_artifact_from_prompt(
            AssistantRunId::new(),
            "帮我从 channels.weixin.qq.com/sph/AhfmOtV8P5 里面提取课件",
            Utc::now(),
        )
        .expect("channels video source should produce handoff");
        let runtime = assistant_run_wechat_video_handoff_runtime_manifest("external_channel");

        assert_eq!(artifact.payload["short_code"], json!("AhfmOtV8P5"));
        assert_eq!(artifact.payload["source_platform"], json!("微信视频号"));
        assert_eq!(runtime["mode"], json!("direct_answer"));
        assert_eq!(runtime["lane"], json!("external_channel"));
        assert_eq!(
            runtime["reason"],
            json!("login_gated_video_source_not_supported")
        );
    }

    #[test]
    fn handoff_artifact_ignores_plain_chat_and_non_slide_video_chat() {
        assert!(wechat_video_login_handoff_artifact_from_prompt(
            AssistantRunId::new(),
            "猫为什么喜欢晒太阳",
            Utc::now(),
        )
        .is_none());
        assert!(wechat_video_login_handoff_artifact_from_prompt(
            AssistantRunId::new(),
            "这个视频号内容讲的是什么",
            Utc::now(),
        )
        .is_none());
    }
}
