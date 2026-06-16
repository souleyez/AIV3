use contracts::AssistantRunCodexActionContractView;
use serde_json::{json, Value};

pub(crate) fn assistant_run_codex_action_contracts(
    selected_scope: &Value,
) -> Vec<AssistantRunCodexActionContractView> {
    let mut actions = vec![
        AssistantRunCodexActionContractView::new(
            "retrieve_evidence",
            "检索供料证据",
            "在 DataMax 可见范围内检索数据集、文档或对话记忆证据。",
            json!({
                "type": "object",
                "properties": {
                    "dataset_id": {"type": "string"},
                    "query": {"type": "string"}
                }
            }),
            false,
        ),
        AssistantRunCodexActionContractView::new(
            "web_search",
            "请求外部/网页搜索",
            "请求 DataMax 执行受控只读外部/网页搜索；只有 DataMax 返回带来源和时间的 search evidence 后，模型才可引用搜索结果。",
            assistant_run_codex_web_search_schema(),
            false,
        ),
        AssistantRunCodexActionContractView::new(
            "read_document_detail",
            "读取文档详情",
            "读取已选中或已命中供料范围内的文档详情。",
            json!({
                "type": "object",
                "properties": {
                    "document_id": {"type": "string"},
                    "reason": {"type": "string"}
                },
                "required": ["document_id"]
            }),
            false,
        ),
        AssistantRunCodexActionContractView::new(
            "recall_conversation_memory",
            "召回对话记忆",
            "按 DataMax 判断召回当前本地会话的隐藏对话记忆。",
            json!({"type": "object", "properties": {"reason": {"type": "string"}}}),
            false,
        ),
        AssistantRunCodexActionContractView::new(
            "resolve_video_url",
            "解析公开视频地址",
            "只允许 DataMax 解析直接视频 URL 或公开页面可解析视频地址；不支持登录态、扫码、Cookie 或录屏绕过。",
            json!({
                "type": "object",
                "properties": {
                    "source_url": {"type": "string"},
                    "prompt": {"type": "string"},
                    "allowed_source_types": {
                        "type": "array",
                        "items": {"type": "string", "enum": ["direct_video_url", "public_page_resolvable_video"]}
                    },
                    "disallowed_source_types": {
                        "type": "array",
                        "items": {"type": "string", "enum": ["login_gated_page", "qr_login", "cookies", "screen_recording_bypass"]}
                    }
                }
            }),
            true,
        ),
        AssistantRunCodexActionContractView::new(
            "extract_video_ppt_transcript",
            "提取视频 PPT 和原文",
            "在 DataMax 已登记的视频素材上排后台任务，生成原文、关键帧/PPT 候选、页面映射和缺失证据说明。",
            json!({
                "type": "object",
                "properties": {
                    "asset_id": {"type": "string"},
                    "document_id": {"type": "string"},
                    "deliverables": {
                        "type": "array",
                        "items": {"type": "string", "enum": ["transcript_text", "slide_image_candidates", "ppt_outline_or_pptx", "timestamp_map"]}
                    },
                    "reason": {"type": "string"}
                }
            }),
            true,
        ),
        AssistantRunCodexActionContractView::new(
            "create_static_page_draft",
            "创建静态页草稿",
            "基于用户意图和供料状态创建静态页模块规划草稿。",
            json!({"type": "object", "properties": {"objective": {"type": "string"}}}),
            true,
        ),
        AssistantRunCodexActionContractView::new(
            "update_static_page_module",
            "更新静态页模块",
            "只更新当前可见静态页草稿的模块、布局、内容、数据或图表配置。",
            json!({
                "type": "object",
                "properties": {
                    "module_id": {"type": "string"},
                    "patch": {"type": "object"},
                    "operations": {"type": "array"}
                }
            }),
            true,
        ),
        AssistantRunCodexActionContractView::new(
            "submit_static_page_image_preview",
            "提交可视化生成",
            "把当前静态页草稿提交到 DataMax 控制的可视化队列。",
            json!({"type": "object", "properties": {"draft_id": {"type": "string"}}}),
            true,
        ),
        AssistantRunCodexActionContractView::new(
            "render_static_page",
            "制作最终静态页",
            "在已确认且未过期的可视化视觉合同下生成最终静态页。",
            json!({"type": "object", "properties": {"draft_id": {"type": "string"}}}),
            true,
        ),
        AssistantRunCodexActionContractView::new(
            "publish_static_page_revision",
            "发布当前静态页修订版",
            "严格仅当当前打开产物是已发布静态页，且用户本轮明确要求修改/调整/修复/优化报表页面或刷新当前报表数据并发布新链接时使用；Host 会复用 existing_artifact 并排队 static_page_image2_data_publish。",
            json!({
                "type": "object",
                "properties": {
                    "instruction": {"type": "string"},
                    "preserve_style": {"type": "boolean"},
                    "redesign": {"type": "boolean"}
                },
                "required": ["instruction"]
            }),
            true,
        ),
        AssistantRunCodexActionContractView::new(
            "list_report_options",
            "列出报表选项",
            "根据当前供料范围列出可创建的报表或静态页方向。",
            json!({"type": "object", "properties": {"reason": {"type": "string"}}}),
            false,
        ),
        AssistantRunCodexActionContractView::new(
            "create_report_draft",
            "创建报表草稿",
            "基于 DataMax 供料创建报表草稿。",
            json!({"type": "object", "properties": {"objective": {"type": "string"}}}),
            true,
        ),
        AssistantRunCodexActionContractView::new(
            "report_choice",
            "选择报表流向",
            "在模型需要用户确认时记录报表方向选择。",
            json!({"type": "object", "properties": {"choice": {"type": "string"}}}),
            true,
        ),
        AssistantRunCodexActionContractView::new(
            "submit_html_artifact_event",
            "提交 HTML 产物事件",
            "仅通过 DataMax 校验后的 html_artifact.patch 或 html_artifact.action_intent 更新受支持产物。",
            json!({
                "type": "object",
                "properties": {
                    "artifact_id": {"type": "string"},
                    "event_type": {
                        "type": "string",
                        "enum": ["html_artifact.patch", "html_artifact.action_intent"]
                    },
                    "payload": {"type": "object"},
                    "assistant_run_id": {"type": "string"},
                    "local_thread_id": {"type": "string"}
                },
                "required": ["artifact_id", "event_type", "payload"]
            }),
            true,
        ),
        AssistantRunCodexActionContractView::new(
            "final_answer",
            "模型生成最终回答",
            "不调用平台工具，直接返回模型撰写的回答。",
            json!({"type": "object", "properties": {"answer": {"type": "string"}}}),
            false,
        ),
    ];

    if selected_scope.get("type").and_then(Value::as_str) == Some("external_channel") {
        actions.extend([
            AssistantRunCodexActionContractView::new(
                "external_artifact.status",
                "查询第三方产物状态",
                "只读查询第三方产物或投递请求状态；不得直接写第三方系统。",
                assistant_run_codex_external_action_schema(false, false),
                false,
            ),
            AssistantRunCodexActionContractView::new(
                "external_artifact.publish",
                "发布第三方产物",
                "通过 DataMax 校验、审计和幂等边界向第三方产物系统发布当前产物。",
                assistant_run_codex_external_action_schema(true, false),
                true,
            ),
            AssistantRunCodexActionContractView::new(
                "external_artifact.revoke",
                "撤回第三方产物",
                "撤回或下线第三方产物，属于高风险写入，必须先通过原聊天通道或可信页面确认。",
                assistant_run_codex_external_action_schema(true, true),
                true,
            ),
            AssistantRunCodexActionContractView::new(
                "external_business_action.invoke",
                "执行第三方事务动作",
                "跨系统业务动作只生成 DataMax 受控意图；高风险或跨系统写入必须确认后再执行。",
                assistant_run_codex_external_business_action_schema(),
                true,
            ),
        ]);
    }

    actions
}

