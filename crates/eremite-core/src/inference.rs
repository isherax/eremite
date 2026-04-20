use std::path::Path;
use std::sync::atomic::AtomicBool;

use anyhow::{anyhow, Result};
use eremite_inference::{ChatMessage, InferenceEngine, InferenceEvent, InferenceParams, ModelMetadata};

/// Abstracts the inference boundary so `CoreEngine` can be tested with a mock
/// that requires no GPU or GGUF model.
pub trait InferenceProvider {
    fn load_model(&mut self, path: &Path, params: &InferenceParams) -> Result<ModelMetadata>;

    fn generate_chat(
        &mut self,
        messages: &[ChatMessage],
        params: &InferenceParams,
        on_event: &mut dyn FnMut(InferenceEvent),
        shutdown: &AtomicBool,
    ) -> Result<String>;

    fn generate(
        &mut self,
        prompt: &str,
        params: &InferenceParams,
        on_event: &mut dyn FnMut(InferenceEvent),
        shutdown: &AtomicBool,
    ) -> Result<String>;

    /// Count the tokens that `generate_chat` would feed to the model for the
    /// given messages after applying the model's chat template.
    ///
    /// Implementations that have no model loaded should return an error.
    fn count_prompt_tokens(&self, messages: &[ChatMessage]) -> Result<usize>;

    fn model_metadata(&self) -> Option<&ModelMetadata>;
}

/// Production implementation that wraps `eremite_inference::InferenceEngine`.
pub struct LlamaInference {
    engine: Option<InferenceEngine>,
}

impl LlamaInference {
    pub fn new() -> Self {
        Self { engine: None }
    }

    fn engine_mut(&mut self) -> Result<&mut InferenceEngine> {
        self.engine.as_mut().ok_or_else(|| anyhow!("no model loaded"))
    }

    fn engine_ref(&self) -> Result<&InferenceEngine> {
        self.engine.as_ref().ok_or_else(|| anyhow!("no model loaded"))
    }
}

impl Default for LlamaInference {
    fn default() -> Self {
        Self::new()
    }
}

impl InferenceProvider for LlamaInference {
    fn load_model(&mut self, path: &Path, params: &InferenceParams) -> Result<ModelMetadata> {
        // Drop any existing engine before creating a new one. The llama.cpp
        // backend is a global singleton; it must be fully torn down before
        // re-initialization.
        self.engine = None;

        let engine = InferenceEngine::load(path, params)?;
        let metadata = engine.model_metadata().clone();
        self.engine = Some(engine);
        Ok(metadata)
    }

    fn generate_chat(
        &mut self,
        messages: &[ChatMessage],
        params: &InferenceParams,
        on_event: &mut dyn FnMut(InferenceEvent),
        shutdown: &AtomicBool,
    ) -> Result<String> {
        self.engine_mut()?
            .generate_chat(messages, params, on_event, shutdown)
    }

    fn generate(
        &mut self,
        prompt: &str,
        params: &InferenceParams,
        on_event: &mut dyn FnMut(InferenceEvent),
        shutdown: &AtomicBool,
    ) -> Result<String> {
        self.engine_mut()?
            .generate(prompt, params, on_event, shutdown)
    }

    fn count_prompt_tokens(&self, messages: &[ChatMessage]) -> Result<usize> {
        self.engine_ref()?.count_prompt_tokens(messages)
    }

    fn model_metadata(&self) -> Option<&ModelMetadata> {
        self.engine.as_ref().map(InferenceEngine::model_metadata)
    }
}
