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
  status: "no_models" | "loading" | "ready" | "failed";
  model_info?: ModelInfo;
  loading_model?: ModelRef;
  error?: string;
}

interface ModelReady {
  model_info: ModelInfo;
  repo_id: string;
  filename: string;
}

type View = "init" | "models" | "chat";

export default function App() {
  const [view, setView] = useState<View>("init");
  const [model, setModel] = useState<ModelInfo | null>(null);
  const [loadingModel, setLoadingModel] = useState<ModelRef | null>(null);
  const [loadedModelRef, setLoadedModelRef] = useState<ModelRef | null>(null);

  useEffect(() => {
    const init = async () => {
      const state = await invoke<StartupState>("get_startup_state");

      if (state.status === "ready" && state.model_info) {
        setModel(state.model_info);
        setLoadedModelRef(state.loading_model ?? null);
        setView("chat");
      } else if (state.status === "loading" && state.loading_model) {
        setLoadingModel(state.loading_model);
        setLoadedModelRef(null);
        setView("chat");
      } else {
        setView("models");
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

  if (view === "init") {
    return (
      <div className="app">
        <div className="loading-screen">
          <p>Starting up...</p>
        </div>
      </div>
    );
  }

  if (view === "models") {
    return (
      <ModelLibrary
        loadedModelRef={loadedModelRef}
        onModelLoaded={handleModelLoaded}
      />
    );
  }

  return (
    <Chat
      model={model}
      loadingModel={loadingModel}
      onNavigateToModels={() => setView("models")}
    />
  );
}
