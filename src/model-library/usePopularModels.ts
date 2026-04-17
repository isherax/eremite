import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { HubSearchResult } from "../types/model";

const POPULAR_CACHE_KEY = "eremite_popular_models_v1";

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
        const parsed = JSON.parse(raw) as unknown;
        if (Array.isArray(parsed)) {
          setPopular(parsed as HubSearchResult[]);
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
