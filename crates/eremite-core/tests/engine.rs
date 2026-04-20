use std::path::Path;
use std::sync::atomic::AtomicBool;

use anyhow::Result;
use eremite_inference::{ChatMessage, InferenceEvent, InferenceParams, ModelMetadata};

use eremite_core::inference::InferenceProvider;
use eremite_core::{CoreConfig, CoreEngine, ConversationId};

/// Mock inference provider that returns canned responses without requiring
/// a GPU or GGUF model.
struct MockInference {
    metadata: Option<ModelMetadata>,
    /// Tracks the messages passed to the last `generate_chat` call.
    last_messages: Vec<ChatMessage>,
    response: String,
    /// `n_ctx_train` reported by `load_model`. Lets tests drive the auto-sizing
    /// logic without touching production defaults.
    n_ctx_train: u32,
    /// Every `count_prompt_tokens` call records its argument here so tests can
    /// assert trimming behaviour without running real inference.
    token_count_invocations: std::cell::RefCell<Vec<Vec<(String, String)>>>,
}

impl MockInference {
    fn new(response: impl Into<String>) -> Self {
        Self {
            metadata: None,
            last_messages: Vec::new(),
            response: response.into(),
            n_ctx_train: 2048,
            token_count_invocations: std::cell::RefCell::new(Vec::new()),
        }
    }

    fn with_ctx_train(mut self, n_ctx_train: u32) -> Self {
        self.n_ctx_train = n_ctx_train;
        self
    }
}

impl InferenceProvider for MockInference {
    fn load_model(&mut self, path: &Path, _params: &InferenceParams) -> Result<ModelMetadata> {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("mock-model")
            .to_string();
        let metadata = ModelMetadata {
            description: name,
            n_params: 1_000_000,
            n_ctx_train: self.n_ctx_train,
        };
        self.metadata = Some(metadata.clone());
        Ok(metadata)
    }

    fn generate_chat(
        &mut self,
        messages: &[ChatMessage],
        _params: &InferenceParams,
        on_event: &mut dyn FnMut(InferenceEvent),
        _shutdown: &AtomicBool,
    ) -> Result<String> {
        self.last_messages = messages
            .iter()
            .map(|m| ChatMessage::new(&m.role, &m.content))
            .collect();

        for word in self.response.split_whitespace() {
            on_event(InferenceEvent::Token(format!("{word} ")));
        }
        on_event(InferenceEvent::Done {
            tokens_generated: self.response.split_whitespace().count() as u32,
            duration_ms: 42,
        });

        Ok(self.response.clone())
    }

    fn generate(
        &mut self,
        _prompt: &str,
        _params: &InferenceParams,
        on_event: &mut dyn FnMut(InferenceEvent),
        _shutdown: &AtomicBool,
    ) -> Result<String> {
        on_event(InferenceEvent::Token(self.response.clone()));
        on_event(InferenceEvent::Done {
            tokens_generated: 1,
            duration_ms: 1,
        });
        Ok(self.response.clone())
    }

    fn count_prompt_tokens(&self, messages: &[ChatMessage]) -> Result<usize> {
        self.token_count_invocations.borrow_mut().push(
            messages
                .iter()
                .map(|m| (m.role.clone(), m.content.clone()))
                .collect(),
        );
        // Cheap deterministic proxy: 1 token per character of content plus a
        // fixed 4-token overhead per message to approximate chat-template
        // boilerplate. Enough to make budget arithmetic meaningful in tests.
        let total: usize = messages
            .iter()
            .map(|m| m.content.chars().count() + 4)
            .sum();
        Ok(total)
    }

    fn model_metadata(&self) -> Option<&ModelMetadata> {
        self.metadata.as_ref()
    }
}

fn make_engine(response: &str) -> CoreEngine<MockInference> {
    CoreEngine::new(MockInference::new(response), CoreConfig::default())
}

fn no_shutdown() -> AtomicBool {
    AtomicBool::new(false)
}

// -- Tests ----------------------------------------------------------------

#[test]
fn create_conversation_returns_unique_ids() {
    let mut engine = make_engine("hello");
    let id1 = engine.create_conversation(None);
    let id2 = engine.create_conversation(None);
    assert_ne!(id1, id2);
}

#[test]
fn create_conversation_sets_active() {
    let mut engine = make_engine("hello");
    let id = engine.create_conversation(None);
    assert_eq!(engine.active_conversation(), Some(id));
}

#[test]
fn conversation_lookup() {
    let mut engine = make_engine("hello");
    let id = engine.create_conversation(Some("Be concise.".to_string()));

    let conv = engine.conversation(id).unwrap();
    assert_eq!(conv.system_prompt(), Some("Be concise."));
    assert!(conv.messages().is_empty());
}

#[test]
fn conversation_not_found() {
    let engine = make_engine("hello");
    let fake_id = ConversationId::new();
    assert!(engine.conversation(fake_id).is_none());
}

