import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { HubSearchResult } from "../types/model";

const POPULAR_CACHE_KEY = "eremite_popular_models_v1";

function isHubSearchResultArray(value: unknown): value is HubSearchResult[] {
  if (!Array.isArray(value)) return false;
  return value.every(
    (item) =>
      typeof item === "object" &&
      item !== null &&
      typeof (item as { repo_id?: unknown }).repo_id === "string" &&
      Array.isArray((item as { gguf_files?: unknown }).gguf_files),
  );
}

interface PopularModelsState {
  popular: HubSearchResult[] | null;
  loading: boolean;
  error: string | null;
}

export function usePopularModels(): PopularModelsState {
  const [popular, setPopular] = useState<HubSearchResult[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const raw = sessionStorage.getItem(POPULAR_CACHE_KEY);
    if (raw) {
      try {
        const parsed: unknown = JSON.parse(raw);
        if (isHubSearchResultArray(parsed)) {
          setPopular(parsed);
          return;
        }
      } catch {
        /* ignore bad cache */
      }
    }

    let cancelled = false;
    setLoading(true);
    setError(null);

    (async () => {
      try {
        const list = await invoke<HubSearchResult[]>("popular_models");
        if (cancelled) return;
        setPopular(list);
        try {
          sessionStorage.setItem(POPULAR_CACHE_KEY, JSON.stringify(list));
        } catch {
          /* storage full or disabled */
        }
      } catch (err) {
        if (!cancelled) setError(`Could not load popular models: ${err}`);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, []);

  return { popular, loading, error };
}
