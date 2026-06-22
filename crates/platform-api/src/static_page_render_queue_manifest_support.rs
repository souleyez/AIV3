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

type StaticPageQueuedExportPackageFilePath = &'static str;
type StaticPageQueuedExportPackageFileRole = &'static str;
type StaticPageQueuedExportPackageFileMime = &'static str;
type StaticPageQueuedExportPackageFileSpec = (
    StaticPageQueuedExportPackageFilePath,
    StaticPageQueuedExportPackageFileRole,
    StaticPageQueuedExportPackageFileMime,
);
type StaticPageQueuedExportPackageModuleCount = usize;
type StaticPageQueuedExportPackageModuleCounts = (
    StaticPageQueuedExportPackageModuleCount,
    StaticPageQueuedExportPackageModuleCount,
);
type StaticPageQueuedExportPackageLifecycleScalar = &'static str;
type StaticPageQueuedExportPackageVersion = u64;
type StaticPageQueuedExportPackageLifecycleFields = (
    StaticPageQueuedExportPackageLifecycleScalar,
    StaticPageQueuedExportPackageVersion,
    StaticPageQueuedExportPackageLifecycleScalar,
);
type StaticPageRenderQueueLifecycleScalar = &'static str;
type StaticPageQueuedExportPackageDebugRenderer = StaticPageRenderQueueLifecycleScalar;
type StaticPageQueuedExportPackageDataSnapshotSource<'a> = &'a str;
type StaticPageQueuedExportPackageDebugFields<'a> = (
    StaticPageQueuedExportPackageDebugRenderer,
    StaticPageQueuedExportPackageModuleCount,
    StaticPageQueuedExportPackageModuleCount,
    StaticPageQueuedExportPackageDataSnapshotSource<'a>,
);
type StaticPageRenderQueueLifecycleFields = (
    StaticPageRenderQueueLifecycleScalar,
    StaticPageRenderQueueLifecycleScalar,
    StaticPageRenderQueueLifecycleScalar,
);
type StaticPageRenderQueueManifestValue = Value;
type StaticPageRenderQueueImageContextFields = (Option<StaticPageImageJobId>, Option<String>);
type StaticPageRenderQueueIdentityFields = (StaticPageDraftId, AssistantRunId);
type StaticPageRenderQueueWorkflowManifestValue = Value;
type StaticPageRenderQueueWorkflowContextFields = (
    StaticPageRenderQueueWorkflowManifestValue,
    Option<WorkflowExecutionId>,
    Option<WorkflowTaskId>,
);
type StaticPageRenderQueueWorkflowIdsFields = (Option<WorkflowExecutionId>, Option<WorkflowTaskId>);
type StaticPageRenderQueueWorkflowManifestFields = (
    StaticPageRenderQueueLifecycleScalar,
    Option<WorkflowExecutionId>,
    Option<WorkflowTaskId>,
);
type StaticPageRenderQueuePayloadAliasList = &'static [&'static str];
type StaticPageRenderQueueVisualFallbackStyle = &'static str;
type StaticPageRenderQueueModulesValue = Value;
type StaticPageRenderQueueVisualSpecValue = Value;
type StaticPageRenderQueueRenderSpecValue = Value;
type StaticPageRenderQueueVisualSpecOption = Option<StaticPageRenderQueueVisualSpecValue>;
type StaticPageRenderQueueRenderSpecOption = Option<StaticPageRenderQueueRenderSpecValue>;
type StaticPageRenderQueueSpecsFields = (
    StaticPageRenderQueueVisualSpecValue,
    StaticPageRenderQueueRenderSpecValue,
);
type StaticPageRenderQueueDataSnapshotValue = Value;
type StaticPageRenderQueueDataSnapshotOption = Option<StaticPageRenderQueueDataSnapshotValue>;
type StaticPageRenderQueueExportPackageValue = Value;
type StaticPageQueuedExportPackageModuleValue = Value;
type StaticPageQueuedExportPackageFilesValue = Value;
type StaticPageQueuedExportPackageDynamicPageContractValue = Value;
type StaticPageQueuedExportPackageDebugValue = Value;
type StaticPageQueuedExportPackageFileValue = Value;
type StaticPageQueuedExportPackageFileEntriesValue = Vec<StaticPageQueuedExportPackageFileValue>;
type StaticPageRenderQueueExportPackageFields<'a> = (
    &'a StaticPageDraft,
    &'a StaticPageRenderQueueModulesValue,
    &'a StaticPageRenderQueueDataSnapshotValue,
);
type StaticPageRenderQueueDataContextFields = (
    StaticPageRenderQueueDataSnapshotValue,
    StaticPageRenderQueueExportPackageValue,
);
type StaticPageQueuedExportPackageBaseContextFields = (
    StaticPageQueuedExportPackageLifecycleScalar,
    StaticPageQueuedExportPackageVersion,
    StaticPageQueuedExportPackageLifecycleScalar,
    StaticPageDraftId,
    StaticPageQueuedExportPackageFilesValue,
    StaticPageQueuedExportPackageDynamicPageContractValue,
);
type StaticPageQueuedExportPackageManifestFields = (
    StaticPageQueuedExportPackageLifecycleScalar,
    StaticPageQueuedExportPackageVersion,
    StaticPageQueuedExportPackageLifecycleScalar,
    StaticPageDraftId,
    StaticPageQueuedExportPackageFilesValue,
    StaticPageQueuedExportPackageDynamicPageContractValue,
    StaticPageQueuedExportPackageDebugValue,
);
type StaticPageRenderQueueManifestFields = (
    StaticPageDraftId,
    AssistantRunId,
    StaticPageRenderQueueLifecycleScalar,
    StaticPageRenderQueueLifecycleScalar,
    Option<WorkflowExecutionId>,
    Option<WorkflowTaskId>,
    StaticPageRenderQueueWorkflowManifestValue,
    Option<StaticPageImageJobId>,
    Option<String>,
    StaticPageRenderQueueVisualSpecValue,
    StaticPageRenderQueueRenderSpecValue,
    StaticPageRenderQueueDataSnapshotValue,
    StaticPageRenderQueueExportPackageValue,
    StaticPageRenderQueueLifecycleScalar,
);

pub(crate) fn build_static_page_render_queue_manifest(
    draft: &StaticPageDraft,
    image_job: Option<&StaticPageImageJob>,
    workflow_execution: Option<&WorkflowExecution>,
    workflow_task_id: Option<WorkflowTaskId>,
) -> StaticPageRenderQueueManifestValue {
    let (
        draft_id,
        assistant_run_id,
        status,
        renderer,
        workflow_execution_id,
        workflow_task_id_value,
        workflow_manifest,
        image_job_id,
        preview_asset_key,
        visual_spec,
        render_spec,
        data_snapshot,
        export_package,
        queue_copy,
    ) = build_static_page_render_queue_manifest_fields(
        draft,
        image_job,
        workflow_execution,
        workflow_task_id,
    );
    build_static_page_render_queue_manifest_from_fields(
        draft_id,
        assistant_run_id,
        status,
        renderer,
        workflow_execution_id,
        workflow_task_id_value,
        workflow_manifest,
        image_job_id,
        preview_asset_key,
        visual_spec,
        render_spec,
        data_snapshot,
        export_package,
        queue_copy,
    )
}

pub(crate) fn build_static_page_render_queue_manifest_fields(
    draft: &StaticPageDraft,
    image_job: Option<&StaticPageImageJob>,
    workflow_execution: Option<&WorkflowExecution>,
    workflow_task_id: Option<WorkflowTaskId>,
) -> StaticPageRenderQueueManifestFields {
    let payload = &draft.draft_payload;
    let (data_snapshot, export_package) = build_static_page_render_queue_data_context(draft);
    let (workflow_manifest, workflow_execution_id, workflow_task_id_value) =
        build_static_page_render_queue_workflow_context(workflow_execution, workflow_task_id);
    let (image_job_id, preview_asset_key) = build_static_page_render_queue_image_context(image_job);
    let (draft_id, assistant_run_id) = build_static_page_render_queue_identity(draft);
    let (status, renderer, queue_copy) = build_static_page_render_queue_lifecycle();
    let (visual_spec, render_spec) = build_static_page_render_queue_specs(payload);
    (
        draft_id,
        assistant_run_id,
        status,
        renderer,
        workflow_execution_id,
        workflow_task_id_value,
        workflow_manifest,
        image_job_id,
        preview_asset_key,
        visual_spec,
        render_spec,
        data_snapshot,
        export_package,
        queue_copy,
    )
}

pub(crate) fn build_static_page_render_queue_manifest_from_fields(
    draft_id: StaticPageDraftId,
    assistant_run_id: AssistantRunId,
    status: StaticPageRenderQueueLifecycleScalar,
    renderer: StaticPageRenderQueueLifecycleScalar,
    workflow_execution_id: Option<WorkflowExecutionId>,
    workflow_task_id_value: Option<WorkflowTaskId>,
    workflow_manifest: StaticPageRenderQueueWorkflowManifestValue,
    image_job_id: Option<StaticPageImageJobId>,
    preview_asset_key: Option<String>,
    visual_spec: StaticPageRenderQueueVisualSpecValue,
    render_spec: StaticPageRenderQueueRenderSpecValue,
    data_snapshot: StaticPageRenderQueueDataSnapshotValue,
    export_package: StaticPageRenderQueueExportPackageValue,
    queue_copy: StaticPageRenderQueueLifecycleScalar,
) -> StaticPageRenderQueueManifestValue {
    build_static_page_render_queue_manifest_payload_fields(
        draft_id,
        assistant_run_id,
        status,
        renderer,
        workflow_execution_id,
        workflow_task_id_value,
        workflow_manifest,
        image_job_id,
        preview_asset_key,
        visual_spec,
        render_spec,
        data_snapshot,
        export_package,
        queue_copy,
    )
}

pub(crate) fn build_static_page_render_queue_manifest_payload_fields(
    draft_id: StaticPageDraftId,
    assistant_run_id: AssistantRunId,
    status: StaticPageRenderQueueLifecycleScalar,
    renderer: StaticPageRenderQueueLifecycleScalar,
    workflow_execution_id: Option<WorkflowExecutionId>,
    workflow_task_id_value: Option<WorkflowTaskId>,
    workflow_manifest: StaticPageRenderQueueWorkflowManifestValue,
    image_job_id: Option<StaticPageImageJobId>,
    preview_asset_key: Option<String>,
    visual_spec: StaticPageRenderQueueVisualSpecValue,
    render_spec: StaticPageRenderQueueRenderSpecValue,
    data_snapshot: StaticPageRenderQueueDataSnapshotValue,
    export_package: StaticPageRenderQueueExportPackageValue,
    queue_copy: StaticPageRenderQueueLifecycleScalar,
) -> StaticPageRenderQueueManifestValue {
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
        "visual_spec": visual_spec,
        "render_spec": render_spec,
        "data_snapshot": data_snapshot,
        "export_package": export_package,
        "queue_copy": queue_copy,
    })
}

pub(crate) fn build_static_page_render_queue_specs(
    payload: &Value,
) -> StaticPageRenderQueueSpecsFields {
    build_static_page_render_queue_specs_from_payload(payload)
}

pub(crate) fn build_static_page_render_queue_specs_from_payload(
    payload: &Value,
) -> StaticPageRenderQueueSpecsFields {
    let (visual_spec, render_spec) =
        build_static_page_render_queue_specs_from_payload_fields(payload);
    build_static_page_render_queue_specs_from_fields(visual_spec, render_spec)
}

pub(crate) fn build_static_page_render_queue_specs_from_payload_fields(
    payload: &Value,
) -> StaticPageRenderQueueSpecsFields {
    let visual_spec = build_static_page_render_queue_visual_spec(payload);
    let render_spec = build_static_page_render_queue_render_spec(payload);
    (visual_spec, render_spec)
}

pub(crate) fn build_static_page_render_queue_specs_from_fields(
    visual_spec: StaticPageRenderQueueVisualSpecValue,
    render_spec: StaticPageRenderQueueRenderSpecValue,
) -> StaticPageRenderQueueSpecsFields {
    (visual_spec, render_spec)
}

pub(crate) fn build_static_page_render_queue_data_context(
    draft: &StaticPageDraft,
) -> StaticPageRenderQueueDataContextFields {
    let (data_snapshot, export_package) = build_static_page_render_queue_data_context_fields(draft);
    build_static_page_render_queue_data_context_from_fields(data_snapshot, export_package)
}

pub(crate) fn build_static_page_render_queue_data_context_fields(
    draft: &StaticPageDraft,
) -> StaticPageRenderQueueDataContextFields {
    let payload = &draft.draft_payload;
    build_static_page_render_queue_data_context_fields_from_payload(draft, payload)
}

pub(crate) fn build_static_page_render_queue_data_context_fields_from_payload(
    draft: &StaticPageDraft,
    payload: &Value,
) -> StaticPageRenderQueueDataContextFields {
    let data_snapshot =
        build_static_page_render_queue_data_snapshot(payload, &draft.selected_scope);
    let export_package =
        build_static_page_render_queue_data_context_export_package(draft, payload, &data_snapshot);
    (data_snapshot, export_package)
}

pub(crate) fn build_static_page_render_queue_data_context_export_package(
    draft: &StaticPageDraft,
    payload: &Value,
    data_snapshot: &StaticPageRenderQueueDataSnapshotValue,
) -> StaticPageRenderQueueExportPackageValue {
    let modules = build_static_page_render_queue_data_context_export_package_fields(payload);
    build_static_page_render_queue_data_context_export_package_from_fields(
        draft,
        &modules,
        data_snapshot,
    )
}

pub(crate) fn build_static_page_render_queue_data_context_export_package_fields(
    payload: &Value,
) -> StaticPageRenderQueueModulesValue {
    build_static_page_render_queue_modules(payload)
}

pub(crate) fn build_static_page_render_queue_data_context_export_package_from_fields(
    draft: &StaticPageDraft,
    modules: &StaticPageRenderQueueModulesValue,
    data_snapshot: &StaticPageRenderQueueDataSnapshotValue,
) -> StaticPageRenderQueueExportPackageValue {
    build_static_page_render_queue_export_package(draft, modules, data_snapshot)
}

pub(crate) fn build_static_page_render_queue_data_context_from_fields(
    data_snapshot: StaticPageRenderQueueDataSnapshotValue,
    export_package: StaticPageRenderQueueExportPackageValue,
) -> StaticPageRenderQueueDataContextFields {
    (data_snapshot, export_package)
}

pub(crate) fn build_static_page_render_queue_lifecycle() -> StaticPageRenderQueueLifecycleFields {
    let (status, renderer, queue_copy) = build_static_page_render_queue_lifecycle_fields();
    build_static_page_render_queue_lifecycle_from_fields(status, renderer, queue_copy)
}

pub(crate) fn build_static_page_render_queue_lifecycle_fields(
) -> StaticPageRenderQueueLifecycleFields {
    (
        build_static_page_render_queue_status(),
        build_static_page_render_queue_renderer(),
        build_static_page_render_queue_queue_copy(),
    )
}

pub(crate) fn build_static_page_render_queue_lifecycle_from_fields(
    status: StaticPageRenderQueueLifecycleScalar,
    renderer: StaticPageRenderQueueLifecycleScalar,
    queue_copy: StaticPageRenderQueueLifecycleScalar,
) -> StaticPageRenderQueueLifecycleFields {
    (status, renderer, queue_copy)
}

pub(crate) fn build_static_page_render_queue_status() -> StaticPageRenderQueueLifecycleScalar {
    "queued"
}

pub(crate) fn build_static_page_render_queue_renderer() -> StaticPageRenderQueueLifecycleScalar {
    "static-page-renderer-v1"
}

pub(crate) fn build_static_page_render_queue_queue_copy() -> StaticPageRenderQueueLifecycleScalar {
    "最终静态页正在后台制作，可以继续聊天或修改其他内容。"
}

pub(crate) fn build_static_page_render_queue_image_context(
    image_job: Option<&StaticPageImageJob>,
) -> StaticPageRenderQueueImageContextFields {
    let (image_job_id, preview_asset_key) =
        build_static_page_render_queue_image_context_fields(image_job);
    build_static_page_render_queue_image_context_from_fields(image_job_id, preview_asset_key)
}

pub(crate) fn build_static_page_render_queue_image_context_fields(
    image_job: Option<&StaticPageImageJob>,
) -> StaticPageRenderQueueImageContextFields {
    (
        build_static_page_render_queue_image_job_id(image_job),
        build_static_page_render_queue_preview_asset_key(image_job),
    )
}

pub(crate) fn build_static_page_render_queue_image_context_from_fields(
    image_job_id: Option<StaticPageImageJobId>,
    preview_asset_key: Option<String>,
) -> StaticPageRenderQueueImageContextFields {
    (image_job_id, preview_asset_key)
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
) -> StaticPageRenderQueueIdentityFields {
    let (draft_id, assistant_run_id) = build_static_page_render_queue_identity_fields(draft);
    build_static_page_render_queue_identity_from_fields(draft_id, assistant_run_id)
}

pub(crate) fn build_static_page_render_queue_identity_fields(
    draft: &StaticPageDraft,
) -> StaticPageRenderQueueIdentityFields {
    (
        build_static_page_render_queue_draft_id(draft),
        build_static_page_render_queue_assistant_run_id(draft),
    )
}

