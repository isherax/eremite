import type { ModelEntry, ModelRef } from "../types/model";
import { formatBytes, formatDate, modelKey } from "../utils/format";

interface DownloadedModelsProps {
  models: ModelEntry[];
  loadedModelRef: ModelRef | null;
  selectingKey: string | null;
  downloading: boolean;
  onLoad: (entry: ModelEntry) => void;
  onDelete: (entry: ModelEntry) => void;
}

export default function DownloadedModels({
  models,
  loadedModelRef,
  selectingKey,
  downloading,
  onLoad,
  onDelete,
}: DownloadedModelsProps) {
  return (
    <section className="models-section">
      <h3>Downloaded models</h3>

      {models.length === 0 ? (
        <p className="section-hint">
          No models downloaded yet. Pick a file from Popular or Search above,
          or use Advanced.
        </p>
      ) : (
        <div className="models-list">
          {models.map((entry) => {
            const key = modelKey(entry);
            const isLoaded =
              loadedModelRef?.repo_id === entry.repo_id &&
              loadedModelRef?.filename === entry.filename;
            const isSelecting = selectingKey === key;

            return (
              <div
                key={key}
                className={`model-card ${isLoaded ? "active" : ""}`}
              >
                <div className="model-card-info">
                  <span className="model-card-repo">{entry.repo_id}</span>
                  <span className="model-card-filename">{entry.filename}</span>
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
                      onClick={() => onLoad(entry)}
                      disabled={isSelecting}
                    >
                      {isSelecting ? "Loading…" : "Load"}
                    </button>
                  )}
                  <button
                    type="button"
                    className="action-button danger"
                    onClick={() => onDelete(entry)}
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
  );
}
