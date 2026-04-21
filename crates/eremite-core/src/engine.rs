use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use anyhow::{anyhow, bail, Result};
use eremite_inference::{ChatMessage, InferenceEvent, ModelMetadata};

use crate::config::CoreConfig;
use crate::conversation::{Conversation, ConversationId, Message};
use crate::inference::InferenceProvider;

/// Safety margin (in tokens) left on top of `max_tokens` when trimming chat
/// history to fit `n_ctx`. Accounts for chat-template boilerplate and
/// tokenizer quirks so we don't brush up against the hard `n_ctx` limit.
pub const CTX_RESERVE: u32 = 128;

/// Upper bound on the auto-selected context window.
///
/// Many modern GGUF models advertise training context lengths of 128k or more.
/// Using the trained value directly would allocate a huge KV cache that most
/// consumer machines cannot afford. 16 KiB is a pragmatic default that covers
/// long chats comfortably without exploding memory use.
pub const DEFAULT_CTX_CAP: u32 = 16_384;

/// Lower bound on the auto-selected context window. Avoids degenerate values
/// from small or mislabeled models while still honouring the model when its
/// trained context is above this floor.
pub const DEFAULT_CTX_FLOOR: u32 = 2_048;

/// Pick an effective context window size for a newly loaded model.
///
/// A caller-provided `user_override` always wins (this is the plumbing for a
/// future settings surface). Otherwise the model's trained context length is
/// clamped into `[DEFAULT_CTX_FLOOR, DEFAULT_CTX_CAP]`.
pub fn resolve_ctx_size(n_ctx_train: u32, user_override: Option<u32>) -> u32 {
    if let Some(value) = user_override {
        return value;
    }
    n_ctx_train.clamp(DEFAULT_CTX_FLOOR, DEFAULT_CTX_CAP)
}

/// Core orchestrator. Manages conversations and delegates generation to an
/// [`InferenceProvider`].
pub struct CoreEngine<I: InferenceProvider> {
    inference: I,
    conversations: HashMap<ConversationId, Conversation>,
    active_conversation: Option<ConversationId>,
    config: CoreConfig,
}

impl<I: InferenceProvider> CoreEngine<I> {
    pub fn new(inference: I, config: CoreConfig) -> Self {
        Self {
            inference,
            conversations: HashMap::new(),
            active_conversation: None,
            config,
        }
    }

    pub fn config(&self) -> &CoreConfig {
        &self.config
    }

    /// Read-only access to the underlying inference provider. Primarily
    /// exposed so test harnesses can inspect state captured by their mock
    /// implementation without having to thread separate handles through.
    pub fn inference(&self) -> &I {
        &self.inference
    }

    // -- Model management -------------------------------------------------

    /// Load a GGUF model from `model_path`. The Tauri layer resolves
    /// repo_id/filename to a filesystem path via `ModelManager::model_path()`
    /// before calling this.
    ///
    /// After load, the effective `n_ctx` is auto-sized via [`resolve_ctx_size`]
    /// from the model's trained context length (or a caller-supplied override
    /// stored in `CoreConfig::ctx_size_override`).
    pub fn load_model(&mut self, model_path: &Path) -> Result<ModelMetadata> {
        let metadata = self
            .inference
            .load_model(model_path, &self.config.inference_params)?;
        self.config.inference_params.n_ctx =
            resolve_ctx_size(metadata.n_ctx_train, self.config.ctx_size_override);
        Ok(metadata)
    }

    /// Return metadata for the currently loaded model, if any.
    pub fn model_metadata(&self) -> Option<&ModelMetadata> {
        self.inference.model_metadata()
    }

    /// Update the default system prompt used by newly created conversations
    /// and apply it to the currently active conversation (if any).
    ///
    /// Pass `None` to clear the prompt. Message history on the active
    /// conversation is preserved; only the prompt prepended at inference
    /// time changes.
    pub fn set_system_prompt(&mut self, prompt: Option<String>) {
        self.config.system_prompt = prompt.clone();
        if let Some(id) = self.active_conversation {
            if let Some(conv) = self.conversations.get_mut(&id) {
                conv.set_system_prompt(prompt);
            }
        }
    }

    // -- Conversation management ------------------------------------------

    /// Create a new conversation, optionally with a system prompt.
    /// Falls back to `config.system_prompt` if `system_prompt` is `None`.
    pub fn create_conversation(&mut self, system_prompt: Option<String>) -> ConversationId {
        let prompt = system_prompt.or_else(|| self.config.system_prompt.clone());
        let conv = Conversation::new(prompt);
        let id = conv.id();
        self.conversations.insert(id, conv);
        self.active_conversation = Some(id);
        id
    }

    /// Return a reference to a conversation by ID.
    pub fn conversation(&self, id: ConversationId) -> Option<&Conversation> {
        self.conversations.get(&id)
    }

