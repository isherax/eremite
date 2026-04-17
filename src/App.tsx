import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Chat from "./Chat";
import ModelLibrary from "./ModelLibrary";

interface ModelInfo {
  description: string;
  n_params: number;
  n_ctx_train: number;
}

interface ModelRef {
  repo_id: string;
  filename: string;
}

interface StartupState {
  status: "loading" | "ready" | "failed";
  model_info?: ModelInfo;
  loading_model?: ModelRef;
  error?: string;
}

interface ModelReady {
  model_info: ModelInfo;
  repo_id: string;
  filename: string;
}

type View = "models" | "chat";

function formatParams(n: number): string {
  if (n >= 1_000_000_000) return `${(n / 1_000_000_000).toFixed(1)}B`;
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(0)}M`;
  return n.toLocaleString();
}

export default function App() {
  const [view, setView] = useState<View>("models");
  const [model, setModel] = useState<ModelInfo | null>(null);
  const [loadingModel, setLoadingModel] = useState<ModelRef | null>(null);
  const [loadedModelRef, setLoadedModelRef] = useState<ModelRef | null>(null);

  useEffect(() => {
    const init = async () => {
      const state = await invoke<StartupState | null>("get_startup_state");

      if (state?.status === "ready" && state.model_info) {
        setModel(state.model_info);
        setLoadedModelRef(state.loading_model ?? null);
        setView("chat");
      } else if (state?.status === "loading" && state.loading_model) {
        setLoadingModel(state.loading_model);
        setLoadedModelRef(null);
        setView("chat");
      }
    };

    init();
  }, []);

  useEffect(() => {
    const setup = async () => {
      const unlisten = await listen<ModelReady>("model:ready", (event) => {
        const { model_info, repo_id, filename } = event.payload;
        setModel(model_info);
        setLoadedModelRef({ repo_id, filename });
        setLoadingModel(null);
      });

      return unlisten;
    };

    const promise = setup();
    return () => {
      promise.then((unlisten) => unlisten());
    };
  }, []);

  function handleModelLoaded(info: ModelInfo, ref_: ModelRef) {
    setModel(info);
    setLoadedModelRef(ref_);
    setView("chat");
  }

  const loadingName =
    loadingModel?.filename ?? loadingModel?.repo_id ?? "model";
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
        />
      ) : (
        <Chat model={model} loadingModel={loadingModel} />
      )}
    </div>
  );
}
