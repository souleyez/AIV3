use contracts::CreateExternalChannelConnectionRequest;
use uuid::Uuid;

use crate::{
    external_channel_support::validate_external_channel_connection_id,
    external_document_object_support::external_document_parse_dataset_key_component,
    text_normalization::trim_optional, ApiError,
};

pub(crate) fn external_channel_create_connection_id(
    request: &CreateExternalChannelConnectionRequest,
) -> std::result::Result<String, ApiError> {
    let connection_id = trim_optional(request.connection_id.clone()).unwrap_or_else(|| {
        let seed = trim_optional(request.customer_key.clone())
            .or_else(|| trim_optional(request.display_name.clone()))
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        format!(
            "generic-chat-{}",
            external_document_parse_dataset_key_component(&seed)
        )
    });
    validate_external_channel_connection_id(&connection_id)?;
    Ok(connection_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_connection_id_uses_trimmed_explicit_id() {
        let id = external_channel_create_connection_id(&CreateExternalChannelConnectionRequest {
            connection_id: Some(" generic-chat.main_01 ".to_string()),
            ..Default::default()
        })
        .expect("explicit id should be valid");

        assert_eq!(id, "generic-chat.main_01");
    }

    #[test]
    fn create_connection_id_prefers_customer_key_then_display_name_for_slug() {
        let customer_id =
            external_channel_create_connection_id(&CreateExternalChannelConnectionRequest {
                customer_key: Some(" Tenant 001 ".to_string()),
                display_name: Some("Display Name".to_string()),
                ..Default::default()
            })
            .expect("customer key should generate id");
        let display_id =
            external_channel_create_connection_id(&CreateExternalChannelConnectionRequest {
                display_name: Some("Display Name".to_string()),
                ..Default::default()
            })
            .expect("display name should generate id");

        assert_eq!(customer_id, "generic-chat-tenant-001");
        assert_eq!(display_id, "generic-chat-display-name");
    }

    #[test]
    fn create_connection_id_rejects_invalid_explicit_id() {
        let error =
            external_channel_create_connection_id(&CreateExternalChannelConnectionRequest {
                connection_id: Some("generic/chat".to_string()),
                ..Default::default()
            })
            .expect_err("invalid explicit id should fail");

        assert_eq!(error.payload.code, "external_channel_connection_id_invalid");
    }
}
