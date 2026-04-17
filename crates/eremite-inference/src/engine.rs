#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::num::NonZeroU32;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use anyhow::{bail, Context, Result};
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaChatMessage, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;

use serde::{Deserialize, Serialize};

use crate::event::InferenceEvent;
use crate::params::{ChatMessage, InferenceParams};

/// Metadata read from a loaded GGUF model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    /// Human-readable model name from the GGUF `general.name` field.
    pub description: String,
    /// Total number of parameters in the model.
    pub n_params: u64,
    /// Context length the model was trained with.
    pub n_ctx_train: u32,
}

/// Loads a GGUF model and runs inference via llama.cpp.
///
/// The engine owns the llama.cpp backend and model. A fresh
/// [`LlamaContext`](llama_cpp_2::context::LlamaContext) is created for each
/// generation call so the struct avoids self-referential borrows.
pub struct InferenceEngine {
    backend: LlamaBackend,
    model: LlamaModel,
    metadata: ModelMetadata,
}

impl InferenceEngine {
    /// Load a GGUF model file and prepare the inference backend.
    ///
    /// `params.n_gpu_layers` controls how many layers are offloaded to the GPU.
    /// On macOS this uses Metal automatically.
    pub fn load(model_path: impl AsRef<Path>, params: &InferenceParams) -> Result<Self> {
        let backend =
            LlamaBackend::init().context("failed to initialize llama.cpp backend")?;

        let model_params =
            LlamaModelParams::default().with_n_gpu_layers(params.n_gpu_layers);

        let model =
            LlamaModel::load_from_file(&backend, model_path.as_ref(), &model_params)
                .map_err(|e| anyhow::anyhow!("failed to load model: {e:?}"))?;

        let metadata = read_metadata(&model);

        Ok(Self {
            backend,
            model,
            metadata,
        })
    }

    /// Generate text from a raw prompt string.
    ///
    /// Tokens are streamed to `on_event` as they are produced. Returns the
    /// full generated text when complete.
    pub fn generate(
        &mut self,
        prompt: &str,
        params: &InferenceParams,
        mut on_event: impl FnMut(InferenceEvent),
        shutdown: &AtomicBool,
    ) -> Result<String> {
        let start = Instant::now();

        let ctx_params =
            LlamaContextParams::default().with_n_ctx(NonZeroU32::new(params.n_ctx));

        let mut ctx = self
            .model
            .new_context(&self.backend, ctx_params)
            .map_err(|e| anyhow::anyhow!("failed to create context: {e:?}"))?;

        let tokens = self
            .model
            .str_to_token(prompt, AddBos::Always)
            .map_err(|e| anyhow::anyhow!("failed to tokenize prompt: {e:?}"))?;

        let n_ctx = ctx.n_ctx() as i32;
        let n_prompt = tokens.len() as i32;

        if n_prompt + params.max_tokens as i32 > n_ctx {
            bail!(
                "prompt ({n_prompt} tokens) + max_tokens ({}) exceeds context size ({n_ctx})",
                params.max_tokens,
            );
        }

        let mut batch = LlamaBatch::new(n_ctx as usize, 1);
        let last_idx = n_prompt - 1;
        for (i, token) in (0_i32..).zip(tokens.into_iter()) {
            batch
                .add(token, i, &[0], i == last_idx)
                .context("failed to add token to batch")?;
        }

        ctx.decode(&mut batch).context("failed to decode prompt")?;

        let mut sampler = self.build_sampler(params);
        let mut decoder = encoding_rs::UTF_8.new_decoder();
        let mut n_cur = batch.n_tokens();
        let mut tokens_generated: u32 = 0;
        let mut output = String::new();

        while tokens_generated < params.max_tokens {
            if shutdown.load(Ordering::Relaxed) {
                bail!("inference cancelled: shutting down");
            }

            let token = sampler.sample(&ctx, batch.n_tokens() - 1);
            sampler.accept(token);

            if self.model.is_eog_token(token) {
                break;
            }

            let piece = self
                .model
                .token_to_piece(token, &mut decoder, true, None)
                .map_err(|e| anyhow::anyhow!("failed to decode token: {e:?}"))?;

            output.push_str(&piece);
            on_event(InferenceEvent::Token(piece));

            batch.clear();
            batch
                .add(token, n_cur, &[0], true)
                .context("failed to add token to batch")?;
            ctx.decode(&mut batch)
                .context("failed to decode generated token")?;

            n_cur += 1;
            tokens_generated += 1;
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        on_event(InferenceEvent::Done {
            tokens_generated,
            duration_ms,
        });

        Ok(output)
    }

    /// Generate text from a chat conversation.
    ///
    /// Applies the model's embedded chat template to `messages`, then runs
    /// generation on the formatted prompt. The template determines how
    /// system / user / assistant turns are formatted.
    pub fn generate_chat(
        &mut self,
        messages: &[ChatMessage],
        params: &InferenceParams,
        on_event: impl FnMut(InferenceEvent),
        shutdown: &AtomicBool,
    ) -> Result<String> {
        let llama_messages: Vec<LlamaChatMessage> = messages
            .iter()
            .map(|m| {
                LlamaChatMessage::new(m.role.clone(), m.content.clone())
                    .map_err(|e| anyhow::anyhow!("invalid chat message: {e:?}"))
            })
            .collect::<Result<Vec<_>>>()?;

        let template = self
            .model
            .chat_template(None)
            .map_err(|e| anyhow::anyhow!("model has no chat template: {e:?}"))?;

        let prompt = self
            .model
            .apply_chat_template(&template, &llama_messages, true)
            .map_err(|e| anyhow::anyhow!("failed to apply chat template: {e:?}"))?;

        self.generate(&prompt, params, on_event, shutdown)
    }

    /// Read metadata from the loaded model.
    pub fn model_metadata(&self) -> &ModelMetadata {
        &self.metadata
    }

    fn build_sampler(&self, params: &InferenceParams) -> LlamaSampler {
        let seed = params.seed.unwrap_or(1234);

        if params.temperature <= 0.0 {
            LlamaSampler::chain_simple([LlamaSampler::greedy()])
        } else {
            LlamaSampler::chain_simple([
                LlamaSampler::top_k(params.top_k as i32),
                LlamaSampler::top_p(params.top_p, 1),
                LlamaSampler::temp(params.temperature),
                LlamaSampler::dist(seed),
            ])
        }
    }
}

fn read_metadata(model: &LlamaModel) -> ModelMetadata {
    let description = model
        .meta_val_str("general.name")
        .unwrap_or_else(|_| "Unknown".to_string());

    ModelMetadata {
        description,
        n_params: model.n_params(),
        n_ctx_train: model.n_ctx_train(),
    }
}
