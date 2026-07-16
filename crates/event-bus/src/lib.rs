use anyhow::Result;
use async_nats::{jetstream, Client, Subscriber};
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, timeout, Duration};

pub const DEFAULT_LOCAL_NATS_URL: &str = "nats://127.0.0.1:4222";

const WORKFLOW_EVENTS_STREAM: &str = "WORKFLOW_EVENTS";
const TASK_ENQUEUED_SUBJECT_PREFIX: &str = "workflow.task.enqueued";
const EXECUTION_TRANSITIONED_SUBJECT_PREFIX: &str = "workflow.execution.transitioned";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EventEnvelope {
    pub subject: String,
    pub payload: Value,
    pub published_at: DateTime<Utc>,
}

#[derive(Clone, Default)]
pub enum EventBus {
    #[default]
    Disabled,
    InMemory(InMemoryEventBus),
    Nats(Arc<NatsEventBus>),
}

impl EventBus {
    pub fn in_memory() -> Self {
        Self::InMemory(InMemoryEventBus::default())
    }

    pub fn is_enabled(&self) -> bool {
        matches!(self, Self::InMemory(_) | Self::Nats(_))
    }

    pub fn published_events(&self) -> Vec<EventEnvelope> {
        match self {
            Self::InMemory(bus) => bus.events(),
            Self::Disabled | Self::Nats(_) => Vec::new(),
        }
    }

    pub async fn connect_from_env_or_disabled(env_key: &str) -> Self {
        let nats_url =
            std::env::var(env_key).unwrap_or_else(|_| DEFAULT_LOCAL_NATS_URL.to_string());
        let nats_endpoint = nats_endpoint_for_logging(&nats_url);

        match NatsEventBus::connect(&nats_url).await {
            Ok(bus) => {
                tracing::info!(%env_key, %nats_endpoint, "event bus connected");
                Self::Nats(Arc::new(bus))
            }
            Err(_error) => {
                tracing::warn!(
                    %env_key,
                    %nats_endpoint,
                    error_kind = "connect_failed",
                    "event bus disabled"
                );
                Self::Disabled
            }
        }
    }

    pub async fn publish(&self, event: EventEnvelope) {
        match self {
            Self::Disabled => {}
            Self::InMemory(bus) => bus.publish(event),
            Self::Nats(bus) => {
                if let Err(error) = bus.publish(event).await {
                    tracing::warn!(error = ?error, "event bus publish failed");
                }
            }
        }
    }

    pub async fn subscribe_queue_or_disabled(
        &self,
        subject: &str,
        queue_group: Option<&str>,
    ) -> EventSubscription {
        match self {
            Self::Disabled | Self::InMemory(_) => EventSubscription::Disabled,
            Self::Nats(bus) => match bus.subscribe_queue(subject, queue_group).await {
                Ok(subscriber) => EventSubscription::Nats(subscriber),
                Err(error) => {
                    tracing::warn!(
                        %subject,
                        queue_group = queue_group.unwrap_or_default(),
                        error = ?error,
                        "event bus subscription disabled"
                    );
                    EventSubscription::Disabled
                }
            },
        }
    }
}

fn nats_endpoint_for_logging(nats_url: &str) -> String {
    observability::redact_connection_endpoint(nats_url)
}

pub enum EventSubscription {
    Disabled,
    Nats(Subscriber),
}

impl EventSubscription {
    pub async fn wait_for_event(&mut self, wait_duration: Duration) -> Option<EventEnvelope> {
        match self {
            Self::Disabled => {
                sleep(wait_duration).await;
                None
            }
            Self::Nats(subscriber) => match timeout(wait_duration, subscriber.next()).await {
                Ok(Some(message)) => {
                    match serde_json::from_slice::<EventEnvelope>(&message.payload) {
                        Ok(event) => Some(event),
                        Err(error) => {
                            tracing::warn!(error = ?error, "event bus received invalid payload");
                            None
                        }
                    }
                }
                Ok(None) | Err(_) => None,
            },
        }
    }
}

