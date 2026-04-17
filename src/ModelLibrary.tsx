import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  HubSearchResult,
  ModelEntry,
  ModelInfo,
  ModelRef,
} from "./types/model";
import { formatBytes, modelKey } from "./utils/format";
import PopularSection from "./model-library/PopularSection";
import SearchSection from "./model-library/SearchSection";
import AdvancedSection from "./model-library/AdvancedSection";
import DownloadedModels from "./model-library/DownloadedModels";
import { usePopularModels } from "./model-library/usePopularModels";
import { useDownloadProgress } from "./model-library/useDownloadProgress";

interface ModelLibraryProps {
  loadedModelRef: ModelRef | null;
  onModelLoaded: (info: ModelInfo, ref: ModelRef) => void;
  startupError?: string | null;
  onDismissStartupError?: () => void;
}

export default function ModelLibrary({
  loadedModelRef,
  onModelLoaded,
  startupError,
  onDismissStartupError,
}: ModelLibraryProps) {
  const [models, setModels] = useState<ModelEntry[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<HubSearchResult[]>([]);
  const [searchLoading, setSearchLoading] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);

  const { popular, loading: popularLoading, error: popularError } =
    usePopularModels();

  /** Disambiguate the same repo appearing in both Popular and Search. */
  const [expandedKey, setExpandedKey] = useState<string | null>(null);

  const [downloading, setDownloading] = useState(false);
  const progress = useDownloadProgress(downloading);
  const [selectingKey, setSelectingKey] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refreshModels = useCallback(async () => {
    try {
      const list = await invoke<ModelEntry[]>("list_models");
      setModels(list);
    } catch (err) {
      setError(`Failed to list models: ${err}`);
    }
  }, []);

  useEffect(() => {
    refreshModels();
  }, [refreshModels]);

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
    setError(null);
    setSearchError(null);

    try {
      await invoke<ModelEntry>("download_model", {
        repoId,
        filename,
      });
      await refreshModels();
    } catch (err) {
      setError(`Download failed: ${err}`);
    } finally {
      setDownloading(false);
    }
  }

  async function handleSelect(entry: ModelEntry) {
    const key = modelKey(entry);
    setSelectingKey(key);
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
      setSelectingKey(null);
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

  return (
    <main className="model-library">
      {startupError && (
        <div className="library-error" role="alert">
          <p>{startupError}</p>
          {onDismissStartupError && (
            <button
              type="button"
              className="action-button"
              onClick={onDismissStartupError}
            >
              Dismiss
            </button>
          )}
        </div>
      )}

      <PopularSection
        popular={popular}
        loading={popularLoading}
        error={popularError}
        expandedKey={expandedKey}
        onToggleExpand={(repoId) => toggleExpand("popular", repoId)}
        onDownload={downloadFromHub}
        downloading={downloading}
      />

      <SearchSection
        query={searchQuery}
        onQueryChange={setSearchQuery}
        results={searchResults}
        loading={searchLoading}
        error={searchError}
        onRun={runSearch}
        expandedKey={expandedKey}
        onToggleExpand={(repoId) => toggleExpand("search", repoId)}
        onDownload={downloadFromHub}
        downloading={downloading}
      />

      <AdvancedSection
        onDownload={downloadFromHub}
        downloading={downloading}
      />

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

      <DownloadedModels
        models={models}
        loadedModelRef={loadedModelRef}
        selectingKey={selectingKey}
        downloading={downloading}
        onLoad={handleSelect}
        onDelete={handleDelete}
      />
    </main>
  );
}
