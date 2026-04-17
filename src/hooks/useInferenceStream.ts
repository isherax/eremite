import { useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/**
 * Subscribes to `inference:token` while `active` is true and exposes the
 * accumulated streamed content. Buffers raw payload chunks through a single
 * `requestAnimationFrame` tick so React only re-renders once per paint even
 * at high token rates.
 *
 * When `active` flips back to false the accumulated content resets to "" and
 * any in-flight animation frame is cancelled. This matches how a new
 * inference request should start from a blank canvas.
 */
export function useInferenceStream(active: boolean): string {
  const [content, setContent] = useState("");
  const bufferRef = useRef("");
  const rafRef = useRef(0);

  useEffect(() => {
    if (!active) {
      if (rafRef.current) cancelAnimationFrame(rafRef.current);
      rafRef.current = 0;
      bufferRef.current = "";
      setContent("");
      return;
    }

    let cancelled = false;
    let unlisten: UnlistenFn | undefined;

    listen<string>("inference:token", (event) => {
      bufferRef.current += event.payload;
      if (!rafRef.current) {
        rafRef.current = requestAnimationFrame(() => {
          setContent(bufferRef.current);
          rafRef.current = 0;
        });
      }
    })
      .then((u) => {
        if (cancelled) u();
        else unlisten = u;
      })
      .catch((err) => {
        console.error("listen(inference:token) failed", err);
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [active]);

  return content;
}
