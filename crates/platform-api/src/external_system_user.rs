use super::{external_document_parse_dataset_key_component, sha256_hex};

const EXTERNAL_SYSTEM_USER_EMAIL_DOMAIN: &str = "aidp.local";
const EXTERNAL_SYSTEM_USER_SLUG_LIMIT: usize = 24;

pub(crate) fn external_system_user_email(scope_kind: &str, scope_id: &str) -> String {
    let scope_kind = external_document_parse_dataset_key_component(scope_kind);
    let scope_id = scope_id.trim();
    let mut slug = external_document_parse_dataset_key_component(scope_id);
    if slug.len() > EXTERNAL_SYSTEM_USER_SLUG_LIMIT {
        slug.truncate(EXTERNAL_SYSTEM_USER_SLUG_LIMIT);
        while slug.ends_with('-') {
            slug.pop();
        }
    }
    let hash = sha256_hex([scope_kind.as_bytes(), b":", scope_id.as_bytes()]);
    format!(
        "third-party-{}-{}-{}@{}",
        scope_kind,
        slug,
        &hash[..12],
        EXTERNAL_SYSTEM_USER_EMAIL_DOMAIN
    )
}

pub(crate) fn external_system_user_display_name(display_label: &str, fallback: &str) -> String {
    let label = display_label.trim();
    let label = if label.is_empty() {
        fallback.trim()
    } else {
        label
    };
    if label.is_empty() {
        "第三方系统账户".to_string()
    } else {
        format!("第三方系统账户 - {label}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_system_user_email_is_stable_scoped_and_hashed() {
        let left = external_system_user_email("channel", "generic-chat-main");
        let right = external_system_user_email("channel", "another-chat-main");
        let legacy_channel_source = external_system_user_email(
            "channel-source",
            "generic-chat-main:third-party-source-main",
        );

        assert_ne!(left, right);
        assert!(left.ends_with("@aidp.local"));
        assert_eq!(
            left,
            external_system_user_email("channel", "generic-chat-main")
        );
        assert_eq!(
            legacy_channel_source,
            "third-party-channel-source-generic-chat-main-third-7463c6883750@aidp.local"
        );
    }

    #[test]
    fn external_system_user_display_name_prefers_label_then_fallback() {
        assert_eq!(
            external_system_user_display_name(" generic-chat-main ", "fallback"),
            "第三方系统账户 - generic-chat-main"
        );
        assert_eq!(
            external_system_user_display_name("   ", " fallback "),
            "第三方系统账户 - fallback"
        );
        assert_eq!(
            external_system_user_display_name("   ", "   "),
            "第三方系统账户"
        );
    }
}
