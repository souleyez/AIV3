use contracts::ChatSessionView;
use domain_model::{ChatSession, DatasetId, SecretBindingId, UserId};

use crate::{
    hydrate_chat_session_view, load_visible_dataset_for_user,
    resource_access::owner_user_id_is_visible, ApiError, AppState,
};

pub(crate) async fn list_chat_session_views_for_user(
    state: &AppState,
    dataset_id: DatasetId,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
) -> std::result::Result<Vec<ChatSessionView>, ApiError> {
    load_visible_dataset_for_user(
        state,
        dataset_id,
        active_secret_binding_ids,
        current_user_id,
    )
    .await?;

    let sessions = state
        .storage
        .chat_sessions()
        .list_by_dataset(state.tenant_id, dataset_id)
        .await
        .map_err(ApiError::from_storage)?;
    let sessions = filter_chat_sessions_visible_to_user(sessions, current_user_id);
    let mut views = Vec::with_capacity(sessions.len());
    for session in sessions {
        views.push(hydrate_chat_session_view(state, session).await?);
    }
    Ok(views)
}

fn filter_chat_sessions_visible_to_user(
    sessions: Vec<ChatSession>,
    current_user_id: Option<UserId>,
) -> Vec<ChatSession> {
    sessions
        .into_iter()
        .filter(|session| owner_user_id_is_visible(session.user_id, current_user_id))
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;
    use uuid::Uuid;

    use domain_model::{ChatSessionId, TenantId, WorkflowExecutionId};

    use super::*;

    fn chat_session(dataset_id: DatasetId, user_id: Option<UserId>, title: &str) -> ChatSession {
        ChatSession {
            id: ChatSessionId::new(),
            tenant_id: TenantId(Uuid::from_u128(1)),
            dataset_id,
            user_id,
            execution_id: WorkflowExecutionId::new(),
            title: title.to_string(),
            latest_memory_directory_id: None,
            latest_dataset_output_id: None,
            session_manifest: json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn chat_session_list_filter_keeps_public_and_owned_sessions_only() {
        let dataset_id = DatasetId::new();
        let current_user_id = UserId::new();
        let other_user_id = UserId::new();
        let public = chat_session(dataset_id, None, "public");
        let public_id = public.id;
        let owned = chat_session(dataset_id, Some(current_user_id), "owned");
        let owned_id = owned.id;
        let other = chat_session(dataset_id, Some(other_user_id), "other");

        let visible =
            filter_chat_sessions_visible_to_user(vec![public, other, owned], Some(current_user_id));

        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0].id, public_id);
        assert_eq!(visible[1].id, owned_id);
    }
}
