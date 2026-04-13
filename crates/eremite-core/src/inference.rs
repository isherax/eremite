use std::path::Path;

use anyhow::Result;
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
    ) -> Result<String>;

    fn generate(
        &mut self,
        prompt: &str,
        params: &InferenceParams,
        on_event: &mut dyn FnMut(InferenceEvent),
    ) -> Result<String>;

    fn model_metadata(&self) -> Option<&ModelMetadata>;
}

/// Production implementation that wraps `eremite_inference::InferenceEngine`.
pub struct LlamaInference {
    engine: Option<InferenceEngine>,
    metadata: Option<ModelMetadata>,
}

impl LlamaInference {
    pub fn new() -> Self {
        Self {
            engine: None,
            metadata: None,
        }
    }
}

impl InferenceProvider for LlamaInference {
    fn load_model(&mut self, path: &Path, params: &InferenceParams) -> Result<ModelMetadata> {
        // Drop any existing engine before creating a new one. The llama.cpp
        // backend is a global singleton; it must be fully torn down before
        // re-initialization.
        self.engine = None;
        self.metadata = None;

        let engine = InferenceEngine::load(path, params)?;
        let metadata = engine.model_metadata();
        self.metadata = Some(metadata.clone());
        self.engine = Some(engine);
        Ok(metadata)
    }

    fn generate_chat(
        &mut self,
        messages: &[ChatMessage],
        params: &InferenceParams,
        on_event: &mut dyn FnMut(InferenceEvent),
    ) -> Result<String> {
        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("no model loaded"))?;
        engine.generate_chat(messages, params, on_event)
    }

    fn generate(
        &mut self,
        prompt: &str,
        params: &InferenceParams,
        on_event: &mut dyn FnMut(InferenceEvent),
    ) -> Result<String> {
        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("no model loaded"))?;
        engine.generate(prompt, params, on_event)
    }

    fn model_metadata(&self) -> Option<&ModelMetadata> {
        self.metadata.as_ref()
    }
}