#[test]
fn delete_conversation() {
    let mut engine = make_engine("hello");
    let id = engine.create_conversation(None);
    assert!(engine.delete_conversation(id));
    assert!(engine.conversation(id).is_none());
    assert_eq!(engine.active_conversation(), None);
}

#[test]
fn delete_nonexistent_conversation() {
    let mut engine = make_engine("hello");
    let fake_id = ConversationId::new();
    assert!(!engine.delete_conversation(fake_id));
}

#[test]
fn send_message_adds_user_and_assistant_messages() {
    let mut engine = make_engine("I'm doing well, thanks!");
    let id = engine.create_conversation(None);
    let shutdown = no_shutdown();

    let mut events = Vec::new();
    let response = engine
        .send_message(id, "How are you?", &mut |e| events.push(e), &shutdown)
        .unwrap();

    assert_eq!(response, "I'm doing well, thanks!");

    let conv = engine.conversation(id).unwrap();
    assert_eq!(conv.messages().len(), 2);
    assert_eq!(conv.messages()[0].role, "user");
    assert_eq!(conv.messages()[0].content, "How are you?");
    assert_eq!(conv.messages()[1].role, "assistant");
    assert_eq!(conv.messages()[1].content, "I'm doing well, thanks!");
}

#[test]
fn send_message_streams_token_events() {
    let mut engine = make_engine("hello world");
    let id = engine.create_conversation(None);
    let shutdown = no_shutdown();

    let mut tokens = Vec::new();
    let mut got_done = false;
    engine
        .send_message(id, "hi", &mut |e| match e {
            InferenceEvent::Token(t) => tokens.push(t),
            InferenceEvent::Done { .. } => got_done = true,
        }, &shutdown)
        .unwrap();

    assert_eq!(tokens, vec!["hello ", "world "]);
    assert!(got_done);
}

#[test]
fn send_message_to_nonexistent_conversation_returns_error() {
    let mut engine = make_engine("hello");
    let fake_id = ConversationId::new();
    let shutdown = no_shutdown();

    let result = engine.send_message(fake_id, "hi", &mut |_| {}, &shutdown);
    assert!(result.is_err());
}

#[test]
fn conversation_history_accumulates() {
    let mut engine = make_engine("response");
    let id = engine.create_conversation(None);
    let shutdown = no_shutdown();

    engine.send_message(id, "first", &mut |_| {}, &shutdown).unwrap();
    engine.send_message(id, "second", &mut |_| {}, &shutdown).unwrap();

    let conv = engine.conversation(id).unwrap();
    assert_eq!(conv.messages().len(), 4);
    assert_eq!(conv.messages()[0].content, "first");
    assert_eq!(conv.messages()[1].content, "response");
    assert_eq!(conv.messages()[2].content, "second");
    assert_eq!(conv.messages()[3].content, "response");
}

#[test]
fn system_prompt_included_in_conversation() {
    let mut engine = make_engine("ok");
    let id = engine.create_conversation(Some("You are helpful.".to_string()));
    let shutdown = no_shutdown();

    engine.send_message(id, "hi", &mut |_| {}, &shutdown).unwrap();

    let conv = engine.conversation(id).unwrap();
    assert_eq!(conv.system_prompt(), Some("You are helpful."));

    let chat_messages = conv.to_chat_messages();
    assert_eq!(chat_messages[0].role, "system");
    assert_eq!(chat_messages[0].content, "You are helpful.");
    assert_eq!(chat_messages[1].role, "user");
    assert_eq!(chat_messages[2].role, "assistant");
}

#[test]
fn config_system_prompt_used_as_fallback() {
    let config = CoreConfig {
        system_prompt: Some("Default prompt.".to_string()),
        ..CoreConfig::default()
    };
    let mut engine = CoreEngine::new(MockInference::new("ok"), config);
    let id = engine.create_conversation(None);

    let conv = engine.conversation(id).unwrap();
    assert_eq!(conv.system_prompt(), Some("Default prompt."));
}

#[test]
fn explicit_system_prompt_overrides_config() {
    let config = CoreConfig {
        system_prompt: Some("Default prompt.".to_string()),
        ..CoreConfig::default()
    };
    let mut engine = CoreEngine::new(MockInference::new("ok"), config);
    let id = engine.create_conversation(Some("Custom prompt.".to_string()));

    let conv = engine.conversation(id).unwrap();
    assert_eq!(conv.system_prompt(), Some("Custom prompt."));
}

#[test]
fn load_model_populates_metadata() {
    let mut engine = make_engine("hello");
    assert!(engine.model_metadata().is_none());

    let metadata = engine.load_model(Path::new("/fake/test-model.gguf")).unwrap();
    assert_eq!(metadata.description, "test-model");
    assert!(engine.model_metadata().is_some());
}

