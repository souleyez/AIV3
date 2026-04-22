use std::collections::BTreeMap;

pub const DATASET_OUTPUT_PLACEHOLDER_PROMPT_KEY: &str = "dataset_output.placeholder";
pub const CHAT_SESSION_PLACEHOLDER_PROMPT_KEY: &str = "chat_session.placeholder";

#[derive(Clone, Debug)]
pub struct PromptDefinition {
    pub key: String,
    pub surface: String,
    pub active_version: String,
    pub body: String,
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryPromptRegistry {
    prompts: BTreeMap<String, PromptDefinition>,
}

impl InMemoryPromptRegistry {
    pub fn register(&mut self, prompt: PromptDefinition) {
        self.prompts.insert(prompt.key.clone(), prompt);
    }

    pub fn active(&self, key: &str) -> Option<&PromptDefinition> {
        self.prompts.get(key)
    }

    pub fn list(&self) -> Vec<&PromptDefinition> {
        self.prompts.values().collect()
    }
}

pub fn bootstrap_default_prompt_registry() -> InMemoryPromptRegistry {
    let mut registry = InMemoryPromptRegistry::default();
    registry.register(PromptDefinition {
        key: DATASET_OUTPUT_PLACEHOLDER_PROMPT_KEY.to_string(),
        surface: "dataset_output".to_string(),
        active_version: "v1".to_string(),
        body: "Placeholder dataset output system prompt".to_string(),
    });
    registry.register(PromptDefinition {
        key: CHAT_SESSION_PLACEHOLDER_PROMPT_KEY.to_string(),
        surface: "chat_session".to_string(),
        active_version: "v1".to_string(),
        body: "Placeholder chat session system prompt".to_string(),
    });
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_default_prompt_registry_registers_placeholder_prompts() {
        let registry = bootstrap_default_prompt_registry();

        assert_eq!(
            registry
                .active(DATASET_OUTPUT_PLACEHOLDER_PROMPT_KEY)
                .map(|prompt| prompt.active_version.as_str()),
            Some("v1")
        );
        assert_eq!(
            registry
                .active(CHAT_SESSION_PLACEHOLDER_PROMPT_KEY)
                .map(|prompt| prompt.surface.as_str()),
            Some("chat_session")
        );
    }
}