pub(crate) fn build_static_page_render_queue_identity_from_fields(
    draft_id: StaticPageDraftId,
    assistant_run_id: AssistantRunId,
) -> StaticPageRenderQueueIdentityFields {
    (draft_id, assistant_run_id)
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

pub(crate) fn build_static_page_render_queue_workflow_context(
    workflow_execution: Option<&WorkflowExecution>,
    workflow_task_id: Option<WorkflowTaskId>,
) -> StaticPageRenderQueueWorkflowContextFields {
    let (workflow_manifest, workflow_execution_id, workflow_task_id_value) =
        build_static_page_render_queue_workflow_context_fields(
            workflow_execution,
            workflow_task_id,
        );
    build_static_page_render_queue_workflow_context_from_fields(
        workflow_manifest,
        workflow_execution_id,
        workflow_task_id_value,
    )
}

pub(crate) fn build_static_page_render_queue_workflow_context_fields(
    workflow_execution: Option<&WorkflowExecution>,
    workflow_task_id: Option<WorkflowTaskId>,
) -> StaticPageRenderQueueWorkflowContextFields {
    let workflow_manifest =
        build_static_page_render_queue_workflow_manifest(workflow_execution, workflow_task_id);
    let (workflow_execution_id, workflow_task_id_value) =
        build_static_page_render_queue_workflow_ids(workflow_execution, workflow_task_id);
    (
        workflow_manifest,
        workflow_execution_id,
        workflow_task_id_value,
    )
}

pub(crate) fn build_static_page_render_queue_workflow_context_from_fields(
    workflow_manifest: StaticPageRenderQueueWorkflowManifestValue,
    workflow_execution_id: Option<WorkflowExecutionId>,
    workflow_task_id: Option<WorkflowTaskId>,
) -> StaticPageRenderQueueWorkflowContextFields {
    (workflow_manifest, workflow_execution_id, workflow_task_id)
}

pub(crate) fn build_static_page_render_queue_workflow_ids(
    workflow_execution: Option<&WorkflowExecution>,
    workflow_task_id: Option<WorkflowTaskId>,
) -> StaticPageRenderQueueWorkflowIdsFields {
    let (workflow_execution_id, workflow_task_id_value) =
        build_static_page_render_queue_workflow_ids_fields(workflow_execution, workflow_task_id);
    build_static_page_render_queue_workflow_ids_from_fields(
        workflow_execution_id,
        workflow_task_id_value,
    )
}

pub(crate) fn build_static_page_render_queue_workflow_ids_fields(
    workflow_execution: Option<&WorkflowExecution>,
    workflow_task_id: Option<WorkflowTaskId>,
) -> StaticPageRenderQueueWorkflowIdsFields {
    (
        build_static_page_render_queue_workflow_execution_id(workflow_execution),
        build_static_page_render_queue_workflow_task_id(workflow_task_id),
    )
}

pub(crate) fn build_static_page_render_queue_workflow_ids_from_fields(
    workflow_execution_id: Option<WorkflowExecutionId>,
    workflow_task_id: Option<WorkflowTaskId>,
) -> StaticPageRenderQueueWorkflowIdsFields {
    (workflow_execution_id, workflow_task_id)
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
) -> StaticPageRenderQueueWorkflowManifestValue {
    let (status, execution_id, task_id) = build_static_page_render_queue_workflow_manifest_fields(
        workflow_execution,
        workflow_task_id,
    );
    build_static_page_render_queue_workflow_manifest_from_fields(status, execution_id, task_id)
}

pub(crate) fn build_static_page_render_queue_workflow_manifest_fields(
    workflow_execution: Option<&WorkflowExecution>,
    workflow_task_id: Option<WorkflowTaskId>,
) -> StaticPageRenderQueueWorkflowManifestFields {
    let status = build_static_page_render_queue_workflow_status();
    let execution_id = build_static_page_render_queue_workflow_execution_id(workflow_execution);
    let task_id = build_static_page_render_queue_workflow_task_id(workflow_task_id);
    build_static_page_render_queue_workflow_manifest_fields_from_fields(
        status,
        execution_id,
        task_id,
    )
}

pub(crate) fn build_static_page_render_queue_workflow_manifest_fields_from_fields(
    status: StaticPageRenderQueueLifecycleScalar,
    execution_id: Option<WorkflowExecutionId>,
    task_id: Option<WorkflowTaskId>,
) -> StaticPageRenderQueueWorkflowManifestFields {
    (status, execution_id, task_id)
}

pub(crate) fn build_static_page_render_queue_workflow_manifest_from_fields(
    status: StaticPageRenderQueueLifecycleScalar,
    execution_id: Option<WorkflowExecutionId>,
    task_id: Option<WorkflowTaskId>,
) -> StaticPageRenderQueueWorkflowManifestValue {
    build_static_page_render_queue_workflow_manifest_payload_fields(status, execution_id, task_id)
}

pub(crate) fn build_static_page_render_queue_workflow_manifest_payload_fields(
    status: StaticPageRenderQueueLifecycleScalar,
    execution_id: Option<WorkflowExecutionId>,
    task_id: Option<WorkflowTaskId>,
) -> StaticPageRenderQueueWorkflowManifestValue {
    json!({
        "status": status,
        "executionId": execution_id,
        "taskId": task_id,
    })
}

pub(crate) fn build_static_page_render_queue_workflow_status(
) -> StaticPageRenderQueueLifecycleScalar {
    build_static_page_render_queue_status()
}

pub(crate) fn build_static_page_render_queue_visual_spec(
    payload: &Value,
) -> StaticPageRenderQueueVisualSpecValue {
    build_static_page_render_queue_visual_spec_from_optional(
        build_static_page_render_queue_visual_spec_fields(payload),
    )
}

pub(crate) fn build_static_page_render_queue_visual_spec_fields(
    payload: &Value,
) -> StaticPageRenderQueueVisualSpecOption {
    build_static_page_render_queue_visual_spec_from_payload(payload)
}

pub(crate) fn build_static_page_render_queue_visual_spec_from_optional(
    visual_spec: StaticPageRenderQueueVisualSpecOption,
) -> StaticPageRenderQueueVisualSpecValue {
    build_static_page_render_queue_visual_spec_from_optional_fields(visual_spec)
}

pub(crate) fn build_static_page_render_queue_visual_spec_from_optional_fields(
    visual_spec: StaticPageRenderQueueVisualSpecOption,
) -> StaticPageRenderQueueVisualSpecValue {
    visual_spec.unwrap_or_else(build_static_page_render_queue_visual_fallback_spec)
}

pub(crate) fn build_static_page_render_queue_visual_spec_from_payload(
    payload: &Value,
) -> StaticPageRenderQueueVisualSpecOption {
    build_static_page_render_queue_visual_spec_from_payload_fields(payload)
}

pub(crate) fn build_static_page_render_queue_visual_spec_from_payload_fields(
    payload: &Value,
) -> StaticPageRenderQueueVisualSpecOption {
    static_page_payload_value(
        payload,
        build_static_page_render_queue_visual_spec_aliases(),
    )
}

pub(crate) fn build_static_page_render_queue_visual_spec_aliases(
) -> StaticPageRenderQueuePayloadAliasList {
    &["visualSpec", "visual_spec"]
}

pub(crate) fn build_static_page_render_queue_visual_fallback_style(
) -> StaticPageRenderQueueVisualFallbackStyle {
    "client-delivery"
}

pub(crate) fn build_static_page_render_queue_visual_fallback_spec(
) -> StaticPageRenderQueueVisualSpecValue {
    build_static_page_render_queue_visual_fallback_spec_from_fields(
        build_static_page_render_queue_visual_fallback_style(),
    )
}

pub(crate) fn build_static_page_render_queue_visual_fallback_spec_from_fields(
    style: StaticPageRenderQueueVisualFallbackStyle,
) -> StaticPageRenderQueueVisualSpecValue {
    build_static_page_visual_spec(style)
}

pub(crate) fn build_static_page_render_queue_render_spec(
    payload: &Value,
) -> StaticPageRenderQueueRenderSpecValue {
    build_static_page_render_queue_render_spec_from_optional(
        build_static_page_render_queue_render_spec_fields(payload),
    )
}

pub(crate) fn build_static_page_render_queue_render_spec_fields(
    payload: &Value,
) -> StaticPageRenderQueueRenderSpecOption {
    build_static_page_render_queue_render_spec_from_payload(payload)
}

pub(crate) fn build_static_page_render_queue_render_spec_from_optional(
    render_spec: StaticPageRenderQueueRenderSpecOption,
) -> StaticPageRenderQueueRenderSpecValue {
    build_static_page_render_queue_render_spec_from_optional_fields(render_spec)
}

pub(crate) fn build_static_page_render_queue_render_spec_from_optional_fields(
    render_spec: StaticPageRenderQueueRenderSpecOption,
) -> StaticPageRenderQueueRenderSpecValue {
    render_spec.unwrap_or_else(build_static_page_render_queue_render_fallback_spec)
}

pub(crate) fn build_static_page_render_queue_render_spec_from_payload(
    payload: &Value,
) -> StaticPageRenderQueueRenderSpecOption {
    build_static_page_render_queue_render_spec_from_payload_fields(payload)
}

pub(crate) fn build_static_page_render_queue_render_spec_from_payload_fields(
    payload: &Value,
) -> StaticPageRenderQueueRenderSpecOption {
    static_page_payload_value(
        payload,
        build_static_page_render_queue_render_spec_aliases(),
    )
}

pub(crate) fn build_static_page_render_queue_render_spec_aliases(
) -> StaticPageRenderQueuePayloadAliasList {
    &["renderSpec", "render_spec"]
}

pub(crate) fn build_static_page_render_queue_render_fallback_spec(
) -> StaticPageRenderQueueRenderSpecValue {
    build_static_page_render_queue_render_fallback_spec_from_fields()
}

pub(crate) fn build_static_page_render_queue_render_fallback_spec_from_fields(
) -> StaticPageRenderQueueRenderSpecValue {
    build_static_page_render_spec()
}

pub(crate) fn build_static_page_render_queue_data_snapshot(
    payload: &Value,
    selected_scope: &Value,
) -> StaticPageRenderQueueDataSnapshotValue {
    build_static_page_render_queue_data_snapshot_from_optional(
        build_static_page_render_queue_data_snapshot_fields(payload),
        payload,
        selected_scope,
    )
}

pub(crate) fn build_static_page_render_queue_data_snapshot_fields(
    payload: &Value,
) -> StaticPageRenderQueueDataSnapshotOption {
    build_static_page_render_queue_data_snapshot_from_payload(payload)
}

pub(crate) fn build_static_page_render_queue_data_snapshot_from_optional(
    data_snapshot: StaticPageRenderQueueDataSnapshotOption,
    payload: &Value,
    selected_scope: &Value,
) -> StaticPageRenderQueueDataSnapshotValue {
    build_static_page_render_queue_data_snapshot_from_optional_fields(
        data_snapshot,
        payload,
        selected_scope,
    )
}

pub(crate) fn build_static_page_render_queue_data_snapshot_from_optional_fields(
    data_snapshot: StaticPageRenderQueueDataSnapshotOption,
    payload: &Value,
    selected_scope: &Value,
) -> StaticPageRenderQueueDataSnapshotValue {
    data_snapshot.unwrap_or_else(|| {
        build_static_page_render_queue_fallback_data_snapshot(payload, selected_scope)
    })
}

pub(crate) fn build_static_page_render_queue_data_snapshot_from_payload(
    payload: &Value,
) -> StaticPageRenderQueueDataSnapshotOption {
    build_static_page_render_queue_data_snapshot_from_payload_fields(payload)
}

pub(crate) fn build_static_page_render_queue_data_snapshot_from_payload_fields(
    payload: &Value,
) -> StaticPageRenderQueueDataSnapshotOption {
    static_page_payload_value(
        payload,
        build_static_page_render_queue_data_snapshot_aliases(),
    )
}

pub(crate) fn build_static_page_render_queue_data_snapshot_aliases(
) -> StaticPageRenderQueuePayloadAliasList {
    &["dataSnapshot", "data_snapshot"]
}

pub(crate) fn build_static_page_render_queue_fallback_data_snapshot(
    payload: &Value,
    selected_scope: &Value,
) -> StaticPageRenderQueueDataSnapshotValue {
    build_static_page_render_queue_fallback_data_snapshot_from_fields(payload, selected_scope)
}

pub(crate) fn build_static_page_render_queue_fallback_data_snapshot_from_fields(
    payload: &Value,
    selected_scope: &Value,
) -> StaticPageRenderQueueDataSnapshotValue {
    build_static_page_data_snapshot(payload, selected_scope)
}

pub(crate) fn build_static_page_render_queue_modules(
    payload: &Value,
) -> StaticPageRenderQueueModulesValue {
    build_static_page_render_queue_modules_from_value(
        build_static_page_render_queue_modules_fields(payload),
    )
}

pub(crate) fn build_static_page_render_queue_modules_fields(
    payload: &Value,
) -> StaticPageRenderQueueModulesValue {
    build_static_page_render_queue_modules_from_payload(payload)
}

pub(crate) fn build_static_page_render_queue_modules_from_payload(
    payload: &Value,
) -> StaticPageRenderQueueModulesValue {
    build_static_page_render_queue_modules_from_payload_fields(payload)
}

pub(crate) fn build_static_page_render_queue_modules_from_payload_fields(
    payload: &Value,
) -> StaticPageRenderQueueModulesValue {
    static_page_payload_modules(payload)
}

pub(crate) fn build_static_page_render_queue_modules_from_value(
    modules: StaticPageRenderQueueModulesValue,
) -> StaticPageRenderQueueModulesValue {
    build_static_page_render_queue_modules_from_value_fields(modules)
}

pub(crate) fn build_static_page_render_queue_modules_from_value_fields(
    modules: StaticPageRenderQueueModulesValue,
) -> StaticPageRenderQueueModulesValue {
    modules
}

pub(crate) fn build_static_page_render_queue_export_package(
    draft: &StaticPageDraft,
    modules: &StaticPageRenderQueueModulesValue,
    data_snapshot: &StaticPageRenderQueueDataSnapshotValue,
) -> StaticPageRenderQueueExportPackageValue {
    let (draft, modules, data_snapshot) =
        build_static_page_render_queue_export_package_fields(draft, modules, data_snapshot);
    build_static_page_render_queue_export_package_from_fields(draft, modules, data_snapshot)
}

pub(crate) fn build_static_page_render_queue_export_package_fields<'a>(
    draft: &'a StaticPageDraft,
    modules: &'a StaticPageRenderQueueModulesValue,
    data_snapshot: &'a StaticPageRenderQueueDataSnapshotValue,
) -> StaticPageRenderQueueExportPackageFields<'a> {
    (draft, modules, data_snapshot)
}

pub(crate) fn build_static_page_render_queue_export_package_from_fields(
    draft: &StaticPageDraft,
    modules: &StaticPageRenderQueueModulesValue,
    data_snapshot: &StaticPageRenderQueueDataSnapshotValue,
) -> StaticPageRenderQueueExportPackageValue {
    build_static_page_queued_export_package_manifest(draft, modules, data_snapshot)
}

pub(crate) fn build_static_page_queued_export_package_manifest(
    draft: &StaticPageDraft,
    modules: &StaticPageRenderQueueModulesValue,
    data_snapshot: &StaticPageRenderQueueDataSnapshotValue,
) -> StaticPageRenderQueueExportPackageValue {
    let (kind, version, status, draft_id, files, dynamic_page_contract, debug) =
        build_static_page_queued_export_package_manifest_fields(draft, modules, data_snapshot);
    build_static_page_queued_export_package_manifest_from_fields(
        kind,
        version,
        status,
        draft_id,
        files,
        dynamic_page_contract,
        debug,
    )
}

pub(crate) fn build_static_page_queued_export_package_manifest_fields(
    draft: &StaticPageDraft,
    modules: &StaticPageRenderQueueModulesValue,
    data_snapshot: &StaticPageRenderQueueDataSnapshotValue,
) -> StaticPageQueuedExportPackageManifestFields {
    let (kind, version, status, draft_id, files, dynamic_page_contract, debug) =
        build_static_page_queued_export_package_manifest_context_fields(
            draft,
            modules,
            data_snapshot,
        );
    build_static_page_queued_export_package_manifest_fields_from_fields(
        kind,
        version,
        status,
        draft_id,
        files,
        dynamic_page_contract,
        debug,
    )
}

pub(crate) fn build_static_page_queued_export_package_manifest_context_fields(
    draft: &StaticPageDraft,
    modules: &StaticPageRenderQueueModulesValue,
    data_snapshot: &StaticPageRenderQueueDataSnapshotValue,
) -> StaticPageQueuedExportPackageManifestFields {
    let debug = build_static_page_queued_export_package_debug_context(modules, data_snapshot);
    let (kind, version, status, draft_id, files, dynamic_page_contract) =
        build_static_page_queued_export_package_base_context(draft);
    (
        kind,
        version,
        status,
        draft_id,
        files,
        dynamic_page_contract,
        debug,
    )
}

pub(crate) fn build_static_page_queued_export_package_manifest_fields_from_fields(
    kind: StaticPageQueuedExportPackageLifecycleScalar,
    version: StaticPageQueuedExportPackageVersion,
    status: StaticPageQueuedExportPackageLifecycleScalar,
    draft_id: StaticPageDraftId,
    files: StaticPageQueuedExportPackageFilesValue,
    dynamic_page_contract: StaticPageQueuedExportPackageDynamicPageContractValue,
    debug: StaticPageQueuedExportPackageDebugValue,
) -> StaticPageQueuedExportPackageManifestFields {
    (
        kind,
        version,
        status,
        draft_id,
        files,
        dynamic_page_contract,
        debug,
    )
}

pub(crate) fn build_static_page_queued_export_package_manifest_from_fields(
    kind: StaticPageQueuedExportPackageLifecycleScalar,
    version: StaticPageQueuedExportPackageVersion,
    status: StaticPageQueuedExportPackageLifecycleScalar,
    draft_id: StaticPageDraftId,
    files: StaticPageQueuedExportPackageFilesValue,
    dynamic_page_contract: StaticPageQueuedExportPackageDynamicPageContractValue,
    debug: StaticPageQueuedExportPackageDebugValue,
) -> StaticPageRenderQueueExportPackageValue {
    build_static_page_queued_export_package_manifest_payload_fields(
        kind,
        version,
        status,
        draft_id,
        files,
        dynamic_page_contract,
        debug,
    )
}

pub(crate) fn build_static_page_queued_export_package_manifest_payload_fields(
    kind: StaticPageQueuedExportPackageLifecycleScalar,
    version: StaticPageQueuedExportPackageVersion,
    status: StaticPageQueuedExportPackageLifecycleScalar,
    draft_id: StaticPageDraftId,
    files: StaticPageQueuedExportPackageFilesValue,
    dynamic_page_contract: StaticPageQueuedExportPackageDynamicPageContractValue,
    debug: StaticPageQueuedExportPackageDebugValue,
) -> StaticPageRenderQueueExportPackageValue {
    json!({
        "kind": kind,
        "version": version,
        "status": status,
        "draft_id": draft_id,
        "files": files,
        "dynamic_page_contract": dynamic_page_contract,
        "debug": debug
    })
}

pub(crate) fn build_static_page_queued_export_package_base_context(
    draft: &StaticPageDraft,
) -> StaticPageQueuedExportPackageBaseContextFields {
    let (kind, version, status, draft_id, files, dynamic_page_contract) =
        build_static_page_queued_export_package_base_context_fields(draft);
    build_static_page_queued_export_package_base_context_from_fields(
        kind,
        version,
        status,
        draft_id,
        files,
        dynamic_page_contract,
    )
}

pub(crate) fn build_static_page_queued_export_package_base_context_fields(
    draft: &StaticPageDraft,
) -> StaticPageQueuedExportPackageBaseContextFields {
    let (kind, version, status) = build_static_page_queued_export_package_lifecycle();
    let draft_id = build_static_page_queued_export_package_draft_id(draft);
    let files = build_static_page_queued_export_package_files();
    let dynamic_page_contract = build_static_page_queued_export_package_dynamic_page_contract();
    build_static_page_queued_export_package_base_context_fields_from_fields(
        kind,
        version,
        status,
        draft_id,
        files,
        dynamic_page_contract,
    )
}

pub(crate) fn build_static_page_queued_export_package_base_context_fields_from_fields(
    kind: StaticPageQueuedExportPackageLifecycleScalar,
    version: StaticPageQueuedExportPackageVersion,
    status: StaticPageQueuedExportPackageLifecycleScalar,
    draft_id: StaticPageDraftId,
    files: StaticPageQueuedExportPackageFilesValue,
    dynamic_page_contract: StaticPageQueuedExportPackageDynamicPageContractValue,
) -> StaticPageQueuedExportPackageBaseContextFields {
    build_static_page_queued_export_package_base_context_from_fields(
        kind,
        version,
        status,
        draft_id,
        files,
        dynamic_page_contract,
    )
}

pub(crate) fn build_static_page_queued_export_package_base_context_from_fields(
    kind: StaticPageQueuedExportPackageLifecycleScalar,
    version: StaticPageQueuedExportPackageVersion,
    status: StaticPageQueuedExportPackageLifecycleScalar,
    draft_id: StaticPageDraftId,
    files: StaticPageQueuedExportPackageFilesValue,
    dynamic_page_contract: StaticPageQueuedExportPackageDynamicPageContractValue,
) -> StaticPageQueuedExportPackageBaseContextFields {
    (
        kind,
        version,
        status,
        draft_id,
        files,
        dynamic_page_contract,
    )
}

pub(crate) fn build_static_page_queued_export_package_debug_context(
    modules: &StaticPageRenderQueueModulesValue,
    data_snapshot: &StaticPageRenderQueueDataSnapshotValue,
) -> StaticPageQueuedExportPackageDebugValue {
    let (module_count, echarts_requested_modules) =
        build_static_page_queued_export_package_debug_context_fields(modules);
    build_static_page_queued_export_package_debug_context_from_fields(
        module_count,
        echarts_requested_modules,
        data_snapshot,
    )
}

pub(crate) fn build_static_page_queued_export_package_debug_context_fields(
    modules: &StaticPageRenderQueueModulesValue,
) -> StaticPageQueuedExportPackageModuleCounts {
    build_static_page_queued_export_package_module_counts(modules)
}

pub(crate) fn build_static_page_queued_export_package_debug_context_from_fields(
    module_count: StaticPageQueuedExportPackageModuleCount,
    echarts_requested_modules: StaticPageQueuedExportPackageModuleCount,
    data_snapshot: &StaticPageRenderQueueDataSnapshotValue,
) -> StaticPageQueuedExportPackageDebugValue {
    build_static_page_queued_export_package_debug(
        module_count,
        echarts_requested_modules,
        data_snapshot,
    )
}

pub(crate) fn build_static_page_queued_export_package_lifecycle(
) -> StaticPageQueuedExportPackageLifecycleFields {
    let (kind, version, status) = build_static_page_queued_export_package_lifecycle_fields();
    build_static_page_queued_export_package_lifecycle_from_fields(kind, version, status)
}

