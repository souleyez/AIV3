use contracts::ExternalRequestedSkillView;
use serde_json::json;

use crate::{
    ApiError, EXTERNAL_CHANNEL_REQUESTED_SKILL_ARGUMENTS_LIMIT,
    EXTERNAL_CHANNEL_REQUESTED_SKILL_ID_LIMIT, EXTERNAL_CHANNEL_REQUESTED_SKILL_LIMIT,
    EXTERNAL_CHANNEL_REQUESTED_SKILL_MODE_LIMIT, EXTERNAL_CHANNEL_REQUESTED_SKILL_VERSION_LIMIT,
};

pub(crate) fn validate_and_normalize_external_requested_skills(
    requested_skills: &mut Vec<ExternalRequestedSkillView>,
) -> std::result::Result<(), ApiError> {
    if requested_skills.len() > EXTERNAL_CHANNEL_REQUESTED_SKILL_LIMIT {
        return Err(external_requested_skills_bad_request(
            "too_many_requested_skills",
            format!(
                "requested_skills accepts at most {EXTERNAL_CHANNEL_REQUESTED_SKILL_LIMIT} items"
            ),
        ));
    }

    for (index, skill) in requested_skills.iter_mut().enumerate() {
        skill.skill_id = skill.skill_id.trim().to_string();
        if skill.skill_id.is_empty() {
            return Err(external_requested_skills_bad_request(
                "empty_skill_id",
                format!("requested_skills[{index}].skill_id must be a non-empty string"),
            ));
        }
        if skill.skill_id.chars().count() > EXTERNAL_CHANNEL_REQUESTED_SKILL_ID_LIMIT
            || skill.skill_id.chars().any(char::is_control)
        {
            return Err(external_requested_skills_bad_request(
                "invalid_skill_id",
                format!(
                    "requested_skills[{index}].skill_id must be printable text within {EXTERNAL_CHANNEL_REQUESTED_SKILL_ID_LIMIT} characters"
                ),
            ));
        }

        if let Some(version) = skill.version.take() {
            let normalized = version.trim().to_string();
            if normalized.is_empty() {
                skill.version = None;
            } else if normalized.chars().count() > EXTERNAL_CHANNEL_REQUESTED_SKILL_VERSION_LIMIT
                || normalized.chars().any(char::is_control)
            {
                return Err(external_requested_skills_bad_request(
                    "invalid_skill_version",
                    format!(
                        "requested_skills[{index}].version must be printable text within {EXTERNAL_CHANNEL_REQUESTED_SKILL_VERSION_LIMIT} characters"
                    ),
                ));
            } else {
                skill.version = Some(normalized);
            }
        }

        if let Some(mode) = skill.mode.take() {
            let normalized = mode.trim().to_ascii_lowercase();
            if normalized.is_empty() {
                skill.mode = None;
            } else if normalized.chars().count() > EXTERNAL_CHANNEL_REQUESTED_SKILL_MODE_LIMIT
                || !matches!(normalized.as_str(), "required" | "preferred" | "disabled")
            {
                return Err(external_requested_skills_bad_request(
                    "invalid_skill_mode",
                    format!(
                        "requested_skills[{index}].mode must be one of required, preferred, or disabled"
                    ),
                ));
            } else {
                skill.mode = Some(normalized);
            }
        }

        if let Some(arguments) = skill.arguments.as_ref() {
            if !arguments.is_object() {
                return Err(external_requested_skills_bad_request(
                    "invalid_skill_arguments",
                    format!("requested_skills[{index}].arguments must be a JSON object"),
                ));
            }
            if arguments.to_string().chars().count()
                > EXTERNAL_CHANNEL_REQUESTED_SKILL_ARGUMENTS_LIMIT
            {
                return Err(external_requested_skills_bad_request(
                    "skill_arguments_too_large",
                    format!(
                        "requested_skills[{index}].arguments must fit within {EXTERNAL_CHANNEL_REQUESTED_SKILL_ARGUMENTS_LIMIT} JSON characters"
                    ),
                ));
            }
        }
    }

    Ok(())
}

pub(crate) fn external_requested_skills_bad_request(
    reason: &str,
    message: impl Into<String>,
) -> ApiError {
    ApiError::bad_request_with_details(
        "external_channel_requested_skills_invalid",
        message.into(),
        json!({
            "reason": reason,
            "schema": {
                "requested_skills": [{
                    "skill_id": "stable skill id",
                    "version": "optional version",
                    "mode": "required | preferred | disabled",
                    "arguments": {}
                }]
            }
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_requested_skill_shape() {
        let mut skills = vec![ExternalRequestedSkillView {
            skill_id: "  report_focus  ".to_string(),
            version: Some("  v2  ".to_string()),
            mode: Some("REQUIRED".to_string()),
            arguments: Some(json!({"focus": "risk"})),
        }];

        validate_and_normalize_external_requested_skills(&mut skills).expect("valid skill");

        assert_eq!(skills[0].skill_id, "report_focus");
        assert_eq!(skills[0].version.as_deref(), Some("v2"));
        assert_eq!(skills[0].mode.as_deref(), Some("required"));
    }

    #[test]
    fn clears_empty_requested_skill_optionals() {
        let mut skills = vec![ExternalRequestedSkillView {
            skill_id: "chat".to_string(),
            version: Some("  ".to_string()),
            mode: Some("\t".to_string()),
            arguments: Some(json!({})),
        }];

        validate_and_normalize_external_requested_skills(&mut skills).expect("valid skill");

        assert_eq!(skills[0].skill_id, "chat");
        assert!(skills[0].version.is_none());
        assert!(skills[0].mode.is_none());
    }

    #[test]
    fn rejects_invalid_requested_skill_shape() {
        let mut invalid_mode = vec![ExternalRequestedSkillView {
            skill_id: "report".to_string(),
            version: None,
            mode: Some("force".to_string()),
            arguments: Some(json!({})),
        }];
        assert!(validate_and_normalize_external_requested_skills(&mut invalid_mode).is_err());

        let mut invalid_arguments = vec![ExternalRequestedSkillView {
            skill_id: "report".to_string(),
            version: None,
            mode: Some("preferred".to_string()),
            arguments: Some(json!([])),
        }];
        assert!(validate_and_normalize_external_requested_skills(&mut invalid_arguments).is_err());
    }
}
