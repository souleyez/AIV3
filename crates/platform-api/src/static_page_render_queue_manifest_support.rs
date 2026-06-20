use domain_model::{StaticPageDraft, StaticPageImageJob, WorkflowExecution, WorkflowTaskId};
use serde_json::{json, Value};

use crate::build_static_page_data_snapshot;
use crate::static_page_dynamic_contract_support::build_static_page_dynamic_page_contract;
use crate::static_page_module_binding_support::static_page_module_chart_runtime;
use crate::static_page_payload_support::{static_page_payload_modules, static_page_payload_value};
use crate::static_page_visual_render_spec_support::{
    build_static_page_render_spec, build_static_page_visual_spec,
};

pub(crate) fn build_static_page_render_queue_manifest(
    draft: &StaticPageDraft,
    image_job: Option<&StaticPageImageJob>,
    workflow_execution: Option<&WorkflowExecution>,
    workflow_task_id: Option<WorkflowTaskId>,
) -> Value {
    let payload = &draft.draft_payload;
    let data_snapshot = static_page_payload_value(payload, &["dataSnapshot", "data_snapshot"])
        .unwrap_or_else(|| build_static_page_data_snapshot(payload, &draft.selected_scope));
    let modules = static_page_payload_modules(payload);
    let export_package =
        build_static_page_queued_export_package_manifest(draft, &modules, &data_snapshot);
    let workflow_manifest =
        build_static_page_render_queue_workflow_manifest(workflow_execution, workflow_task_id);
    json!({
        "draft_id": draft.id,
        "assistant_run_id": draft.assistant_run_id,
        "status": "queued",
        "renderer": "static-page-renderer-v1",
        "workflow_execution_id": workflow_execution.map(|execution| execution.id),
        "workflow_task_id": workflow_task_id,
        "workflow": workflow_manifest,
        "image_job_id": image_job.map(|job| job.id),
        "preview_asset_key": image_job.and_then(|job| job.preview_asset_key.clone()),
        "visual_spec": static_page_payload_value(payload, &["visualSpec", "visual_spec"])
            .unwrap_or_else(|| build_static_page_visual_spec("client-delivery")),
        "render_spec": static_page_payload_value(payload, &["renderSpec", "render_spec"])
            .unwrap_or_else(build_static_page_render_spec),
        "data_snapshot": data_snapshot,
        "export_package": export_package,
        "queue_copy": "最终静态页正在后台制作，可以继续聊天或修改其他内容。",
    })
}

pub(crate) fn build_static_page_render_queue_workflow_manifest(
    workflow_execution: Option<&WorkflowExecution>,
    workflow_task_id: Option<WorkflowTaskId>,
) -> Value {
    json!({
        "status": "queued",
        "executionId": workflow_execution.map(|execution| execution.id),
        "taskId": workflow_task_id,
    })
}

