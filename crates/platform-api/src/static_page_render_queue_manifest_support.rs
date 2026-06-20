use domain_model::{
    AssistantRunId, StaticPageDraft, StaticPageDraftId, StaticPageImageJob, StaticPageImageJobId,
    WorkflowExecution, WorkflowExecutionId, WorkflowTaskId,
};
use serde_json::{json, Value};

use crate::build_static_page_data_snapshot;
use crate::static_page_dynamic_contract_support::build_static_page_dynamic_page_contract;
use crate::static_page_module_binding_support::static_page_module_chart_runtime;
use crate::static_page_payload_support::{static_page_payload_modules, static_page_payload_value};
use crate::static_page_visual_render_spec_support::{
    build_static_page_render_spec, build_static_page_visual_spec,
};

type StaticPageQueuedExportPackageFileSpec = (&'static str, &'static str, &'static str);

pub(crate) fn build_static_page_render_queue_manifest(
    draft: &StaticPageDraft,
    image_job: Option<&StaticPageImageJob>,
    workflow_execution: Option<&WorkflowExecution>,
    workflow_task_id: Option<WorkflowTaskId>,
) -> Value {
    let payload = &draft.draft_payload;
    let data_snapshot =
        build_static_page_render_queue_data_snapshot(payload, &draft.selected_scope);
    let modules = build_static_page_render_queue_modules(payload);
    let export_package =
        build_static_page_render_queue_export_package(draft, &modules, &data_snapshot);
    let workflow_manifest =
        build_static_page_render_queue_workflow_manifest(workflow_execution, workflow_task_id);
    let (workflow_execution_id, workflow_task_id_value) =
        build_static_page_render_queue_workflow_ids(workflow_execution, workflow_task_id);
    let (image_job_id, preview_asset_key) = build_static_page_render_queue_image_context(image_job);
    let (draft_id, assistant_run_id) = build_static_page_render_queue_identity(draft);
    let (status, renderer, queue_copy) = build_static_page_render_queue_lifecycle();
    json!({
        "draft_id": draft_id,
        "assistant_run_id": assistant_run_id,
        "status": status,
        "renderer": renderer,
        "workflow_execution_id": workflow_execution_id,
        "workflow_task_id": workflow_task_id_value,
        "workflow": workflow_manifest,
        "image_job_id": image_job_id,
        "preview_asset_key": preview_asset_key,
        "visual_spec": build_static_page_render_queue_visual_spec(payload),
        "render_spec": build_static_page_render_queue_render_spec(payload),
        "data_snapshot": data_snapshot,
        "export_package": export_package,
        "queue_copy": queue_copy,
    })
}

pub(crate) fn build_static_page_render_queue_lifecycle(
) -> (&'static str, &'static str, &'static str) {
    (
        "queued",
        "static-page-renderer-v1",
        "最终静态页正在后台制作，可以继续聊天或修改其他内容。",
    )
}

pub(crate) fn build_static_page_render_queue_image_context(
    image_job: Option<&StaticPageImageJob>,
) -> (Option<StaticPageImageJobId>, Option<String>) {
    (
        build_static_page_render_queue_image_job_id(image_job),
        build_static_page_render_queue_preview_asset_key(image_job),
    )
}

pub(crate) fn build_static_page_render_queue_image_job_id(
    image_job: Option<&StaticPageImageJob>,
) -> Option<StaticPageImageJobId> {
    image_job.map(|job| job.id)
}

pub(crate) fn build_static_page_render_queue_preview_asset_key(
    image_job: Option<&StaticPageImageJob>,
) -> Option<String> {
    image_job.and_then(|job| job.preview_asset_key.clone())
}

pub(crate) fn build_static_page_render_queue_identity(
    draft: &StaticPageDraft,
) -> (StaticPageDraftId, AssistantRunId) {
    (
        build_static_page_render_queue_draft_id(draft),
        build_static_page_render_queue_assistant_run_id(draft),
    )
}

pub(crate) fn build_static_page_render_queue_draft_id(
    draft: &StaticPageDraft,
) -> StaticPageDraftId {
    draft.id
}

pub(crate) fn build_static_page_render_queue_assistant_run_id(
    draft: &StaticPageDraft,
) -> AssistantRunId {
    draft.assistant_run_id
}

pub(crate) fn build_static_page_render_queue_workflow_ids(
    workflow_execution: Option<&WorkflowExecution>,
    workflow_task_id: Option<WorkflowTaskId>,
) -> (Option<WorkflowExecutionId>, Option<WorkflowTaskId>) {
    (
        build_static_page_render_queue_workflow_execution_id(workflow_execution),
        build_static_page_render_queue_workflow_task_id(workflow_task_id),
    )
}

pub(crate) fn build_static_page_render_queue_workflow_execution_id(
    workflow_execution: Option<&WorkflowExecution>,
) -> Option<WorkflowExecutionId> {
    workflow_execution.map(|execution| execution.id)
}

pub(crate) fn build_static_page_render_queue_workflow_task_id(
    workflow_task_id: Option<WorkflowTaskId>,
) -> Option<WorkflowTaskId> {
    workflow_task_id
}

pub(crate) fn build_static_page_render_queue_workflow_manifest(
    workflow_execution: Option<&WorkflowExecution>,
    workflow_task_id: Option<WorkflowTaskId>,
) -> Value {
    let status = build_static_page_render_queue_workflow_status();
    let execution_id = build_static_page_render_queue_workflow_execution_id(workflow_execution);
    let task_id = build_static_page_render_queue_workflow_task_id(workflow_task_id);
    json!({
        "status": status,
        "executionId": execution_id,
        "taskId": task_id,
    })
}

pub(crate) fn build_static_page_render_queue_workflow_status() -> &'static str {
    "queued"
}

