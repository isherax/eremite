pub mod config;
pub mod conversation;
pub mod engine;
pub mod inference;

pub use config::CoreConfig;
pub use conversation::{Conversation, ConversationId, Message};
pub use engine::{
    history_token_budget, resolve_ctx_size, trim_history_to_budget, CoreEngine,
    CTX_RESERVE, DEFAULT_CTX_CAP, DEFAULT_CTX_FLOOR,
};
pub use inference::{InferenceProvider, LlamaInference};
