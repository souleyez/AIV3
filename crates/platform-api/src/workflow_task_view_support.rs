use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use contracts::{self, WorkflowTaskView};
use domain_model::{WorkflowTask, WorkflowTaskStatus};
use serde_json::Value;

use crate::{
    model_gateway_runtime::percentile_latency,
    static_page_template_prewarm_support::STATIC_PAGE_TEMPLATE_PREWARM_TASK_KEY,
};

type WorkflowTaskDurationPercentiles = (Option<u64>, Option<u64>);

pub(crate) fn to_workflow_task_view(task: WorkflowTask) -> WorkflowTaskView {
    let logical_queue = workflow_task_logical_queue(&task);
    let logical_task_key = workflow_task_logical_task_key(&task);
    let remote_task_id = workflow_task_remote_task_id(&task.payload);
    let next_poll_at = workflow_task_next_poll_at(&task.payload);
    WorkflowTaskView {
        id: task.id,
        status: task.status,
        queue: task.queue,
        task_key: task.task_key,
        logical_queue,
        logical_task_key,
        remote_task_id,
        next_poll_at,
        attempt: task.attempt,
        max_attempts: task.max_attempts,
        available_at: task.available_at,
        claimed_at: task.claimed_at,
        finished_at: task.finished_at,
        error: task.error,
        updated_at: task.updated_at,
        payload: task.payload,
    }
}

fn workflow_task_logical_queue(task: &WorkflowTask) -> Option<String> {
    task.payload
        .get("logical_queue")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| match task.task_key.as_str() {
            STATIC_PAGE_TEMPLATE_PREWARM_TASK_KEY => {
                Some("static_page_template_prewarm".to_string())
            }
            "generate_static_page_image" => Some("static_page_image_preview".to_string()),
            "render_static_page" => Some("static_page_render".to_string()),
            "run_codex_host_task" => {
                if workflow_task_is_static_page_publish(task) {
                    Some("static_page_publish".to_string())
                } else {
                    Some("codex_fixed_task".to_string())
                }
            }
            _ => None,
        })
}

fn workflow_task_logical_task_key(task: &WorkflowTask) -> Option<String> {
    task.payload
        .get("logical_task_key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| match task.task_key.as_str() {
            STATIC_PAGE_TEMPLATE_PREWARM_TASK_KEY => {
                Some("prepare_template_when_low_load".to_string())
            }
            "generate_static_page_image" => Some("submit_static_page_image_preview".to_string()),
            "render_static_page" => Some("render_static_page".to_string()),
            "run_codex_host_task" => {
                let has_remote_task = workflow_task_remote_task_id(&task.payload).is_some();
                let is_static_page_publish = workflow_task_is_static_page_publish(task);
                Some(match (is_static_page_publish, has_remote_task) {
                    (true, true) => "poll_static_page_publish".to_string(),
                    (true, false) => "submit_static_page_publish".to_string(),
                    (false, true) => "poll_codex_fixed_task".to_string(),
                    (false, false) => "submit_codex_fixed_task".to_string(),
                })
            }
            _ => None,
        })
}

fn workflow_task_is_static_page_publish(task: &WorkflowTask) -> bool {
    task.task_key == "run_codex_host_task"
        && workflow_task_codex_template_id(&task.payload).as_deref()
            == Some("static_page_image2_data_publish")
}