pub(crate) fn build_static_page_render_queue_visual_spec(payload: &Value) -> Value {
    static_page_payload_value(
        payload,
        build_static_page_render_queue_visual_spec_aliases(),
    )
    .unwrap_or_else(|| {
        build_static_page_visual_spec(build_static_page_render_queue_visual_fallback_style())
    })
}

pub(crate) fn build_static_page_render_queue_visual_spec_aliases() -> &'static [&'static str] {
    &["visualSpec", "visual_spec"]
}

pub(crate) fn build_static_page_render_queue_visual_fallback_style() -> &'static str {
    "client-delivery"
}

pub(crate) fn build_static_page_render_queue_render_spec(payload: &Value) -> Value {
    static_page_payload_value(
        payload,
        build_static_page_render_queue_render_spec_aliases(),
    )
    .unwrap_or_else(build_static_page_render_queue_render_fallback_spec)
}

pub(crate) fn build_static_page_render_queue_render_spec_aliases() -> &'static [&'static str] {
    &["renderSpec", "render_spec"]
}

pub(crate) fn build_static_page_render_queue_render_fallback_spec() -> Value {
    build_static_page_render_spec()
}

pub(crate) fn build_static_page_render_queue_data_snapshot(
    payload: &Value,
    selected_scope: &Value,
) -> Value {
    static_page_payload_value(
        payload,
        build_static_page_render_queue_data_snapshot_aliases(),
    )
    .unwrap_or_else(|| {
        build_static_page_render_queue_fallback_data_snapshot(payload, selected_scope)
    })
}

pub(crate) fn build_static_page_render_queue_data_snapshot_aliases() -> &'static [&'static str] {
    &["dataSnapshot", "data_snapshot"]
}

pub(crate) fn build_static_page_render_queue_fallback_data_snapshot(
    payload: &Value,
    selected_scope: &Value,
) -> Value {
    build_static_page_data_snapshot(payload, selected_scope)
}

pub(crate) fn build_static_page_render_queue_modules(payload: &Value) -> Value {
    static_page_payload_modules(payload)
}

pub(crate) fn build_static_page_render_queue_export_package(
    draft: &StaticPageDraft,
    modules: &Value,
    data_snapshot: &Value,
) -> Value {
    build_static_page_queued_export_package_manifest(draft, modules, data_snapshot)
}

