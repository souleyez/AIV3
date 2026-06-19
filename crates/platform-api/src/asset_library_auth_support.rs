use axum::http::HeaderMap;
use domain_model::User;

use crate::{current_auth_session, ApiError, AppState};

pub(crate) async fn require_asset_library_user_session(
    state: &AppState,
    headers: &HeaderMap,
) -> std::result::Result<User, ApiError> {
    let Some((user, _session)) = current_auth_session(state, headers).await? else {
        return Err(asset_library_auth_session_required_error());
    };
    Ok(user)
}

fn asset_library_auth_session_required_error() -> ApiError {
    ApiError::unauthorized(
        "auth_session_required",
        "请先登录主系统后再管理企业资产库".to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_library_auth_support_uses_existing_session_required_error() {
        let error = asset_library_auth_session_required_error();

        assert_eq!(error.payload.code, "auth_session_required");
        assert_eq!(error.status, axum::http::StatusCode::UNAUTHORIZED);
        assert!(error.payload.message.contains("请先登录主系统"));
    }
}