    /// Return the active conversation ID, if any.
    pub fn active_conversation(&self) -> Option<ConversationId> {
        self.active_conversation
    }

    /// Set the active conversation.
    pub fn set_active_conversation(&mut self, id: ConversationId) -> Result<()> {
        if !self.conversations.contains_key(&id) {
            bail!("conversation {id} not found");
        }
        self.active_conversation = Some(id);
        Ok(())
    }

    /// Delete a conversation. If it was the active conversation, clears
    /// the active selection.
    pub fn delete_conversation(&mut self, id: ConversationId) -> bool {
        let removed = self.conversations.remove(&id).is_some();
        if removed && self.active_conversation == Some(id) {
            self.active_conversation = None;
        }
        removed
    }

    // -- Inference ---------------------------------------------------------

    /// Send a user message in the given conversation, run inference, and
    /// return the assistant's full response.
    ///
    /// Tokens are streamed to `on_event` as they arrive. After generation
    /// completes, the assistant's response is appended to the conversation
    /// history.
    ///
    /// Before generation, the chat history handed to the model is trimmed by
    /// a sliding window so it fits within `n_ctx - max_tokens - CTX_RESERVE`.
    /// The stored [`Conversation`] is never mutated by trimming -- UI callers
    /// continue to see the full history via [`Self::conversation`].
    pub fn send_message(
        &mut self,
        conversation_id: ConversationId,
        content: &str,
        on_event: &mut dyn FnMut(InferenceEvent),
        shutdown: &AtomicBool,
    ) -> Result<String> {
        {
            let conv = self
                .conversations
                .get_mut(&conversation_id)
                .ok_or_else(|| anyhow!("conversation {conversation_id} not found"))?;
            conv.add_message(Message::user(content));
        }

        let chat_messages = {
            let conv = self
                .conversations
                .get(&conversation_id)
                .ok_or_else(|| anyhow!("conversation {conversation_id} not found"))?;
            let budget = history_token_budget(&self.config.inference_params);
            trim_history_to_budget(
                &self.inference,
                conv.system_prompt(),
                conv.messages(),
                budget,
            )?
        };

        let params = &self.config.inference_params;
        let response = self
            .inference
            .generate_chat(&chat_messages, params, on_event, shutdown)?;

        let conv = self
            .conversations
            .get_mut(&conversation_id)
            .ok_or_else(|| anyhow!("conversation {conversation_id} not found"))?;
        conv.add_message(Message::assistant(&response));

        Ok(response)
    }

    /// Run raw text generation without conversation context.
    pub fn generate(
        &mut self,
        prompt: &str,
        on_event: &mut dyn FnMut(InferenceEvent),
        shutdown: &AtomicBool,
    ) -> Result<String> {
        self.inference
            .generate(prompt, &self.config.inference_params, on_event, shutdown)
    }
}

/// Compute the token budget available for chat history given current
/// inference parameters. Reserves room for `max_tokens` of output plus a
/// [`CTX_RESERVE`] safety margin.
///
/// Returns 0 when `max_tokens + CTX_RESERVE` already meets or exceeds
/// `n_ctx`; callers treat a zero budget as "drop history down to the last
/// turn and let the inference layer surface the overflow."
pub fn history_token_budget(params: &eremite_inference::InferenceParams) -> usize {
    params
        .n_ctx
        .saturating_sub(params.max_tokens)
        .saturating_sub(CTX_RESERVE) as usize
}