pub(crate) fn build_static_page_queued_export_package_manifest(
    draft: &StaticPageDraft,
    modules: &Value,
    data_snapshot: &Value,
) -> Value {
    let (module_count, echarts_requested_modules) =
        build_static_page_queued_export_package_module_counts(modules);
    let debug = build_static_page_queued_export_package_debug(
        module_count,
        echarts_requested_modules,
        data_snapshot,
    );
    let (kind, version, status) = build_static_page_queued_export_package_lifecycle();
    let draft_id = build_static_page_queued_export_package_draft_id(draft);
    json!({
        "kind": kind,
        "version": version,
        "status": status,
        "draft_id": draft_id,
        "files": build_static_page_queued_export_package_files(),
        "dynamic_page_contract": build_static_page_queued_export_package_dynamic_page_contract(),
        "debug": debug
    })
}

pub(crate) fn build_static_page_queued_export_package_lifecycle(
) -> (&'static str, u64, &'static str) {
    ("static-page-export-package", 1, "queued")
}

pub(crate) fn build_static_page_queued_export_package_draft_id(
    draft: &StaticPageDraft,
) -> StaticPageDraftId {
    draft.id
}

pub(crate) fn build_static_page_queued_export_package_dynamic_page_contract() -> Value {
    build_static_page_dynamic_page_contract()
}

pub(crate) fn build_static_page_queued_export_package_module_counts(
    modules: &Value,
) -> (usize, usize) {
    let Some(items) = modules.as_array() else {
        return (0, 0);
    };
    let module_count = build_static_page_queued_export_package_module_count(items);
    let echarts_requested_modules =
        build_static_page_queued_export_package_echarts_requested_module_count(items);
    (module_count, echarts_requested_modules)
}

pub(crate) fn build_static_page_queued_export_package_module_count(items: &[Value]) -> usize {
    items.len()
}

pub(crate) fn build_static_page_queued_export_package_echarts_requested_module_count(
    items: &[Value],
) -> usize {
    items
        .iter()
        .filter(|module| static_page_module_chart_runtime(module) == "echarts")
        .count()
}

pub(crate) fn build_static_page_queued_export_package_debug(
    module_count: usize,
    echarts_requested_modules: usize,
    data_snapshot: &Value,
) -> Value {
    let renderer = build_static_page_queued_export_package_debug_renderer();
    let data_snapshot_source =
        build_static_page_queued_export_package_data_snapshot_source(data_snapshot);
    json!({
        "renderer": renderer,
        "module_count": module_count,
        "echarts_requested_modules": echarts_requested_modules,
        "data_snapshot_source": data_snapshot_source
    })
}

pub(crate) fn build_static_page_queued_export_package_debug_renderer() -> &'static str {
    "static-page-renderer-v1"
}

pub(crate) fn build_static_page_queued_export_package_data_snapshot_source(
    data_snapshot: &Value,
) -> &str {
    data_snapshot
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
}

pub(crate) fn build_static_page_queued_export_package_files() -> Value {
    build_static_page_queued_export_package_files_from_specs(
        build_static_page_queued_export_package_file_specs(),
    )
}

pub(crate) fn build_static_page_queued_export_package_files_from_specs(
    specs: &[StaticPageQueuedExportPackageFileSpec],
) -> Value {
    Value::Array(
        specs
            .iter()
            .map(|&(path, role, mime)| {
                build_static_page_queued_export_package_file(path, role, mime)
            })
            .collect(),
    )
}

pub(crate) fn build_static_page_queued_export_package_file_specs(
) -> &'static [StaticPageQueuedExportPackageFileSpec] {
    static FILE_SPECS: [StaticPageQueuedExportPackageFileSpec; 5] = [
        build_static_page_queued_export_package_file_spec(
            build_static_page_queued_export_package_index_path(),
            build_static_page_queued_export_package_rendered_page_role(),
            build_static_page_queued_export_package_html_mime(),
        ),
        build_static_page_queued_export_package_file_spec(
            build_static_page_queued_export_package_asset_manifest_path(),
            build_static_page_queued_export_package_renderer_manifest_role(),
            build_static_page_queued_export_package_json_mime(),
        ),
        build_static_page_queued_export_package_file_spec(
            build_static_page_queued_export_package_data_snapshot_path(),
            build_static_page_queued_export_package_render_data_snapshot_role(),
            build_static_page_queued_export_package_json_mime(),
        ),
        build_static_page_queued_export_package_file_spec(
            build_static_page_queued_export_package_dynamic_data_path(),
            build_static_page_queued_export_package_dynamic_data_snapshot_role(),
            build_static_page_queued_export_package_json_mime(),
        ),
        build_static_page_queued_export_package_file_spec(
            build_static_page_queued_export_package_modules_path(),
            build_static_page_queued_export_package_editable_module_plan_role(),
            build_static_page_queued_export_package_json_mime(),
        ),
    ];
    &FILE_SPECS
}

