import type { HubSearchResult } from "../types/model";
import HubList from "./HubList";

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
        <HubList
          section="search"
          results={results}
          expandedKey={expandedKey}
          onToggleExpand={onToggleExpand}
          onDownload={onDownload}
          downloading={downloading}
          className="search-results"
        />
      )}
    </section>
  );
}