pub(crate) fn build_static_page_queued_export_package_lifecycle_fields(
) -> StaticPageQueuedExportPackageLifecycleFields {
    (
        build_static_page_queued_export_package_kind(),
        build_static_page_queued_export_package_version(),
        build_static_page_queued_export_package_status(),
    )
}

pub(crate) fn build_static_page_queued_export_package_lifecycle_from_fields(
    kind: StaticPageQueuedExportPackageLifecycleScalar,
    version: StaticPageQueuedExportPackageVersion,
    status: StaticPageQueuedExportPackageLifecycleScalar,
) -> StaticPageQueuedExportPackageLifecycleFields {
    (kind, version, status)
}

pub(crate) fn build_static_page_queued_export_package_kind(
) -> StaticPageQueuedExportPackageLifecycleScalar {
    build_static_page_queued_export_package_kind_fields()
}

pub(crate) fn build_static_page_queued_export_package_kind_fields(
) -> StaticPageQueuedExportPackageLifecycleScalar {
    "static-page-export-package"
}

pub(crate) fn build_static_page_queued_export_package_version(
) -> StaticPageQueuedExportPackageVersion {
    build_static_page_queued_export_package_version_fields()
}

pub(crate) fn build_static_page_queued_export_package_version_fields(
) -> StaticPageQueuedExportPackageVersion {
    1
}

pub(crate) fn build_static_page_queued_export_package_status(
) -> StaticPageQueuedExportPackageLifecycleScalar {
    build_static_page_queued_export_package_status_fields()
}

pub(crate) fn build_static_page_queued_export_package_status_fields(
) -> StaticPageQueuedExportPackageLifecycleScalar {
    build_static_page_render_queue_status()
}

pub(crate) fn build_static_page_queued_export_package_draft_id(
    draft: &StaticPageDraft,
) -> StaticPageDraftId {
    build_static_page_queued_export_package_draft_id_fields(draft)
}

pub(crate) fn build_static_page_queued_export_package_draft_id_fields(
    draft: &StaticPageDraft,
) -> StaticPageDraftId {
    build_static_page_render_queue_draft_id(draft)
}

pub(crate) fn build_static_page_queued_export_package_dynamic_page_contract(
) -> StaticPageQueuedExportPackageDynamicPageContractValue {
    build_static_page_queued_export_package_dynamic_page_contract_from_fields(
        build_static_page_queued_export_package_dynamic_page_contract_fields(),
    )
}

pub(crate) fn build_static_page_queued_export_package_dynamic_page_contract_fields(
) -> StaticPageQueuedExportPackageDynamicPageContractValue {
    build_static_page_dynamic_page_contract()
}

pub(crate) fn build_static_page_queued_export_package_dynamic_page_contract_from_fields(
    dynamic_page_contract: StaticPageQueuedExportPackageDynamicPageContractValue,
) -> StaticPageQueuedExportPackageDynamicPageContractValue {
    dynamic_page_contract
}

pub(crate) fn build_static_page_queued_export_package_module_counts(
    modules: &StaticPageRenderQueueModulesValue,
) -> StaticPageQueuedExportPackageModuleCounts {
    build_static_page_queued_export_package_module_counts_from_optional_items(
        build_static_page_queued_export_package_module_counts_fields(modules),
    )
}

pub(crate) fn build_static_page_queued_export_package_module_counts_fields(
    modules: &StaticPageRenderQueueModulesValue,
) -> Option<&[Value]> {
    build_static_page_queued_export_package_module_items(modules)
}

pub(crate) fn build_static_page_queued_export_package_module_items(
    modules: &StaticPageRenderQueueModulesValue,
) -> Option<&[Value]> {
    build_static_page_queued_export_package_module_items_fields(modules)
}

pub(crate) fn build_static_page_queued_export_package_module_items_fields(
    modules: &StaticPageRenderQueueModulesValue,
) -> Option<&[Value]> {
    modules.as_array().map(Vec::as_slice)
}

pub(crate) fn build_static_page_queued_export_package_module_counts_from_optional_items(
    items: Option<&[Value]>,
) -> StaticPageQueuedExportPackageModuleCounts {
    let items =
        build_static_page_queued_export_package_module_counts_from_optional_items_fields(items);
    let Some(items) = items else {
        return build_static_page_queued_export_package_empty_module_counts();
    };
    build_static_page_queued_export_package_module_counts_from_items(items)
}

pub(crate) fn build_static_page_queued_export_package_module_counts_from_optional_items_fields(
    items: Option<&[Value]>,
) -> Option<&[Value]> {
    items
}

pub(crate) fn build_static_page_queued_export_package_module_counts_from_items(
    items: &[Value],
) -> StaticPageQueuedExportPackageModuleCounts {
    let (module_count, echarts_requested_modules) =
        build_static_page_queued_export_package_module_counts_from_items_fields(items);
    build_static_page_queued_export_package_module_counts_from_fields(
        module_count,
        echarts_requested_modules,
    )
}

pub(crate) fn build_static_page_queued_export_package_module_counts_from_items_fields(
    items: &[Value],
) -> StaticPageQueuedExportPackageModuleCounts {
    let module_count = build_static_page_queued_export_package_module_count(items);
    let echarts_requested_modules =
        build_static_page_queued_export_package_echarts_requested_module_count(items);
    (module_count, echarts_requested_modules)
}

pub(crate) fn build_static_page_queued_export_package_empty_module_counts(
) -> StaticPageQueuedExportPackageModuleCounts {
    let (module_count, echarts_requested_modules) =
        build_static_page_queued_export_package_empty_module_counts_fields();
    build_static_page_queued_export_package_module_counts_from_fields(
        module_count,
        echarts_requested_modules,
    )
}

pub(crate) fn build_static_page_queued_export_package_empty_module_counts_fields(
) -> StaticPageQueuedExportPackageModuleCounts {
    (0, 0)
}

pub(crate) fn build_static_page_queued_export_package_module_counts_from_fields(
    module_count: StaticPageQueuedExportPackageModuleCount,
    echarts_requested_modules: StaticPageQueuedExportPackageModuleCount,
) -> StaticPageQueuedExportPackageModuleCounts {
    (module_count, echarts_requested_modules)
}

pub(crate) fn build_static_page_queued_export_package_module_count(
    items: &[Value],
) -> StaticPageQueuedExportPackageModuleCount {
    build_static_page_queued_export_package_module_count_fields(items)
}

pub(crate) fn build_static_page_queued_export_package_module_count_fields(
    items: &[Value],
) -> StaticPageQueuedExportPackageModuleCount {
    items.len()
}

pub(crate) fn build_static_page_queued_export_package_echarts_requested_module_count(
    items: &[Value],
) -> StaticPageQueuedExportPackageModuleCount {
    build_static_page_queued_export_package_echarts_requested_module_count_fields(items)
}

pub(crate) fn build_static_page_queued_export_package_echarts_requested_module_count_fields(
    items: &[Value],
) -> StaticPageQueuedExportPackageModuleCount {
    items
        .iter()
        .filter(|module| build_static_page_queued_export_package_module_requests_echarts(module))
        .count()
}

pub(crate) fn build_static_page_queued_export_package_module_requests_echarts(
    module: &StaticPageQueuedExportPackageModuleValue,
) -> bool {
    build_static_page_queued_export_package_module_requests_echarts_fields(module)
}

pub(crate) fn build_static_page_queued_export_package_module_requests_echarts_fields(
    module: &StaticPageQueuedExportPackageModuleValue,
) -> bool {
    static_page_module_chart_runtime(module) == "echarts"
}

pub(crate) fn build_static_page_queued_export_package_debug(
    module_count: StaticPageQueuedExportPackageModuleCount,
    echarts_requested_modules: StaticPageQueuedExportPackageModuleCount,
    data_snapshot: &StaticPageRenderQueueDataSnapshotValue,
) -> StaticPageQueuedExportPackageDebugValue {
    let (renderer, module_count, echarts_requested_modules, data_snapshot_source) =
        build_static_page_queued_export_package_debug_fields(
            module_count,
            echarts_requested_modules,
            data_snapshot,
        );
    build_static_page_queued_export_package_debug_from_fields(
        renderer,
        module_count,
        echarts_requested_modules,
        data_snapshot_source,
    )
}

pub(crate) fn build_static_page_queued_export_package_debug_fields<'a>(
    module_count: StaticPageQueuedExportPackageModuleCount,
    echarts_requested_modules: StaticPageQueuedExportPackageModuleCount,
    data_snapshot: &'a StaticPageRenderQueueDataSnapshotValue,
) -> StaticPageQueuedExportPackageDebugFields<'a> {
    build_static_page_queued_export_package_debug_fields_from_fields(
        build_static_page_queued_export_package_debug_renderer(),
        module_count,
        echarts_requested_modules,
        build_static_page_queued_export_package_data_snapshot_source(data_snapshot),
    )
}

pub(crate) fn build_static_page_queued_export_package_debug_fields_from_fields<'a>(
    renderer: StaticPageQueuedExportPackageDebugRenderer,
    module_count: StaticPageQueuedExportPackageModuleCount,
    echarts_requested_modules: StaticPageQueuedExportPackageModuleCount,
    data_snapshot_source: StaticPageQueuedExportPackageDataSnapshotSource<'a>,
) -> StaticPageQueuedExportPackageDebugFields<'a> {
    (
        renderer,
        module_count,
        echarts_requested_modules,
        data_snapshot_source,
    )
}

pub(crate) fn build_static_page_queued_export_package_debug_from_fields(
    renderer: StaticPageQueuedExportPackageDebugRenderer,
    module_count: StaticPageQueuedExportPackageModuleCount,
    echarts_requested_modules: StaticPageQueuedExportPackageModuleCount,
    data_snapshot_source: StaticPageQueuedExportPackageDataSnapshotSource<'_>,
) -> StaticPageQueuedExportPackageDebugValue {
    build_static_page_queued_export_package_debug_payload_fields(
        renderer,
        module_count,
        echarts_requested_modules,
        data_snapshot_source,
    )
}

pub(crate) fn build_static_page_queued_export_package_debug_payload_fields(
    renderer: StaticPageQueuedExportPackageDebugRenderer,
    module_count: StaticPageQueuedExportPackageModuleCount,
    echarts_requested_modules: StaticPageQueuedExportPackageModuleCount,
    data_snapshot_source: StaticPageQueuedExportPackageDataSnapshotSource<'_>,
) -> StaticPageQueuedExportPackageDebugValue {
    json!({
        "renderer": renderer,
        "module_count": module_count,
        "echarts_requested_modules": echarts_requested_modules,
        "data_snapshot_source": data_snapshot_source
    })
}

pub(crate) fn build_static_page_queued_export_package_debug_renderer(
) -> StaticPageQueuedExportPackageDebugRenderer {
    build_static_page_queued_export_package_debug_renderer_fields()
}

pub(crate) fn build_static_page_queued_export_package_debug_renderer_fields(
) -> StaticPageQueuedExportPackageDebugRenderer {
    build_static_page_render_queue_renderer()
}

pub(crate) fn build_static_page_queued_export_package_data_snapshot_source(
    data_snapshot: &StaticPageRenderQueueDataSnapshotValue,
) -> StaticPageQueuedExportPackageDataSnapshotSource<'_> {
    build_static_page_queued_export_package_data_snapshot_source_from_optional(
        build_static_page_queued_export_package_data_snapshot_source_fields(data_snapshot),
    )
}

pub(crate) fn build_static_page_queued_export_package_data_snapshot_source_fields(
    data_snapshot: &StaticPageRenderQueueDataSnapshotValue,
) -> Option<StaticPageQueuedExportPackageDataSnapshotSource<'_>> {
    build_static_page_queued_export_package_data_snapshot_source_value(data_snapshot)
}

pub(crate) fn build_static_page_queued_export_package_data_snapshot_source_value(
    data_snapshot: &StaticPageRenderQueueDataSnapshotValue,
) -> Option<StaticPageQueuedExportPackageDataSnapshotSource<'_>> {
    build_static_page_queued_export_package_data_snapshot_source_value_fields(data_snapshot)
}

pub(crate) fn build_static_page_queued_export_package_data_snapshot_source_value_fields(
    data_snapshot: &StaticPageRenderQueueDataSnapshotValue,
) -> Option<StaticPageQueuedExportPackageDataSnapshotSource<'_>> {
    data_snapshot.get("source").and_then(Value::as_str)
}

pub(crate) fn build_static_page_queued_export_package_data_snapshot_source_from_optional<'a>(
    source: Option<StaticPageQueuedExportPackageDataSnapshotSource<'a>>,
) -> StaticPageQueuedExportPackageDataSnapshotSource<'a> {
    build_static_page_queued_export_package_data_snapshot_source_from_optional_fields(source)
}

pub(crate) fn build_static_page_queued_export_package_data_snapshot_source_from_optional_fields<
    'a,
>(
    source: Option<StaticPageQueuedExportPackageDataSnapshotSource<'a>>,
) -> StaticPageQueuedExportPackageDataSnapshotSource<'a> {
    source.unwrap_or_else(|| build_static_page_queued_export_package_unknown_data_snapshot_source())
}

pub(crate) fn build_static_page_queued_export_package_unknown_data_snapshot_source(
) -> StaticPageQueuedExportPackageDataSnapshotSource<'static> {
    build_static_page_queued_export_package_unknown_data_snapshot_source_fields()
}

pub(crate) fn build_static_page_queued_export_package_unknown_data_snapshot_source_fields(
) -> StaticPageQueuedExportPackageDataSnapshotSource<'static> {
    "unknown"
}

pub(crate) fn build_static_page_queued_export_package_files(
) -> StaticPageQueuedExportPackageFilesValue {
    build_static_page_queued_export_package_files_from_specs(
        build_static_page_queued_export_package_files_fields(),
    )
}

pub(crate) fn build_static_page_queued_export_package_files_fields(
) -> &'static [StaticPageQueuedExportPackageFileSpec] {
    build_static_page_queued_export_package_file_specs()
}

pub(crate) fn build_static_page_queued_export_package_files_from_specs(
    specs: &[StaticPageQueuedExportPackageFileSpec],
) -> StaticPageQueuedExportPackageFilesValue {
    build_static_page_queued_export_package_files_from_entries(
        build_static_page_queued_export_package_files_from_specs_fields(specs),
    )
}

pub(crate) fn build_static_page_queued_export_package_files_from_specs_fields(
    specs: &[StaticPageQueuedExportPackageFileSpec],
) -> StaticPageQueuedExportPackageFileEntriesValue {
    build_static_page_queued_export_package_file_entries_from_specs(specs)
}

pub(crate) fn build_static_page_queued_export_package_files_from_entries(
    entries: StaticPageQueuedExportPackageFileEntriesValue,
) -> StaticPageQueuedExportPackageFilesValue {
    Value::Array(build_static_page_queued_export_package_files_from_entries_fields(entries))
}

pub(crate) fn build_static_page_queued_export_package_files_from_entries_fields(
    entries: StaticPageQueuedExportPackageFileEntriesValue,
) -> StaticPageQueuedExportPackageFileEntriesValue {
    entries
}

pub(crate) fn build_static_page_queued_export_package_file_entries_from_specs(
    specs: &[StaticPageQueuedExportPackageFileSpec],
) -> StaticPageQueuedExportPackageFileEntriesValue {
    build_static_page_queued_export_package_file_entries_from_specs_fields(specs)
}

pub(crate) fn build_static_page_queued_export_package_file_entries_from_specs_fields(
    specs: &[StaticPageQueuedExportPackageFileSpec],
) -> StaticPageQueuedExportPackageFileEntriesValue {
    build_static_page_queued_export_package_file_entries_from_specs_fields_entries(specs)
}

pub(crate) fn build_static_page_queued_export_package_file_entries_from_specs_fields_entries(
    specs: &[StaticPageQueuedExportPackageFileSpec],
) -> StaticPageQueuedExportPackageFileEntriesValue {
    specs
        .iter()
        .map(|&spec| build_static_page_queued_export_package_file_from_spec(spec))
        .collect()
}

pub(crate) fn build_static_page_queued_export_package_file_specs(
) -> &'static [StaticPageQueuedExportPackageFileSpec] {
    build_static_page_queued_export_package_file_specs_fields()
}

pub(crate) fn build_static_page_queued_export_package_file_specs_fields(
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
    path: StaticPageQueuedExportPackageFilePath,
    role: StaticPageQueuedExportPackageFileRole,
    mime: StaticPageQueuedExportPackageFileMime,
) -> StaticPageQueuedExportPackageFileSpec {
    build_static_page_queued_export_package_file_spec_fields(path, role, mime)
}

pub(crate) const fn build_static_page_queued_export_package_file_spec_fields(
    path: StaticPageQueuedExportPackageFilePath,
    role: StaticPageQueuedExportPackageFileRole,
    mime: StaticPageQueuedExportPackageFileMime,
) -> StaticPageQueuedExportPackageFileSpec {
    (path, role, mime)
}

pub(crate) fn build_static_page_queued_export_package_file_from_spec(
    spec: StaticPageQueuedExportPackageFileSpec,
) -> StaticPageQueuedExportPackageFileValue {
    let (path, role, mime) = build_static_page_queued_export_package_file_from_spec_fields(spec);
    build_static_page_queued_export_package_file(path, role, mime)
}

pub(crate) fn build_static_page_queued_export_package_file_from_spec_fields(
    spec: StaticPageQueuedExportPackageFileSpec,
) -> StaticPageQueuedExportPackageFileSpec {
    (
        build_static_page_queued_export_package_file_spec_path(spec),
        build_static_page_queued_export_package_file_spec_role(spec),
        build_static_page_queued_export_package_file_spec_mime(spec),
    )
}

pub(crate) const fn build_static_page_queued_export_package_file_spec_path(
    spec: StaticPageQueuedExportPackageFileSpec,
) -> StaticPageQueuedExportPackageFilePath {
    build_static_page_queued_export_package_file_spec_path_fields(spec)
}

pub(crate) const fn build_static_page_queued_export_package_file_spec_path_fields(
    spec: StaticPageQueuedExportPackageFileSpec,
) -> StaticPageQueuedExportPackageFilePath {
    spec.0
}

pub(crate) const fn build_static_page_queued_export_package_file_spec_role(
    spec: StaticPageQueuedExportPackageFileSpec,
) -> StaticPageQueuedExportPackageFileRole {
    build_static_page_queued_export_package_file_spec_role_fields(spec)
}

pub(crate) const fn build_static_page_queued_export_package_file_spec_role_fields(
    spec: StaticPageQueuedExportPackageFileSpec,
) -> StaticPageQueuedExportPackageFileRole {
    spec.1
}

pub(crate) const fn build_static_page_queued_export_package_file_spec_mime(
    spec: StaticPageQueuedExportPackageFileSpec,
) -> StaticPageQueuedExportPackageFileMime {
    build_static_page_queued_export_package_file_spec_mime_fields(spec)
}

pub(crate) const fn build_static_page_queued_export_package_file_spec_mime_fields(
    spec: StaticPageQueuedExportPackageFileSpec,
) -> StaticPageQueuedExportPackageFileMime {
    spec.2
}

pub(crate) const fn build_static_page_queued_export_package_rendered_page_role(
) -> StaticPageQueuedExportPackageFileRole {
    build_static_page_queued_export_package_rendered_page_role_fields()
}

pub(crate) const fn build_static_page_queued_export_package_rendered_page_role_fields(
) -> StaticPageQueuedExportPackageFileRole {
    "rendered_static_page"
}

pub(crate) const fn build_static_page_queued_export_package_renderer_manifest_role(
) -> StaticPageQueuedExportPackageFileRole {
    build_static_page_queued_export_package_renderer_manifest_role_fields()
}

pub(crate) const fn build_static_page_queued_export_package_renderer_manifest_role_fields(
) -> StaticPageQueuedExportPackageFileRole {
    "renderer_manifest"
}

pub(crate) const fn build_static_page_queued_export_package_render_data_snapshot_role(
) -> StaticPageQueuedExportPackageFileRole {
    build_static_page_queued_export_package_render_data_snapshot_role_fields()
}

