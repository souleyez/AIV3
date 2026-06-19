use crate::ApiError;

pub(crate) fn client_config_package_not_found_error(package_id: &str) -> ApiError {
    ApiError::not_found(
        "client_config_package_not_found",
        format!("client config package {package_id} was not found"),
    )
}

pub(crate) fn client_artifact_not_found_error(artifact_id: &str) -> ApiError {
    ApiError::not_found(
        "client_artifact_not_found",
        format!("client artifact {artifact_id} was not found"),
    )
}

pub(crate) fn client_artifact_file_not_found_error(artifact_id: &str, file_index: i32) -> ApiError {
    ApiError::not_found(
        "client_artifact_file_not_found",
        format!("client artifact file {artifact_id}/{file_index} was not found"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_artifact_error_support_builds_config_package_not_found() {
        let error = client_config_package_not_found_error("pkg-1");

        assert_eq!(error.payload.code, "client_config_package_not_found");
        assert_eq!(
            error.payload.message,
            "client config package pkg-1 was not found"
        );
    }

    #[test]
    fn client_artifact_error_support_builds_artifact_not_found() {
        let error = client_artifact_not_found_error("artifact-1");

        assert_eq!(error.payload.code, "client_artifact_not_found");
        assert_eq!(
            error.payload.message,
            "client artifact artifact-1 was not found"
        );
    }

    #[test]
    fn client_artifact_error_support_builds_file_not_found() {
        let error = client_artifact_file_not_found_error("artifact-1", 3);

        assert_eq!(error.payload.code, "client_artifact_file_not_found");
        assert_eq!(
            error.payload.message,
            "client artifact file artifact-1/3 was not found"
        );
    }
}
