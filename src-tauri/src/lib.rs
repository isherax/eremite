mod config;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use eremite_core::{CoreConfig, CoreEngine, LlamaInference, Message};
use eremite_inference::{InferenceEvent, ModelMetadata};
use eremite_models::manifest::ModelEntry;
use eremite_models::ModelManager;
use eremite_models::SearchResult;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};

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
    loaded_model: Option<ModelRef>,
    error: Option<String>,
}

// ---------------------------------------------------------------------------
// Startup state
// ---------------------------------------------------------------------------

enum StartupStatus {
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
    model_manager: Arc<tokio::sync::Mutex<ModelManager>>,
    config: Arc<Mutex<AppConfig>>,
    config_path: PathBuf,
    startup_status: Arc<Mutex<Option<StartupStatus>>>,
    shutdown: Arc<AtomicBool>,
}

/// Locks a mutex, recovering from poisoning by taking the inner guard.
/// A poisoned engine/status mutex still has valid data for our purposes:
/// the worst case is a panicked inference thread, and we'd rather keep
/// the UI responsive than propagate the panic further.
fn lock_tolerant<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_startup_state(
    state: State<'_, AppState>,
) -> Result<Option<StartupStateResponse>, String> {
    let status = state.startup_status.lock().map_err(|e| e.to_string())?;
    Ok(match &*status {
        None => None,
        Some(StartupStatus::Loading { repo_id, filename }) => Some(StartupStateResponse {
            status: "loading".to_string(),
            model_info: None,
            loading_model: Some(ModelRef {
                repo_id: repo_id.clone(),
                filename: filename.clone(),
            }),
            loaded_model: None,
            error: None,
        }),
        Some(StartupStatus::Ready {
            model_info,
            repo_id,
            filename,
        }) => Some(StartupStateResponse {
            status: "ready".to_string(),
            model_info: Some(model_info.clone()),
            loading_model: None,
            loaded_model: Some(ModelRef {
                repo_id: repo_id.clone(),
                filename: filename.clone(),
            }),
            error: None,
        }),
        Some(StartupStatus::Failed { error }) => Some(StartupStateResponse {
            status: "failed".to_string(),
            model_info: None,
            loading_model: None,
            loaded_model: None,
            error: Some(error.clone()),
        }),
    })
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
    let path_for_task = path.clone();
    let metadata = tauri::async_runtime::spawn_blocking(move || {
        let mut engine = engine_clone
            .lock()
            .map_err(|e| format!("lock poisoned: {e}"))?;
        let metadata = engine
            .load_model(&path_for_task)
            .map_err(|e| format!("failed to load model: {e}"))?;
        engine.create_conversation(None);
        Ok::<ModelMetadata, String>(metadata)
    })
    .await
    .map_err(|e| format!("task panicked: {e}"))??;

    let model_ref = ModelRef { repo_id, filename };

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
    let engine = Arc::clone(&state.engine);
    let shutdown = Arc::clone(&state.shutdown);
    let emitter = app.clone();

    let join_handle = tauri::async_runtime::spawn_blocking(move || {
        let mut engine = engine.lock().map_err(|e| format!("lock poisoned: {e}"))?;
        let conv_id = engine
            .active_conversation()
            .ok_or_else(|| "no active conversation".to_string())?;

        engine
            .send_message(
                conv_id,
                &content,
                &mut |event| match event {
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
                },
                &shutdown,
            )
            .map_err(|e| format!("inference failed: {e}"))
    });

    join_handle
        .await
        .map_err(|e| format!("task panicked: {e}"))?
}

#[tauri::command]
fn set_system_prompt(state: State<'_, AppState>, prompt: String) -> Result<(), String> {
    let trimmed = if prompt.trim().is_empty() {
        None
    } else {
        Some(prompt)
    };
    let mut engine = state.engine.lock().map_err(|e| e.to_string())?;
    engine.set_system_prompt(trimmed);
    Ok(())
}

#[tauri::command]
fn get_messages(state: State<'_, AppState>) -> Result<Vec<MessageView>, String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    let conv_id = engine
        .active_conversation()
        .ok_or_else(|| "no active conversation".to_string())?;
    let conv = engine
        .conversation(conv_id)
        .ok_or_else(|| "conversation not found".to_string())?;

    Ok(conv.messages().iter().map(MessageView::from).collect())
}

// ---------------------------------------------------------------------------
// App setup
// ---------------------------------------------------------------------------

const FALLBACK_MODEL_DIR: &str = "/tmp/eremite-fallback";

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

