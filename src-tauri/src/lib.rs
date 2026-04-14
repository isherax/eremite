mod config;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use eremite_core::{CoreConfig, CoreEngine, ConversationId, LlamaInference, Message};
use eremite_inference::{InferenceEvent, ModelMetadata};
use eremite_models::manifest::ModelEntry;
use eremite_models::ModelManager;
use eremite_models::SearchResult;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};
use tokio::sync::mpsc;

use crate::config::{AppConfig, ModelRef};

// ---------------------------------------------------------------------------
// DTOs -- the IPC contract with the frontend
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize)]
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

#[derive(Clone, Serialize)]
struct ModelEntryView {
    repo_id: String,
    filename: String,
    size_bytes: u64,
    sha256: String,
    downloaded_at: String,
}

impl From<&ModelEntry> for ModelEntryView {
    fn from(e: &ModelEntry) -> Self {
        Self {
            repo_id: e.repo_id.clone(),
            filename: e.filename.clone(),
            size_bytes: e.size_bytes,
            sha256: e.sha256.clone(),
            downloaded_at: e.downloaded_at.to_rfc3339(),
        }
    }
}

#[derive(Clone, Serialize)]
struct DownloadProgress {
    repo_id: String,
    filename: String,
    bytes_downloaded: u64,
    total_bytes: Option<u64>,
}

#[derive(Clone, Serialize)]
struct ModelReady {
    model_info: ModelInfo,
    repo_id: String,
    filename: String,
}

#[derive(Clone, Serialize)]
struct StartupStateResponse {
    status: String,
    model_info: Option<ModelInfo>,
    loading_model: Option<ModelRef>,
    error: Option<String>,
}

// ---------------------------------------------------------------------------
// Startup state
// ---------------------------------------------------------------------------