/// Build the chat message list to hand to inference, dropping the oldest
/// user/assistant turns until the prompt fits `budget` tokens.
///
/// The system prompt (if any) is always preserved. If a single remaining
/// history message already exceeds the budget, the loop stops and returns it
/// so `InferenceEngine::generate` can surface the overflow with its existing
/// "prompt exceeds context size" error.
pub fn trim_history_to_budget<I: InferenceProvider + ?Sized>(
    inference: &I,
    system_prompt: Option<&str>,
    history: &[Message],
    budget: usize,
) -> Result<Vec<ChatMessage>> {
    let mut remaining: Vec<ChatMessage> =
        history.iter().map(Message::to_chat_message).collect();

    loop {
        let mut combined: Vec<ChatMessage> = Vec::with_capacity(remaining.len() + 1);
        if let Some(prompt) = system_prompt {
            combined.push(ChatMessage::system(prompt));
        }
        combined.extend(remaining.iter().cloned());

        let tokens = inference.count_prompt_tokens(&combined)?;
        if tokens <= budget || remaining.len() <= 1 {
            return Ok(combined);
        }

        remaining.remove(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eremite_inference::{InferenceParams, ModelMetadata};
    use std::cell::RefCell;
    use std::path::Path;

    #[test]
    fn resolve_ctx_size_uses_trained_when_in_range() {
        assert_eq!(resolve_ctx_size(8_192, None), 8_192);
    }

    #[test]
    fn resolve_ctx_size_caps_huge_models() {
        assert_eq!(resolve_ctx_size(131_072, None), DEFAULT_CTX_CAP);
    }

    #[test]
    fn resolve_ctx_size_floors_tiny_models() {
        assert_eq!(resolve_ctx_size(512, None), DEFAULT_CTX_FLOOR);
    }

    #[test]
    fn resolve_ctx_size_honours_override() {
        // Override wins even when it's larger than the default cap.
        assert_eq!(resolve_ctx_size(8_192, Some(32_768)), 32_768);
        // And when it's smaller than the floor.
        assert_eq!(resolve_ctx_size(8_192, Some(1_024)), 1_024);
    }

    #[test]
    fn history_budget_reserves_output_and_margin() {
        let params = InferenceParams {
            n_ctx: 4_096,
            max_tokens: 1_024,
            ..InferenceParams::default()
        };
        assert_eq!(
            history_token_budget(&params),
            (4_096 - 1_024 - CTX_RESERVE) as usize
        );
    }

    #[test]
    fn history_budget_saturates_when_output_exceeds_ctx() {
        let params = InferenceParams {
            n_ctx: 512,
            max_tokens: 1_024,
            ..InferenceParams::default()
        };
        assert_eq!(history_token_budget(&params), 0);
    }

    /// Minimal counting provider for trim tests: reports a caller-supplied
    /// number of tokens per message so we can drive the trim loop with exact
    /// arithmetic.
    struct CountingInference {
        tokens_per_message: usize,
        calls: RefCell<Vec<usize>>,
    }

    impl CountingInference {
        fn new(tokens_per_message: usize) -> Self {
            Self {
                tokens_per_message,
                calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl InferenceProvider for CountingInference {
        fn load_model(
            &mut self,
            _path: &Path,
            _params: &InferenceParams,
        ) -> Result<ModelMetadata> {
            unimplemented!("not exercised by trim tests")
        }

        fn generate_chat(
            &mut self,
            _messages: &[ChatMessage],
            _params: &InferenceParams,
            _on_event: &mut dyn FnMut(InferenceEvent),
            _shutdown: &std::sync::atomic::AtomicBool,
        ) -> Result<String> {
            unimplemented!("not exercised by trim tests")
        }

        fn generate(
            &mut self,
            _prompt: &str,
            _params: &InferenceParams,
            _on_event: &mut dyn FnMut(InferenceEvent),
            _shutdown: &std::sync::atomic::AtomicBool,
        ) -> Result<String> {
            unimplemented!("not exercised by trim tests")
        }

        fn count_prompt_tokens(&self, messages: &[ChatMessage]) -> Result<usize> {
            let count = messages.len() * self.tokens_per_message;
            self.calls.borrow_mut().push(count);
            Ok(count)
        }

        fn model_metadata(&self) -> Option<&ModelMetadata> {
            None
        }
    }

    fn history(roles: &[&str]) -> Vec<Message> {
        roles
            .iter()
            .map(|r| match *r {
                "user" => Message::user("x"),
                "assistant" => Message::assistant("x"),
                other => panic!("unexpected role {other}"),
            })
            .collect()
    }

    #[test]
    fn trim_returns_full_history_when_under_budget() {
        let inf = CountingInference::new(10);
        let hist = history(&["user", "assistant", "user"]);
        let out = trim_history_to_budget(&inf, None, &hist, 100).unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].role, "user");
        assert_eq!(out[2].role, "user");
    }

    #[test]
    fn trim_drops_oldest_turns_first() {
        // 10 tokens per message, budget 25 -> at most 2 messages (plus system
        // if present). Start with 4 messages, expect the last 2 survivors.
        let inf = CountingInference::new(10);
        let hist = vec![
            Message::user("first"),
            Message::assistant("second"),
            Message::user("third"),
            Message::assistant("fourth"),
        ];
        let out = trim_history_to_budget(&inf, None, &hist, 25).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].content, "third");
        assert_eq!(out[1].content, "fourth");
    }

    #[test]
    fn trim_preserves_system_prompt() {
        // Budget of 20 with 10 tokens/msg means only 2 messages fit. The
        // system prompt must be one of them; the other should be the most
        // recent history message.
        let inf = CountingInference::new(10);
        let hist = vec![
            Message::user("old"),
            Message::assistant("older-reply"),
            Message::user("latest"),
        ];
        let out = trim_history_to_budget(&inf, Some("Be concise."), &hist, 20).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].role, "system");
        assert_eq!(out[0].content, "Be concise.");
        assert_eq!(out[1].content, "latest");
    }

    #[test]
    fn trim_keeps_last_message_even_if_over_budget() {
        // Budget too small for a single message; the loop must bail out
        // rather than spin forever. The caller (inference layer) will
        // surface the overflow.
        let inf = CountingInference::new(50);
        let hist = vec![Message::user("huge")];
        let out = trim_history_to_budget(&inf, None, &hist, 10).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].content, "huge");
    }
}
