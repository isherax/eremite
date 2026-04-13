import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

interface ModelInfo {
  description: string;
  n_params: number;
  n_ctx_train: number;
}

interface ModelEntry {
  repo_id: string;
  filename: string;
  size_bytes: number;
  sha256: string;
  downloaded_at: string;
}

interface ModelRef {
  repo_id: string;
  filename: string;
}

interface DownloadProgress {
  repo_id: string;
  filename: string;
  bytes_downloaded: number;
  total_bytes: number | null;
}

interface ModelLibraryProps {
  loadedModelRef: ModelRef | null;
  onModelLoaded: (info: ModelInfo, ref: ModelRef) => void;
}

function formatBytes(bytes: number): string {
  if (bytes >= 1_000_000_000) return `${(bytes / 1_000_000_000).toFixed(1)} GB`;
  if (bytes >= 1_000_000) return `${(bytes / 1_000_000).toFixed(0)} MB`;
  if (bytes >= 1_000) return `${(bytes / 1_000).toFixed(0)} KB`;
  return `${bytes} B`;
}

function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

export default function ModelLibrary({
  loadedModelRef,
  onModelLoaded,
}: ModelLibraryProps) {
  const [models, setModels] = useState<ModelEntry[]>([]);
  const [repoId, setRepoId] = useState("");
  const [filename, setFilename] = useState("");
  const [downloading, setDownloading] = useState(false);
  const [progress, setProgress] = useState<DownloadProgress | null>(null);
  const [selectingModel, setSelectingModel] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    refreshModels();
  }, []);

  useEffect(() => {
    if (!downloading) return;

    let unlisten: UnlistenFn | undefined;
    const setup = async () => {
      unlisten = await listen<DownloadProgress>(
        "download:progress",
        (event) => {
          setProgress(event.payload);
        },
      );
    };
    setup();

    return () => {
      unlisten?.();
    };
  }, [downloading]);

  async function refreshModels() {
    try {
      const list = await invoke<ModelEntry[]>("list_models");
      setModels(list);
    } catch (err) {
      setError(`Failed to list models: ${err}`);
    }
  }

  async function handleDownload() {
    const rid = repoId.trim();
    const fname = filename.trim();
    if (!rid || !fname) return;

    setDownloading(true);
    setProgress(null);
    setError(null);

    try {
      await invoke<ModelEntry>("download_model", {
        repoId: rid,
        filename: fname,
      });
      setRepoId("");
      setFilename("");
      await refreshModels();
    } catch (err) {
      setError(`Download failed: ${err}`);
    } finally {
      setDownloading(false);
      setProgress(null);
    }
  }

  async function handleSelect(entry: ModelEntry) {
    const key = `${entry.repo_id}/${entry.filename}`;
    setSelectingModel(key);
    setError(null);

    try {
      const info = await invoke<ModelInfo>("select_model", {
        repoId: entry.repo_id,
        filename: entry.filename,
      });
      onModelLoaded(info, {
        repo_id: entry.repo_id,
        filename: entry.filename,
      });
    } catch (err) {
      setError(`Failed to load model: ${err}`);
    } finally {
      setSelectingModel(null);
    }
  }

  async function handleDelete(entry: ModelEntry) {
    setError(null);

    try {
      await invoke("delete_model", {
        repoId: entry.repo_id,
        filename: entry.filename,
      });
      await refreshModels();
    } catch (err) {
      setError(`Failed to delete model: ${err}`);
    }
  }

  const progressPercent =
    progress && progress.total_bytes
      ? Math.round((progress.bytes_downloaded / progress.total_bytes) * 100)
      : null;

  return (
    <div className="app">
      <header className="header">
        <span className="model-name">Model Library</span>
      </header>

      <main className="model-library">
        <section className="download-section">
          <h3>Download a Model</h3>
          <p className="section-hint">
            Enter a Hugging Face repo ID and GGUF filename to download.
          </p>

          <div className="download-form">
            <input
              type="text"
              className="form-input"
              placeholder="e.g. bartowski/Llama-3.2-1B-Instruct-GGUF"
              value={repoId}
              onChange={(e) => setRepoId(e.target.value)}
              disabled={downloading}
            />
            <input
              type="text"
              className="form-input"
              placeholder="e.g. Llama-3.2-1B-Instruct-Q4_K_M.gguf"
              value={filename}
              onChange={(e) => setFilename(e.target.value)}
              disabled={downloading}
            />
            <button
              className="action-button primary"
              onClick={handleDownload}
              disabled={downloading || !repoId.trim() || !filename.trim()}
            >
              {downloading ? "Downloading..." : "Download"}
            </button>
          </div>

          {downloading && progress && (
            <div className="progress-container">
              <div className="progress-bar">
                <div
                  className="progress-fill"
                  style={{ width: `${progressPercent ?? 0}%` }}
                />
              </div>
              <span className="progress-text">
                {formatBytes(progress.bytes_downloaded)}
                {progress.total_bytes
                  ? ` / ${formatBytes(progress.total_bytes)} (${progressPercent}%)`
                  : ""}
              </span>
            </div>
          )}
        </section>

        {error && (
          <div className="library-error">
            <p>{error}</p>
          </div>
        )}

        <section className="models-section">
          <h3>Downloaded Models</h3>

          {models.length === 0 ? (
            <p className="section-hint">
              No models downloaded yet. Download one above to get started.
            </p>
          ) : (
            <div className="models-list">
              {models.map((entry) => {
                const key = `${entry.repo_id}/${entry.filename}`;
                const isLoaded =
                  loadedModelRef?.repo_id === entry.repo_id &&
                  loadedModelRef?.filename === entry.filename;
                const isSelecting = selectingModel === key;

                return (
                  <div
                    key={key}
                    className={`model-card ${isLoaded ? "active" : ""}`}
                  >
                    <div className="model-card-info">
                      <span className="model-card-repo">{entry.repo_id}</span>
                      <span className="model-card-filename">
                        {entry.filename}
                      </span>
                      <span className="model-card-meta">
                        {formatBytes(entry.size_bytes)} &middot; Downloaded{" "}
                        {formatDate(entry.downloaded_at)}
                      </span>
                    </div>
                    <div className="model-card-actions">
                      {isLoaded ? (
                        <span className="loaded-badge">Loaded</span>
                      ) : (
                        <button
                          className="action-button primary"
                          onClick={() => handleSelect(entry)}
                          disabled={isSelecting}
                        >
                          {isSelecting ? "Loading..." : "Load"}
                        </button>
                      )}
                      <button
                        className="action-button danger"
                        onClick={() => handleDelete(entry)}
                        disabled={isLoaded || downloading}
                      >
                        Delete
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </section>
      </main>
    </div>
  );
}