pub(crate) fn build_static_page_queued_export_package_manifest(
    draft: &StaticPageDraft,
    modules: &Value,
    data_snapshot: &Value,
) -> Value {
    let module_count = modules.as_array().map(Vec::len).unwrap_or(0);
    let echarts_requested_modules = modules
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter(|module| static_page_module_chart_runtime(module) == "echarts")
                .count()
        })
        .unwrap_or(0);
    json!({
        "kind": "static-page-export-package",
        "version": 1,
        "status": "queued",
        "draft_id": draft.id,
        "files": [
            {
                "path": "index.html",
                "role": "rendered_static_page",
                "mime": "text/html"
            },
            {
                "path": "asset-manifest.json",
                "role": "renderer_manifest",
                "mime": "application/json"
            },
            {
                "path": "data-snapshot.json",
                "role": "render_data_snapshot",
                "mime": "application/json"
            },
            {
                "path": "data.json",
                "role": "dynamic_data_snapshot",
                "mime": "application/json"
            },
            {
                "path": "modules.json",
                "role": "editable_module_plan",
                "mime": "application/json"
            }
        ],
        "dynamic_page_contract": build_static_page_dynamic_page_contract(),
        "debug": {
            "renderer": "static-page-renderer-v1",
            "module_count": module_count,
            "echarts_requested_modules": echarts_requested_modules,
            "data_snapshot_source": data_snapshot.get("source")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{
        AssistantRunId, StaticPageDraftId, StaticPageDraftStatus, StaticPageImageJobId,
        StaticPageImageJobStatus, TenantId, WorkflowExecutionId, WorkflowKind, WorkflowStatus,
    };

    fn draft_with_payload(payload: Value) -> StaticPageDraft {
        StaticPageDraft {
            id: StaticPageDraftId::new(),
            tenant_id: TenantId::new(),
            assistant_run_id: AssistantRunId::new(),
            owner_user_id: None,
            title: "经营月报".to_string(),
            status: StaticPageDraftStatus::Draft,
            selected_scope: json!({"datasets": ["dataset-a"]}),
            visibility_snapshot: json!({}),
            source_refs: json!([]),
            draft_payload: payload,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn image_job_for_draft(draft: &StaticPageDraft) -> StaticPageImageJob {
        StaticPageImageJob {
            id: StaticPageImageJobId::new(),
            tenant_id: draft.tenant_id,
            draft_id: draft.id,
            assistant_run_id: draft.assistant_run_id,
            status: StaticPageImageJobStatus::PreviewReady,
            queue_position: Some(1),
            image_prompt_payload: json!({}),
            preview_asset_key: Some("static-page-previews/preview.png".to_string()),
            failure_reason: None,
            confirmed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn workflow_execution() -> WorkflowExecution {
        WorkflowExecution {
            id: WorkflowExecutionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: None,
            report_plan_id: None,
            kind: WorkflowKind::StaticPageRender,
            version: "v1".to_string(),
            stage: "queued".to_string(),
            status: WorkflowStatus::Pending,
            attempt: 1,
            context: json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn render_queue_manifest_preserves_existing_specs_and_job_refs() {
        let draft = draft_with_payload(json!({
            "visualSpec": {"theme": "dark"},
            "renderSpec": {"runtime": "safe-echarts"},
            "dataSnapshot": {"source": "provided"},
            "modules": [
                {
                    "id": "trend",
                    "visualization": {
                        "type": "line",
                        "chartRuntime": "echarts"
                    }
                },
                {
                    "id": "summary",
                    "visualization": {
                        "type": "text"
                    }
                }
            ]
        }));
        let image_job = image_job_for_draft(&draft);
        let execution = workflow_execution();
        let task_id = WorkflowTaskId::new();

        let manifest = build_static_page_render_queue_manifest(
            &draft,
            Some(&image_job),
            Some(&execution),
            Some(task_id),
        );

        assert_eq!(manifest["status"], json!("queued"));
        assert_eq!(manifest["renderer"], json!("static-page-renderer-v1"));
        assert_eq!(manifest["image_job_id"], json!(image_job.id));
        assert_eq!(
            manifest["preview_asset_key"],
            json!("static-page-previews/preview.png")
        );
        assert_eq!(manifest["workflow_execution_id"], json!(execution.id));
        assert_eq!(manifest["workflow_task_id"], json!(task_id));
        assert_eq!(manifest["visual_spec"], json!({"theme": "dark"}));
        assert_eq!(manifest["render_spec"], json!({"runtime": "safe-echarts"}));
        assert_eq!(manifest["data_snapshot"], json!({"source": "provided"}));
        assert_eq!(
            manifest["export_package"]["debug"]["module_count"],
            json!(2)
        );
        assert_eq!(
            manifest["export_package"]["debug"]["echarts_requested_modules"],
            json!(1)
        );
        assert_eq!(
            manifest["export_package"]["debug"]["data_snapshot_source"],
            json!("provided")
        );
    }

    #[test]
    fn render_queue_workflow_manifest_preserves_status_execution_and_task() {
        let execution = workflow_execution();
        let task_id = WorkflowTaskId::new();

        let workflow =
            build_static_page_render_queue_workflow_manifest(Some(&execution), Some(task_id));
        let empty_workflow = build_static_page_render_queue_workflow_manifest(None, None);

        assert_eq!(workflow["status"], json!("queued"));
        assert_eq!(workflow["executionId"], json!(execution.id));
        assert_eq!(workflow["taskId"], json!(task_id));
        assert_eq!(empty_workflow["status"], json!("queued"));
        assert_eq!(empty_workflow["executionId"], Value::Null);
        assert_eq!(empty_workflow["taskId"], Value::Null);
    }

    #[test]
    fn render_queue_manifest_falls_back_to_generated_specs_and_data_snapshot() {
        let draft = draft_with_payload(json!({
            "modules": [
                {"id": "hero"}
            ]
        }));

        let manifest = build_static_page_render_queue_manifest(&draft, None, None, None);

        assert_eq!(manifest["image_job_id"], Value::Null);
        assert_eq!(manifest["preview_asset_key"], Value::Null);
        assert_eq!(manifest["workflow_execution_id"], Value::Null);
        assert_eq!(manifest["workflow_task_id"], Value::Null);
        assert_eq!(
            manifest["data_snapshot"]["source"],
            json!("static_page_draft")
        );
        assert_eq!(
            manifest["data_snapshot"]["selected_scope"],
            draft.selected_scope
        );
        assert_eq!(
            manifest["export_package"]["dynamic_page_contract"]["data_file"],
            json!("data.json")
        );
        assert_eq!(
            manifest["export_package"]["dynamic_page_contract"]["source_snapshot_file"],
            json!("data-snapshot.json")
        );
        assert_eq!(
            manifest["export_package"]["files"][0]["path"],
            json!("index.html")
        );
        assert_eq!(
            manifest["export_package"]["debug"]["module_count"],
            json!(1)
        );
        assert_eq!(
            manifest["export_package"]["debug"]["echarts_requested_modules"],
            json!(0)
        );
    }

    #[test]
    fn queued_export_package_handles_non_array_modules_and_missing_source() {
        let draft = draft_with_payload(json!({}));
        let export_package =
            build_static_page_queued_export_package_manifest(&draft, &json!("invalid"), &json!({}));

        assert_eq!(export_package["status"], json!("queued"));
        assert_eq!(export_package["debug"]["module_count"], json!(0));
        assert_eq!(
            export_package["debug"]["echarts_requested_modules"],
            json!(0)
        );
        assert_eq!(
            export_package["debug"]["data_snapshot_source"],
            json!("unknown")
        );
        assert_eq!(export_package["files"].as_array().map(Vec::len), Some(5));
    }
}