#[derive(Clone, Default)]
pub struct InMemoryEventBus {
    published: Arc<Mutex<Vec<EventEnvelope>>>,
}

impl InMemoryEventBus {
    pub fn events(&self) -> Vec<EventEnvelope> {
        self.published.lock().expect("lock poisoned").clone()
    }

    fn publish(&self, event: EventEnvelope) {
        self.published.lock().expect("lock poisoned").push(event);
    }
}

pub struct NatsEventBus {
    client: Client,
}

impl NatsEventBus {
    async fn connect(nats_url: &str) -> Result<Self> {
        let client = async_nats::connect(nats_url).await?;
        let jetstream = jetstream::new(client.clone());
        let _stream = jetstream
            .get_or_create_stream(jetstream::stream::Config {
                name: WORKFLOW_EVENTS_STREAM.to_string(),
                subjects: vec![
                    format!("{TASK_ENQUEUED_SUBJECT_PREFIX}.>"),
                    format!("{EXECUTION_TRANSITIONED_SUBJECT_PREFIX}.>"),
                ],
                ..Default::default()
            })
            .await?;

        Ok(Self { client })
    }

    async fn publish(&self, event: EventEnvelope) -> Result<()> {
        let payload = serde_json::to_vec(&event)?;
        self.client.publish(event.subject, payload.into()).await?;
        self.client.flush().await?;
        Ok(())
    }

    async fn subscribe_queue(
        &self,
        subject: &str,
        queue_group: Option<&str>,
    ) -> Result<Subscriber> {
        let subscriber = match queue_group {
            Some(group) if !group.is_empty() => {
                self.client
                    .queue_subscribe(subject.to_string(), group.to_string())
                    .await?
            }
            Some(_) | None => self.client.subscribe(subject.to_string()).await?,
        };

        Ok(subscriber)
    }
}

pub fn workflow_task_enqueued_subject(queue: &str, task_key: &str) -> String {
    format!(
        "{TASK_ENQUEUED_SUBJECT_PREFIX}.{}.{}",
        subject_token(queue),
        subject_token(task_key)
    )
}

pub fn workflow_execution_transition_subject(workflow_kind: &str) -> String {
    format!(
        "{EXECUTION_TRANSITIONED_SUBJECT_PREFIX}.{}",
        subject_token(workflow_kind)
    )
}

fn subject_token(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' => ch,
            _ => '_',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn in_memory_bus_records_published_events() {
        let bus = EventBus::in_memory();
        let event = EventEnvelope {
            subject: workflow_task_enqueued_subject("report", "plan_report_ast"),
            payload: json!({ "execution_id": "ex_123" }),
            published_at: Utc::now(),
        };

        bus.publish(event.clone()).await;

        assert_eq!(bus.published_events(), vec![event]);
    }

    #[test]
    fn workflow_subjects_are_normalized_for_nats() {
        assert_eq!(
            workflow_task_enqueued_subject("report.queue", "plan report ast"),
            "workflow.task.enqueued.report_queue.plan_report_ast"
        );
        assert_eq!(
            workflow_execution_transition_subject("report_render_workflow"),
            "workflow.execution.transitioned.report_render_workflow"
        );
    }

    #[test]
    fn event_bus_diagnostic_endpoint_never_contains_credentials() {
        let raw =
            "nats://pilot%40user:s%40per-secret@nats.internal:4222/private?token=hidden#credential";

        let endpoint = nats_endpoint_for_logging(raw);

        assert_eq!(endpoint, "nats://nats.internal:4222");
        assert!(!endpoint.contains("pilot"));
        assert!(!endpoint.contains("secret"));
        assert!(!endpoint.contains("private"));
        assert!(!endpoint.contains("hidden"));
        assert!(!endpoint.contains("credential"));
        assert_eq!(
            nats_endpoint_for_logging("secret-only-value"),
            "<redacted-endpoint>"
        );
    }
}
