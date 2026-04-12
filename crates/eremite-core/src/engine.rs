use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Result};
use eremite_inference::{InferenceEvent, InferenceParams, ModelMetadata};

use crate::config::CoreConfig;
use crate::conversation::{Conversation, ConversationId, Message};
use crate::inference::InferenceProvider;

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

    // -- Model management -------------------------------------------------

    /// Load a GGUF model from `model_path`. The Tauri layer resolves
    /// repo_id/filename to a filesystem path via `ModelManager::model_path()`
    /// before calling this.
    pub fn load_model(&mut self, model_path: &Path) -> Result<ModelMetadata> {
        self.inference
            .load_model(model_path, &self.config.inference_params)
    }

    /// Load a model with explicit inference parameters (overriding config defaults).
    pub fn load_model_with_params(
        &mut self,
        model_path: &Path,
        params: &InferenceParams,
    ) -> Result<ModelMetadata> {
        self.inference.load_model(model_path, params)
    }

    /// Return metadata for the currently loaded model, if any.
    pub fn model_metadata(&self) -> Option<&ModelMetadata> {
        self.inference.model_metadata()
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

    /// Return all conversations as a slice of `(id, conversation)` pairs.
    pub fn conversations(&self) -> Vec<(&ConversationId, &Conversation)> {
        self.conversations.iter().collect()
    }

    /// Return the active conversation ID, if any.
    pub fn active_conversation(&self) -> Option<ConversationId> {
        self.active_conversation
    }

    /// Set the active conversation.
    pub fn set_active_conversation(&mut self, id: ConversationId) -> Result<()> {
        if !self.conversations.contains_key(&id) {
            bail!("conversation {} not found", id);
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
    pub fn send_message(
        &mut self,
        conversation_id: ConversationId,
        content: &str,
        on_event: &mut dyn FnMut(InferenceEvent),
    ) -> Result<String> {
        let conv = self
            .conversations
            .get_mut(&conversation_id)
            .ok_or_else(|| anyhow::anyhow!("conversation {} not found", conversation_id))?;

        conv.add_message(Message::user(content));

        let chat_messages = conv.to_chat_messages();
        let params = &self.config.inference_params;

        let response = self
            .inference
            .generate_chat(&chat_messages, params, on_event)?;

        let conv = self
            .conversations
            .get_mut(&conversation_id)
            .expect("conversation disappeared during generation");
        conv.add_message(Message::assistant(&response));

        Ok(response)
    }

    /// Run raw text generation without conversation context.
    pub fn generate(
        &mut self,
        prompt: &str,
        on_event: &mut dyn FnMut(InferenceEvent),
    ) -> Result<String> {
        self.inference
            .generate(prompt, &self.config.inference_params, on_event)
    }
}
