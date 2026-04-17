import type { HubSearchResult } from "../types/model";
import HubCard from "./HubCard";

interface HubListProps {
  section: "popular" | "search";
  results: HubSearchResult[];
  expandedKey: string | null;
  onToggleExpand: (repoId: string) => void;
  onDownload: (repoId: string, filename: string) => void;
  downloading: boolean;
  className?: string;
}

export default function HubList({
  section,
  results,
  expandedKey,
  onToggleExpand,
  onDownload,
  downloading,
  className,
}: HubListProps) {
  return (
    <div className={className ? `hub-results ${className}` : "hub-results"}>
      {results.map((r) => {
        const cardKey = `${section}:${r.repo_id}`;
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
  );
}