pub(crate) const fn build_static_page_queued_export_package_render_data_snapshot_role_fields(
) -> StaticPageQueuedExportPackageFileRole {
    "render_data_snapshot"
}

pub(crate) const fn build_static_page_queued_export_package_dynamic_data_snapshot_role(
) -> StaticPageQueuedExportPackageFileRole {
    build_static_page_queued_export_package_dynamic_data_snapshot_role_fields()
}

pub(crate) const fn build_static_page_queued_export_package_dynamic_data_snapshot_role_fields(
) -> StaticPageQueuedExportPackageFileRole {
    "dynamic_data_snapshot"
}

pub(crate) const fn build_static_page_queued_export_package_editable_module_plan_role(
) -> StaticPageQueuedExportPackageFileRole {
    build_static_page_queued_export_package_editable_module_plan_role_fields()
}

pub(crate) const fn build_static_page_queued_export_package_editable_module_plan_role_fields(
) -> StaticPageQueuedExportPackageFileRole {
    "editable_module_plan"
}

pub(crate) const fn build_static_page_queued_export_package_index_path(
) -> StaticPageQueuedExportPackageFilePath {
    build_static_page_queued_export_package_index_path_fields()
}

pub(crate) const fn build_static_page_queued_export_package_index_path_fields(
) -> StaticPageQueuedExportPackageFilePath {
    "index.html"
}

pub(crate) const fn build_static_page_queued_export_package_asset_manifest_path(
) -> StaticPageQueuedExportPackageFilePath {
    build_static_page_queued_export_package_asset_manifest_path_fields()
}

pub(crate) const fn build_static_page_queued_export_package_asset_manifest_path_fields(
) -> StaticPageQueuedExportPackageFilePath {
    "asset-manifest.json"
}

pub(crate) const fn build_static_page_queued_export_package_data_snapshot_path(
) -> StaticPageQueuedExportPackageFilePath {
    build_static_page_queued_export_package_data_snapshot_path_fields()
}

pub(crate) const fn build_static_page_queued_export_package_data_snapshot_path_fields(
) -> StaticPageQueuedExportPackageFilePath {
    "data-snapshot.json"
}

pub(crate) const fn build_static_page_queued_export_package_dynamic_data_path(
) -> StaticPageQueuedExportPackageFilePath {
    build_static_page_queued_export_package_dynamic_data_path_fields()
}

pub(crate) const fn build_static_page_queued_export_package_dynamic_data_path_fields(
) -> StaticPageQueuedExportPackageFilePath {
    "data.json"
}

pub(crate) const fn build_static_page_queued_export_package_modules_path(
) -> StaticPageQueuedExportPackageFilePath {
    build_static_page_queued_export_package_modules_path_fields()
}

pub(crate) const fn build_static_page_queued_export_package_modules_path_fields(
) -> StaticPageQueuedExportPackageFilePath {
    "modules.json"
}

pub(crate) const fn build_static_page_queued_export_package_html_mime(
) -> StaticPageQueuedExportPackageFileMime {
    build_static_page_queued_export_package_html_mime_fields()
}

pub(crate) const fn build_static_page_queued_export_package_html_mime_fields(
) -> StaticPageQueuedExportPackageFileMime {
    "text/html"
}

pub(crate) const fn build_static_page_queued_export_package_json_mime(
) -> StaticPageQueuedExportPackageFileMime {
    build_static_page_queued_export_package_json_mime_fields()
}

pub(crate) const fn build_static_page_queued_export_package_json_mime_fields(
) -> StaticPageQueuedExportPackageFileMime {
    "application/json"
}

pub(crate) fn build_static_page_queued_export_package_file(
    path: StaticPageQueuedExportPackageFilePath,
    role: StaticPageQueuedExportPackageFileRole,
    mime: StaticPageQueuedExportPackageFileMime,
) -> StaticPageQueuedExportPackageFileValue {
    let (path, role, mime) = build_static_page_queued_export_package_file_fields(path, role, mime);
    build_static_page_queued_export_package_file_from_fields(path, role, mime)
}

pub(crate) const fn build_static_page_queued_export_package_file_fields(
    path: StaticPageQueuedExportPackageFilePath,
    role: StaticPageQueuedExportPackageFileRole,
    mime: StaticPageQueuedExportPackageFileMime,
) -> StaticPageQueuedExportPackageFileSpec {
    (path, role, mime)
}

pub(crate) fn build_static_page_queued_export_package_file_from_fields(
    path: StaticPageQueuedExportPackageFilePath,
    role: StaticPageQueuedExportPackageFileRole,
    mime: StaticPageQueuedExportPackageFileMime,
) -> StaticPageQueuedExportPackageFileValue {
    build_static_page_queued_export_package_file_payload_fields(path, role, mime)
}