fn load_persisted_state() -> (AppConfig, PathBuf, ModelManager) {
    let config = AppConfig::load_default();
    let config_path = AppConfig::default_path().expect("could not determine config path");
    let model_manager = ModelManager::default_path()
        .unwrap_or_else(|_| ModelManager::new(PathBuf::from(FALLBACK_MODEL_DIR)).unwrap());
    (config, config_path, model_manager)
}

fn resolve_auto_target(config: &AppConfig, manager: &ModelManager) -> Option<ModelRef> {
    config
        .last_used_model
        .clone()
        .or_else(|| most_recent_download(manager))
}

struct EagerLoadContext {
    engine: Arc<Mutex<CoreEngine<LlamaInference>>>,
    startup_status: Arc<Mutex<Option<StartupStatus>>>,
    shutdown: Arc<AtomicBool>,
}

fn spawn_eager_load(
    handle: AppHandle,
    ctx: EagerLoadContext,
    target: ModelRef,
    path: PathBuf,
) {
    std::thread::spawn(move || {
        let result = {
            let mut eng = lock_tolerant(&ctx.engine);
            eng.load_model(&path).inspect(|_| {
                eng.create_conversation(None);
            })
        };

        if ctx.shutdown.load(Ordering::Relaxed) {
            return;
        }

        match result {
            Ok(metadata) => {
                let model_info = ModelInfo::from(metadata);

                *lock_tolerant(&ctx.startup_status) = Some(StartupStatus::Ready {
                    model_info: model_info.clone(),
                    repo_id: target.repo_id.clone(),
                    filename: target.filename.clone(),
                });

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
                *lock_tolerant(&ctx.startup_status) = Some(StartupStatus::Failed {
                    error: format!("{e}"),
                });
            }
        }
    });
}

fn install_exit_handler(app: tauri::App, shutdown: Arc<AtomicBool>) {
    app.run(move |app_handle, event| {
        if let RunEvent::ExitRequested { api, .. } = &event {
            // Latch so the `ExitRequested` we re-enter via handle.exit(0)
            // isn't prevented a second time.
            if shutdown.swap(true, Ordering::AcqRel) {
                return;
            }
            api.prevent_exit();

            let engine = {
                let state = app_handle.state::<AppState>();
                Arc::clone(&state.engine)
            };
            #[cfg(not(target_os = "macos"))]
            let handle = app_handle.clone();

            std::thread::spawn(move || {
                // Wait for any in-flight inference to see the shutdown flag
                // and bail, then hold the lock so nothing new starts.
                let _guard = lock_tolerant(&engine);

                // macOS: skip atexit/C++ static dtors -- ggml-metal's static
                // device vector aborts in `ggml_metal_rsets_free` at exit.
                #[cfg(target_os = "macos")]
                unsafe {
                    libc::_exit(0);
                }

                #[cfg(not(target_os = "macos"))]
                {
                    drop(_guard);
                    handle.exit(0);
                }
            });
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (config, config_path, model_manager) = load_persisted_state();
    let auto_target = resolve_auto_target(&config, &model_manager);

    let engine = Arc::new(Mutex::new(CoreEngine::new(
        LlamaInference::new(),
        CoreConfig::default(),
    )));

    let startup_status = Arc::new(Mutex::new(auto_target.as_ref().map(|target| {
        StartupStatus::Loading {
            repo_id: target.repo_id.clone(),
            filename: target.filename.clone(),
        }
    })));
    let shutdown = Arc::new(AtomicBool::new(false));

    // Resolve eager-load path before the manager is moved into AppState.
    let eager_load = auto_target.as_ref().map(|target| {
        let path = model_manager.model_path(&target.repo_id, &target.filename);
        (target.clone(), path)
    });

    let app_state = AppState {
        engine: Arc::clone(&engine),
        model_manager: Arc::new(tokio::sync::Mutex::new(model_manager)),
        config: Arc::new(Mutex::new(config)),
        config_path,
        startup_status: Arc::clone(&startup_status),
        shutdown: Arc::clone(&shutdown),
    };

    let eager_ctx = EagerLoadContext {
        engine: Arc::clone(&engine),
        startup_status: Arc::clone(&startup_status),
        shutdown: Arc::clone(&shutdown),
    };

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .setup(move |app| {
            if let Some((target, path)) = eager_load {
                spawn_eager_load(app.handle().clone(), eager_ctx, target, path);
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
            set_system_prompt,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    install_exit_handler(app, shutdown);
}
