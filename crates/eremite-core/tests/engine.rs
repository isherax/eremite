use std::path::Path;

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
}

impl MockInference {
    fn new(response: impl Into<String>) -> Self {
        Self {
            metadata: None,
            last_messages: Vec::new(),
            response: response.into(),
        }
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
            n_ctx_train: 2048,
        };
        self.metadata = Some(metadata.clone());
        Ok(metadata)
    }

    fn generate_chat(
        &mut self,
        messages: &[ChatMessage],
        _params: &InferenceParams,
        on_event: &mut dyn FnMut(InferenceEvent),
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
    ) -> Result<String> {
        on_event(InferenceEvent::Token(self.response.clone()));
        on_event(InferenceEvent::Done {
            tokens_generated: 1,
            duration_ms: 1,
        });
        Ok(self.response.clone())
    }

    fn model_metadata(&self) -> Option<&ModelMetadata> {
        self.metadata.as_ref()
    }
}

fn make_engine(response: &str) -> CoreEngine<MockInference> {
    CoreEngine::new(MockInference::new(response), CoreConfig::default())
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

    let mut events = Vec::new();
    let response = engine
        .send_message(id, "How are you?", &mut |e| events.push(e))
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

    let mut tokens = Vec::new();
    let mut got_done = false;
    engine
        .send_message(id, "hi", &mut |e| match e {
            InferenceEvent::Token(t) => tokens.push(t),
            InferenceEvent::Done { .. } => got_done = true,
        })
        .unwrap();

    assert_eq!(tokens, vec!["hello ", "world "]);
    assert!(got_done);
}

#[test]
fn send_message_to_nonexistent_conversation_returns_error() {
    let mut engine = make_engine("hello");
    let fake_id = ConversationId::new();

    let result = engine.send_message(fake_id, "hi", &mut |_| {});
    assert!(result.is_err());
}

#[test]
fn conversation_history_accumulates() {
    let mut engine = make_engine("response");
    let id = engine.create_conversation(None);

    engine.send_message(id, "first", &mut |_| {}).unwrap();
    engine.send_message(id, "second", &mut |_| {}).unwrap();

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

    engine.send_message(id, "hi", &mut |_| {}).unwrap();

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

    let id1 = engine.create_conversation(Some("Prompt A".to_string()));
    engine.send_message(id1, "msg1", &mut |_| {}).unwrap();

    let id2 = engine.create_conversation(Some("Prompt B".to_string()));
    engine.send_message(id2, "msg2", &mut |_| {}).unwrap();

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

    let mut events = Vec::new();
    let result = engine.generate("Tell me a joke", &mut |e| events.push(e)).unwrap();

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
