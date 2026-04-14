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

interface GgufFileInfo {
  filename: string;
  size_bytes: number | null;
  quantization_label: string | null;
}

interface HubSearchResult {
  repo_id: string;
  author?: string | null;
  downloads: number;
  likes: number;
  tags: string[];
  gguf_files: GgufFileInfo[];
}

interface ModelLibraryProps {
  loadedModelRef: ModelRef | null;
  onModelLoaded: (info: ModelInfo, ref: ModelRef) => void;
}

const POPULAR_CACHE_KEY = "eremite_popular_models_v1";

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

function formatDownloads(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return `${n}`;
}

export default function ModelLibrary({
  loadedModelRef,
  onModelLoaded,
}: ModelLibraryProps) {
  const [models, setModels] = useState<ModelEntry[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<HubSearchResult[]>([]);
  const [searchLoading, setSearchLoading] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);

  const [popular, setPopular] = useState<HubSearchResult[] | null>(null);
  const [popularLoading, setPopularLoading] = useState(false);
  const [popularError, setPopularError] = useState<string | null>(null);

  /** Disambiguate the same repo appearing in both Popular and Search. */
  const [expandedKey, setExpandedKey] = useState<string | null>(null);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [advancedRepoId, setAdvancedRepoId] = useState("");
  const [advancedFilename, setAdvancedFilename] = useState("");

  const [downloading, setDownloading] = useState(false);
  const [progress, setProgress] = useState<DownloadProgress | null>(null);
  const [selectingModel, setSelectingModel] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    refreshModels();
  }, []);

  useEffect(() => {
    const raw = sessionStorage.getItem(POPULAR_CACHE_KEY);
    if (raw) {
      try {
        const parsed = JSON.parse(raw) as HubSearchResult[];
        if (Array.isArray(parsed)) {
          setPopular(parsed);
          return;
        }
      } catch {
        /* ignore bad cache */
      }
    }

    let cancelled = false;
    setPopularLoading(true);
    setPopularError(null);

    (async () => {
      try {
        const list = await invoke<HubSearchResult[]>("popular_models");
        if (!cancelled) {
          setPopular(list);
          try {
            sessionStorage.setItem(POPULAR_CACHE_KEY, JSON.stringify(list));
          } catch {
            /* storage full or disabled */
          }
        }
      } catch (err) {
        if (!cancelled) {
          setPopularError(`Could not load popular models: ${err}`);
        }
      } finally {
        if (!cancelled) setPopularLoading(false);
      }
    })();

    return () => {
      cancelled = true;
    };
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

  async function runSearch() {
    const q = searchQuery.trim();
    if (!q) return;

    setSearchLoading(true);
    setSearchError(null);

    try {
      const list = await invoke<HubSearchResult[]>("search_models", {
        query: q,
      });
      setSearchResults(list);
      setExpandedKey(null);
    } catch (err) {
      setSearchError(`Search failed: ${err}`);
    } finally {
      setSearchLoading(false);
    }
  }

  async function downloadFromHub(repoId: string, filename: string) {
    setDownloading(true);
    setProgress(null);
    setError(null);
    setSearchError(null);

    try {
      await invoke<ModelEntry>("download_model", {
        repoId: repoId,
        filename: filename,
      });
      await refreshModels();
    } catch (err) {
      setError(`Download failed: ${err}`);
    } finally {
      setDownloading(false);
      setProgress(null);
    }
  }

  async function handleAdvancedDownload() {
    const rid = advancedRepoId.trim();
    const fname = advancedFilename.trim();
    if (!rid || !fname) return;
    await downloadFromHub(rid, fname);
    setAdvancedRepoId("");
    setAdvancedFilename("");
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

  function toggleExpand(section: "popular" | "search", repoId: string) {
    const key = `${section}:${repoId}`;
    setExpandedKey((prev) => (prev === key ? null : key));
  }

  const progressPercent =
    progress && progress.total_bytes
      ? Math.round((progress.bytes_downloaded / progress.total_bytes) * 100)
      : null;

  function renderHubCard(
    result: HubSearchResult,
    section: "popular" | "search",
  ) {
    const cardKey = `${section}:${result.repo_id}`;
    const expanded = expandedKey === cardKey;
    const ggufCount = result.gguf_files.length;

    return (
      <div key={cardKey} className="hub-card">
        <button
          type="button"
          className="hub-card-header"
          onClick={() => toggleExpand(section, result.repo_id)}
          disabled={downloading}
        >
          <div className="hub-card-titles">
            <span className="hub-card-repo">{result.repo_id}</span>
            {result.author && (
              <span className="hub-card-author">{result.author}</span>
            )}
          </div>
          <div className="hub-card-meta-row">
            <span className="hub-card-stat">
              {formatDownloads(result.downloads)} downloads
            </span>
            <span className="hub-card-stat">{result.likes} likes</span>
            <span className="hub-card-stat">{ggufCount} GGUF files</span>
            <span className="hub-card-chevron" aria-hidden>
              {expanded ? "\u2212" : "+"}
            </span>
          </div>
        </button>

        {expanded && (
          <div className="hub-card-body">
            {result.gguf_files.length === 0 ? (
              <p className="section-hint hub-card-empty">
                No GGUF files listed for this repo in the Hub response.
              </p>
            ) : (
              <ul className="hub-file-list">
                {result.gguf_files.map((f) => (
                  <li key={f.filename} className="hub-file-row">
                    <div className="hub-file-info">
                      <span className="hub-file-name">{f.filename}</span>
                      <span className="hub-file-tags">
                        {f.quantization_label && (
                          <span className="hub-quant">
                            {f.quantization_label}
                          </span>
                        )}
                        {f.size_bytes != null && (
                          <span className="hub-size">
                            {formatBytes(f.size_bytes)}
                          </span>
                        )}
                        {f.size_bytes == null && (
                          <span className="hub-size-muted">Size unknown</span>
                        )}
                      </span>
                    </div>
                    <button
                      type="button"
                      className="action-button primary hub-file-download"
                      onClick={() =>
                        downloadFromHub(result.repo_id, f.filename)
                      }
                      disabled={downloading}
                    >
                      Download
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}
      </div>
    );
  }

  return (
    <main className="model-library">
        <section className="popular-section">
          <h3>Popular models</h3>
          <p className="section-hint">
            GGUF text-generation repos on Hugging Face, sorted by downloads.
            Expand a repo to pick a file.
          </p>
          {popularLoading && popular === null && (
            <p className="section-hint">Loading popular models…</p>
          )}
          {popularError && (
            <div className="library-error hub-inline-error">
              <p>{popularError}</p>
            </div>
          )}
          {popular && popular.length > 0 && (
            <div className="hub-results">
              {popular.map((r) => renderHubCard(r, "popular"))}
            </div>
          )}
          {popular && popular.length === 0 && !popularLoading && (
            <p className="section-hint">No popular models returned.</p>
          )}
        </section>

        <section className="download-section">
          <h3>Search Hugging Face</h3>
          <p className="section-hint">
            Search for GGUF repos by name or topic, then expand and download a
            file.
          </p>

          <div className="search-row">
            <input
              type="search"
              className="form-input search-input"
              placeholder="e.g. llama, mistral, gemma"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") runSearch();
              }}
              disabled={searchLoading || downloading}
            />
            <button
              type="button"
              className="action-button primary search-button"
              onClick={runSearch}
              disabled={
                searchLoading || downloading || !searchQuery.trim()
              }
            >
              {searchLoading ? "Searching…" : "Search"}
            </button>
          </div>

          {searchError && (
            <div className="library-error hub-inline-error">
              <p>{searchError}</p>
            </div>
          )}

          {searchResults.length > 0 && (
            <div className="hub-results search-results">
              {searchResults.map((r) => renderHubCard(r, "search"))}
            </div>
          )}
        </section>

        <section className="advanced-section">
          <button
            type="button"
            className="advanced-toggle"
            onClick={() => setAdvancedOpen(!advancedOpen)}
            aria-expanded={advancedOpen}
          >
            Advanced: download by repo ID and filename
            <span className="hub-card-chevron" aria-hidden>
              {advancedOpen ? "\u2212" : "+"}
            </span>
          </button>

          {advancedOpen && (
            <div className="advanced-panel">
              <p className="section-hint">
                If you already know the exact Hugging Face repo and GGUF
                filename, enter them here.
              </p>
              <div className="download-form">
                <input
                  type="text"
                  className="form-input"
                  placeholder="e.g. bartowski/Llama-3.2-1B-Instruct-GGUF"
                  value={advancedRepoId}
                  onChange={(e) => setAdvancedRepoId(e.target.value)}
                  disabled={downloading}
                />
                <input
                  type="text"
                  className="form-input"
                  placeholder="e.g. Llama-3.2-1B-Instruct-Q4_K_M.gguf"
                  value={advancedFilename}
                  onChange={(e) => setAdvancedFilename(e.target.value)}
                  disabled={downloading}
                />
                <button
                  type="button"
                  className="action-button primary"
                  onClick={handleAdvancedDownload}
                  disabled={
                    downloading ||
                    !advancedRepoId.trim() ||
                    !advancedFilename.trim()
                  }
                >
                  {downloading ? "Downloading…" : "Download"}
                </button>
              </div>
            </div>
          )}
        </section>

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

        {error && (
          <div className="library-error">
            <p>{error}</p>
          </div>
        )}

        <section className="models-section">
          <h3>Downloaded models</h3>

          {models.length === 0 ? (
            <p className="section-hint">
              No models downloaded yet. Pick a file from Popular or Search
              above, or use Advanced.
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
                          type="button"
                          className="action-button primary"
                          onClick={() => handleSelect(entry)}
                          disabled={isSelecting}
                        >
                          {isSelecting ? "Loading…" : "Load"}
                        </button>
                      )}
                      <button
                        type="button"
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
  );
}
