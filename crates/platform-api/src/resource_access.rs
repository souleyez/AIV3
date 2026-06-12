use crate::ApiError;
use domain_model::UserId;

pub(crate) fn owner_user_id_is_visible(
    owner_user_id: Option<UserId>,
    current_user_id: Option<UserId>,
) -> bool {
    owner_user_id.is_none() || owner_user_id == current_user_id
}

pub(crate) fn ensure_owner_managed_resource(
    resource_kind: &'static str,
    resource_id: String,
    owner_user_id: Option<UserId>,
    current_user_id: Option<UserId>,
) -> std::result::Result<(), ApiError> {
    if owner_user_id.is_some() && owner_user_id != current_user_id {
        return Err(ApiError::not_found(
            &format!("{resource_kind}_not_found"),
            format!("{resource_kind} {resource_id} was not found"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_visibility_preserves_public_and_owner_match_semantics() {
        let owner_user_id = UserId::new();

        assert!(owner_user_id_is_visible(None, None));
        assert!(owner_user_id_is_visible(None, Some(UserId::new())));
        assert!(owner_user_id_is_visible(
            Some(owner_user_id),
            Some(owner_user_id)
        ));
        assert!(!owner_user_id_is_visible(Some(owner_user_id), None));
        assert!(!owner_user_id_is_visible(
            Some(owner_user_id),
            Some(UserId::new())
        ));
    }

    #[test]
    fn owner_managed_resource_preserves_not_found_masking_semantics() {
        let owner_user_id = UserId::new();

        assert!(ensure_owner_managed_resource("dataset", "public".to_string(), None, None).is_ok());
        assert!(ensure_owner_managed_resource(
            "dataset",
            owner_user_id.to_string(),
            Some(owner_user_id),
            Some(owner_user_id)
        )
        .is_ok());

        let error = ensure_owner_managed_resource(
            "dataset",
            owner_user_id.to_string(),
            Some(owner_user_id),
            Some(UserId::new()),
        )
        .expect_err("non-owner should see a masked not_found error");
        assert_eq!(error.status, axum::http::StatusCode::NOT_FOUND);
        assert_eq!(error.payload.code, "dataset_not_found");
        assert!(error
            .payload
            .message
            .contains(&format!("dataset {owner_user_id} was not found")));
    }
}
