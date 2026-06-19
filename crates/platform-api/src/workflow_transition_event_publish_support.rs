use domain_model::{WorkflowEventRecord, WorkflowExecution, WorkflowTask};
use event_bus::{
    workflow_execution_transition_subject, workflow_task_enqueued_subject, EventBus, EventEnvelope,
};
use serde_json::json;

pub(crate) async fn publish_workflow_transition_events(
    event_bus: &EventBus,
    execution: &WorkflowExecution,
    persisted_event: &WorkflowEventRecord,
    persisted_tasks: &[WorkflowTask],
) {
    let execution_event = EventEnvelope {
        subject: workflow_execution_transition_subject(execution.kind.as_str()),
        payload: json!({
            "execution_id": execution.id,
            "tenant_id": execution.tenant_id,
            "kind": execution.kind.as_str(),
            "status": execution.status.as_str(),
            "stage": execution.stage,
            "event_name": persisted_event.event_name,
            "event_sequence_no": persisted_event.sequence_no,
        }),
        published_at: persisted_event.created_at,
    };
    event_bus.publish(execution_event).await;

    for task in persisted_tasks {
        let task_event = EventEnvelope {
            subject: workflow_task_enqueued_subject(&task.queue, &task.task_key),
            payload: json!({
                "task_id": task.id,
                "tenant_id": task.tenant_id,
                "execution_id": task.execution_id,
                "queue": task.queue,
                "task_key": task.task_key,
                "status": task.status.as_str(),
                "available_at": task.available_at,
            }),
            published_at: task.created_at,
        };
        event_bus.publish(task_event).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{
        DatasetId, ReportPlanId, TenantId, WorkflowExecutionId, WorkflowKind, WorkflowStatus,
        WorkflowTaskId, WorkflowTaskStatus,
    };

    fn sample_execution() -> WorkflowExecution {
        let now = Utc::now();
        WorkflowExecution {
            id: WorkflowExecutionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: Some(DatasetId::new()),
            report_plan_id: Some(ReportPlanId::new()),
            kind: WorkflowKind::ReportPlan,
            version: "0.1.0".to_string(),
            stage: "plan_report_ast".to_string(),
            status: WorkflowStatus::Running,
            attempt: 1,
            context: json!({}),
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn publish_transition_events_emits_execution_and_task_notifications() {
        let now = Utc::now();
        let event_bus = EventBus::in_memory();
        let execution = sample_execution();
        let persisted_event = WorkflowEventRecord {
            id: domain_model::WorkflowEventId::new(),
            execution_id: execution.id,
            sequence_no: 2,
            event_name: "workflow.started".to_string(),
            payload: json!({ "task_key": "plan_report_ast" }),
            created_at: now,
        };
        let task = WorkflowTask {
            id: WorkflowTaskId::new(),
            tenant_id: execution.tenant_id,
            execution_id: execution.id,
            queue: "report".to_string(),
            task_key: "plan_report_ast".to_string(),
            payload: json!({ "execution_id": execution.id }),
            status: WorkflowTaskStatus::Queued,
            attempt: 0,
            max_attempts: 3,
            available_at: now,
            claimed_at: None,
            finished_at: None,
            error: None,
            created_at: now,
            updated_at: now,
        };

        publish_workflow_transition_events(
            &event_bus,
            &execution,
            &persisted_event,
            std::slice::from_ref(&task),
        )
        .await;

        let published = event_bus.published_events();
        assert_eq!(published.len(), 2);
        assert_eq!(
            published[0].subject,
            workflow_execution_transition_subject(execution.kind.as_str())
        );
        assert_eq!(
            published[0].payload["event_name"],
            json!("workflow.started")
        );
        assert_eq!(
            published[1].subject,
            workflow_task_enqueued_subject("report", "plan_report_ast")
        );
        assert_eq!(published[1].payload["task_id"], json!(task.id));
    }

    #[tokio::test]
    async fn publish_transition_events_without_tasks_emits_execution_notification_only() {
        let now = Utc::now();
        let event_bus = EventBus::in_memory();
        let execution = sample_execution();
        let persisted_event = WorkflowEventRecord {
            id: domain_model::WorkflowEventId::new(),
            execution_id: execution.id,
            sequence_no: 3,
            event_name: "workflow.step_completed".to_string(),
            payload: json!({}),
            created_at: now,
        };

        publish_workflow_transition_events(&event_bus, &execution, &persisted_event, &[]).await;

        let published = event_bus.published_events();
        assert_eq!(published.len(), 1);
        assert_eq!(
            published[0].subject,
            workflow_execution_transition_subject(execution.kind.as_str())
        );
        assert_eq!(
            published[0].payload["event_sequence_no"],
            json!(persisted_event.sequence_no)
        );
    }
}