pub(crate) const fn build_static_page_queued_export_package_file_spec(
    path: &'static str,
    role: &'static str,
    mime: &'static str,
) -> StaticPageQueuedExportPackageFileSpec {
    (path, role, mime)
}

pub(crate) const fn build_static_page_queued_export_package_rendered_page_role() -> &'static str {
    "rendered_static_page"
}

pub(crate) const fn build_static_page_queued_export_package_renderer_manifest_role() -> &'static str
{
    "renderer_manifest"
}

pub(crate) const fn build_static_page_queued_export_package_render_data_snapshot_role(
) -> &'static str {
    "render_data_snapshot"
}

pub(crate) const fn build_static_page_queued_export_package_dynamic_data_snapshot_role(
) -> &'static str {
    "dynamic_data_snapshot"
}

pub(crate) const fn build_static_page_queued_export_package_editable_module_plan_role(
) -> &'static str {
    "editable_module_plan"
}

pub(crate) const fn build_static_page_queued_export_package_index_path() -> &'static str {
    "index.html"
}

pub(crate) const fn build_static_page_queued_export_package_asset_manifest_path() -> &'static str {
    "asset-manifest.json"
}

pub(crate) const fn build_static_page_queued_export_package_data_snapshot_path() -> &'static str {
    "data-snapshot.json"
}

pub(crate) const fn build_static_page_queued_export_package_dynamic_data_path() -> &'static str {
    "data.json"
}

pub(crate) const fn build_static_page_queued_export_package_modules_path() -> &'static str {
    "modules.json"
}

pub(crate) const fn build_static_page_queued_export_package_html_mime() -> &'static str {
    "text/html"
}

pub(crate) const fn build_static_page_queued_export_package_json_mime() -> &'static str {
    "application/json"
}

