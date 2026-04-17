import type { HubSearchResult } from "../types/model";
import HubCard from "./HubCard";

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
        <div className="hub-results">
          {popular.map((r) => {
            const cardKey = `popular:${r.repo_id}`;
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
      {popular && popular.length === 0 && !loading && (
        <p className="section-hint">No popular models returned.</p>
      )}
    </section>
  );
}
