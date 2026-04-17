import type { HubSearchResult } from "../types/model";
import HubList from "./HubList";

interface PopularSectionProps {
  popular: HubSearchResult[] | null;
  loading: boolean;
  error: string | null;
  expandedKey: string | null;
  onToggleExpand: (repoId: string) => void;
  onDownload: (repoId: string, filename: string) => void;
  downloading: boolean;
}

export default function PopularSection({
  popular,
  loading,
  error,
  expandedKey,
  onToggleExpand,
  onDownload,
  downloading,
}: PopularSectionProps) {
  return (
    <section className="popular-section">
      <h3>Popular models</h3>
      <p className="section-hint">
        GGUF text-generation repos on Hugging Face, sorted by downloads. Expand
        a repo to pick a file.
      </p>
      {loading && popular === null && (
        <p className="section-hint">Loading popular models…</p>
      )}
      {error && (
        <div className="library-error hub-inline-error">
          <p>{error}</p>
        </div>
      )}
      {popular && popular.length > 0 && (
        <HubList
          section="popular"
          results={popular}
          expandedKey={expandedKey}
          onToggleExpand={onToggleExpand}
          onDownload={onDownload}
          downloading={downloading}
        />
      )}
      {popular && popular.length === 0 && !loading && (
        <p className="section-hint">No popular models returned.</p>
      )}
    </section>
  );
}
