use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eremite_core::{CoreConfig, CoreEngine, ConversationId, LlamaInference, Message};
use eremite_inference::{InferenceEvent, ModelMetadata};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// DTOs -- the IPC contract with the frontend
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ModelInfo {
    description: String,
    n_params: u64,
    n_ctx_train: u32,
}

impl From<ModelMetadata> for ModelInfo {
    fn from(m: ModelMetadata) -> Self {
        Self {
            description: m.description,
            n_params: m.n_params,
            n_ctx_train: m.n_ctx_train,
        }
    }
}

#[derive(Serialize)]
struct MessageView {
    role: String,
    content: String,
    created_at: String,
}

impl From<&Message> for MessageView {
    fn from(m: &Message) -> Self {
        Self {
            role: m.role.clone(),
            content: m.content.clone(),
            created_at: m.created_at.to_rfc3339(),
        }
    }
}

#[derive(Clone, Serialize)]
struct DonePayload {
    tokens_generated: u32,
    duration_ms: u64,
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

struct AppState {
    engine: Arc<Mutex<CoreEngine<LlamaInference>>>,
    active_conversation: Arc<Mutex<Option<ConversationId>>>,
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn load_model(state: State<'_, AppState>) -> Result<ModelInfo, String> {
    let mut engine = state.engine.lock().map_err(|e| e.to_string())?;

    if let Some(meta) = engine.model_metadata() {
        return Ok(ModelInfo::from(meta.clone()));
    }

    let model_path = std::env::var("EREMITE_MODEL")
        .map_err(|_| "EREMITE_MODEL environment variable not set".to_string())?;

    let path = PathBuf::from(&model_path);
    if !path.exists() {
        return Err(format!("model file not found: {}", path.display()));
    }

    let metadata = engine
        .load_model(&path)
        .map_err(|e| format!("failed to load model: {e}"))?;

    let conv_id = engine.create_conversation(None);
    *state.active_conversation.lock().map_err(|e| e.to_string())? = Some(conv_id);

    Ok(ModelInfo::from(metadata))
}

#[tauri::command]
async fn send_message(
    app: AppHandle,
    state: State<'_, AppState>,
    content: String,
) -> Result<String, String> {
    let conv_id = state
        .active_conversation
        .lock()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no active conversation".to_string())?;

    let engine = Arc::clone(&state.engine);
    let (tx, mut rx) = mpsc::unbounded_channel::<InferenceEvent>();

    let join_handle = tauri::async_runtime::spawn_blocking(move || {
        let mut engine = engine.lock().map_err(|e| format!("lock poisoned: {e}"))?;

        engine
            .send_message(conv_id, &content, &mut |event| {
                let _ = tx.send(event);
            })
            .map_err(|e| format!("inference failed: {e}"))
    });

    let emitter = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                InferenceEvent::Token(t) => {
                    let _ = emitter.emit("inference:token", t);
                }
                InferenceEvent::Done {
                    tokens_generated,
                    duration_ms,
                } => {
                    let _ = emitter.emit(
                        "inference:done",
                        DonePayload {
                            tokens_generated,
                            duration_ms,
                        },
                    );
                }
            }
        }
    });

    join_handle
        .await
        .map_err(|e| format!("task panicked: {e}"))?
}

#[tauri::command]
fn get_messages(state: State<'_, AppState>) -> Result<Vec<MessageView>, String> {
    let conv_id = state
        .active_conversation
        .lock()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no active conversation".to_string())?;

    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    let conv = engine
        .conversation(conv_id)
        .ok_or_else(|| "conversation not found".to_string())?;

    Ok(conv.messages().iter().map(MessageView::from).collect())
}

// ---------------------------------------------------------------------------
// App setup
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            engine: Arc::new(Mutex::new(CoreEngine::new(
                LlamaInference::new(),
                CoreConfig::default(),
            ))),
            active_conversation: Arc::new(Mutex::new(None)),
        })
        .invoke_handler(tauri::generate_handler![
            load_model,
            send_message,
            get_messages,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
