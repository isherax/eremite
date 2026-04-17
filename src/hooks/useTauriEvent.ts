import { useEffect, useRef } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/**
 * Subscribe to a Tauri event for the lifetime of a component.
 *
 * Handles the common async-subscribe race: if the component unmounts
 * before `listen()` resolves, the returned unlisten function is still
 * called so the listener is never leaked.
 *
 * The subscription is (re)created when `event` or `enabled` change.
 * The handler itself may change between renders without causing a
 * re-subscribe; the latest handler is always invoked via a ref.
 *
 * Pass `enabled: false` to skip subscription entirely.
 */
export function useTauriEvent<T>(
  event: string,
  handler: (payload: T) => void,
  enabled: boolean = true,
): void {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    if (!enabled) return;

    let cancelled = false;
    let unlisten: UnlistenFn | undefined;

    listen<T>(event, (e) => handlerRef.current(e.payload))
      .then((u) => {
        if (cancelled) u();
        else unlisten = u;
      })
      .catch((err) => {
        console.error(`listen(${event}) failed`, err);
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [event, enabled]);
}