fn assistant_run_codex_web_search_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "query": {"type": "string"},
            "reason": {"type": "string"},
            "freshness": {
                "type": "string",
                "enum": ["latest", "recent", "historical", "unspecified"]
            },
            "language": {"type": "string"},
            "evidence_contract": {
                "type": "object",
                "properties": {
                    "requires_source_url": {"type": "boolean", "const": true},
                    "requires_source_title": {"type": "boolean", "const": true},
                    "requires_retrieved_at": {"type": "boolean", "const": true},
                    "requires_query_metadata": {"type": "boolean", "const": true}
                }
            }
        },
        "required": ["query", "reason"]
    })
}

fn assistant_run_codex_external_action_schema(
    artifact_required: bool,
    confirmation_required: bool,
) -> Value {
    let artifact_type = if artifact_required {
        json!("string")
    } else {
        json!(["string", "null"])
    };
    json!({
        "type": "object",
        "properties": {
            "connection_id": {"type": "string"},
            "target_system": {"type": "string"},
            "artifact_ref": {"type": artifact_type},
            "risk_level": {
                "type": "string",
                "enum": ["read_only", "low_risk_write", "high_risk_write", "cross_system"]
            },
            "requires_confirmation": {"type": "boolean", "const": confirmation_required},
            "arguments_redacted": {"type": "object"},
            "source_evidence_refs": {
                "type": "array",
                "items": {"type": "string"}
            },
            "idempotency_key": {"type": "string"}
        },
        "required": [
            "connection_id",
            "target_system",
            "risk_level",
            "requires_confirmation",
            "arguments_redacted"
        ]
    })
}