fn workflow_task_codex_template_id(payload: &Value) -> Option<String> {
    ["/template_id", "/capability", "/fixed_task/template_id"]
        .iter()
        .find_map(|pointer| {
            payload
                .pointer(pointer)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
}

fn workflow_task_remote_task_id(payload: &Value) -> Option<String> {
    [
        "/static_page_image_orchestrator/task_id",
        "/cloudflare_orchestrator/task_id",
        "/remote_task_id",
    ]
    .iter()
    .find_map(|pointer| {
        payload
            .pointer(pointer)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn workflow_task_next_poll_at(payload: &Value) -> Option<DateTime<Utc>> {
    [
        "/static_page_image_orchestrator/next_poll_at",
        "/cloudflare_orchestrator/next_poll_at",
        "/next_poll_at",
    ]
    .iter()
    .find_map(|pointer| {
        payload
            .pointer(pointer)
            .and_then(Value::as_str)
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc))
    })
}

#[derive(Default)]
struct WorkflowTaskQueueStatsAccumulator {
    physical_queues: BTreeSet<String>,
    task_count: u64,
    queued: u64,
    running: u64,
    retrying: u64,
    succeeded: u64,
    failed: u64,
    cancelled: u64,
    dead_lettered: u64,
    next_available_at: Option<DateTime<Utc>>,
    finished_duration_samples_ms: Vec<u64>,
    succeeded_duration_samples_ms: Vec<u64>,
    task_keys: BTreeMap<String, WorkflowTaskKeyStatsAccumulator>,
}

#[derive(Default)]
struct WorkflowTaskKeyStatsAccumulator {
    physical_task_keys: BTreeSet<String>,
    task_count: u64,
    queued: u64,
    running: u64,
    retrying: u64,
    succeeded: u64,
    failed: u64,
    cancelled: u64,
    dead_lettered: u64,
    next_available_at: Option<DateTime<Utc>>,
    finished_duration_samples_ms: Vec<u64>,
    succeeded_duration_samples_ms: Vec<u64>,
}

pub(crate) fn summarize_workflow_task_queue_stats(
    generated_at: DateTime<Utc>,
    execution_count: usize,
    tasks: &[WorkflowTaskView],
) -> contracts::WorkflowTaskQueueStatsView {
    let mut queues = BTreeMap::<String, WorkflowTaskQueueStatsAccumulator>::new();
    for task in tasks {
        let logical_queue = task
            .logical_queue
            .clone()
            .unwrap_or_else(|| task.queue.clone());
        let logical_task_key = task
            .logical_task_key
            .clone()
            .unwrap_or_else(|| task.task_key.clone());
        let queue = queues.entry(logical_queue).or_default();
        queue.physical_queues.insert(task.queue.clone());
        record_workflow_task_queue_stats(queue, task);
        let task_key = queue.task_keys.entry(logical_task_key).or_default();
        task_key.physical_task_keys.insert(task.task_key.clone());
        record_workflow_task_key_stats(task_key, task);
    }

    contracts::WorkflowTaskQueueStatsView {
        generated_at,
        execution_count: execution_count as u64,
        task_count: tasks.len() as u64,
        queues: queues
            .into_iter()
            .map(|(logical_queue, accumulator)| {
                let (finished_duration_p50_ms, finished_duration_p95_ms) =
                    workflow_task_duration_percentiles(&accumulator.finished_duration_samples_ms);
                let (succeeded_duration_p50_ms, succeeded_duration_p95_ms) =
                    workflow_task_duration_percentiles(&accumulator.succeeded_duration_samples_ms);
                contracts::WorkflowTaskQueueSummaryView {
                    logical_queue,
                    physical_queues: accumulator.physical_queues.into_iter().collect(),
                    task_count: accumulator.task_count,
                    queued: accumulator.queued,
                    running: accumulator.running,
                    retrying: accumulator.retrying,
                    succeeded: accumulator.succeeded,
                    failed: accumulator.failed,
                    cancelled: accumulator.cancelled,
                    dead_lettered: accumulator.dead_lettered,
                    next_available_at: accumulator.next_available_at,
                    finished_duration_p50_ms,
                    finished_duration_p95_ms,
                    succeeded_duration_p50_ms,
                    succeeded_duration_p95_ms,
                    task_keys: accumulator
                        .task_keys
                        .into_iter()
                        .map(|(logical_task_key, task_key)| {
                            let (finished_duration_p50_ms, finished_duration_p95_ms) =
                                workflow_task_duration_percentiles(
                                    &task_key.finished_duration_samples_ms,
                                );
                            let (succeeded_duration_p50_ms, succeeded_duration_p95_ms) =
                                workflow_task_duration_percentiles(
                                    &task_key.succeeded_duration_samples_ms,
                                );
                            contracts::WorkflowTaskKeySummaryView {
                                logical_task_key,
                                physical_task_keys: task_key
                                    .physical_task_keys
                                    .into_iter()
                                    .collect(),
                                task_count: task_key.task_count,
                                queued: task_key.queued,
                                running: task_key.running,
                                retrying: task_key.retrying,
                                succeeded: task_key.succeeded,
                                failed: task_key.failed,
                                cancelled: task_key.cancelled,
                                dead_lettered: task_key.dead_lettered,
                                next_available_at: task_key.next_available_at,
                                finished_duration_p50_ms,
                                finished_duration_p95_ms,
                                succeeded_duration_p50_ms,
                                succeeded_duration_p95_ms,
                            }
                        })
                        .collect(),
                }
            })
            .collect(),
    }
}

fn record_workflow_task_queue_stats(
    accumulator: &mut WorkflowTaskQueueStatsAccumulator,
    task: &WorkflowTaskView,
) {
    accumulator.task_count += 1;
    match task.status {
        WorkflowTaskStatus::Queued => {
            accumulator.queued += 1;
            accumulator.next_available_at =
                earliest_optional_datetime(accumulator.next_available_at, task.available_at);
        }
        WorkflowTaskStatus::Claimed => accumulator.running += 1,
        WorkflowTaskStatus::Succeeded => accumulator.succeeded += 1,
        WorkflowTaskStatus::Failed => accumulator.failed += 1,
        WorkflowTaskStatus::Cancelled => accumulator.cancelled += 1,
        WorkflowTaskStatus::DeadLettered => accumulator.dead_lettered += 1,
    }
    if workflow_task_view_is_retrying(task) {
        accumulator.retrying += 1;
    }
    if let Some(duration_ms) = workflow_task_finished_duration_ms(task) {
        accumulator.finished_duration_samples_ms.push(duration_ms);
        if matches!(task.status, WorkflowTaskStatus::Succeeded) {
            accumulator.succeeded_duration_samples_ms.push(duration_ms);
        }
    }
}

fn record_workflow_task_key_stats(
    accumulator: &mut WorkflowTaskKeyStatsAccumulator,
    task: &WorkflowTaskView,
) {
    accumulator.task_count += 1;
    match task.status {
        WorkflowTaskStatus::Queued => {
            accumulator.queued += 1;
            accumulator.next_available_at =
                earliest_optional_datetime(accumulator.next_available_at, task.available_at);
        }
        WorkflowTaskStatus::Claimed => accumulator.running += 1,
        WorkflowTaskStatus::Succeeded => accumulator.succeeded += 1,
        WorkflowTaskStatus::Failed => accumulator.failed += 1,
        WorkflowTaskStatus::Cancelled => accumulator.cancelled += 1,
        WorkflowTaskStatus::DeadLettered => accumulator.dead_lettered += 1,
    }
    if workflow_task_view_is_retrying(task) {
        accumulator.retrying += 1;
    }
    if let Some(duration_ms) = workflow_task_finished_duration_ms(task) {
        accumulator.finished_duration_samples_ms.push(duration_ms);
        if matches!(task.status, WorkflowTaskStatus::Succeeded) {
            accumulator.succeeded_duration_samples_ms.push(duration_ms);
        }
    }
}

fn workflow_task_view_is_retrying(task: &WorkflowTaskView) -> bool {
    matches!(task.status, WorkflowTaskStatus::Queued)
        && (task.attempt > 0 || task.error.is_some() || task.next_poll_at.is_some())
}

fn workflow_task_finished_duration_ms(task: &WorkflowTaskView) -> Option<u64> {
    let finished_at = task.finished_at?;
    let started_at = task.claimed_at.unwrap_or(task.available_at);
    let duration_ms = (finished_at - started_at).num_milliseconds();
    (duration_ms >= 0).then_some(duration_ms as u64)
}

fn workflow_task_duration_percentiles(samples: &[u64]) -> WorkflowTaskDurationPercentiles {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    (
        percentile_latency(&sorted, 50),
        percentile_latency(&sorted, 95),
    )
}

fn earliest_optional_datetime(
    current: Option<DateTime<Utc>>,
    candidate: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    Some(current.map_or(candidate, |value| value.min(candidate)))
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone};
    use domain_model::{TenantId, WorkflowExecutionId, WorkflowTaskId};
    use serde_json::json;

    use super::*;

    fn workflow_task(task_key: &str, payload: Value, now: DateTime<Utc>) -> WorkflowTask {
        WorkflowTask {
            id: WorkflowTaskId::new(),
            tenant_id: TenantId::new(),
            execution_id: WorkflowExecutionId::new(),
            queue: "default".to_string(),
            task_key: task_key.to_string(),
            payload,
            status: WorkflowTaskStatus::Queued,
            attempt: 0,
            max_attempts: 3,
            available_at: now,
            claimed_at: None,
            finished_at: None,
            error: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn workflow_task_view_prefers_explicit_logical_fields_and_poll_metadata() {
        let now = Utc.with_ymd_and_hms(2026, 6, 13, 1, 2, 3).unwrap();
        let next_poll_at = now + Duration::seconds(30);
        let task = workflow_task(
            "generate_static_page_image",
            json!({
                "logical_queue": "custom_queue",
                "logical_task_key": "custom_key",
                "static_page_image_orchestrator": {
                    "task_id": "image-task-1",
                    "next_poll_at": next_poll_at.to_rfc3339()
                }
            }),
            now,
        );

        let view = to_workflow_task_view(task);

        assert_eq!(view.logical_queue.as_deref(), Some("custom_queue"));
        assert_eq!(view.logical_task_key.as_deref(), Some("custom_key"));
        assert_eq!(view.remote_task_id.as_deref(), Some("image-task-1"));
        assert_eq!(view.next_poll_at, Some(next_poll_at));
    }

    #[test]
    fn workflow_task_stats_tracks_retrying_and_duration_percentiles() {
        let now = Utc.with_ymd_and_hms(2026, 6, 13, 1, 2, 3).unwrap();
        let retrying = WorkflowTaskView {
            id: WorkflowTaskId::new(),
            status: WorkflowTaskStatus::Queued,
            queue: "physical".to_string(),
            task_key: "physical_key".to_string(),
            logical_queue: Some("logical".to_string()),
            logical_task_key: Some("logical_key".to_string()),
            remote_task_id: None,
            next_poll_at: Some(now + Duration::seconds(10)),
            attempt: 1,
            max_attempts: 3,
            available_at: now,
            claimed_at: None,
            finished_at: None,
            error: None,
            updated_at: now,
            payload: json!({}),
        };
        let succeeded = WorkflowTaskView {
            id: WorkflowTaskId::new(),
            status: WorkflowTaskStatus::Succeeded,
            queue: "physical".to_string(),
            task_key: "physical_key".to_string(),
            logical_queue: Some("logical".to_string()),
            logical_task_key: Some("logical_key".to_string()),
            remote_task_id: None,
            next_poll_at: None,
            attempt: 0,
            max_attempts: 3,
            available_at: now,
            claimed_at: Some(now + Duration::seconds(1)),
            finished_at: Some(now + Duration::seconds(6)),
            error: None,
            updated_at: now + Duration::seconds(6),
            payload: json!({}),
        };

        let stats = summarize_workflow_task_queue_stats(now, 2, &[retrying, succeeded]);
        let queue = &stats.queues[0];

        assert_eq!(queue.logical_queue, "logical");
        assert_eq!(queue.retrying, 1);
        assert_eq!(queue.succeeded, 1);
        assert_eq!(queue.finished_duration_p50_ms, Some(5_000));
        assert_eq!(queue.succeeded_duration_p50_ms, Some(5_000));
        assert_eq!(queue.task_keys[0].logical_task_key, "logical_key");
        assert_eq!(queue.task_keys[0].retrying, 1);
    }

    #[test]
    fn workflow_task_duration_percentiles_preserve_p50_and_p95_order() {
        let percentiles: WorkflowTaskDurationPercentiles =
            workflow_task_duration_percentiles(&[5_000, 1_000, 2_000]);
        let (p50_ms, p95_ms) = percentiles;

        assert_eq!(p50_ms, Some(2_000));
        assert_eq!(p95_ms, Some(5_000));
    }
}
