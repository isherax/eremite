import type { HubSearchResult } from "../types/model";
import { formatBytes, formatDownloads } from "../utils/format";

interface HubCardProps {
  result: HubSearchResult;
  expanded: boolean;
  onToggle: () => void;
  onDownload: (filename: string) => void;
  disabled: boolean;
  bodyId: string;
}

export default function HubCard({
  result,
  expanded,
  onToggle,
  onDownload,
  disabled,
  bodyId,
}: HubCardProps) {
  const ggufCount = result.gguf_files.length;

  return (
    <div className="hub-card">
      <button
        type="button"
        className="hub-card-header"
        onClick={onToggle}
        disabled={disabled}
        aria-expanded={expanded}
        aria-controls={bodyId}
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
        <div id={bodyId} className="hub-card-body">
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
                      {f.size_bytes != null ? (
                        <span className="hub-size">
                          {formatBytes(f.size_bytes)}
                        </span>
                      ) : (
                        <span className="hub-size-muted">Size unknown</span>
                      )}
                    </span>
                  </div>
                  <button
                    type="button"
                    className="action-button primary hub-file-download"
                    onClick={() => onDownload(f.filename)}
                    disabled={disabled}
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
