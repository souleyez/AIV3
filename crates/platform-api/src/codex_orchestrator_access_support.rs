const ORCHESTRATOR_ACCESS_KEY_ENV: &str = "CODEX_ORCHESTRATOR_ACCESS_KEY";
const ORCHESTRATOR_KEY_FILE_ENV: &str = "CODEX_ORCHESTRATOR_KEY_FILE";

pub(crate) fn orchestrator_access_configured() -> bool {
    orchestrator_access_configured_from(|key| std::env::var(key).ok())
}

fn orchestrator_access_configured_from(read_env: impl Fn(&str) -> Option<String>) -> bool {
    [ORCHESTRATOR_ACCESS_KEY_ENV, ORCHESTRATOR_KEY_FILE_ENV]
        .into_iter()
        .filter_map(read_env)
        .any(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn configured_with(values: &[(&str, &str)]) -> bool {
        let values = values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect::<HashMap<_, _>>();
        orchestrator_access_configured_from(|key| values.get(key).cloned())
    }

    #[test]
    fn returns_false_when_access_key_and_key_file_are_missing() {
        assert!(!configured_with(&[]));
    }

    #[test]
    fn returns_false_when_access_key_and_key_file_are_blank() {
        assert!(!configured_with(&[
            (ORCHESTRATOR_ACCESS_KEY_ENV, "  "),
            (ORCHESTRATOR_KEY_FILE_ENV, "\t\n"),
        ]));
    }

    #[test]
    fn returns_true_when_access_key_is_non_blank() {
        assert!(configured_with(&[(
            ORCHESTRATOR_ACCESS_KEY_ENV,
            "test-key"
        )]));
    }

    #[test]
    fn returns_true_when_key_file_is_non_blank() {
        assert!(configured_with(&[(
            ORCHESTRATOR_KEY_FILE_ENV,
            " /secure/key.txt "
        )]));
    }
}