pub(crate) fn build_static_page_queued_export_package_file(
    path: &'static str,
    role: &'static str,
    mime: &'static str,
) -> Value {
    json!({
        "path": path,
        "role": role,
        "mime": mime
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
    fn render_queue_identity_preserves_draft_and_run_ids() {
        let draft = draft_with_payload(json!({}));

        let (draft_id, assistant_run_id) = build_static_page_render_queue_identity(&draft);

        assert_eq!(draft_id, draft.id);
        assert_eq!(assistant_run_id, draft.assistant_run_id);
    }

    #[test]
    fn render_queue_identity_field_helpers_preserve_draft_and_run_ids() {
        let draft = draft_with_payload(json!({}));

        assert_eq!(build_static_page_render_queue_draft_id(&draft), draft.id);
        assert_eq!(
            build_static_page_render_queue_assistant_run_id(&draft),
            draft.assistant_run_id
        );
    }

    #[test]
    fn render_queue_lifecycle_preserves_status_renderer_and_copy() {
        let (status, renderer, queue_copy) = build_static_page_render_queue_lifecycle();

        assert_eq!(status, "queued");
        assert_eq!(renderer, "static-page-renderer-v1");
        assert_eq!(
            queue_copy,
            "最终静态页正在后台制作，可以继续聊天或修改其他内容。"
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
    fn render_queue_workflow_status_preserves_queued_value() {
        assert_eq!(build_static_page_render_queue_workflow_status(), "queued");
    }

    #[test]
    fn render_queue_workflow_ids_preserve_optional_execution_and_task() {
        let execution = workflow_execution();
        let task_id = WorkflowTaskId::new();

        let (workflow_execution_id, workflow_task_id) =
            build_static_page_render_queue_workflow_ids(Some(&execution), Some(task_id));
        let (empty_workflow_execution_id, empty_workflow_task_id) =
            build_static_page_render_queue_workflow_ids(None, None);

        assert_eq!(workflow_execution_id, Some(execution.id));
        assert_eq!(workflow_task_id, Some(task_id));
        assert_eq!(empty_workflow_execution_id, None);
        assert_eq!(empty_workflow_task_id, None);
    }

    #[test]
    fn render_queue_workflow_id_helpers_preserve_optional_values() {
        let execution = workflow_execution();
        let task_id = WorkflowTaskId::new();

        assert_eq!(
            build_static_page_render_queue_workflow_execution_id(Some(&execution)),
            Some(execution.id)
        );
        assert_eq!(
            build_static_page_render_queue_workflow_execution_id(None),
            None
        );
        assert_eq!(
            build_static_page_render_queue_workflow_task_id(Some(task_id)),
            Some(task_id)
        );
        assert_eq!(build_static_page_render_queue_workflow_task_id(None), None);
    }

    #[test]
    fn render_queue_image_context_preserves_optional_job_id_and_preview_asset_key() {
        let draft = draft_with_payload(json!({}));
        let image_job = image_job_for_draft(&draft);

        let (image_job_id, preview_asset_key) =
            build_static_page_render_queue_image_context(Some(&image_job));
        let (empty_image_job_id, empty_preview_asset_key) =
            build_static_page_render_queue_image_context(None);

        assert_eq!(image_job_id, Some(image_job.id));
        assert_eq!(
            preview_asset_key.as_deref(),
            Some("static-page-previews/preview.png")
        );
        assert_eq!(empty_image_job_id, None);
        assert_eq!(empty_preview_asset_key, None);
    }

    #[test]
    fn render_queue_image_field_helpers_preserve_optional_values() {
        let draft = draft_with_payload(json!({}));
        let image_job = image_job_for_draft(&draft);

        assert_eq!(
            build_static_page_render_queue_image_job_id(Some(&image_job)),
            Some(image_job.id)
        );
        assert_eq!(build_static_page_render_queue_image_job_id(None), None);
        assert_eq!(
            build_static_page_render_queue_preview_asset_key(Some(&image_job)).as_deref(),
            Some("static-page-previews/preview.png")
        );
        assert_eq!(build_static_page_render_queue_preview_asset_key(None), None);
    }

    #[test]
    fn render_queue_spec_helpers_preserve_explicit_and_fallback_specs() {
        let explicit_payload = json!({
            "visual_spec": {"styleDirection": "custom-report"},
            "render_spec": {"renderer": "custom-renderer"}
        });
        let fallback_payload = json!({});

        assert_eq!(
            build_static_page_render_queue_visual_spec(&explicit_payload),
            json!({"styleDirection": "custom-report"})
        );
        assert_eq!(
            build_static_page_render_queue_render_spec(&explicit_payload),
            json!({"renderer": "custom-renderer"})
        );

        let fallback_visual = build_static_page_render_queue_visual_spec(&fallback_payload);
        let fallback_render = build_static_page_render_queue_render_spec(&fallback_payload);

        assert_eq!(fallback_visual["styleDirection"], json!("client-delivery"));
        assert_eq!(fallback_visual["typography"]["density"], json!("balanced"));
        assert_eq!(
            fallback_render["renderer"],
            json!("static-page-renderer-v1")
        );
        assert_eq!(
            fallback_render["dynamicData"]["dataFile"],
            json!("data.json")
        );
    }

    #[test]
    fn render_queue_visual_spec_aliases_preserve_order() {
        assert_eq!(
            build_static_page_render_queue_visual_spec_aliases(),
            &["visualSpec", "visual_spec"]
        );
    }

    #[test]
    fn render_queue_visual_fallback_style_preserves_client_delivery() {
        assert_eq!(
            build_static_page_render_queue_visual_fallback_style(),
            "client-delivery"
        );
    }

    #[test]
    fn render_queue_render_spec_aliases_preserve_order() {
        assert_eq!(
            build_static_page_render_queue_render_spec_aliases(),
            &["renderSpec", "render_spec"]
        );
    }

    #[test]
    fn render_queue_render_fallback_spec_preserves_renderer_and_dynamic_data() {
        let fallback_render = build_static_page_render_queue_render_fallback_spec();

        assert_eq!(
            fallback_render["renderer"],
            json!("static-page-renderer-v1")
        );
        assert_eq!(
            fallback_render["dynamicData"]["dataFile"],
            json!("data.json")
        );
    }

    #[test]
    fn render_queue_data_snapshot_helper_preserves_explicit_and_fallback_snapshot() {
        let explicit_payload = json!({
            "data_snapshot": {"source": "provided", "snapshotVersion": 7}
        });
        let fallback_payload = json!({
            "modules": [{"id": "summary"}]
        });
        let selected_scope = json!({"datasets": ["dataset-a"]});

        assert_eq!(
            build_static_page_render_queue_data_snapshot(&explicit_payload, &selected_scope),
            json!({"source": "provided", "snapshotVersion": 7})
        );

        let fallback_snapshot =
            build_static_page_render_queue_data_snapshot(&fallback_payload, &selected_scope);

        assert_eq!(fallback_snapshot["source"], json!("static_page_draft"));
        assert_eq!(fallback_snapshot["selected_scope"], selected_scope);
        assert_eq!(
            fallback_snapshot["module_bindings"]
                .as_array()
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            fallback_snapshot["validation_summary"]["moduleCount"],
            json!(1)
        );
    }

    #[test]
    fn render_queue_data_snapshot_aliases_preserve_order() {
        assert_eq!(
            build_static_page_render_queue_data_snapshot_aliases(),
            &["dataSnapshot", "data_snapshot"]
        );
    }

    #[test]
    fn render_queue_fallback_data_snapshot_preserves_static_page_snapshot() {
        let payload = json!({
            "modules": [{"id": "summary"}]
        });
        let selected_scope = json!({"datasets": ["dataset-a"]});

        let fallback_snapshot =
            build_static_page_render_queue_fallback_data_snapshot(&payload, &selected_scope);

        assert_eq!(fallback_snapshot["source"], json!("static_page_draft"));
        assert_eq!(fallback_snapshot["selected_scope"], selected_scope);
        assert_eq!(
            fallback_snapshot["module_bindings"]
                .as_array()
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            fallback_snapshot["validation_summary"]["moduleCount"],
            json!(1)
        );
    }

    #[test]
    fn render_queue_modules_preserve_array_and_empty_fallback() {
        let payload = json!({
            "modules": [
                {"id": "summary"},
                {"id": "trend"}
            ]
        });

        assert_eq!(
            build_static_page_render_queue_modules(&payload),
            json!([
                {"id": "summary"},
                {"id": "trend"}
            ])
        );
        assert_eq!(
            build_static_page_render_queue_modules(&json!({"modules": "invalid"})),
            json!([])
        );
        assert_eq!(
            build_static_page_render_queue_modules(&json!({})),
            json!([])
        );
    }

    #[test]
    fn render_queue_export_package_preserves_manifest_fields_and_debug_counts() {
        let draft = draft_with_payload(json!({}));
        let modules = json!([
            {
                "id": "trend",
                "visualization": {
                    "chartRuntime": "echarts"
                }
            },
            {
                "id": "summary"
            }
        ]);
        let data_snapshot = json!({"source": "provided"});

        let export_package =
            build_static_page_render_queue_export_package(&draft, &modules, &data_snapshot);

        assert_eq!(export_package["kind"], json!("static-page-export-package"));
        assert_eq!(export_package["version"], json!(1));
        assert_eq!(export_package["status"], json!("queued"));
        assert_eq!(export_package["draft_id"], json!(draft.id));
        assert_eq!(export_package["debug"]["module_count"], json!(2));
        assert_eq!(
            export_package["debug"]["echarts_requested_modules"],
            json!(1)
        );
        assert_eq!(
            export_package["debug"]["data_snapshot_source"],
            json!("provided")
        );
    }

    #[test]
    fn queued_export_package_lifecycle_preserves_kind_version_and_status() {
        let (kind, version, status) = build_static_page_queued_export_package_lifecycle();

        assert_eq!(kind, "static-page-export-package");
        assert_eq!(version, 1);
        assert_eq!(status, "queued");
    }

    #[test]
    fn queued_export_package_draft_id_preserves_draft_id() {
        let draft = draft_with_payload(json!({}));

        let draft_id = build_static_page_queued_export_package_draft_id(&draft);

        assert_eq!(draft_id, draft.id);
    }

    #[test]
    fn queued_export_package_dynamic_contract_preserves_refresh_defaults() {
        let contract = build_static_page_queued_export_package_dynamic_page_contract();

        assert_eq!(contract["version"], json!(1));
        assert_eq!(contract["data_file"], json!("data.json"));
        assert_eq!(
            contract["source_snapshot_file"],
            json!("data-snapshot.json")
        );
        assert_eq!(contract["refresh_policy"]["interval_seconds"], json!(60));
        assert_eq!(
            contract["rendering_policy"],
            json!("final_html_should_render_stateful_business_modules_from_data_json_when_present")
        );
    }

    #[test]
    fn queued_export_package_files_preserve_file_roles_and_order() {
        let files = build_static_page_queued_export_package_files();
        let items = files.as_array().expect("files should be an array");

        assert_eq!(items.len(), 5);
        assert_eq!(items[0]["path"], json!("index.html"));
        assert_eq!(items[0]["role"], json!("rendered_static_page"));
        assert_eq!(items[0]["mime"], json!("text/html"));
        assert_eq!(items[1]["path"], json!("asset-manifest.json"));
        assert_eq!(items[1]["role"], json!("renderer_manifest"));
        assert_eq!(items[2]["path"], json!("data-snapshot.json"));
        assert_eq!(items[2]["role"], json!("render_data_snapshot"));
        assert_eq!(items[3]["path"], json!("data.json"));
        assert_eq!(items[3]["role"], json!("dynamic_data_snapshot"));
        assert_eq!(items[4]["path"], json!("modules.json"));
        assert_eq!(items[4]["role"], json!("editable_module_plan"));
        assert!(items
            .iter()
            .skip(1)
            .all(|item| item["mime"] == json!("application/json")));
    }

    #[test]
    fn queued_export_package_file_preserves_path_role_and_mime() {
        let file = build_static_page_queued_export_package_file(
            "index.html",
            "rendered_static_page",
            "text/html",
        );

        assert_eq!(file["path"], json!("index.html"));
        assert_eq!(file["role"], json!("rendered_static_page"));
        assert_eq!(file["mime"], json!("text/html"));
    }

    #[test]
    fn queued_export_package_file_specs_preserve_order_and_mimes() {
        let specs = build_static_page_queued_export_package_file_specs();

        assert_eq!(specs.len(), 5);
        assert_eq!(
            specs[0],
            ("index.html", "rendered_static_page", "text/html")
        );
        assert_eq!(
            specs[1],
            (
                "asset-manifest.json",
                "renderer_manifest",
                "application/json"
            )
        );
        assert_eq!(
            specs[2],
            (
                "data-snapshot.json",
                "render_data_snapshot",
                "application/json"
            )
        );
        assert_eq!(
            specs[3],
            ("data.json", "dynamic_data_snapshot", "application/json")
        );
        assert_eq!(
            specs[4],
            ("modules.json", "editable_module_plan", "application/json")
        );
        assert!(specs
            .iter()
            .skip(1)
            .all(|(_, _, mime)| *mime == "application/json"));
    }

    #[test]
    fn queued_export_package_file_mimes_preserve_values() {
        assert_eq!(
            build_static_page_queued_export_package_html_mime(),
            "text/html"
        );
        assert_eq!(
            build_static_page_queued_export_package_json_mime(),
            "application/json"
        );
    }

    #[test]
    fn queued_export_package_file_paths_preserve_values() {
        assert_eq!(
            build_static_page_queued_export_package_index_path(),
            "index.html"
        );
        assert_eq!(
            build_static_page_queued_export_package_asset_manifest_path(),
            "asset-manifest.json"
        );
        assert_eq!(
            build_static_page_queued_export_package_data_snapshot_path(),
            "data-snapshot.json"
        );
        assert_eq!(
            build_static_page_queued_export_package_dynamic_data_path(),
            "data.json"
        );
        assert_eq!(
            build_static_page_queued_export_package_modules_path(),
            "modules.json"
        );
    }

    #[test]
    fn queued_export_package_file_roles_preserve_values() {
        assert_eq!(
            build_static_page_queued_export_package_rendered_page_role(),
            "rendered_static_page"
        );
        assert_eq!(
            build_static_page_queued_export_package_renderer_manifest_role(),
            "renderer_manifest"
        );
        assert_eq!(
            build_static_page_queued_export_package_render_data_snapshot_role(),
            "render_data_snapshot"
        );
        assert_eq!(
            build_static_page_queued_export_package_dynamic_data_snapshot_role(),
            "dynamic_data_snapshot"
        );
        assert_eq!(
            build_static_page_queued_export_package_editable_module_plan_role(),
            "editable_module_plan"
        );
    }

    #[test]
    fn queued_export_package_file_spec_preserves_tuple_order() {
        assert_eq!(
            build_static_page_queued_export_package_file_spec(
                "index.html",
                "rendered_static_page",
                "text/html"
            ),
            ("index.html", "rendered_static_page", "text/html")
        );
    }

    #[test]
    fn queued_export_package_files_from_specs_preserve_order_and_fields() {
        let files = build_static_page_queued_export_package_files_from_specs(&[
            ("first.html", "primary_page", "text/html"),
            ("second.json", "secondary_data", "application/json"),
        ]);
        let items = files.as_array().expect("files should be an array");

        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["path"], json!("first.html"));
        assert_eq!(items[0]["role"], json!("primary_page"));
        assert_eq!(items[0]["mime"], json!("text/html"));
        assert_eq!(items[1]["path"], json!("second.json"));
        assert_eq!(items[1]["role"], json!("secondary_data"));
        assert_eq!(items[1]["mime"], json!("application/json"));
    }

    #[test]
    fn queued_export_package_debug_preserves_renderer_counts_and_snapshot_source() {
        let debug =
            build_static_page_queued_export_package_debug(3, 2, &json!({"source": "provided"}));
        let missing_source_debug = build_static_page_queued_export_package_debug(0, 0, &json!({}));

        assert_eq!(debug["renderer"], json!("static-page-renderer-v1"));
        assert_eq!(debug["module_count"], json!(3));
        assert_eq!(debug["echarts_requested_modules"], json!(2));
        assert_eq!(debug["data_snapshot_source"], json!("provided"));
        assert_eq!(
            missing_source_debug["data_snapshot_source"],
            json!("unknown")
        );
    }

    #[test]
    fn queued_export_package_debug_renderer_preserves_renderer_name() {
        assert_eq!(
            build_static_page_queued_export_package_debug_renderer(),
            "static-page-renderer-v1"
        );
    }

    #[test]
    fn queued_export_package_data_snapshot_source_preserves_string_and_unknown_fallback() {
        assert_eq!(
            build_static_page_queued_export_package_data_snapshot_source(
                &json!({"source": "provided"})
            ),
            "provided"
        );
        assert_eq!(
            build_static_page_queued_export_package_data_snapshot_source(&json!({})),
            "unknown"
        );
        assert_eq!(
            build_static_page_queued_export_package_data_snapshot_source(&json!({"source": 123})),
            "unknown"
        );
    }

    #[test]
    fn queued_export_package_module_counts_preserve_array_count_echarts_and_fallback() {
        let modules = json!([
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
            },
            {
                "id": "advanced",
                "chartOptions": {
                    "chartRuntime": "echarts"
                }
            }
        ]);

        assert_eq!(
            build_static_page_queued_export_package_module_counts(&modules),
            (3, 2)
        );
        assert_eq!(
            build_static_page_queued_export_package_module_counts(&json!("invalid")),
            (0, 0)
        );
    }

    #[test]
    fn queued_export_package_module_count_preserves_array_len() {
        let modules = vec![
            json!({"id": "hero"}),
            json!({"id": "trend"}),
            json!({"id": "table"}),
        ];

        assert_eq!(
            build_static_page_queued_export_package_module_count(&modules),
            3
        );
        assert_eq!(build_static_page_queued_export_package_module_count(&[]), 0);
    }

    #[test]
    fn queued_export_package_echarts_requested_module_count_preserves_runtime_matching() {
        let modules = vec![
            json!({
                "id": "trend",
                "visualization": {
                    "chartRuntime": "echarts"
                }
            }),
            json!({
                "id": "summary",
                "visualization": {
                    "type": "text"
                }
            }),
            json!({
                "id": "advanced",
                "chartOptions": {
                    "chartRuntime": "echarts"
                }
            }),
        ];

        assert_eq!(
            build_static_page_queued_export_package_echarts_requested_module_count(&modules),
            2
        );
        assert_eq!(
            build_static_page_queued_export_package_echarts_requested_module_count(&[]),
            0
        );
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
