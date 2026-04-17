import { useEffect, useState } from "react";
import type { DownloadProgress } from "../types/model";
import { useTauriEvent } from "../hooks/useTauriEvent";

/**
 * Subscribes to `download:progress` while `enabled` is true and exposes the
 * latest payload. Clears progress when disabled.
 */
export function useDownloadProgress(enabled: boolean): DownloadProgress | null {
  const [progress, setProgress] = useState<DownloadProgress | null>(null);

  useTauriEvent<DownloadProgress>("download:progress", setProgress, enabled);

  useEffect(() => {
    if (!enabled) setProgress(null);
  }, [enabled]);

  return progress;
}