pub(crate) fn build_static_page_queued_export_package_file_payload_fields(
    path: StaticPageQueuedExportPackageFilePath,
    role: StaticPageQueuedExportPackageFileRole,
    mime: StaticPageQueuedExportPackageFileMime,
) -> StaticPageQueuedExportPackageFileValue {
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
    fn render_queue_manifest_fields_preserve_existing_specs_and_job_refs() {
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

        let fields: StaticPageRenderQueueManifestFields =
            build_static_page_render_queue_manifest_fields(
                &draft,
                Some(&image_job),
                Some(&execution),
                Some(task_id),
            );
        let (
            draft_id,
            assistant_run_id,
            status,
            renderer,
            workflow_execution_id,
            workflow_task_id,
            workflow_manifest,
            image_job_id,
            preview_asset_key,
            visual_spec,
            render_spec,
            data_snapshot,
            export_package,
            queue_copy,
        ) = fields;

        assert_eq!(draft_id, draft.id);
        assert_eq!(assistant_run_id, draft.assistant_run_id);
        assert_eq!(status, "queued");
        assert_eq!(renderer, "static-page-renderer-v1");
        assert_eq!(workflow_execution_id, Some(execution.id));
        assert_eq!(workflow_task_id, Some(task_id));
        assert_eq!(workflow_manifest["status"], json!("queued"));
        assert_eq!(image_job_id, Some(image_job.id));
        assert_eq!(
            preview_asset_key,
            Some("static-page-previews/preview.png".to_string())
        );
        assert_eq!(visual_spec, json!({"theme": "dark"}));
        assert_eq!(render_spec, json!({"runtime": "safe-echarts"}));
        assert_eq!(data_snapshot, json!({"source": "provided"}));
        assert_eq!(export_package["debug"]["module_count"], json!(2));
        assert_eq!(
            export_package["debug"]["echarts_requested_modules"],
            json!(1)
        );
        assert_eq!(
            export_package["debug"]["data_snapshot_source"],
            json!("provided")
        );
        assert_eq!(
            queue_copy,
            "最终静态页正在后台制作，可以继续聊天或修改其他内容。"
        );
    }

    #[test]
    fn render_queue_manifest_from_fields_preserves_object_shape() {
        let draft_id = StaticPageDraftId::new();
        let assistant_run_id = AssistantRunId::new();
        let workflow_execution_id = WorkflowExecutionId::new();
        let workflow_task_id = WorkflowTaskId::new();
        let image_job_id = StaticPageImageJobId::new();
        let status: StaticPageRenderQueueLifecycleScalar = "queued";
        let renderer: StaticPageRenderQueueLifecycleScalar = "static-page-renderer-v1";
        let workflow_manifest: StaticPageRenderQueueWorkflowManifestValue = json!({
            "status": "queued",
            "executionId": workflow_execution_id,
            "taskId": workflow_task_id
        });
        let visual_spec: StaticPageRenderQueueVisualSpecValue = json!({"theme": "dark"});
        let render_spec: StaticPageRenderQueueRenderSpecValue = json!({"runtime": "safe-echarts"});
        let data_snapshot: StaticPageRenderQueueDataSnapshotValue = json!({"source": "provided"});
        let export_package: StaticPageRenderQueueExportPackageValue = json!({
            "kind": "static-page-export-package",
            "version": 1,
            "status": "queued"
        });
        let queue_copy: StaticPageRenderQueueLifecycleScalar =
            "最终静态页正在后台制作，可以继续聊天或修改其他内容。";

        let manifest: StaticPageRenderQueueManifestValue =
            build_static_page_render_queue_manifest_from_fields(
                draft_id,
                assistant_run_id,
                status,
                renderer,
                Some(workflow_execution_id),
                Some(workflow_task_id),
                workflow_manifest.clone(),
                Some(image_job_id),
                Some("static-page-previews/preview.png".to_string()),
                visual_spec.clone(),
                render_spec.clone(),
                data_snapshot.clone(),
                export_package.clone(),
                queue_copy,
            );

        assert_eq!(manifest["draft_id"], json!(draft_id));
        assert_eq!(manifest["assistant_run_id"], json!(assistant_run_id));
        assert_eq!(manifest["status"], json!("queued"));
        assert_eq!(manifest["renderer"], json!("static-page-renderer-v1"));
        assert_eq!(
            manifest["workflow_execution_id"],
            json!(workflow_execution_id)
        );
        assert_eq!(manifest["workflow_task_id"], json!(workflow_task_id));
        assert_eq!(manifest["workflow"], workflow_manifest);
        assert_eq!(manifest["image_job_id"], json!(image_job_id));
        assert_eq!(
            manifest["preview_asset_key"],
            json!("static-page-previews/preview.png")
        );
        assert_eq!(manifest["visual_spec"], visual_spec);
        assert_eq!(manifest["render_spec"], render_spec);
        assert_eq!(manifest["data_snapshot"], data_snapshot);
        assert_eq!(manifest["export_package"], export_package);
        assert_eq!(
            manifest["queue_copy"],
            json!("最终静态页正在后台制作，可以继续聊天或修改其他内容。")
        );
    }

    #[test]
    fn render_queue_manifest_payload_fields_preserve_object_shape() {
        let draft_id = StaticPageDraftId::new();
        let assistant_run_id = AssistantRunId::new();
        let workflow_execution_id = WorkflowExecutionId::new();
        let workflow_task_id = WorkflowTaskId::new();
        let image_job_id = StaticPageImageJobId::new();
        let status: StaticPageRenderQueueLifecycleScalar = "queued";
        let renderer: StaticPageRenderQueueLifecycleScalar = "static-page-renderer-v1";
        let workflow_manifest: StaticPageRenderQueueWorkflowManifestValue = json!({
            "status": "queued",
            "executionId": workflow_execution_id,
            "taskId": workflow_task_id
        });
        let visual_spec: StaticPageRenderQueueVisualSpecValue = json!({"theme": "dark"});
        let render_spec: StaticPageRenderQueueRenderSpecValue = json!({"runtime": "safe-echarts"});
        let data_snapshot: StaticPageRenderQueueDataSnapshotValue = json!({"source": "provided"});
        let export_package: StaticPageRenderQueueExportPackageValue = json!({
            "kind": "static-page-export-package",
            "version": 1,
            "status": "queued"
        });
        let queue_copy: StaticPageRenderQueueLifecycleScalar =
            "最终静态页正在后台制作，可以继续聊天或修改其他内容。";

        let manifest: StaticPageRenderQueueManifestValue =
            build_static_page_render_queue_manifest_payload_fields(
                draft_id,
                assistant_run_id,
                status,
                renderer,
                Some(workflow_execution_id),
                Some(workflow_task_id),
                workflow_manifest.clone(),
                Some(image_job_id),
                Some("static-page-previews/preview.png".to_string()),
                visual_spec.clone(),
                render_spec.clone(),
                data_snapshot.clone(),
                export_package.clone(),
                queue_copy,
            );

        assert_eq!(manifest["draft_id"], json!(draft_id));
        assert_eq!(manifest["assistant_run_id"], json!(assistant_run_id));
        assert_eq!(manifest["status"], json!("queued"));
        assert_eq!(manifest["renderer"], json!("static-page-renderer-v1"));
        assert_eq!(
            manifest["workflow_execution_id"],
            json!(workflow_execution_id)
        );
        assert_eq!(manifest["workflow_task_id"], json!(workflow_task_id));
        assert_eq!(manifest["workflow"], workflow_manifest);
        assert_eq!(manifest["image_job_id"], json!(image_job_id));
        assert_eq!(
            manifest["preview_asset_key"],
            json!("static-page-previews/preview.png")
        );
        assert_eq!(manifest["visual_spec"], visual_spec);
        assert_eq!(manifest["render_spec"], render_spec);
        assert_eq!(manifest["data_snapshot"], data_snapshot);
        assert_eq!(manifest["export_package"], export_package);
        assert_eq!(
            manifest["queue_copy"],
            json!("最终静态页正在后台制作，可以继续聊天或修改其他内容。")
        );
    }

    #[test]
    fn render_queue_specs_preserve_visual_and_render_specs() {
        let explicit_payload = json!({
            "visualSpec": {"theme": "dark"},
            "renderSpec": {"runtime": "safe-echarts"}
        });
        let fallback_payload = json!({});

        let fields: StaticPageRenderQueueSpecsFields =
            build_static_page_render_queue_specs(&explicit_payload);
        let (visual_spec, render_spec) = fields;

        assert_eq!(visual_spec, json!({"theme": "dark"}));
        assert_eq!(render_spec, json!({"runtime": "safe-echarts"}));

        let fallback_fields: StaticPageRenderQueueSpecsFields =
            build_static_page_render_queue_specs(&fallback_payload);
        let (fallback_visual_spec, fallback_render_spec) = fallback_fields;

        assert_eq!(
            fallback_visual_spec["styleDirection"],
            json!("client-delivery")
        );
        assert_eq!(
            fallback_render_spec["renderer"],
            json!("static-page-renderer-v1")
        );
    }

    #[test]
    fn render_queue_specs_from_payload_fields_preserve_visual_and_render_specs() {
        let explicit_payload = json!({
            "visualSpec": {"theme": "dark"},
            "renderSpec": {"runtime": "safe-echarts"}
        });
        let fallback_payload = json!({});

        let fields: StaticPageRenderQueueSpecsFields =
            build_static_page_render_queue_specs_from_payload_fields(&explicit_payload);
        let (visual_spec, render_spec) = fields;

        assert_eq!(visual_spec, json!({"theme": "dark"}));
        assert_eq!(render_spec, json!({"runtime": "safe-echarts"}));

        let fallback_fields: StaticPageRenderQueueSpecsFields =
            build_static_page_render_queue_specs_from_payload_fields(&fallback_payload);
        let (fallback_visual_spec, fallback_render_spec) = fallback_fields;

        assert_eq!(
            fallback_visual_spec["styleDirection"],
            json!("client-delivery")
        );
        assert_eq!(
            fallback_render_spec["renderer"],
            json!("static-page-renderer-v1")
        );
    }

    #[test]
    fn render_queue_specs_from_fields_preserve_visual_and_render_specs() {
        let visual_spec: StaticPageRenderQueueVisualSpecValue = json!({"theme": "dark"});
        let render_spec: StaticPageRenderQueueRenderSpecValue = json!({"runtime": "safe-echarts"});

        let fields: StaticPageRenderQueueSpecsFields =
            build_static_page_render_queue_specs_from_fields(
                visual_spec.clone(),
                render_spec.clone(),
            );
        let (resolved_visual_spec, resolved_render_spec) = fields;

        assert_eq!(resolved_visual_spec, visual_spec);
        assert_eq!(resolved_render_spec, render_spec);
    }

    #[test]
    fn render_queue_data_context_preserves_snapshot_and_export_package() {
        let draft = draft_with_payload(json!({
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

        let fields: StaticPageRenderQueueDataContextFields =
            build_static_page_render_queue_data_context(&draft);
        let (data_snapshot, export_package): (
            StaticPageRenderQueueDataSnapshotValue,
            StaticPageRenderQueueExportPackageValue,
        ) = fields;

        assert_eq!(data_snapshot["source"], json!("provided"));
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
    fn render_queue_data_context_from_fields_preserves_snapshot_and_export_package() {
        let data_snapshot: StaticPageRenderQueueDataSnapshotValue = json!({"source": "provided"});
        let export_package: StaticPageRenderQueueExportPackageValue = json!({
            "kind": "static-page-export-package",
            "status": "queued"
        });

        let fields: StaticPageRenderQueueDataContextFields =
            build_static_page_render_queue_data_context_from_fields(
                data_snapshot.clone(),
                export_package.clone(),
            );
        let (resolved_data_snapshot, resolved_export_package) = fields;

        assert_eq!(resolved_data_snapshot, data_snapshot);
        assert_eq!(resolved_export_package, export_package);
    }

    #[test]
    fn render_queue_data_context_fields_preserve_snapshot_and_export_package() {
        let draft = draft_with_payload(json!({
            "dataSnapshot": {"source": "provided"},
            "modules": [
                {
                    "id": "trend",
                    "visualization": {
                        "chartRuntime": "echarts"
                    }
                },
                {"id": "summary"}
            ]
        }));

        let fields: StaticPageRenderQueueDataContextFields =
            build_static_page_render_queue_data_context_fields(&draft);
        let (data_snapshot, export_package) = fields;

        assert_eq!(data_snapshot["source"], json!("provided"));
        assert_eq!(export_package["kind"], json!("static-page-export-package"));
        assert_eq!(export_package["draft_id"], json!(draft.id));
        assert_eq!(export_package["debug"]["module_count"], json!(2));
        assert_eq!(
            export_package["debug"]["echarts_requested_modules"],
            json!(1)
        );
    }

    #[test]
    fn render_queue_data_context_fields_from_payload_preserve_snapshot_and_export_package() {
        let draft = draft_with_payload(json!({}));
        let payload = json!({
            "dataSnapshot": {"source": "provided"},
            "modules": [
                {
                    "id": "trend",
                    "visualization": {
                        "chartRuntime": "echarts"
                    }
                },
                {"id": "summary"}
            ]
        });

        let fields: StaticPageRenderQueueDataContextFields =
            build_static_page_render_queue_data_context_fields_from_payload(&draft, &payload);
        let (data_snapshot, export_package) = fields;

        assert_eq!(data_snapshot["source"], json!("provided"));
        assert_eq!(export_package["kind"], json!("static-page-export-package"));
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
    fn render_queue_data_context_export_package_preserves_modules_and_debug_counts() {
        let draft = draft_with_payload(json!({}));
        let payload = json!({
            "modules": [
                {
                    "id": "trend",
                    "visualization": {
                        "chartRuntime": "echarts"
                    }
                },
                {"id": "summary"}
            ]
        });
        let data_snapshot = json!({"source": "provided"});

        let export_package = build_static_page_render_queue_data_context_export_package(
            &draft,
            &payload,
            &data_snapshot,
        );

        assert_eq!(export_package["kind"], json!("static-page-export-package"));
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
    fn render_queue_data_context_export_package_fields_preserve_modules_and_fallback() {
        let payload = json!({
            "modules": [
                {"id": "trend"},
                {"id": "summary"}
            ]
        });

        let modules = build_static_page_render_queue_data_context_export_package_fields(&payload);
        let fallback_modules =
            build_static_page_render_queue_data_context_export_package_fields(&json!({
                "modules": "invalid"
            }));

        assert_eq!(modules.as_array().map(Vec::len), Some(2));
        assert_eq!(modules[0]["id"], json!("trend"));
        assert_eq!(fallback_modules.as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn render_queue_data_context_export_package_from_fields_preserves_debug_counts() {
        let draft = draft_with_payload(json!({}));
        let modules = json!([
            {
                "id": "trend",
                "visualization": {
                    "chartRuntime": "echarts"
                }
            },
            {"id": "summary"}
        ]);
        let data_snapshot = json!({"source": "provided"});

        let export_package = build_static_page_render_queue_data_context_export_package_from_fields(
            &draft,
            &modules,
            &data_snapshot,
        );

        assert_eq!(export_package["kind"], json!("static-page-export-package"));
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
    fn render_queue_identity_preserves_draft_and_run_ids() {
        let draft = draft_with_payload(json!({}));

        let fields: StaticPageRenderQueueIdentityFields =
            build_static_page_render_queue_identity(&draft);
        let (draft_id, assistant_run_id) = fields;

        assert_eq!(draft_id, draft.id);
        assert_eq!(assistant_run_id, draft.assistant_run_id);
    }

    #[test]
    fn render_queue_identity_fields_preserve_draft_and_run_ids() {
        let draft = draft_with_payload(json!({}));

        let fields: StaticPageRenderQueueIdentityFields =
            build_static_page_render_queue_identity_fields(&draft);
        let (draft_id, assistant_run_id) = fields;

        assert_eq!(draft_id, draft.id);
        assert_eq!(assistant_run_id, draft.assistant_run_id);
    }

    #[test]
    fn render_queue_identity_from_fields_preserves_ids() {
        let draft_id = StaticPageDraftId::new();
        let assistant_run_id = AssistantRunId::new();

        let fields: StaticPageRenderQueueIdentityFields =
            build_static_page_render_queue_identity_from_fields(draft_id, assistant_run_id);
        let (resolved_draft_id, resolved_assistant_run_id) = fields;

        assert_eq!(resolved_draft_id, draft_id);
        assert_eq!(resolved_assistant_run_id, assistant_run_id);
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
        let fields: StaticPageRenderQueueLifecycleFields =
            build_static_page_render_queue_lifecycle();
        let (status, renderer, queue_copy) = fields;

        assert_eq!(status, "queued");
        assert_eq!(renderer, "static-page-renderer-v1");
        assert_eq!(
            queue_copy,
            "最终静态页正在后台制作，可以继续聊天或修改其他内容。"
        );
    }

    #[test]
    fn render_queue_lifecycle_fields_preserve_status_renderer_and_copy() {
        let fields: StaticPageRenderQueueLifecycleFields =
            build_static_page_render_queue_lifecycle_fields();
        let (status, renderer, queue_copy) = fields;

        assert_eq!(status, "queued");
        assert_eq!(renderer, "static-page-renderer-v1");
        assert_eq!(
            queue_copy,
            "最终静态页正在后台制作，可以继续聊天或修改其他内容。"
        );
    }

    #[test]
    fn render_queue_lifecycle_from_fields_preserves_fixed_values() {
        let fields: StaticPageRenderQueueLifecycleFields =
            build_static_page_render_queue_lifecycle_from_fields(
                "queued",
                "static-page-renderer-v1",
                "最终静态页正在后台制作，可以继续聊天或修改其他内容。",
            );
        let (status, renderer, queue_copy) = fields;

        assert_eq!(status, "queued");
        assert_eq!(renderer, "static-page-renderer-v1");
        assert_eq!(
            queue_copy,
            "最终静态页正在后台制作，可以继续聊天或修改其他内容。"
        );
    }

    #[test]
    fn render_queue_lifecycle_field_helpers_preserve_fixed_values() {
        let status: StaticPageRenderQueueLifecycleScalar = build_static_page_render_queue_status();
        let renderer: StaticPageRenderQueueLifecycleScalar =
            build_static_page_render_queue_renderer();
        let queue_copy: StaticPageRenderQueueLifecycleScalar =
            build_static_page_render_queue_queue_copy();

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
    fn render_queue_workflow_manifest_from_fields_preserves_object_shape() {
        let status: StaticPageRenderQueueLifecycleScalar = "queued";
        let execution_id = WorkflowExecutionId::new();
        let task_id = WorkflowTaskId::new();

        let workflow: StaticPageRenderQueueWorkflowManifestValue =
            build_static_page_render_queue_workflow_manifest_from_fields(
                status,
                Some(execution_id),
                Some(task_id),
            );
        let empty_workflow: StaticPageRenderQueueWorkflowManifestValue =
            build_static_page_render_queue_workflow_manifest_from_fields(status, None, None);

        assert_eq!(workflow["status"], json!("queued"));
        assert_eq!(workflow["executionId"], json!(execution_id));
        assert_eq!(workflow["taskId"], json!(task_id));
        assert_eq!(empty_workflow["status"], json!("queued"));
        assert_eq!(empty_workflow["executionId"], Value::Null);
        assert_eq!(empty_workflow["taskId"], Value::Null);
    }

    #[test]
    fn render_queue_workflow_manifest_payload_fields_preserve_object_shape() {
        let status: StaticPageRenderQueueLifecycleScalar = "queued";
        let execution_id = WorkflowExecutionId::new();
        let task_id = WorkflowTaskId::new();

        let workflow: StaticPageRenderQueueWorkflowManifestValue =
            build_static_page_render_queue_workflow_manifest_payload_fields(
                status,
                Some(execution_id),
                Some(task_id),
            );
        let empty_workflow: StaticPageRenderQueueWorkflowManifestValue =
            build_static_page_render_queue_workflow_manifest_payload_fields(status, None, None);

        assert_eq!(workflow["status"], json!("queued"));
        assert_eq!(workflow["executionId"], json!(execution_id));
        assert_eq!(workflow["taskId"], json!(task_id));
        assert_eq!(empty_workflow["status"], json!("queued"));
        assert_eq!(empty_workflow["executionId"], Value::Null);
        assert_eq!(empty_workflow["taskId"], Value::Null);
    }

    #[test]
    fn render_queue_workflow_manifest_fields_preserve_status_execution_and_task() {
        let execution = workflow_execution();
        let task_id = WorkflowTaskId::new();

        let fields: StaticPageRenderQueueWorkflowManifestFields =
            build_static_page_render_queue_workflow_manifest_fields(
                Some(&execution),
                Some(task_id),
            );
        let (status, execution_id, resolved_task_id) = fields;
        let empty_fields: StaticPageRenderQueueWorkflowManifestFields =
            build_static_page_render_queue_workflow_manifest_fields(None, None);
        let (empty_status, empty_execution_id, empty_task_id) = empty_fields;

        assert_eq!(status, "queued");
        assert_eq!(execution_id, Some(execution.id));
        assert_eq!(resolved_task_id, Some(task_id));
        assert_eq!(empty_status, "queued");
        assert_eq!(empty_execution_id, None);
        assert_eq!(empty_task_id, None);
    }

    #[test]
    fn render_queue_workflow_manifest_fields_from_fields_preserve_values() {
        let execution_id = WorkflowExecutionId::new();
        let task_id = WorkflowTaskId::new();

        let fields: StaticPageRenderQueueWorkflowManifestFields =
            build_static_page_render_queue_workflow_manifest_fields_from_fields(
                "queued",
                Some(execution_id),
                Some(task_id),
            );
        let (status, resolved_execution_id, resolved_task_id) = fields;
        let empty_fields: StaticPageRenderQueueWorkflowManifestFields =
            build_static_page_render_queue_workflow_manifest_fields_from_fields(
                "queued", None, None,
            );
        let (empty_status, empty_execution_id, empty_task_id) = empty_fields;

        assert_eq!(status, "queued");
        assert_eq!(resolved_execution_id, Some(execution_id));
        assert_eq!(resolved_task_id, Some(task_id));
        assert_eq!(empty_status, "queued");
        assert_eq!(empty_execution_id, None);
        assert_eq!(empty_task_id, None);
    }

    #[test]
    fn render_queue_workflow_context_preserves_manifest_and_top_level_ids() {
        let execution = workflow_execution();
        let task_id = WorkflowTaskId::new();

        let fields: StaticPageRenderQueueWorkflowContextFields =
            build_static_page_render_queue_workflow_context(Some(&execution), Some(task_id));
        let (workflow, workflow_execution_id, workflow_task_id) = fields;
        let empty_fields: StaticPageRenderQueueWorkflowContextFields =
            build_static_page_render_queue_workflow_context(None, None);
        let (empty_workflow, empty_workflow_execution_id, empty_workflow_task_id) = empty_fields;

        assert_eq!(workflow["status"], json!("queued"));
        assert_eq!(workflow["executionId"], json!(execution.id));
        assert_eq!(workflow["taskId"], json!(task_id));
        assert_eq!(workflow_execution_id, Some(execution.id));
        assert_eq!(workflow_task_id, Some(task_id));
        assert_eq!(empty_workflow["status"], json!("queued"));
        assert_eq!(empty_workflow["executionId"], Value::Null);
        assert_eq!(empty_workflow["taskId"], Value::Null);
        assert_eq!(empty_workflow_execution_id, None);
        assert_eq!(empty_workflow_task_id, None);
    }

    #[test]
    fn render_queue_workflow_context_fields_preserve_manifest_and_top_level_ids() {
        let execution = workflow_execution();
        let task_id = WorkflowTaskId::new();

        let fields: StaticPageRenderQueueWorkflowContextFields =
            build_static_page_render_queue_workflow_context_fields(Some(&execution), Some(task_id));
        let (workflow, workflow_execution_id, workflow_task_id) = fields;
        let empty_fields: StaticPageRenderQueueWorkflowContextFields =
            build_static_page_render_queue_workflow_context_fields(None, None);
        let (empty_workflow, empty_workflow_execution_id, empty_workflow_task_id) = empty_fields;

        assert_eq!(workflow["status"], json!("queued"));
        assert_eq!(workflow["executionId"], json!(execution.id));
        assert_eq!(workflow["taskId"], json!(task_id));
        assert_eq!(workflow_execution_id, Some(execution.id));
        assert_eq!(workflow_task_id, Some(task_id));
        assert_eq!(empty_workflow["status"], json!("queued"));
        assert_eq!(empty_workflow["executionId"], Value::Null);
        assert_eq!(empty_workflow["taskId"], Value::Null);
        assert_eq!(empty_workflow_execution_id, None);
        assert_eq!(empty_workflow_task_id, None);
    }

    #[test]
    fn render_queue_workflow_context_from_fields_preserves_values() {
        let execution_id = WorkflowExecutionId::new();
        let task_id = WorkflowTaskId::new();
        let workflow: StaticPageRenderQueueWorkflowManifestValue = json!({
            "status": "queued",
            "executionId": execution_id,
            "taskId": task_id
        });

        let fields: StaticPageRenderQueueWorkflowContextFields =
            build_static_page_render_queue_workflow_context_from_fields(
                workflow.clone(),
                Some(execution_id),
                Some(task_id),
            );
        let (resolved_workflow, workflow_execution_id, workflow_task_id) = fields;
        let empty_fields: StaticPageRenderQueueWorkflowContextFields =
            build_static_page_render_queue_workflow_context_from_fields(
                json!({
                    "status": "queued",
                    "executionId": Value::Null,
                    "taskId": Value::Null
                }),
                None,
                None,
            );
        let (empty_workflow, empty_workflow_execution_id, empty_workflow_task_id) = empty_fields;

        assert_eq!(resolved_workflow, workflow);
        assert_eq!(workflow_execution_id, Some(execution_id));
        assert_eq!(workflow_task_id, Some(task_id));
        assert_eq!(empty_workflow["status"], json!("queued"));
        assert_eq!(empty_workflow["executionId"], Value::Null);
        assert_eq!(empty_workflow["taskId"], Value::Null);
        assert_eq!(empty_workflow_execution_id, None);
        assert_eq!(empty_workflow_task_id, None);
    }

    #[test]
    fn render_queue_workflow_status_preserves_queued_value() {
        let status: StaticPageRenderQueueLifecycleScalar =
            build_static_page_render_queue_workflow_status();

        assert_eq!(status, "queued");
    }

    #[test]
    fn render_queue_workflow_status_matches_top_level_status() {
        assert_eq!(
            build_static_page_render_queue_workflow_status(),
            build_static_page_render_queue_status()
        );
    }

    #[test]
    fn render_queue_workflow_ids_preserve_optional_execution_and_task() {
        let execution = workflow_execution();
        let task_id = WorkflowTaskId::new();

        let fields: StaticPageRenderQueueWorkflowIdsFields =
            build_static_page_render_queue_workflow_ids(Some(&execution), Some(task_id));
        let (workflow_execution_id, workflow_task_id) = fields;
        let empty_fields: StaticPageRenderQueueWorkflowIdsFields =
            build_static_page_render_queue_workflow_ids(None, None);
        let (empty_workflow_execution_id, empty_workflow_task_id) = empty_fields;

        assert_eq!(workflow_execution_id, Some(execution.id));
        assert_eq!(workflow_task_id, Some(task_id));
        assert_eq!(empty_workflow_execution_id, None);
        assert_eq!(empty_workflow_task_id, None);
    }

    #[test]
    fn render_queue_workflow_ids_fields_preserve_optional_execution_and_task() {
        let execution = workflow_execution();
        let task_id = WorkflowTaskId::new();

        let fields: StaticPageRenderQueueWorkflowIdsFields =
            build_static_page_render_queue_workflow_ids_fields(Some(&execution), Some(task_id));
        let (workflow_execution_id, workflow_task_id) = fields;
        let empty_fields: StaticPageRenderQueueWorkflowIdsFields =
            build_static_page_render_queue_workflow_ids_fields(None, None);
        let (empty_workflow_execution_id, empty_workflow_task_id) = empty_fields;

        assert_eq!(workflow_execution_id, Some(execution.id));
        assert_eq!(workflow_task_id, Some(task_id));
        assert_eq!(empty_workflow_execution_id, None);
        assert_eq!(empty_workflow_task_id, None);
    }

    #[test]
    fn render_queue_workflow_ids_from_fields_preserve_optional_values() {
        let execution_id = WorkflowExecutionId::new();
        let task_id = WorkflowTaskId::new();

        let fields: StaticPageRenderQueueWorkflowIdsFields =
            build_static_page_render_queue_workflow_ids_from_fields(
                Some(execution_id),
                Some(task_id),
            );
        let (workflow_execution_id, workflow_task_id) = fields;
        let empty_fields: StaticPageRenderQueueWorkflowIdsFields =
            build_static_page_render_queue_workflow_ids_from_fields(None, None);
        let (empty_workflow_execution_id, empty_workflow_task_id) = empty_fields;

        assert_eq!(workflow_execution_id, Some(execution_id));
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

        let fields: StaticPageRenderQueueImageContextFields =
            build_static_page_render_queue_image_context(Some(&image_job));
        let (image_job_id, preview_asset_key) = fields;
        let empty_fields: StaticPageRenderQueueImageContextFields =
            build_static_page_render_queue_image_context(None);
        let (empty_image_job_id, empty_preview_asset_key) = empty_fields;

        assert_eq!(image_job_id, Some(image_job.id));
        assert_eq!(
            preview_asset_key.as_deref(),
            Some("static-page-previews/preview.png")
        );
        assert_eq!(empty_image_job_id, None);
        assert_eq!(empty_preview_asset_key, None);
    }

    #[test]
    fn render_queue_image_context_fields_preserve_optional_job_id_and_preview_asset_key() {
        let draft = draft_with_payload(json!({}));
        let image_job = image_job_for_draft(&draft);

        let fields: StaticPageRenderQueueImageContextFields =
            build_static_page_render_queue_image_context_fields(Some(&image_job));
        let (image_job_id, preview_asset_key) = fields;
        let empty_fields: StaticPageRenderQueueImageContextFields =
            build_static_page_render_queue_image_context_fields(None);
        let (empty_image_job_id, empty_preview_asset_key) = empty_fields;

        assert_eq!(image_job_id, Some(image_job.id));
        assert_eq!(
            preview_asset_key.as_deref(),
            Some("static-page-previews/preview.png")
        );
        assert_eq!(empty_image_job_id, None);
        assert_eq!(empty_preview_asset_key, None);
    }

    #[test]
    fn render_queue_image_context_from_fields_preserves_optional_values() {
        let image_job_id = StaticPageImageJobId::new();

        let fields: StaticPageRenderQueueImageContextFields =
            build_static_page_render_queue_image_context_from_fields(
                Some(image_job_id),
                Some("static-page-previews/preview.png".to_string()),
            );
        let (resolved_image_job_id, preview_asset_key) = fields;
        let empty_fields: StaticPageRenderQueueImageContextFields =
            build_static_page_render_queue_image_context_from_fields(None, None);
        let (empty_image_job_id, empty_preview_asset_key) = empty_fields;

        assert_eq!(resolved_image_job_id, Some(image_job_id));
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
    fn render_queue_visual_spec_fields_preserve_alias_lookup_and_missing() {
        let camel_payload = json!({
            "visualSpec": {"styleDirection": "camel"}
        });
        let snake_payload = json!({
            "visual_spec": {"styleDirection": "snake"}
        });
        let camel_visual: StaticPageRenderQueueVisualSpecOption =
            build_static_page_render_queue_visual_spec_fields(&camel_payload);
        let snake_visual: StaticPageRenderQueueVisualSpecOption =
            build_static_page_render_queue_visual_spec_fields(&snake_payload);
        let missing_visual: StaticPageRenderQueueVisualSpecOption =
            build_static_page_render_queue_visual_spec_fields(&json!({}));

        assert_eq!(camel_visual, Some(json!({"styleDirection": "camel"})));
        assert_eq!(snake_visual, Some(json!({"styleDirection": "snake"})));
        assert_eq!(missing_visual, None);
    }

    #[test]
    fn render_queue_render_spec_fields_preserve_alias_lookup_and_missing() {
        let camel_payload = json!({
            "renderSpec": {"renderer": "camel-renderer"}
        });
        let snake_payload = json!({
            "render_spec": {"renderer": "snake-renderer"}
        });
        let camel_render: StaticPageRenderQueueRenderSpecOption =
            build_static_page_render_queue_render_spec_fields(&camel_payload);
        let snake_render: StaticPageRenderQueueRenderSpecOption =
            build_static_page_render_queue_render_spec_fields(&snake_payload);
        let missing_render: StaticPageRenderQueueRenderSpecOption =
            build_static_page_render_queue_render_spec_fields(&json!({}));

        assert_eq!(camel_render, Some(json!({"renderer": "camel-renderer"})));
        assert_eq!(snake_render, Some(json!({"renderer": "snake-renderer"})));
        assert_eq!(missing_render, None);
    }

    #[test]
    fn render_queue_specs_from_payload_preserve_explicit_and_fallback_specs() {
        let explicit_payload = json!({
            "visualSpec": {"styleDirection": "custom-report"},
            "renderSpec": {"renderer": "custom-renderer"}
        });
        let fallback_payload = json!({});

        let (explicit_visual, explicit_render): (
            StaticPageRenderQueueVisualSpecValue,
            StaticPageRenderQueueRenderSpecValue,
        ) = build_static_page_render_queue_specs_from_payload(&explicit_payload);
        let (fallback_visual, fallback_render): (
            StaticPageRenderQueueVisualSpecValue,
            StaticPageRenderQueueRenderSpecValue,
        ) = build_static_page_render_queue_specs_from_payload(&fallback_payload);

        assert_eq!(explicit_visual["styleDirection"], json!("custom-report"));
        assert_eq!(explicit_render["renderer"], json!("custom-renderer"));
        assert_eq!(fallback_visual["styleDirection"], json!("client-delivery"));
        assert_eq!(
            fallback_render["renderer"],
            json!("static-page-renderer-v1")
        );
    }

    #[test]
    fn render_queue_visual_spec_aliases_preserve_order() {
        let aliases: StaticPageRenderQueuePayloadAliasList =
            build_static_page_render_queue_visual_spec_aliases();

        assert_eq!(aliases, &["visualSpec", "visual_spec"]);
    }

    #[test]
    fn render_queue_visual_spec_from_payload_preserves_alias_lookup_and_missing() {
        let camel_payload = json!({
            "visualSpec": {"styleDirection": "camel"}
        });
        let snake_payload = json!({
            "visual_spec": {"styleDirection": "snake"}
        });
        let camel_visual: StaticPageRenderQueueVisualSpecOption =
            build_static_page_render_queue_visual_spec_from_payload(&camel_payload);
        let snake_visual: StaticPageRenderQueueVisualSpecOption =
            build_static_page_render_queue_visual_spec_from_payload(&snake_payload);
        let missing_visual: StaticPageRenderQueueVisualSpecOption =
            build_static_page_render_queue_visual_spec_from_payload(&json!({}));

        assert_eq!(camel_visual, Some(json!({"styleDirection": "camel"})));
        assert_eq!(snake_visual, Some(json!({"styleDirection": "snake"})));
        assert_eq!(missing_visual, None);
    }

    #[test]
    fn render_queue_visual_spec_from_payload_fields_preserve_alias_lookup_and_missing() {
        let camel_payload = json!({
            "visualSpec": {"styleDirection": "camel"}
        });
        let snake_payload = json!({
            "visual_spec": {"styleDirection": "snake"}
        });
        let camel_visual: StaticPageRenderQueueVisualSpecOption =
            build_static_page_render_queue_visual_spec_from_payload_fields(&camel_payload);
        let snake_visual: StaticPageRenderQueueVisualSpecOption =
            build_static_page_render_queue_visual_spec_from_payload_fields(&snake_payload);
        let missing_visual: StaticPageRenderQueueVisualSpecOption =
            build_static_page_render_queue_visual_spec_from_payload_fields(&json!({}));

        assert_eq!(camel_visual, Some(json!({"styleDirection": "camel"})));
        assert_eq!(snake_visual, Some(json!({"styleDirection": "snake"})));
        assert_eq!(missing_visual, None);
    }

    #[test]
    fn render_queue_visual_spec_from_optional_preserves_explicit_and_fallback() {
        let explicit_visual: StaticPageRenderQueueVisualSpecValue =
            json!({"styleDirection": "explicit"});
        let explicit_option: StaticPageRenderQueueVisualSpecOption = Some(explicit_visual.clone());

        assert_eq!(
            build_static_page_render_queue_visual_spec_from_optional(explicit_option),
            explicit_visual
        );
        assert_eq!(
            build_static_page_render_queue_visual_spec_from_optional(None)["styleDirection"],
            json!("client-delivery")
        );
    }

    #[test]
    fn render_queue_visual_spec_from_optional_fields_preserves_explicit_and_fallback() {
        let explicit_visual: StaticPageRenderQueueVisualSpecValue =
            json!({"styleDirection": "explicit"});
        let explicit_option: StaticPageRenderQueueVisualSpecOption = Some(explicit_visual.clone());

        assert_eq!(
            build_static_page_render_queue_visual_spec_from_optional_fields(explicit_option),
            explicit_visual
        );
        assert_eq!(
            build_static_page_render_queue_visual_spec_from_optional_fields(None)["styleDirection"],
            json!("client-delivery")
        );
    }

    #[test]
    fn render_queue_visual_fallback_style_preserves_client_delivery() {
        let style: StaticPageRenderQueueVisualFallbackStyle =
            build_static_page_render_queue_visual_fallback_style();

        assert_eq!(style, "client-delivery");
    }

    #[test]
    fn render_queue_visual_fallback_spec_preserves_client_delivery_style() {
        let fallback_visual = build_static_page_render_queue_visual_fallback_spec();

        assert_eq!(fallback_visual["styleDirection"], json!("client-delivery"));
    }

    #[test]
    fn render_queue_visual_fallback_spec_from_fields_preserves_style() {
        let style: StaticPageRenderQueueVisualFallbackStyle = "client-delivery";
        let fallback_visual =
            build_static_page_render_queue_visual_fallback_spec_from_fields(style);

        assert_eq!(fallback_visual["styleDirection"], json!("client-delivery"));
    }

    #[test]
    fn render_queue_render_spec_aliases_preserve_order() {
        let aliases: StaticPageRenderQueuePayloadAliasList =
            build_static_page_render_queue_render_spec_aliases();

        assert_eq!(aliases, &["renderSpec", "render_spec"]);
    }

    #[test]
    fn render_queue_render_spec_from_payload_preserves_alias_lookup_and_missing() {
        let camel_payload = json!({
            "renderSpec": {"renderer": "camel-renderer"}
        });
        let snake_payload = json!({
            "render_spec": {"renderer": "snake-renderer"}
        });
        let camel_render: StaticPageRenderQueueRenderSpecOption =
            build_static_page_render_queue_render_spec_from_payload(&camel_payload);
        let snake_render: StaticPageRenderQueueRenderSpecOption =
            build_static_page_render_queue_render_spec_from_payload(&snake_payload);
        let missing_render: StaticPageRenderQueueRenderSpecOption =
            build_static_page_render_queue_render_spec_from_payload(&json!({}));

        assert_eq!(camel_render, Some(json!({"renderer": "camel-renderer"})));
        assert_eq!(snake_render, Some(json!({"renderer": "snake-renderer"})));
        assert_eq!(missing_render, None);
    }

    #[test]
    fn render_queue_render_spec_from_payload_fields_preserve_alias_lookup_and_missing() {
        let camel_payload = json!({
            "renderSpec": {"renderer": "camel-renderer"}
        });
        let snake_payload = json!({
            "render_spec": {"renderer": "snake-renderer"}
        });
        let camel_render: StaticPageRenderQueueRenderSpecOption =
            build_static_page_render_queue_render_spec_from_payload_fields(&camel_payload);
        let snake_render: StaticPageRenderQueueRenderSpecOption =
            build_static_page_render_queue_render_spec_from_payload_fields(&snake_payload);
        let missing_render: StaticPageRenderQueueRenderSpecOption =
            build_static_page_render_queue_render_spec_from_payload_fields(&json!({}));

        assert_eq!(camel_render, Some(json!({"renderer": "camel-renderer"})));
        assert_eq!(snake_render, Some(json!({"renderer": "snake-renderer"})));
        assert_eq!(missing_render, None);
    }

    #[test]
    fn render_queue_render_spec_from_optional_preserves_explicit_and_fallback() {
        let explicit_render: StaticPageRenderQueueRenderSpecValue =
            json!({"renderer": "custom-renderer"});
        let explicit_option: StaticPageRenderQueueRenderSpecOption = Some(explicit_render.clone());

        assert_eq!(
            build_static_page_render_queue_render_spec_from_optional(explicit_option),
            explicit_render
        );

        let fallback_render = build_static_page_render_queue_render_spec_from_optional(None);

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
    fn render_queue_render_spec_from_optional_fields_preserves_explicit_and_fallback() {
        let explicit_render: StaticPageRenderQueueRenderSpecValue =
            json!({"renderer": "custom-renderer"});
        let explicit_option: StaticPageRenderQueueRenderSpecOption = Some(explicit_render.clone());

        assert_eq!(
            build_static_page_render_queue_render_spec_from_optional_fields(explicit_option),
            explicit_render
        );

        let fallback_render = build_static_page_render_queue_render_spec_from_optional_fields(None);

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
    fn render_queue_render_fallback_spec_from_fields_preserves_renderer_and_dynamic_data() {
        let fallback_render = build_static_page_render_queue_render_fallback_spec_from_fields();

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
    fn render_queue_data_snapshot_fields_preserve_alias_lookup_and_missing() {
        let camel_payload = json!({
            "dataSnapshot": {"source": "camel"}
        });
        let snake_payload = json!({
            "data_snapshot": {"source": "snake"}
        });
        let camel_snapshot: StaticPageRenderQueueDataSnapshotOption =
            build_static_page_render_queue_data_snapshot_fields(&camel_payload);
        let snake_snapshot: StaticPageRenderQueueDataSnapshotOption =
            build_static_page_render_queue_data_snapshot_fields(&snake_payload);
        let missing_snapshot: StaticPageRenderQueueDataSnapshotOption =
            build_static_page_render_queue_data_snapshot_fields(&json!({}));

        assert_eq!(camel_snapshot, Some(json!({"source": "camel"})));
        assert_eq!(snake_snapshot, Some(json!({"source": "snake"})));
        assert_eq!(missing_snapshot, None);
    }

    #[test]
    fn render_queue_data_snapshot_aliases_preserve_order() {
        let aliases: StaticPageRenderQueuePayloadAliasList =
            build_static_page_render_queue_data_snapshot_aliases();

        assert_eq!(aliases, &["dataSnapshot", "data_snapshot"]);
    }

    #[test]
    fn render_queue_data_snapshot_from_payload_preserves_alias_lookup_and_missing() {
        let camel_payload = json!({
            "dataSnapshot": {"source": "camel"}
        });
        let snake_payload = json!({
            "data_snapshot": {"source": "snake"}
        });
        let camel_snapshot: StaticPageRenderQueueDataSnapshotOption =
            build_static_page_render_queue_data_snapshot_from_payload(&camel_payload);
        let snake_snapshot: StaticPageRenderQueueDataSnapshotOption =
            build_static_page_render_queue_data_snapshot_from_payload(&snake_payload);
        let missing_snapshot: StaticPageRenderQueueDataSnapshotOption =
            build_static_page_render_queue_data_snapshot_from_payload(&json!({}));

        assert_eq!(camel_snapshot, Some(json!({"source": "camel"})));
        assert_eq!(snake_snapshot, Some(json!({"source": "snake"})));
        assert_eq!(missing_snapshot, None);
    }

    #[test]
    fn render_queue_data_snapshot_from_payload_fields_preserve_alias_lookup_and_missing() {
        let camel_payload = json!({
            "dataSnapshot": {"source": "camel"}
        });
        let snake_payload = json!({
            "data_snapshot": {"source": "snake"}
        });
        let camel_snapshot: StaticPageRenderQueueDataSnapshotOption =
            build_static_page_render_queue_data_snapshot_from_payload_fields(&camel_payload);
        let snake_snapshot: StaticPageRenderQueueDataSnapshotOption =
            build_static_page_render_queue_data_snapshot_from_payload_fields(&snake_payload);
        let missing_snapshot: StaticPageRenderQueueDataSnapshotOption =
            build_static_page_render_queue_data_snapshot_from_payload_fields(&json!({}));

        assert_eq!(camel_snapshot, Some(json!({"source": "camel"})));
        assert_eq!(snake_snapshot, Some(json!({"source": "snake"})));
        assert_eq!(missing_snapshot, None);
    }

    #[test]
    fn render_queue_data_snapshot_from_optional_preserves_explicit_and_fallback() {
        let explicit_snapshot: StaticPageRenderQueueDataSnapshotValue =
            json!({"source": "provided"});
        let explicit_option: StaticPageRenderQueueDataSnapshotOption =
            Some(explicit_snapshot.clone());
        let fallback_payload = json!({
            "modules": [{"id": "summary"}]
        });
        let selected_scope = json!({"datasets": ["dataset-a"]});

        assert_eq!(
            build_static_page_render_queue_data_snapshot_from_optional(
                explicit_option,
                &fallback_payload,
                &selected_scope,
            ),
            explicit_snapshot
        );

        let fallback_snapshot = build_static_page_render_queue_data_snapshot_from_optional(
            None,
            &fallback_payload,
            &selected_scope,
        );

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
    fn render_queue_data_snapshot_from_optional_fields_preserve_explicit_and_fallback() {
        let explicit_snapshot: StaticPageRenderQueueDataSnapshotValue =
            json!({"source": "provided"});
        let explicit_option: StaticPageRenderQueueDataSnapshotOption =
            Some(explicit_snapshot.clone());
        let fallback_payload = json!({
            "modules": [{"id": "summary"}]
        });
        let selected_scope = json!({"datasets": ["dataset-a"]});

        assert_eq!(
            build_static_page_render_queue_data_snapshot_from_optional_fields(
                explicit_option,
                &fallback_payload,
                &selected_scope,
            ),
            explicit_snapshot
        );

        let fallback_snapshot = build_static_page_render_queue_data_snapshot_from_optional_fields(
            None,
            &fallback_payload,
            &selected_scope,
        );

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
    fn render_queue_fallback_data_snapshot_from_fields_preserves_static_page_snapshot() {
        let payload = json!({
            "modules": [{"id": "summary"}]
        });
        let selected_scope = json!({"datasets": ["dataset-a"]});

        let fallback_snapshot = build_static_page_render_queue_fallback_data_snapshot_from_fields(
            &payload,
            &selected_scope,
        );

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

        let modules: StaticPageRenderQueueModulesValue =
            build_static_page_render_queue_modules(&payload);

        assert_eq!(
            modules,
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
    fn render_queue_modules_fields_preserve_array_and_empty_fallback() {
        let payload = json!({
            "modules": [
                {"id": "summary"},
                {"id": "trend"}
            ]
        });

        let modules: StaticPageRenderQueueModulesValue =
            build_static_page_render_queue_modules_fields(&payload);

        assert_eq!(
            modules,
            json!([
                {"id": "summary"},
                {"id": "trend"}
            ])
        );
        assert_eq!(
            build_static_page_render_queue_modules_fields(&json!({"modules": "invalid"})),
            json!([])
        );
        assert_eq!(
            build_static_page_render_queue_modules_fields(&json!({})),
            json!([])
        );
    }

    #[test]
    fn render_queue_modules_from_payload_preserves_array_and_empty_fallback() {
        let payload = json!({
            "modules": [
                {"id": "summary"},
                {"id": "trend"}
            ]
        });

        let modules: StaticPageRenderQueueModulesValue =
            build_static_page_render_queue_modules_from_payload(&payload);

        assert_eq!(
            modules,
            json!([
                {"id": "summary"},
                {"id": "trend"}
            ])
        );
        assert_eq!(
            build_static_page_render_queue_modules_from_payload(&json!({"modules": "invalid"})),
            json!([])
        );
        assert_eq!(
            build_static_page_render_queue_modules_from_payload(&json!({})),
            json!([])
        );
    }

    #[test]
    fn render_queue_modules_from_payload_fields_preserve_array_and_empty_fallback() {
        let payload = json!({
            "modules": [
                {"id": "summary"},
                {"id": "trend"}
            ]
        });

        let modules: StaticPageRenderQueueModulesValue =
            build_static_page_render_queue_modules_from_payload_fields(&payload);

        assert_eq!(
            modules,
            json!([
                {"id": "summary"},
                {"id": "trend"}
            ])
        );
        assert_eq!(
            build_static_page_render_queue_modules_from_payload_fields(
                &json!({"modules": "invalid"})
            ),
            json!([])
        );
        assert_eq!(
            build_static_page_render_queue_modules_from_payload_fields(&json!({})),
            json!([])
        );
    }

    #[test]
    fn render_queue_modules_from_value_preserves_array_and_empty_values() {
        let modules: StaticPageRenderQueueModulesValue = json!([
            {"id": "summary"},
            {"id": "trend"}
        ]);

        assert_eq!(
            build_static_page_render_queue_modules_from_value(modules.clone()),
            modules
        );
        assert_eq!(
            build_static_page_render_queue_modules_from_value(json!([])),
            json!([])
        );
    }

    #[test]
    fn render_queue_modules_from_value_fields_preserve_array_and_empty_values() {
        let modules: StaticPageRenderQueueModulesValue = json!([
            {"id": "summary"},
            {"id": "trend"}
        ]);

        assert_eq!(
            build_static_page_render_queue_modules_from_value_fields(modules.clone()),
            modules
        );
        assert_eq!(
            build_static_page_render_queue_modules_from_value_fields(json!([])),
            json!([])
        );
    }

    #[test]
    fn render_queue_export_package_preserves_manifest_fields_and_debug_counts() {
        let draft = draft_with_payload(json!({}));
        let modules: StaticPageRenderQueueModulesValue = json!([
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
        let data_snapshot: StaticPageRenderQueueDataSnapshotValue = json!({"source": "provided"});

        let export_package: StaticPageRenderQueueExportPackageValue =
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
    fn render_queue_export_package_fields_preserve_inputs() {
        let draft = draft_with_payload(json!({}));
        let modules: StaticPageRenderQueueModulesValue = json!([
            {
                "id": "trend",
                "visualization": {
                    "chartRuntime": "echarts"
                }
            }
        ]);
        let data_snapshot: StaticPageRenderQueueDataSnapshotValue = json!({"source": "provided"});

        let fields: StaticPageRenderQueueExportPackageFields<'_> =
            build_static_page_render_queue_export_package_fields(&draft, &modules, &data_snapshot);
        let (resolved_draft, resolved_modules, resolved_data_snapshot) = fields;

        assert_eq!(resolved_draft.id, draft.id);
        assert_eq!(resolved_modules, &modules);
        assert_eq!(resolved_data_snapshot, &data_snapshot);
    }

    #[test]
    fn render_queue_export_package_from_fields_preserves_manifest_fields_and_debug_counts() {
        let draft = draft_with_payload(json!({}));
        let modules: StaticPageRenderQueueModulesValue = json!([
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
        let data_snapshot: StaticPageRenderQueueDataSnapshotValue = json!({"source": "provided"});

        let export_package: StaticPageRenderQueueExportPackageValue =
            build_static_page_render_queue_export_package_from_fields(
                &draft,
                &modules,
                &data_snapshot,
            );

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
    fn queued_export_package_debug_context_preserves_counts_and_source() {
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

        let debug = build_static_page_queued_export_package_debug_context(&modules, &data_snapshot);

        assert_eq!(debug["renderer"], json!("static-page-renderer-v1"));
        assert_eq!(debug["module_count"], json!(2));
        assert_eq!(debug["echarts_requested_modules"], json!(1));
        assert_eq!(debug["data_snapshot_source"], json!("provided"));
    }

    #[test]
    fn queued_export_package_debug_context_fields_preserve_module_counts() {
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

        let counts: StaticPageQueuedExportPackageModuleCounts =
            build_static_page_queued_export_package_debug_context_fields(&modules);
        assert_eq!(counts, (2, 1));
        let invalid_counts: StaticPageQueuedExportPackageModuleCounts =
            build_static_page_queued_export_package_debug_context_fields(&json!("invalid"));
        assert_eq!(invalid_counts, (0, 0));
    }

    #[test]
    fn queued_export_package_module_items_fields_preserve_array_and_invalid_fallback() {
        let modules = json!([
            {
                "id": "trend"
            },
            {
                "id": "summary"
            }
        ]);

        let items = build_static_page_queued_export_package_module_items_fields(&modules)
            .expect("module array should resolve to items");

        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["id"], json!("trend"));
        assert!(
            build_static_page_queued_export_package_module_items_fields(&json!("invalid"))
                .is_none()
        );
    }

    #[test]
    fn queued_export_package_module_counts_optional_items_fields_preserve_items_and_none() {
        let modules = json!([
            {
                "id": "trend"
            },
            {
                "id": "summary"
            }
        ]);
        let items = modules
            .as_array()
            .map(Vec::as_slice)
            .expect("module array should resolve to items");

        let resolved_items =
            build_static_page_queued_export_package_module_counts_from_optional_items_fields(Some(
                items,
            ))
            .expect("optional items should pass through");

        assert_eq!(resolved_items.len(), 2);
        assert_eq!(resolved_items[1]["id"], json!("summary"));
        assert!(
            build_static_page_queued_export_package_module_counts_from_optional_items_fields(None)
                .is_none()
        );
    }

    #[test]
    fn queued_export_package_module_counts_from_items_fields_preserve_counts() {
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

        let counts: StaticPageQueuedExportPackageModuleCounts =
            build_static_page_queued_export_package_module_counts_from_items_fields(&modules);
        let (module_count, echarts_requested_modules): (
            StaticPageQueuedExportPackageModuleCount,
            StaticPageQueuedExportPackageModuleCount,
        ) = counts;
        assert_eq!(module_count, 3);
        assert_eq!(echarts_requested_modules, 2);
        let empty_counts: StaticPageQueuedExportPackageModuleCounts =
            build_static_page_queued_export_package_module_counts_from_items_fields(&[]);
        assert_eq!(empty_counts, (0, 0));
    }

    #[test]
    fn queued_export_package_module_count_helpers_preserve_scalar_values() {
        let modules = vec![
            json!({
                "id": "trend",
                "visualization": {
                    "chartRuntime": "echarts"
                }
            }),
            json!({
                "id": "summary"
            }),
        ];

        let module_count: StaticPageQueuedExportPackageModuleCount =
            build_static_page_queued_export_package_module_count(&modules);
        let echarts_count: StaticPageQueuedExportPackageModuleCount =
            build_static_page_queued_export_package_echarts_requested_module_count(&modules);

        assert_eq!(module_count, 2);
        assert_eq!(echarts_count, 1);
    }

    #[test]
    fn queued_export_package_empty_module_counts_fields_preserve_zero_pair() {
        let counts: StaticPageQueuedExportPackageModuleCounts =
            build_static_page_queued_export_package_empty_module_counts_fields();
        assert_eq!(counts, (0, 0));
    }

    #[test]
    fn queued_export_package_debug_context_from_fields_preserves_debug_shape() {
        let debug = build_static_page_queued_export_package_debug_context_from_fields(
            3,
            2,
            &json!({"source": "provided"}),
        );
        let missing_source_debug =
            build_static_page_queued_export_package_debug_context_from_fields(0, 0, &json!({}));

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
    fn queued_export_package_base_context_preserves_static_fields() {
        let draft = draft_with_payload(json!({}));

        let (kind, version, status, draft_id, files, dynamic_page_contract) =
            build_static_page_queued_export_package_base_context(&draft);

        assert_eq!(kind, "static-page-export-package");
        assert_eq!(version, 1);
        assert_eq!(status, "queued");
        assert_eq!(draft_id, draft.id);
        assert_eq!(files.as_array().map(Vec::len), Some(5));
        assert_eq!(dynamic_page_contract["data_file"], json!("data.json"));
        assert_eq!(
            dynamic_page_contract["source_snapshot_file"],
            json!("data-snapshot.json")
        );
    }

    #[test]
    fn queued_export_package_base_context_from_fields_preserves_values() {
        let kind: StaticPageQueuedExportPackageLifecycleScalar = "static-page-export-package";
        let version: StaticPageQueuedExportPackageVersion = 1;
        let status: StaticPageQueuedExportPackageLifecycleScalar = "queued";
        let draft_id = StaticPageDraftId::new();
        let files: StaticPageQueuedExportPackageFilesValue = json!([
            {"path": "index.html", "role": "rendered_static_page", "mime": "text/html"}
        ]);
        let dynamic_page_contract: StaticPageQueuedExportPackageDynamicPageContractValue = json!({
            "data_file": "data.json",
            "source_snapshot_file": "data-snapshot.json"
        });

        let fields: StaticPageQueuedExportPackageBaseContextFields =
            build_static_page_queued_export_package_base_context_from_fields(
                kind,
                version,
                status,
                draft_id,
                files.clone(),
                dynamic_page_contract.clone(),
            );
        let (
            kind,
            version,
            status,
            resolved_draft_id,
            resolved_files,
            resolved_dynamic_page_contract,
        ) = fields;

        assert_eq!(kind, "static-page-export-package");
        assert_eq!(version, 1);
        assert_eq!(status, "queued");
        assert_eq!(resolved_draft_id, draft_id);
        assert_eq!(resolved_files, files);
        assert_eq!(resolved_dynamic_page_contract, dynamic_page_contract);
    }

    #[test]
    fn queued_export_package_base_context_fields_from_fields_preserves_values() {
        let kind: StaticPageQueuedExportPackageLifecycleScalar = "static-page-export-package";
        let version: StaticPageQueuedExportPackageVersion = 1;
        let status: StaticPageQueuedExportPackageLifecycleScalar = "queued";
        let draft_id = StaticPageDraftId::new();
        let files: StaticPageQueuedExportPackageFilesValue = json!([
            {"path": "index.html", "role": "rendered_static_page", "mime": "text/html"}
        ]);
        let dynamic_page_contract: StaticPageQueuedExportPackageDynamicPageContractValue = json!({
            "data_file": "data.json",
            "source_snapshot_file": "data-snapshot.json"
        });

        let fields: StaticPageQueuedExportPackageBaseContextFields =
            build_static_page_queued_export_package_base_context_fields_from_fields(
                kind,
                version,
                status,
                draft_id,
                files.clone(),
                dynamic_page_contract.clone(),
            );
        let (
            kind,
            version,
            status,
            resolved_draft_id,
            resolved_files,
            resolved_dynamic_page_contract,
        ) = fields;

        assert_eq!(kind, "static-page-export-package");
        assert_eq!(version, 1);
        assert_eq!(status, "queued");
        assert_eq!(resolved_draft_id, draft_id);
        assert_eq!(resolved_files, files);
        assert_eq!(resolved_dynamic_page_contract, dynamic_page_contract);
    }

    #[test]
    fn queued_export_package_base_context_fields_preserve_static_values() {
        let draft = draft_with_payload(json!({}));

        let fields: StaticPageQueuedExportPackageBaseContextFields =
            build_static_page_queued_export_package_base_context_fields(&draft);
        let (kind, version, status, draft_id, files, dynamic_page_contract) = fields;

        assert_eq!(kind, "static-page-export-package");
        assert_eq!(version, 1);
        assert_eq!(status, "queued");
        assert_eq!(draft_id, draft.id);
        assert_eq!(files.as_array().map(Vec::len), Some(5));
        assert_eq!(dynamic_page_contract["data_file"], json!("data.json"));
        assert_eq!(
            dynamic_page_contract["source_snapshot_file"],
            json!("data-snapshot.json")
        );
    }

    #[test]
    fn queued_export_package_manifest_from_fields_preserves_object_shape() {
        let draft_id = StaticPageDraftId::new();
        let files: StaticPageQueuedExportPackageFilesValue = json!([
            {"path": "index.html", "role": "rendered_static_page", "mime": "text/html"}
        ]);
        let dynamic_page_contract: StaticPageQueuedExportPackageDynamicPageContractValue =
            json!({"version": 1, "data_file": "data.json"});
        let debug: StaticPageQueuedExportPackageDebugValue = json!({
            "renderer": "static-page-renderer-v1",
            "module_count": 2,
            "echarts_requested_modules": 1,
            "data_snapshot_source": "provided"
        });

        let export_package: StaticPageRenderQueueExportPackageValue =
            build_static_page_queued_export_package_manifest_from_fields(
                "static-page-export-package",
                1,
                "queued",
                draft_id,
                files.clone(),
                dynamic_page_contract.clone(),
                debug.clone(),
            );

        assert_eq!(export_package["kind"], json!("static-page-export-package"));
        assert_eq!(export_package["version"], json!(1));
        assert_eq!(export_package["status"], json!("queued"));
        assert_eq!(export_package["draft_id"], json!(draft_id));
        assert_eq!(export_package["files"], files);
        assert_eq!(
            export_package["dynamic_page_contract"],
            dynamic_page_contract
        );
        assert_eq!(export_package["debug"], debug);
    }

    #[test]
    fn queued_export_package_manifest_payload_fields_preserve_object_shape() {
        let draft_id = StaticPageDraftId::new();
        let files: StaticPageQueuedExportPackageFilesValue = json!([
            {"path": "index.html", "role": "rendered_static_page", "mime": "text/html"}
        ]);
        let dynamic_page_contract: StaticPageQueuedExportPackageDynamicPageContractValue =
            json!({"version": 1, "data_file": "data.json"});
        let debug: StaticPageQueuedExportPackageDebugValue = json!({
            "renderer": "static-page-renderer-v1",
            "module_count": 2,
            "echarts_requested_modules": 1,
            "data_snapshot_source": "provided"
        });

        let export_package: StaticPageRenderQueueExportPackageValue =
            build_static_page_queued_export_package_manifest_payload_fields(
                "static-page-export-package",
                1,
                "queued",
                draft_id,
                files.clone(),
                dynamic_page_contract.clone(),
                debug.clone(),
            );

        assert_eq!(export_package["kind"], json!("static-page-export-package"));
        assert_eq!(export_package["version"], json!(1));
        assert_eq!(export_package["status"], json!("queued"));
        assert_eq!(export_package["draft_id"], json!(draft_id));
        assert_eq!(export_package["files"], files);
        assert_eq!(
            export_package["dynamic_page_contract"],
            dynamic_page_contract
        );
        assert_eq!(export_package["debug"], debug);
    }

    #[test]
    fn queued_export_package_manifest_fields_preserve_base_context_and_debug() {
        let draft = draft_with_payload(json!({}));
        let modules: StaticPageRenderQueueModulesValue = json!([
            {
                "id": "trend",
                "visualization": {
                    "chartRuntime": "echarts"
                }
            },
            {"id": "summary"}
        ]);
        let data_snapshot: StaticPageRenderQueueDataSnapshotValue = json!({"source": "provided"});

        let fields: StaticPageQueuedExportPackageManifestFields =
            build_static_page_queued_export_package_manifest_fields(
                &draft,
                &modules,
                &data_snapshot,
            );
        let (kind, version, status, draft_id, files, dynamic_page_contract, debug) = fields;

        assert_eq!(kind, "static-page-export-package");
        assert_eq!(version, 1);
        assert_eq!(status, "queued");
        assert_eq!(draft_id, draft.id);
        assert_eq!(files.as_array().map(Vec::len), Some(5));
        assert_eq!(
            dynamic_page_contract["refresh_policy"]["mode"],
            json!("poll_data_json_when_published")
        );
        assert_eq!(debug["module_count"], json!(2));
        assert_eq!(debug["echarts_requested_modules"], json!(1));
        assert_eq!(debug["data_snapshot_source"], json!("provided"));
    }

    #[test]
    fn queued_export_package_manifest_context_fields_preserve_base_context_and_debug() {
        let draft = draft_with_payload(json!({}));
        let modules: StaticPageRenderQueueModulesValue = json!([
            {
                "id": "trend",
                "visualization": {
                    "chartRuntime": "echarts"
                }
            },
            {"id": "summary"}
        ]);
        let data_snapshot: StaticPageRenderQueueDataSnapshotValue = json!({"source": "provided"});

        let (kind, version, status, draft_id, files, dynamic_page_contract, debug) =
            build_static_page_queued_export_package_manifest_context_fields(
                &draft,
                &modules,
                &data_snapshot,
            );

        assert_eq!(kind, "static-page-export-package");
        assert_eq!(version, 1);
        assert_eq!(status, "queued");
        assert_eq!(draft_id, draft.id);
        assert_eq!(files.as_array().map(Vec::len), Some(5));
        assert_eq!(
            dynamic_page_contract["refresh_policy"]["mode"],
            json!("poll_data_json_when_published")
        );
        assert_eq!(debug["module_count"], json!(2));
        assert_eq!(debug["echarts_requested_modules"], json!(1));
        assert_eq!(debug["data_snapshot_source"], json!("provided"));
    }

    #[test]
    fn queued_export_package_manifest_fields_from_fields_preserve_values() {
        let kind: StaticPageQueuedExportPackageLifecycleScalar = "static-page-export-package";
        let version: StaticPageQueuedExportPackageVersion = 1;
        let status: StaticPageQueuedExportPackageLifecycleScalar = "queued";
        let draft_id = StaticPageDraftId::new();
        let files: StaticPageQueuedExportPackageFilesValue = json!([
            {"path": "index.html", "role": "rendered_static_page", "mime": "text/html"}
        ]);
        let dynamic_page_contract: StaticPageQueuedExportPackageDynamicPageContractValue =
            json!({"version": 1, "data_file": "data.json"});
        let debug: StaticPageQueuedExportPackageDebugValue = json!({
            "renderer": "static-page-renderer-v1",
            "module_count": 2,
            "echarts_requested_modules": 1,
            "data_snapshot_source": "provided"
        });

        let (
            kind,
            version,
            status,
            resolved_draft_id,
            resolved_files,
            resolved_dynamic_page_contract,
            resolved_debug,
        ) = build_static_page_queued_export_package_manifest_fields_from_fields(
            kind,
            version,
            status,
            draft_id,
            files.clone(),
            dynamic_page_contract.clone(),
            debug.clone(),
        );

        assert_eq!(kind, "static-page-export-package");
        assert_eq!(version, 1);
        assert_eq!(status, "queued");
        assert_eq!(resolved_draft_id, draft_id);
        assert_eq!(resolved_files, files);
        assert_eq!(resolved_dynamic_page_contract, dynamic_page_contract);
        assert_eq!(resolved_debug, debug);
    }

    #[test]
    fn queued_export_package_lifecycle_preserves_kind_version_and_status() {
        let fields: StaticPageQueuedExportPackageLifecycleFields =
            build_static_page_queued_export_package_lifecycle();
        let (kind, version, status) = fields;

        assert_eq!(kind, "static-page-export-package");
        assert_eq!(version, 1);
        assert_eq!(status, "queued");
    }

    #[test]
    fn queued_export_package_lifecycle_fields_preserve_kind_version_and_status() {
        let fields: StaticPageQueuedExportPackageLifecycleFields =
            build_static_page_queued_export_package_lifecycle_fields();
        let (kind, version, status) = fields;

        assert_eq!(kind, "static-page-export-package");
        assert_eq!(version, 1);
        assert_eq!(status, "queued");
    }

    #[test]
    fn queued_export_package_lifecycle_from_fields_preserves_fixed_values() {
        let fields: StaticPageQueuedExportPackageLifecycleFields =
            build_static_page_queued_export_package_lifecycle_from_fields(
                "static-page-export-package",
                1,
                "queued",
            );
        let (kind, version, status) = fields;

        assert_eq!(kind, "static-page-export-package");
        assert_eq!(version, 1);
        assert_eq!(status, "queued");
    }

    #[test]
    fn queued_export_package_lifecycle_field_helpers_preserve_fixed_values() {
        let kind: StaticPageQueuedExportPackageLifecycleScalar =
            build_static_page_queued_export_package_kind();
        let version: StaticPageQueuedExportPackageVersion =
            build_static_page_queued_export_package_version();
        let status: StaticPageQueuedExportPackageLifecycleScalar =
            build_static_page_queued_export_package_status();

        assert_eq!(kind, "static-page-export-package");
        assert_eq!(version, 1);
        assert_eq!(status, "queued");
        assert_eq!(
            build_static_page_queued_export_package_status(),
            build_static_page_render_queue_status()
        );
    }

    #[test]
    fn queued_export_package_kind_fields_preserve_fixed_value() {
        assert_eq!(
            build_static_page_queued_export_package_kind_fields(),
            "static-page-export-package"
        );
    }

    #[test]
    fn queued_export_package_version_fields_preserve_fixed_value() {
        assert_eq!(build_static_page_queued_export_package_version_fields(), 1);
    }

    #[test]
    fn queued_export_package_status_fields_match_top_level_status() {
        assert_eq!(
            build_static_page_queued_export_package_status_fields(),
            build_static_page_render_queue_status()
        );
    }

    #[test]
    fn queued_export_package_draft_id_preserves_draft_id() {
        let draft = draft_with_payload(json!({}));

        let draft_id = build_static_page_queued_export_package_draft_id(&draft);

        assert_eq!(draft_id, draft.id);
    }

    #[test]
    fn queued_export_package_draft_id_matches_render_queue_draft_id() {
        let draft = draft_with_payload(json!({}));

        assert_eq!(
            build_static_page_queued_export_package_draft_id(&draft),
            build_static_page_render_queue_draft_id(&draft)
        );
    }

    #[test]
    fn queued_export_package_draft_id_fields_match_render_queue_draft_id() {
        let draft = draft_with_payload(json!({}));

        assert_eq!(
            build_static_page_queued_export_package_draft_id_fields(&draft),
            build_static_page_render_queue_draft_id(&draft)
        );
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
    fn queued_export_package_dynamic_contract_fields_preserve_refresh_defaults() {
        let contract = build_static_page_queued_export_package_dynamic_page_contract_fields();

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
    fn queued_export_package_dynamic_contract_from_fields_preserves_contract() {
        let contract = json!({
            "version": 2,
            "data_file": "custom-data.json",
            "refresh_policy": {
                "interval_seconds": 30
            }
        });

        assert_eq!(
            build_static_page_queued_export_package_dynamic_page_contract_from_fields(
                contract.clone()
            ),
            contract
        );
    }

    #[test]
    fn queued_export_package_files_preserve_file_roles_and_order() {
        let files: StaticPageQueuedExportPackageFilesValue =
            build_static_page_queued_export_package_files();
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
    fn queued_export_package_files_fields_preserve_default_specs() {
        let specs = build_static_page_queued_export_package_files_fields();

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
    }

    #[test]
    fn queued_export_package_file_preserves_path_role_and_mime() {
        let file: StaticPageQueuedExportPackageFileValue =
            build_static_page_queued_export_package_file(
                "index.html",
                "rendered_static_page",
                "text/html",
            );

        assert_eq!(file["path"], json!("index.html"));
        assert_eq!(file["role"], json!("rendered_static_page"));
        assert_eq!(file["mime"], json!("text/html"));
    }

    #[test]
    fn queued_export_package_file_fields_preserve_path_role_and_mime_tuple() {
        assert_eq!(
            build_static_page_queued_export_package_file_fields(
                "index.html",
                "rendered_static_page",
                "text/html"
            ),
            ("index.html", "rendered_static_page", "text/html")
        );
    }

    #[test]
    fn queued_export_package_file_from_fields_preserves_path_role_and_mime() {
        let file: StaticPageQueuedExportPackageFileValue =
            build_static_page_queued_export_package_file_from_fields(
                "index.html",
                "rendered_static_page",
                "text/html",
            );

        assert_eq!(file["path"], json!("index.html"));
        assert_eq!(file["role"], json!("rendered_static_page"));
        assert_eq!(file["mime"], json!("text/html"));
    }

    #[test]
    fn queued_export_package_file_payload_fields_preserve_path_role_and_mime() {
        let file: StaticPageQueuedExportPackageFileValue =
            build_static_page_queued_export_package_file_payload_fields(
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
    fn queued_export_package_file_specs_fields_preserve_order_and_mimes() {
        let specs = build_static_page_queued_export_package_file_specs_fields();

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
    fn queued_export_package_html_mime_fields_preserve_value() {
        assert_eq!(
            build_static_page_queued_export_package_html_mime_fields(),
            "text/html"
        );
    }

    #[test]
    fn queued_export_package_json_mime_fields_preserve_value() {
        assert_eq!(
            build_static_page_queued_export_package_json_mime_fields(),
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
    fn queued_export_package_index_path_fields_preserve_value() {
        assert_eq!(
            build_static_page_queued_export_package_index_path_fields(),
            "index.html"
        );
    }

    #[test]
    fn queued_export_package_asset_manifest_path_fields_preserve_value() {
        assert_eq!(
            build_static_page_queued_export_package_asset_manifest_path_fields(),
            "asset-manifest.json"
        );
    }

    #[test]
    fn queued_export_package_data_snapshot_path_fields_preserve_value() {
        assert_eq!(
            build_static_page_queued_export_package_data_snapshot_path_fields(),
            "data-snapshot.json"
        );
    }

    #[test]
    fn queued_export_package_dynamic_data_path_fields_preserve_value() {
        assert_eq!(
            build_static_page_queued_export_package_dynamic_data_path_fields(),
            "data.json"
        );
    }

    #[test]
    fn queued_export_package_modules_path_fields_preserve_value() {
        assert_eq!(
            build_static_page_queued_export_package_modules_path_fields(),
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
    fn queued_export_package_rendered_page_role_fields_preserve_value() {
        assert_eq!(
            build_static_page_queued_export_package_rendered_page_role_fields(),
            "rendered_static_page"
        );
    }

    #[test]
    fn queued_export_package_renderer_manifest_role_fields_preserve_value() {
        assert_eq!(
            build_static_page_queued_export_package_renderer_manifest_role_fields(),
            "renderer_manifest"
        );
    }

    #[test]
    fn queued_export_package_render_data_snapshot_role_fields_preserve_value() {
        assert_eq!(
            build_static_page_queued_export_package_render_data_snapshot_role_fields(),
            "render_data_snapshot"
        );
    }

    #[test]
    fn queued_export_package_dynamic_data_snapshot_role_fields_preserve_value() {
        assert_eq!(
            build_static_page_queued_export_package_dynamic_data_snapshot_role_fields(),
            "dynamic_data_snapshot"
        );
    }

    #[test]
    fn queued_export_package_editable_module_plan_role_fields_preserve_value() {
        assert_eq!(
            build_static_page_queued_export_package_editable_module_plan_role_fields(),
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
    fn queued_export_package_file_spec_fields_preserve_tuple_order() {
        assert_eq!(
            build_static_page_queued_export_package_file_spec_fields(
                "index.html",
                "rendered_static_page",
                "text/html"
            ),
            ("index.html", "rendered_static_page", "text/html")
        );
    }

    #[test]
    fn queued_export_package_file_from_spec_preserves_path_role_and_mime() {
        let file: StaticPageQueuedExportPackageFileValue =
            build_static_page_queued_export_package_file_from_spec((
                "index.html",
                "rendered_static_page",
                "text/html",
            ));

        assert_eq!(file["path"], json!("index.html"));
        assert_eq!(file["role"], json!("rendered_static_page"));
        assert_eq!(file["mime"], json!("text/html"));
    }

    #[test]
    fn queued_export_package_file_from_spec_fields_preserve_tuple_order() {
        let fields: StaticPageQueuedExportPackageFileSpec =
            build_static_page_queued_export_package_file_from_spec_fields((
                "index.html",
                "rendered_static_page",
                "text/html",
            ));

        assert_eq!(fields, ("index.html", "rendered_static_page", "text/html"));
    }

    #[test]
    fn queued_export_package_file_spec_field_helpers_preserve_tuple_values() {
        let spec = ("index.html", "rendered_static_page", "text/html");
        let path: StaticPageQueuedExportPackageFilePath =
            build_static_page_queued_export_package_file_spec_path(spec);
        let role: StaticPageQueuedExportPackageFileRole =
            build_static_page_queued_export_package_file_spec_role(spec);
        let mime: StaticPageQueuedExportPackageFileMime =
            build_static_page_queued_export_package_file_spec_mime(spec);

        assert_eq!(path, "index.html");
        assert_eq!(role, "rendered_static_page");
        assert_eq!(mime, "text/html");
    }

    #[test]
    fn queued_export_package_file_spec_path_fields_preserve_tuple_value() {
        assert_eq!(
            build_static_page_queued_export_package_file_spec_path_fields((
                "index.html",
                "rendered_static_page",
                "text/html",
            )),
            "index.html"
        );
    }

    #[test]
    fn queued_export_package_file_spec_role_fields_preserve_tuple_value() {
        assert_eq!(
            build_static_page_queued_export_package_file_spec_role_fields((
                "index.html",
                "rendered_static_page",
                "text/html",
            )),
            "rendered_static_page"
        );
    }

    #[test]
    fn queued_export_package_file_spec_mime_fields_preserve_tuple_value() {
        assert_eq!(
            build_static_page_queued_export_package_file_spec_mime_fields((
                "index.html",
                "rendered_static_page",
                "text/html",
            )),
            "text/html"
        );
    }

    #[test]
    fn queued_export_package_files_from_specs_preserve_order_and_fields() {
        let files: StaticPageQueuedExportPackageFilesValue =
            build_static_page_queued_export_package_files_from_specs(&[
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
    fn queued_export_package_files_from_specs_fields_preserve_order_and_fields() {
        let items: StaticPageQueuedExportPackageFileEntriesValue =
            build_static_page_queued_export_package_files_from_specs_fields(&[
                ("first.html", "primary_page", "text/html"),
                ("second.json", "secondary_data", "application/json"),
            ]);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["path"], json!("first.html"));
        assert_eq!(items[0]["role"], json!("primary_page"));
        assert_eq!(items[0]["mime"], json!("text/html"));
        assert_eq!(items[1]["path"], json!("second.json"));
        assert_eq!(items[1]["role"], json!("secondary_data"));
        assert_eq!(items[1]["mime"], json!("application/json"));
    }

    #[test]
    fn queued_export_package_file_entries_from_specs_preserve_order_and_fields() {
        let items: StaticPageQueuedExportPackageFileEntriesValue =
            build_static_page_queued_export_package_file_entries_from_specs(&[
                ("first.html", "primary_page", "text/html"),
                ("second.json", "secondary_data", "application/json"),
            ]);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["path"], json!("first.html"));
        assert_eq!(items[0]["role"], json!("primary_page"));
        assert_eq!(items[0]["mime"], json!("text/html"));
        assert_eq!(items[1]["path"], json!("second.json"));
        assert_eq!(items[1]["role"], json!("secondary_data"));
        assert_eq!(items[1]["mime"], json!("application/json"));
    }

    #[test]
    fn queued_export_package_file_entries_from_specs_fields_preserve_order_and_fields() {
        let items: StaticPageQueuedExportPackageFileEntriesValue =
            build_static_page_queued_export_package_file_entries_from_specs_fields(&[
                ("first.html", "primary_page", "text/html"),
                ("second.json", "secondary_data", "application/json"),
            ]);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["path"], json!("first.html"));
        assert_eq!(items[0]["role"], json!("primary_page"));
        assert_eq!(items[0]["mime"], json!("text/html"));
        assert_eq!(items[1]["path"], json!("second.json"));
        assert_eq!(items[1]["role"], json!("secondary_data"));
        assert_eq!(items[1]["mime"], json!("application/json"));
    }

    #[test]
    fn queued_export_package_file_entries_from_specs_fields_entries_preserve_order_and_fields() {
        let items: StaticPageQueuedExportPackageFileEntriesValue =
            build_static_page_queued_export_package_file_entries_from_specs_fields_entries(&[
                ("first.html", "primary_page", "text/html"),
                ("second.json", "secondary_data", "application/json"),
            ]);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["path"], json!("first.html"));
        assert_eq!(items[0]["role"], json!("primary_page"));
        assert_eq!(items[0]["mime"], json!("text/html"));
        assert_eq!(items[1]["path"], json!("second.json"));
        assert_eq!(items[1]["role"], json!("secondary_data"));
        assert_eq!(items[1]["mime"], json!("application/json"));
    }

    #[test]
    fn queued_export_package_files_from_entries_preserve_array_order() {
        let entries: StaticPageQueuedExportPackageFileEntriesValue = vec![
            json!({"path": "first.html", "role": "primary_page", "mime": "text/html"}),
            json!({"path": "second.json", "role": "secondary_data", "mime": "application/json"}),
        ];
        let files: StaticPageQueuedExportPackageFilesValue =
            build_static_page_queued_export_package_files_from_entries(entries);
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
    fn queued_export_package_files_from_entries_fields_preserve_array_order() {
        let entries: StaticPageQueuedExportPackageFileEntriesValue = vec![
            json!({"path": "first.html", "role": "primary_page", "mime": "text/html"}),
            json!({"path": "second.json", "role": "secondary_data", "mime": "application/json"}),
        ];
        let items: StaticPageQueuedExportPackageFileEntriesValue =
            build_static_page_queued_export_package_files_from_entries_fields(entries);

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
    fn queued_export_package_debug_from_fields_preserves_object_shape() {
        let renderer: StaticPageQueuedExportPackageDebugRenderer = "static-page-renderer-v1";
        let module_count: StaticPageQueuedExportPackageModuleCount = 3;
        let echarts_requested_modules: StaticPageQueuedExportPackageModuleCount = 2;
        let data_snapshot_source: StaticPageQueuedExportPackageDataSnapshotSource<'_> = "provided";
        let debug = build_static_page_queued_export_package_debug_from_fields(
            renderer,
            module_count,
            echarts_requested_modules,
            data_snapshot_source,
        );

        assert_eq!(debug["renderer"], json!("static-page-renderer-v1"));
        assert_eq!(debug["module_count"], json!(3));
        assert_eq!(debug["echarts_requested_modules"], json!(2));
        assert_eq!(debug["data_snapshot_source"], json!("provided"));
    }

    #[test]
    fn queued_export_package_debug_payload_fields_preserve_object_shape() {
        let renderer: StaticPageQueuedExportPackageDebugRenderer = "static-page-renderer-v1";
        let module_count: StaticPageQueuedExportPackageModuleCount = 3;
        let echarts_requested_modules: StaticPageQueuedExportPackageModuleCount = 2;
        let data_snapshot_source: StaticPageQueuedExportPackageDataSnapshotSource<'_> = "provided";
        let debug = build_static_page_queued_export_package_debug_payload_fields(
            renderer,
            module_count,
            echarts_requested_modules,
            data_snapshot_source,
        );

        assert_eq!(debug["renderer"], json!("static-page-renderer-v1"));
        assert_eq!(debug["module_count"], json!(3));
        assert_eq!(debug["echarts_requested_modules"], json!(2));
        assert_eq!(debug["data_snapshot_source"], json!("provided"));
    }

    #[test]
    fn queued_export_package_debug_fields_preserve_renderer_counts_and_snapshot_source() {
        let data_snapshot = json!({"source": "provided"});
        let fields: StaticPageQueuedExportPackageDebugFields<'_> =
            build_static_page_queued_export_package_debug_fields(3, 2, &data_snapshot);
        let (renderer, module_count, echarts_requested_modules, data_snapshot_source) = fields;

        assert_eq!(renderer, "static-page-renderer-v1");
        assert_eq!(module_count, 3);
        assert_eq!(echarts_requested_modules, 2);
        assert_eq!(data_snapshot_source, "provided");

        let missing_snapshot = json!({});
        let (_, _, _, missing_source) =
            build_static_page_queued_export_package_debug_fields(0, 0, &missing_snapshot);
        assert_eq!(missing_source, "unknown");
    }

    #[test]
    fn queued_export_package_debug_fields_from_fields_preserve_tuple_values() {
        let renderer: StaticPageQueuedExportPackageDebugRenderer = "static-page-renderer-v1";
        let module_count: StaticPageQueuedExportPackageModuleCount = 3;
        let echarts_requested_modules: StaticPageQueuedExportPackageModuleCount = 2;
        let data_snapshot_source: StaticPageQueuedExportPackageDataSnapshotSource<'_> = "provided";
        let fields: StaticPageQueuedExportPackageDebugFields<'_> =
            build_static_page_queued_export_package_debug_fields_from_fields(
                renderer,
                module_count,
                echarts_requested_modules,
                data_snapshot_source,
            );
        let (renderer, module_count, echarts_requested_modules, data_snapshot_source) = fields;

        assert_eq!(renderer, "static-page-renderer-v1");
        assert_eq!(module_count, 3);
        assert_eq!(echarts_requested_modules, 2);
        assert_eq!(data_snapshot_source, "provided");
    }

    #[test]
    fn queued_export_package_debug_renderer_preserves_renderer_name() {
        let renderer: StaticPageQueuedExportPackageDebugRenderer =
            build_static_page_queued_export_package_debug_renderer();

        assert_eq!(renderer, "static-page-renderer-v1");
    }

    #[test]
    fn queued_export_package_debug_renderer_matches_render_queue_renderer() {
        assert_eq!(
            build_static_page_queued_export_package_debug_renderer(),
            build_static_page_render_queue_renderer()
        );
    }

    #[test]
    fn queued_export_package_debug_renderer_fields_match_render_queue_renderer() {
        assert_eq!(
            build_static_page_queued_export_package_debug_renderer_fields(),
            build_static_page_render_queue_renderer()
        );
    }

    #[test]
    fn queued_export_package_data_snapshot_source_preserves_string_and_unknown_fallback() {
        let provided_snapshot: StaticPageRenderQueueDataSnapshotValue =
            json!({"source": "provided"});
        let missing_snapshot: StaticPageRenderQueueDataSnapshotValue = json!({});
        let non_string_snapshot: StaticPageRenderQueueDataSnapshotValue = json!({"source": 123});
        let source: StaticPageQueuedExportPackageDataSnapshotSource<'_> =
            build_static_page_queued_export_package_data_snapshot_source(&provided_snapshot);

        assert_eq!(source, "provided");
        assert_eq!(
            build_static_page_queued_export_package_data_snapshot_source(&missing_snapshot),
            "unknown"
        );
        assert_eq!(
            build_static_page_queued_export_package_data_snapshot_source(&non_string_snapshot),
            "unknown"
        );
    }

    #[test]
    fn queued_export_package_data_snapshot_source_fields_preserve_optional_string() {
        let provided_snapshot: StaticPageRenderQueueDataSnapshotValue =
            json!({"source": "provided"});
        let missing_snapshot: StaticPageRenderQueueDataSnapshotValue = json!({});
        let non_string_snapshot: StaticPageRenderQueueDataSnapshotValue = json!({"source": 123});

        assert_eq!(
            build_static_page_queued_export_package_data_snapshot_source_fields(&provided_snapshot),
            Some("provided")
        );
        assert_eq!(
            build_static_page_queued_export_package_data_snapshot_source_fields(&missing_snapshot),
            None
        );
        assert_eq!(
            build_static_page_queued_export_package_data_snapshot_source_fields(
                &non_string_snapshot
            ),
            None
        );
    }

    #[test]
    fn queued_export_package_data_snapshot_source_value_preserves_string_and_missing() {
        let provided_snapshot: StaticPageRenderQueueDataSnapshotValue =
            json!({"source": "provided"});
        let missing_snapshot: StaticPageRenderQueueDataSnapshotValue = json!({});
        let non_string_snapshot: StaticPageRenderQueueDataSnapshotValue = json!({"source": 123});

        assert_eq!(
            build_static_page_queued_export_package_data_snapshot_source_value(&provided_snapshot),
            Some("provided")
        );
        assert_eq!(
            build_static_page_queued_export_package_data_snapshot_source_value(&missing_snapshot),
            None
        );
        assert_eq!(
            build_static_page_queued_export_package_data_snapshot_source_value(
                &non_string_snapshot
            ),
            None
        );
    }

    #[test]
    fn queued_export_package_data_snapshot_source_value_fields_preserve_string_and_missing() {
        let provided_snapshot: StaticPageRenderQueueDataSnapshotValue =
            json!({"source": "provided"});
        let missing_snapshot: StaticPageRenderQueueDataSnapshotValue = json!({});
        let non_string_snapshot: StaticPageRenderQueueDataSnapshotValue = json!({"source": 123});

        assert_eq!(
            build_static_page_queued_export_package_data_snapshot_source_value_fields(
                &provided_snapshot
            ),
            Some("provided")
        );
        assert_eq!(
            build_static_page_queued_export_package_data_snapshot_source_value_fields(
                &missing_snapshot
            ),
            None
        );
        assert_eq!(
            build_static_page_queued_export_package_data_snapshot_source_value_fields(
                &non_string_snapshot
            ),
            None
        );
    }

    #[test]
    fn queued_export_package_data_snapshot_source_from_optional_preserves_fallback() {
        let source: StaticPageQueuedExportPackageDataSnapshotSource<'_> = "provided";

        assert_eq!(
            build_static_page_queued_export_package_data_snapshot_source_from_optional(Some(
                source
            )),
            "provided"
        );
        assert_eq!(
            build_static_page_queued_export_package_data_snapshot_source_from_optional(None),
            "unknown"
        );
    }

    #[test]
    fn queued_export_package_data_snapshot_source_from_optional_fields_preserve_fallback() {
        assert_eq!(
            build_static_page_queued_export_package_data_snapshot_source_from_optional_fields(
                Some("provided")
            ),
            "provided"
        );
        assert_eq!(
            build_static_page_queued_export_package_data_snapshot_source_from_optional_fields(None),
            "unknown"
        );
    }

    #[test]
    fn queued_export_package_unknown_data_snapshot_source_preserves_value() {
        assert_eq!(
            build_static_page_queued_export_package_unknown_data_snapshot_source(),
            "unknown"
        );
    }

    #[test]
    fn queued_export_package_unknown_data_snapshot_source_fields_preserve_value() {
        assert_eq!(
            build_static_page_queued_export_package_unknown_data_snapshot_source_fields(),
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

        let counts: StaticPageQueuedExportPackageModuleCounts =
            build_static_page_queued_export_package_module_counts(&modules);
        assert_eq!(counts, (3, 2));
        let invalid_counts: StaticPageQueuedExportPackageModuleCounts =
            build_static_page_queued_export_package_module_counts(&json!("invalid"));
        assert_eq!(invalid_counts, (0, 0));
    }

    #[test]
    fn queued_export_package_module_counts_fields_preserve_array_and_invalid_fallback() {
        let modules = json!([
            {"id": "trend"},
            {"id": "summary"}
        ]);

        assert_eq!(
            build_static_page_queued_export_package_module_counts_fields(&modules)
                .map(|items| items.len()),
            Some(2)
        );
        assert_eq!(
            build_static_page_queued_export_package_module_counts_fields(&json!("invalid"))
                .map(|items| items.len()),
            None
        );
    }

    #[test]
    fn queued_export_package_module_items_preserve_array_and_invalid_fallback() {
        let modules = json!([
            {"id": "trend"},
            {"id": "summary"}
        ]);

        assert_eq!(
            build_static_page_queued_export_package_module_items(&modules).map(|items| items.len()),
            Some(2)
        );
        assert_eq!(
            build_static_page_queued_export_package_module_items(&json!("invalid"))
                .map(|items| items.len()),
            None
        );
    }

    #[test]
    fn queued_export_package_module_counts_from_optional_items_preserve_counts_and_fallback() {
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
        ];

        let counts: StaticPageQueuedExportPackageModuleCounts =
            build_static_page_queued_export_package_module_counts_from_optional_items(Some(
                modules.as_slice(),
            ));
        assert_eq!(counts, (2, 1));
        let fallback_counts: StaticPageQueuedExportPackageModuleCounts =
            build_static_page_queued_export_package_module_counts_from_optional_items(None);
        assert_eq!(fallback_counts, (0, 0));
    }

    #[test]
    fn queued_export_package_empty_module_counts_preserve_zero_pair() {
        let counts: StaticPageQueuedExportPackageModuleCounts =
            build_static_page_queued_export_package_empty_module_counts();
        assert_eq!(counts, (0, 0));
    }

    #[test]
    fn queued_export_package_module_counts_from_items_preserve_counts() {
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

        let counts: StaticPageQueuedExportPackageModuleCounts =
            build_static_page_queued_export_package_module_counts_from_items(&modules);
        assert_eq!(counts, (3, 2));
        let empty_counts: StaticPageQueuedExportPackageModuleCounts =
            build_static_page_queued_export_package_module_counts_from_items(&[]);
        assert_eq!(empty_counts, (0, 0));
    }

    #[test]
    fn queued_export_package_module_counts_from_fields_preserve_counts() {
        let counts: StaticPageQueuedExportPackageModuleCounts =
            build_static_page_queued_export_package_module_counts_from_fields(3, 2);
        assert_eq!(counts, (3, 2));
        let empty_counts: StaticPageQueuedExportPackageModuleCounts =
            build_static_page_queued_export_package_module_counts_from_fields(0, 0);
        assert_eq!(empty_counts, (0, 0));
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
    fn queued_export_package_module_count_fields_preserve_array_len() {
        let modules = vec![
            json!({"id": "hero"}),
            json!({"id": "trend"}),
            json!({"id": "table"}),
        ];

        assert_eq!(
            build_static_page_queued_export_package_module_count_fields(&modules),
            3
        );
        assert_eq!(
            build_static_page_queued_export_package_module_count_fields(&[]),
            0
        );
    }

    #[test]
    fn queued_export_package_module_requests_echarts_preserves_runtime_matching() {
        let visualization_echarts: StaticPageQueuedExportPackageModuleValue = json!({
            "id": "trend",
            "visualization": {
                "chartRuntime": "echarts"
            }
        });
        let chart_options_echarts: StaticPageQueuedExportPackageModuleValue = json!({
            "id": "advanced",
            "chartOptions": {
                "chartRuntime": "echarts"
            }
        });
        let non_echarts: StaticPageQueuedExportPackageModuleValue = json!({
            "id": "summary",
            "visualization": {
                "type": "text"
            }
        });

        assert!(
            build_static_page_queued_export_package_module_requests_echarts(&visualization_echarts)
        );
        assert!(
            build_static_page_queued_export_package_module_requests_echarts(&chart_options_echarts)
        );
        assert!(!build_static_page_queued_export_package_module_requests_echarts(&non_echarts));
    }

    #[test]
    fn queued_export_package_module_requests_echarts_fields_preserve_runtime_matching() {
        let visualization_echarts: StaticPageQueuedExportPackageModuleValue = json!({
            "id": "trend",
            "visualization": {
                "chartRuntime": "echarts"
            }
        });
        let chart_options_echarts: StaticPageQueuedExportPackageModuleValue = json!({
            "id": "advanced",
            "chartOptions": {
                "chartRuntime": "echarts"
            }
        });
        let non_echarts: StaticPageQueuedExportPackageModuleValue = json!({
            "id": "summary",
            "visualization": {
                "type": "text"
            }
        });

        assert!(
            build_static_page_queued_export_package_module_requests_echarts_fields(
                &visualization_echarts
            )
        );
        assert!(
            build_static_page_queued_export_package_module_requests_echarts_fields(
                &chart_options_echarts
            )
        );
        assert!(
            !build_static_page_queued_export_package_module_requests_echarts_fields(&non_echarts)
        );
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
    fn queued_export_package_echarts_requested_module_count_fields_preserve_runtime_matching() {
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
            build_static_page_queued_export_package_echarts_requested_module_count_fields(&modules),
            2
        );
        assert_eq!(
            build_static_page_queued_export_package_echarts_requested_module_count_fields(&[]),
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
