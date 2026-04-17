import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import Chat from "./Chat";
import ModelLibrary from "./ModelLibrary";
import type {
  ModelInfo,
  ModelReady,
  ModelRef,
  StartupState,
} from "./types/model";
import { formatLoadingModelName, formatParams } from "./utils/format";
import { useTauriEvent } from "./hooks/useTauriEvent";

type View = "models" | "chat";

export default function App() {
  const [view, setView] = useState<View>("models");
  const [model, setModel] = useState<ModelInfo | null>(null);
  const [loadingModel, setLoadingModel] = useState<ModelRef | null>(null);
  const [loadedModelRef, setLoadedModelRef] = useState<ModelRef | null>(null);
  const [startupError, setStartupError] = useState<string | null>(null);

  useEffect(() => {
    const init = async () => {
      const state = await invoke<StartupState | null>("get_startup_state");

      if (state?.status === "ready" && state.model_info) {
        setModel(state.model_info);
        setLoadedModelRef(state.loaded_model ?? null);
        setView("chat");
      } else if (state?.status === "loading" && state.loading_model) {
        setLoadingModel(state.loading_model);
        setLoadedModelRef(null);
        setView("chat");
      } else if (state?.status === "failed") {
        setStartupError(
          state.error ?? "Failed to auto-load the last used model.",
        );
      }
    };

    init();
  }, []);

  useTauriEvent<ModelReady>("model:ready", (payload) => {
    setModel(payload.model_info);
    setLoadedModelRef({ repo_id: payload.repo_id, filename: payload.filename });
    setLoadingModel(null);
  });

  function handleModelLoaded(info: ModelInfo, ref_: ModelRef) {
    setModel(info);
    setLoadedModelRef(ref_);
    setStartupError(null);
    setView("chat");
  }

  const loadingName = formatLoadingModelName(loadingModel);
  const chatHeaderTitle =
    model?.description ?? (loadingModel ? loadingName : "Eremite");
  const canOpenChat = model !== null || loadingModel !== null;

  return (
    <div className="app">
      <header className="header">
        <nav className="nav-tabs" aria-label="Main">
          <button
            type="button"
            className={`nav-tab ${view === "chat" ? "active" : ""}`}
            onClick={() => canOpenChat && setView("chat")}
            disabled={!canOpenChat}
            aria-current={view === "chat" ? "page" : undefined}
          >
            Chat
          </button>
          <button
            type="button"
            className={`nav-tab ${view === "models" ? "active" : ""}`}
            onClick={() => setView("models")}
            aria-current={view === "models" ? "page" : undefined}
          >
            Models
          </button>
        </nav>
        {view === "chat" && (
          <div className="header-chat-context">
            <span className="header-separator" aria-hidden />
            <span className="model-name">{chatHeaderTitle}</span>
            {model && (
              <span className="model-meta">
                {formatParams(model.n_params)} params &middot;{" "}
                {model.n_ctx_train} ctx
              </span>
            )}
          </div>
        )}
      </header>

      {view === "models" ? (
        <ModelLibrary
          loadedModelRef={loadedModelRef}
          onModelLoaded={handleModelLoaded}
          startupError={startupError}
          onDismissStartupError={() => setStartupError(null)}
        />
      ) : (
        <Chat model={model} loadingModel={loadingModel} />
      )}
    </div>
  );
}