enum StartupStatus {
    NoModels,
    Loading {
        repo_id: String,
        filename: String,
    },
    Ready {
        model_info: ModelInfo,
        repo_id: String,
        filename: String,
    },
    Failed {
        error: String,
    },
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

struct AppState {
    engine: Arc<Mutex<CoreEngine<LlamaInference>>>,
    active_conversation: Arc<Mutex<Option<ConversationId>>>,
    model_manager: Arc<tokio::sync::Mutex<ModelManager>>,
    config: Arc<Mutex<AppConfig>>,
    config_path: PathBuf,
    startup_status: Arc<Mutex<StartupStatus>>,
    loaded_model_ref: Arc<Mutex<Option<ModelRef>>>,
    shutdown: Arc<AtomicBool>,
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_startup_state(state: State<'_, AppState>) -> Result<StartupStateResponse, String> {
    let status = state.startup_status.lock().map_err(|e| e.to_string())?;
    match &*status {
        StartupStatus::NoModels => Ok(StartupStateResponse {
            status: "no_models".to_string(),
            model_info: None,
            loading_model: None,
            error: None,
        }),
        StartupStatus::Loading { repo_id, filename } => Ok(StartupStateResponse {
            status: "loading".to_string(),
            model_info: None,
            loading_model: Some(ModelRef {
                repo_id: repo_id.clone(),
                filename: filename.clone(),
            }),
            error: None,
        }),
        StartupStatus::Ready {
            model_info,
            repo_id,
            filename,
        } => Ok(StartupStateResponse {
            status: "ready".to_string(),
            model_info: Some(model_info.clone()),
            loading_model: Some(ModelRef {
                repo_id: repo_id.clone(),
                filename: filename.clone(),
            }),
            error: None,
        }),
        StartupStatus::Failed { error } => Ok(StartupStateResponse {
            status: "failed".to_string(),
            model_info: None,
            loading_model: None,
            error: Some(error.clone()),
        }),
    }
}

#[tauri::command]
async fn list_models(state: State<'_, AppState>) -> Result<Vec<ModelEntryView>, String> {
    let manager = state.model_manager.lock().await;
    Ok(manager.list().iter().map(ModelEntryView::from).collect())
}

const HF_SEARCH_LIMIT: u32 = 20;
const HF_POPULAR_LIMIT: u32 = 12;

#[tauri::command]
async fn search_models(query: String) -> Result<Vec<SearchResult>, String> {
    eremite_models::search_gguf_models(
        eremite_models::default_hub_origin(),
        &query,
        HF_SEARCH_LIMIT,
    )
    .await
    .map_err(|e| format!("search failed: {e}"))
}

#[tauri::command]
async fn popular_models() -> Result<Vec<SearchResult>, String> {
    eremite_models::popular_gguf_models(eremite_models::default_hub_origin(), HF_POPULAR_LIMIT)
        .await
        .map_err(|e| format!("popular models failed: {e}"))
}

#[tauri::command]
async fn download_model(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    filename: String,
) -> Result<ModelEntryView, String> {
    let mut manager = state.model_manager.lock().await;

    let emitter = app.clone();
    let rid = repo_id.clone();
    let fname = filename.clone();

    let entry = manager
        .download_with_progress(&repo_id, &filename, None, move |downloaded, total| {
            let _ = emitter.emit(
                "download:progress",
                DownloadProgress {
                    repo_id: rid.clone(),
                    filename: fname.clone(),
                    bytes_downloaded: downloaded,
                    total_bytes: total,
                },
            );
        })
        .await
        .map_err(|e| format!("download failed: {e}"))?;

    Ok(ModelEntryView::from(&entry))
}

#[tauri::command]
async fn delete_model(
    state: State<'_, AppState>,
    repo_id: String,
    filename: String,
) -> Result<(), String> {
    let mut manager = state.model_manager.lock().await;
    manager
        .remove(&repo_id, &filename)
        .map_err(|e| format!("failed to delete model: {e}"))
}

#[tauri::command]
async fn select_model(
    state: State<'_, AppState>,
    repo_id: String,
    filename: String,
) -> Result<ModelInfo, String> {
    let path = {
        let manager = state.model_manager.lock().await;
        manager.model_path(&repo_id, &filename)
    };

    if !path.exists() {
        return Err(format!("model file not found: {}", path.display()));
    }

    let engine_clone = Arc::clone(&state.engine);
    let metadata = tauri::async_runtime::spawn_blocking(move || {
        let mut engine = engine_clone.lock().map_err(|e| format!("lock poisoned: {e}"))?;
        engine
            .load_model(&path)
            .map_err(|e| format!("failed to load model: {e}"))
    })
    .await
    .map_err(|e| format!("task panicked: {e}"))??;

    let conv_id = {
        let mut engine = state.engine.lock().map_err(|e| e.to_string())?;
        engine.create_conversation(None)
    };
    *state
        .active_conversation
        .lock()
        .map_err(|e| e.to_string())? = Some(conv_id);

    let model_ref = ModelRef {
        repo_id,
        filename,
    };
    *state.loaded_model_ref.lock().map_err(|e| e.to_string())? = Some(model_ref.clone());

    {
        let mut config = state.config.lock().map_err(|e| e.to_string())?;
        config.last_used_model = Some(model_ref);
        config
            .save(&state.config_path)
            .map_err(|e| format!("failed to save config: {e}"))?;
    }

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
    let shutdown = Arc::clone(&state.shutdown);
    let (tx, mut rx) = mpsc::unbounded_channel::<InferenceEvent>();

    let join_handle = tauri::async_runtime::spawn_blocking(move || {
        let mut engine = engine.lock().map_err(|e| format!("lock poisoned: {e}"))?;

        engine
            .send_message(conv_id, &content, &mut |event| {
                let _ = tx.send(event);
            }, &shutdown)
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

fn most_recent_download(manager: &ModelManager) -> Option<ModelRef> {
    manager
        .list()
        .iter()
        .max_by_key(|e| e.downloaded_at)
        .map(|e| ModelRef {
            repo_id: e.repo_id.clone(),
            filename: e.filename.clone(),
        })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 1. Read persisted state synchronously (fast, local file I/O)
    let config = AppConfig::load_default();
    let config_path = AppConfig::default_path().expect("could not determine config path");

    let model_manager = ModelManager::default_path()
        .unwrap_or_else(|_| ModelManager::new(PathBuf::from("/tmp/eremite-fallback")).unwrap());

    // 2. Determine which model to auto-load
    let auto_target: Option<ModelRef> = config
        .last_used_model
        .clone()
        .or_else(|| most_recent_download(&model_manager));

    let has_models = !model_manager.list().is_empty();

    // 3. Create the engine before Tauri boots
    let engine = Arc::new(Mutex::new(CoreEngine::new(
        LlamaInference::new(),
        CoreConfig::default(),
    )));

    let startup_status = if !has_models {
        Arc::new(Mutex::new(StartupStatus::NoModels))
    } else if let Some(ref target) = auto_target {
        Arc::new(Mutex::new(StartupStatus::Loading {
            repo_id: target.repo_id.clone(),
            filename: target.filename.clone(),
        }))
    } else {
        Arc::new(Mutex::new(StartupStatus::NoModels))
    };

    let loaded_model_ref: Arc<Mutex<Option<ModelRef>>> = Arc::new(Mutex::new(None));
    let shutdown = Arc::new(AtomicBool::new(false));

    // Prepare eager-load data (resolve path before Tauri starts)
    let eager_load = auto_target.as_ref().map(|target| {
        let path = model_manager.model_path(&target.repo_id, &target.filename);
        (target.clone(), path)
    });

    let app_state = AppState {
        engine: Arc::clone(&engine),
        active_conversation: Arc::new(Mutex::new(None)),
        model_manager: Arc::new(tokio::sync::Mutex::new(model_manager)),
        config: Arc::new(Mutex::new(config)),
        config_path,
        startup_status: Arc::clone(&startup_status),
        loaded_model_ref: Arc::clone(&loaded_model_ref),
        shutdown: Arc::clone(&shutdown),
    };

    // 4. Build the Tauri app; spawn the eager load inside setup() for AppHandle access
    let shutdown_for_setup = Arc::clone(&shutdown);
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .setup(move |app| {
            if let Some((target, path)) = eager_load {
                let engine_clone = Arc::clone(&engine);
                let status_clone = Arc::clone(&startup_status);
                let model_ref_clone = Arc::clone(&loaded_model_ref);
                let shutdown_clone = Arc::clone(&shutdown_for_setup);
                let handle = app.handle().clone();

                std::thread::spawn(move || {
                    let result = {
                        let mut eng = engine_clone.lock().unwrap();
                        eng.load_model(&path)
                    };

                    if shutdown_clone.load(Ordering::Relaxed) {
                        return;
                    }

                    match result {
                        Ok(metadata) => {
                            let model_info = ModelInfo::from(metadata);

                            *model_ref_clone.lock().unwrap() = Some(target.clone());
                            *status_clone.lock().unwrap() = StartupStatus::Ready {
                                model_info: model_info.clone(),
                                repo_id: target.repo_id.clone(),
                                filename: target.filename.clone(),
                            };

                            {
                                let mut eng = engine_clone.lock().unwrap();
                                eng.create_conversation(None);
                            }
                            if let Some(state) = handle.try_state::<AppState>() {
                                if let Ok(mut conv) = state.active_conversation.lock() {
                                    let eng = engine_clone.lock().unwrap();
                                    *conv = eng.active_conversation();
                                }
                            }

                            let _ = handle.emit(
                                "model:ready",
                                ModelReady {
                                    model_info,
                                    repo_id: target.repo_id,
                                    filename: target.filename,
                                },
                            );
                        }
                        Err(e) => {
                            *status_clone.lock().unwrap() = StartupStatus::Failed {
                                error: format!("{e}"),
                            };
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_startup_state,
            list_models,
            search_models,
            popular_models,
            download_model,
            delete_model,
            select_model,
            send_message,
            get_messages,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    let shutdown_for_exit = Arc::clone(&shutdown);
    app.run(move |app_handle, event| {
        if let RunEvent::ExitRequested { api, .. } = &event {
            shutdown_for_exit.store(true, Ordering::Relaxed);
            api.prevent_exit();

            let engine = {
                let state = app_handle.state::<AppState>();
                Arc::clone(&state.engine)
            };
            let handle = app_handle.clone();

            std::thread::spawn(move || {
                // Acquiring the lock waits for any in-flight inference to
                // finish (it will see the shutdown flag and bail early).
                // Once we hold the lock, no new work can start. Dropping
                // the guard lets Tauri tear down the engine via normal RAII.
                let _guard = engine.lock().unwrap_or_else(|e| e.into_inner());
                drop(_guard);
                handle.exit(0);
            });
        }
    });
}
