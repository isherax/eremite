import type { HubSearchResult } from "../types/model";
import HubCard from "./HubCard";

interface SearchSectionProps {
  query: string;
  onQueryChange: (value: string) => void;
  results: HubSearchResult[];
  loading: boolean;
  error: string | null;
  onRun: () => void;
  expandedKey: string | null;
  onToggleExpand: (repoId: string) => void;
  onDownload: (repoId: string, filename: string) => void;
  downloading: boolean;
}

export default function SearchSection({
  query,
  onQueryChange,
  results,
  loading,
  error,
  onRun,
  expandedKey,
  onToggleExpand,
  onDownload,
  downloading,
}: SearchSectionProps) {
  return (
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
          aria-label="Search Hugging Face repositories"
          value={query}
          onChange={(e) => onQueryChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") onRun();
          }}
          disabled={loading || downloading}
        />
        <button
          type="button"
          className="action-button primary search-button"
          onClick={onRun}
          disabled={loading || downloading || !query.trim()}
        >
          {loading ? "Searching…" : "Search"}
        </button>
      </div>

      {error && (
        <div className="library-error hub-inline-error">
          <p>{error}</p>
        </div>
      )}

      {results.length > 0 && (
        <div className="hub-results search-results">
          {results.map((r) => {
            const cardKey = `search:${r.repo_id}`;
            return (
              <HubCard
                key={cardKey}
                result={r}
                expanded={expandedKey === cardKey}
                onToggle={() => onToggleExpand(r.repo_id)}
                onDownload={(fname) => onDownload(r.repo_id, fname)}
                disabled={downloading}
                bodyId={`hub-body-${cardKey}`}
              />
            );
          })}
        </div>
      )}
    </section>
  );
}