fn assistant_run_codex_external_business_action_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "connection_id": {"type": "string"},
            "target_system": {"type": "string"},
            "business_action_type": {"type": "string"},
            "risk_level": {
                "type": "string",
                "enum": ["read_only", "low_risk_write", "high_risk_write", "cross_system"]
            },
            "requires_confirmation": {"type": "boolean", "const": true},
            "arguments_redacted": {"type": "object"},
            "source_evidence_refs": {
                "type": "array",
                "items": {"type": "string"}
            },
            "idempotency_key": {"type": "string"}
        },
        "required": [
            "connection_id",
            "target_system",
            "business_action_type",
            "risk_level",
            "requires_confirmation",
            "arguments_redacted"
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action_types(actions: &[AssistantRunCodexActionContractView]) -> Vec<String> {
        actions
            .iter()
            .map(|action| action.action_type.clone())
            .collect()
    }

    #[test]
    fn base_action_contracts_include_core_read_and_artifact_actions() {
        let actions = assistant_run_codex_action_contracts(&json!({}));
        let action_types = action_types(&actions);

        assert!(action_types.contains(&"retrieve_evidence".to_string()));
        assert!(action_types.contains(&"web_search".to_string()));
        assert!(action_types.contains(&"read_document_detail".to_string()));
        assert!(action_types.contains(&"update_static_page_module".to_string()));
        assert!(action_types.contains(&"submit_html_artifact_event".to_string()));
        assert!(action_types.contains(&"final_answer".to_string()));
        assert!(!action_types.contains(&"external_artifact.publish".to_string()));

        let web_search = actions
            .iter()
            .find(|action| action.action_type == "web_search")
            .expect("web search action exists");
        assert!(web_search.requires_v3_validation);
        assert!(!web_search.mutates_state);
        assert_eq!(
            web_search.input_schema["properties"]["evidence_contract"]["properties"]
                ["requires_retrieved_at"]["const"],
            json!(true)
        );

        let html_artifact = actions
            .iter()
            .find(|action| action.action_type == "submit_html_artifact_event")
            .expect("html artifact action exists");
        assert!(html_artifact.requires_v3_validation);
        assert!(html_artifact.mutates_state);
        assert_eq!(
            html_artifact.input_schema["properties"]["event_type"]["enum"],
            json!(["html_artifact.patch", "html_artifact.action_intent"])
        );
    }

    #[test]
    fn video_contracts_preserve_safe_source_and_deliverable_enums() {
        let actions = assistant_run_codex_action_contracts(&json!({}));
        let resolver = actions
            .iter()
            .find(|action| action.action_type == "resolve_video_url")
            .expect("video resolver action exists");
        let extractor = actions
            .iter()
            .find(|action| action.action_type == "extract_video_ppt_transcript")
            .expect("video ppt extractor action exists");

        assert!(resolver.requires_v3_validation);
        assert!(resolver.mutates_state);
        assert_eq!(
            resolver.input_schema["properties"]["allowed_source_types"]["items"]["enum"],
            json!(["direct_video_url", "public_page_resolvable_video"])
        );
        assert_eq!(
            resolver.input_schema["properties"]["disallowed_source_types"]["items"]["enum"],
            json!([
                "login_gated_page",
                "qr_login",
                "cookies",
                "screen_recording_bypass"
            ])
        );
        assert!(extractor.requires_v3_validation);
        assert!(extractor.mutates_state);
        assert_eq!(
            extractor.input_schema["properties"]["deliverables"]["items"]["enum"],
            json!([
                "transcript_text",
                "slide_image_candidates",
                "ppt_outline_or_pptx",
                "timestamp_map"
            ])
        );
    }

    #[test]
    fn external_channel_scope_adds_external_actions_with_confirmation_boundaries() {
        let actions = assistant_run_codex_action_contracts(&json!({
            "type": "external_channel"
        }));
        let action_types = action_types(&actions);

        assert!(action_types.contains(&"external_artifact.status".to_string()));
        assert!(action_types.contains(&"external_artifact.publish".to_string()));
        assert!(action_types.contains(&"external_artifact.revoke".to_string()));
        assert!(action_types.contains(&"external_business_action.invoke".to_string()));

        let status = actions
            .iter()
            .find(|action| action.action_type == "external_artifact.status")
            .expect("external artifact status action exists");
        let publish = actions
            .iter()
            .find(|action| action.action_type == "external_artifact.publish")
            .expect("external artifact publish action exists");
        let revoke = actions
            .iter()
            .find(|action| action.action_type == "external_artifact.revoke")
            .expect("external artifact revoke action exists");
        let business = actions
            .iter()
            .find(|action| action.action_type == "external_business_action.invoke")
            .expect("external business action exists");

        assert!(status.requires_v3_validation);
        assert!(!status.mutates_state);
        assert_eq!(
            status.input_schema["properties"]["artifact_ref"]["type"],
            json!(["string", "null"])
        );
        assert!(publish.requires_v3_validation);
        assert!(publish.mutates_state);
        assert_eq!(
            publish.input_schema["properties"]["artifact_ref"]["type"],
            json!("string")
        );
        assert_eq!(
            revoke.input_schema["properties"]["requires_confirmation"]["const"],
            json!(true)
        );
        assert_eq!(
            business.input_schema["properties"]["requires_confirmation"]["const"],
            json!(true)
        );
        assert!(business.mutates_state);
    }
}
