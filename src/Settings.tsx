import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export const SYSTEM_PROMPT_STORAGE_KEY = "eremite.systemPrompt";

export default function Settings() {
  const [saved, setSaved] = useState<string>(() => {
    if (typeof window === "undefined") return "";
    return window.localStorage.getItem(SYSTEM_PROMPT_STORAGE_KEY) ?? "";
  });
  const [draft, setDraft] = useState<string>(saved);
  const [status, setStatus] = useState<"idle" | "saving" | "saved" | "error">(
    "idle",
  );
  const [error, setError] = useState<string | null>(null);
  const savedTimerRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (savedTimerRef.current !== null) {
        window.clearTimeout(savedTimerRef.current);
      }
    };
  }, []);

  const isDirty = draft !== saved;

  async function handleSave() {
    setStatus("saving");
    setError(null);
    try {
      await invoke("set_system_prompt", { prompt: draft });
      window.localStorage.setItem(SYSTEM_PROMPT_STORAGE_KEY, draft);
      setSaved(draft);
      setStatus("saved");

      if (savedTimerRef.current !== null) {
        window.clearTimeout(savedTimerRef.current);
      }
      savedTimerRef.current = window.setTimeout(() => {
        setStatus("idle");
        savedTimerRef.current = null;
      }, 1800);
    } catch (err) {
      setStatus("error");
      setError(`Failed to save: ${err}`);
    }
  }

  function handleClear() {
    setDraft("");
  }

  return (
    <main className="settings" aria-label="Settings">
      <section className="settings-section">
        <div className="settings-field">
          <label className="settings-label" htmlFor="system-prompt">
            System prompt
          </label>
          <p className="settings-help">
            Prepended to every conversation. Changes take effect on the next
            message you send.
          </p>
          <textarea
            id="system-prompt"
            className="form-input settings-textarea"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            placeholder="You are a helpful assistant."
            rows={8}
            spellCheck
          />
        </div>

        <div className="settings-actions">
          <button
            type="button"
            className="action-button primary"
            onClick={handleSave}
            disabled={!isDirty || status === "saving"}
          >
            {status === "saving" ? "Saving..." : "Save"}
          </button>
          <button
            type="button"
            className="action-button danger"
            onClick={handleClear}
            disabled={draft === ""}
          >
            Clear
          </button>
          {status === "saved" && (
            <span className="settings-status" role="status">
              Saved
            </span>
          )}
          {status === "error" && error && (
            <span className="settings-status error" role="alert">
              {error}
            </span>
          )}
        </div>
      </section>
    </main>
  );
}
