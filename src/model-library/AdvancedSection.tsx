import { useState } from "react";

interface AdvancedSectionProps {
  onDownload: (repoId: string, filename: string) => Promise<void>;
  downloading: boolean;
}

export default function AdvancedSection({
  onDownload,
  downloading,
}: AdvancedSectionProps) {
  const [open, setOpen] = useState(false);
  const [repoId, setRepoId] = useState("");
  const [filename, setFilename] = useState("");

  async function handleDownload() {
    const rid = repoId.trim();
    const fname = filename.trim();
    if (!rid || !fname) return;
    await onDownload(rid, fname);
    setRepoId("");
    setFilename("");
  }

  return (
    <section className="advanced-section">
      <button
        type="button"
        className="advanced-toggle"
        onClick={() => setOpen(!open)}
        aria-expanded={open}
      >
        Advanced: download by repo ID and filename
        <span className="hub-card-chevron" aria-hidden>
          {open ? "\u2212" : "+"}
        </span>
      </button>

      {open && (
        <div className="advanced-panel">
          <p className="section-hint">
            If you already know the exact Hugging Face repo and GGUF filename,
            enter them here.
          </p>
          <div className="download-form">
            <input
              type="text"
              className="form-input"
              placeholder="e.g. bartowski/Llama-3.2-1B-Instruct-GGUF"
              aria-label="Repository ID"
              value={repoId}
              onChange={(e) => setRepoId(e.target.value)}
              disabled={downloading}
            />
            <input
              type="text"
              className="form-input"
              placeholder="e.g. Llama-3.2-1B-Instruct-Q4_K_M.gguf"
              aria-label="GGUF filename"
              value={filename}
              onChange={(e) => setFilename(e.target.value)}
              disabled={downloading}
            />
            <button
              type="button"
              className="action-button primary"
              onClick={handleDownload}
              disabled={downloading || !repoId.trim() || !filename.trim()}
            >
              {downloading ? "Downloading…" : "Download"}
            </button>
          </div>
        </div>
      )}
    </section>
  );
}
