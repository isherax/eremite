use eremite_inference::InferenceParams;
use serde::{Deserialize, Serialize};

/// Application-level configuration for the core engine.
///
/// Wraps inference parameter defaults and an optional system prompt that is
/// prepended to every new conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    pub inference_params: InferenceParams,
    pub system_prompt: Option<String>,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            inference_params: InferenceParams::default(),
            system_prompt: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_uses_default_inference_params() {
        let config = CoreConfig::default();
        let params = InferenceParams::default();
        assert_eq!(config.inference_params.max_tokens, params.max_tokens);
        assert!((config.inference_params.temperature - params.temperature).abs() < f32::EPSILON);
        assert!(config.system_prompt.is_none());
    }

    #[test]
    fn config_with_system_prompt() {
        let config = CoreConfig {
            system_prompt: Some("You are a helpful assistant.".to_string()),
            ..CoreConfig::default()
        };
        assert_eq!(
            config.system_prompt.as_deref(),
            Some("You are a helpful assistant.")
        );
    }

    #[test]
    fn config_round_trip_json() {
        let config = CoreConfig {
            system_prompt: Some("Be concise.".to_string()),
            ..CoreConfig::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: CoreConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.system_prompt, config.system_prompt);
        assert_eq!(
            deserialized.inference_params.max_tokens,
            config.inference_params.max_tokens
        );
    }
}