#[test]
fn multiple_conversations_are_independent() {
    let mut engine = make_engine("reply");
    let shutdown = no_shutdown();

    let id1 = engine.create_conversation(Some("Prompt A".to_string()));
    engine.send_message(id1, "msg1", &mut |_| {}, &shutdown).unwrap();

    let id2 = engine.create_conversation(Some("Prompt B".to_string()));
    engine.send_message(id2, "msg2", &mut |_| {}, &shutdown).unwrap();

    let conv1 = engine.conversation(id1).unwrap();
    let conv2 = engine.conversation(id2).unwrap();

    assert_eq!(conv1.system_prompt(), Some("Prompt A"));
    assert_eq!(conv1.messages().len(), 2);
    assert_eq!(conv1.messages()[0].content, "msg1");

    assert_eq!(conv2.system_prompt(), Some("Prompt B"));
    assert_eq!(conv2.messages().len(), 2);
    assert_eq!(conv2.messages()[0].content, "msg2");
}

#[test]
fn generate_raw_prompt() {
    let mut engine = make_engine("generated text");
    let shutdown = no_shutdown();

    let mut events = Vec::new();
    let result = engine.generate("Tell me a joke", &mut |e| events.push(e), &shutdown).unwrap();

    assert_eq!(result, "generated text");
    assert!(!events.is_empty());
}

#[test]
fn set_active_conversation() {
    let mut engine = make_engine("hello");
    let id1 = engine.create_conversation(None);
    let id2 = engine.create_conversation(None);

    assert_eq!(engine.active_conversation(), Some(id2));
    engine.set_active_conversation(id1).unwrap();
    assert_eq!(engine.active_conversation(), Some(id1));
}

#[test]
fn set_active_conversation_nonexistent_returns_error() {
    let mut engine = make_engine("hello");
    let fake_id = ConversationId::new();
    assert!(engine.set_active_conversation(fake_id).is_err());
}

// -- Context auto-sizing + sliding-window trim ---------------------------

#[test]
fn load_model_auto_sizes_n_ctx_from_metadata() {
    let mock = MockInference::new("ok").with_ctx_train(8_192);
    let mut engine = CoreEngine::new(mock, CoreConfig::default());

    engine.load_model(Path::new("/fake/model.gguf")).unwrap();
    assert_eq!(engine.config().inference_params.n_ctx, 8_192);
}

#[test]
fn load_model_caps_huge_trained_contexts() {
    let mock = MockInference::new("ok").with_ctx_train(131_072);
    let mut engine = CoreEngine::new(mock, CoreConfig::default());

    engine.load_model(Path::new("/fake/model.gguf")).unwrap();
    assert_eq!(
        engine.config().inference_params.n_ctx,
        eremite_core::DEFAULT_CTX_CAP
    );
}

#[test]
fn load_model_override_wins_over_auto_sizing() {
    let mock = MockInference::new("ok").with_ctx_train(8_192);
    let config = CoreConfig {
        ctx_size_override: Some(32_768),
        ..CoreConfig::default()
    };
    let mut engine = CoreEngine::new(mock, config);

    engine.load_model(Path::new("/fake/model.gguf")).unwrap();
    assert_eq!(engine.config().inference_params.n_ctx, 32_768);
}

#[test]
fn send_message_trims_old_history_but_keeps_it_in_conversation() {
    // Tiny context so the sliding window actually triggers. With the mock
    // counter returning `len(content) + 4` per message, a budget of around
    // 30 tokens keeps roughly 2-3 messages in the model's view.
    let mut config = CoreConfig::default();
    config.inference_params.n_ctx = 256;
    config.inference_params.max_tokens = 96;
    // Budget = 256 - 96 - 128 = 32 tokens for history.
    let mut engine = CoreEngine::new(MockInference::new("ok"), config);

    let id = engine.create_conversation(Some("sys".to_string()));
    let shutdown = no_shutdown();

    for i in 0..6 {
        engine
            .send_message(
                id,
                &format!("user message number {i} with padding"),
                &mut |_| {},
                &shutdown,
            )
            .unwrap();
    }

    // Full history is preserved in the conversation (12 messages: 6 user + 6
    // assistant).
    let conv = engine.conversation(id).unwrap();
    assert_eq!(conv.messages().len(), 12);

    // But the last call into the inference layer received a trimmed list:
    // strictly smaller than 1 (system) + 11 (history up to and including the
    // latest user message, not yet answered) = 12. Also, the first trimmed
    // message must be the system prompt and the last must be the most recent
    // user message.
    let last_sent = &engine.inference().last_messages;
    assert!(
        last_sent.len() < 12,
        "expected sliding window to drop older turns, got {} messages",
        last_sent.len()
    );
    assert_eq!(last_sent.first().unwrap().role, "system");
    assert_eq!(last_sent.first().unwrap().content, "sys");
    let latest = last_sent.last().unwrap();
    assert_eq!(latest.role, "user");
    assert_eq!(latest.content, "user message number 5 with padding");

    // And the trimmer actually invoked count_prompt_tokens at least once.
    assert!(!engine
        .inference()
        .token_count_invocations
        .borrow()
        .is_empty());
}
