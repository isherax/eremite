/// Parameters controlling text generation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InferenceParams {
    /// Maximum number of tokens to generate.
    pub max_tokens: u32,
    /// Sampling temperature. Higher values produce more random output.
    /// When <= 0.0, greedy sampling is used.
    pub temperature: f32,
    /// Nucleus sampling threshold. Only tokens with cumulative probability
    /// above this value are considered.
    pub top_p: f32,
    /// Top-k sampling. Only the top k tokens are considered.
    pub top_k: u32,
    /// Random seed for reproducible generation. `None` uses a default seed.
    pub seed: Option<u32>,
    /// Context window size in tokens.
    pub n_ctx: u32,
    /// Number of model layers to offload to GPU. Use a large value (e.g. 1000)
    /// to offload all layers.
    pub n_gpu_layers: u32,
}

impl Default for InferenceParams {
    fn default() -> Self {
        Self {
            max_tokens: 512,
            temperature: 0.8,
            top_p: 0.95,
            top_k: 40,
            seed: None,
            n_ctx: 2048,
            n_gpu_layers: 1000,
        }
    }
}

/// A single message in a chat conversation.
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self::new("system", content)
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::new("user", content)
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new("assistant", content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_params_are_sensible() {
        let params = InferenceParams::default();
        assert_eq!(params.max_tokens, 512);
        assert!((params.temperature - 0.8).abs() < f32::EPSILON);
        assert!((params.top_p - 0.95).abs() < f32::EPSILON);
        assert_eq!(params.top_k, 40);
        assert!(params.seed.is_none());
        assert_eq!(params.n_ctx, 2048);
        assert_eq!(params.n_gpu_layers, 1000);
    }

    #[test]
    fn chat_message_constructors() {
        let system = ChatMessage::system("You are helpful.");
        assert_eq!(system.role, "system");
        assert_eq!(system.content, "You are helpful.");

        let user = ChatMessage::user("Hello");
        assert_eq!(user.role, "user");
        assert_eq!(user.content, "Hello");

        let assistant = ChatMessage::assistant("Hi there!");
        assert_eq!(assistant.role, "assistant");
        assert_eq!(assistant.content, "Hi there!");
    }

    #[test]
    fn chat_message_new_accepts_str_and_string() {
        let from_str = ChatMessage::new("user", "hello");
        assert_eq!(from_str.role, "user");

        let from_string = ChatMessage::new(String::from("user"), String::from("hello"));
        assert_eq!(from_string.role, "user");
    }
}
