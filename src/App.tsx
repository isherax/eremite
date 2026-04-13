import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import MarkdownContent from "./MarkdownContent";

interface ModelInfo {
  description: string;
  n_params: number;
  n_ctx_train: number;
}

interface ChatMessage {
  role: "user" | "assistant";
  content: string;
}

type AppStatus = "loading" | "ready" | "generating" | "error";

function formatParams(n: number): string {
  if (n >= 1_000_000_000) return `${(n / 1_000_000_000).toFixed(1)}B`;
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(0)}M`;
  return n.toLocaleString();
}

export default function App() {
  const [status, setStatus] = useState<AppStatus>("loading");
  const [error, setError] = useState<string | null>(null);
  const [model, setModel] = useState<ModelInfo | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [streamingContent, setStreamingContent] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streamingContent]);

  useEffect(() => {
    invoke<ModelInfo>("load_model")
      .then((info) => {
        setModel(info);
        setStatus("ready");
      })
      .catch((err) => {
        setError(String(err));
        setStatus("error");
      });
  }, []);

  async function handleSend() {
    const content = input.trim();
    if (!content || status !== "ready") return;

    setMessages((prev) => [...prev, { role: "user", content }]);
    setInput("");
    setStatus("generating");
    setStreamingContent("");

    const unlisteners: UnlistenFn[] = [];

    try {
      const unlistenToken = await listen<string>("inference:token", (event) => {
        setStreamingContent((prev) => prev + event.payload);
      });
      unlisteners.push(unlistenToken);

      const fullResponse = await invoke<string>("send_message", { content });

      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: fullResponse },
      ]);
    } catch (err) {
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: `Error: ${err}` },
      ]);
    } finally {
      unlisteners.forEach((unlisten) => unlisten());
      setStreamingContent("");
      setStatus("ready");
      inputRef.current?.focus();
    }
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  }

  if (status === "error") {
    return (
      <div className="app">
        <div className="error-screen">
          <h2>Failed to load model</h2>
          <p>{error}</p>
          <p className="hint">
            Set the <code>EREMITE_MODEL</code> environment variable to the path
            of a .gguf model file, then restart.
          </p>
        </div>
      </div>
    );
  }

  if (status === "loading") {
    return (
      <div className="app">
        <div className="loading-screen">
          <p>Loading model...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="app">
      <header className="header">
        <span className="model-name">{model?.description ?? "Eremite"}</span>
        {model && (
          <span className="model-meta">
            {formatParams(model.n_params)} params &middot; {model.n_ctx_train}{" "}
            ctx
          </span>
        )}
      </header>

      <main className="messages">
        {messages.length === 0 && status === "ready" && (
          <div className="empty-state">
            <p>Send a message to start chatting.</p>
          </div>
        )}

        {messages.map((msg, i) => (
          <div key={i} className={`message ${msg.role}`}>
            <div className="message-content">
              {msg.role === "assistant" ? (
                <MarkdownContent content={msg.content} />
              ) : (
                msg.content
              )}
            </div>
          </div>
        ))}

        {status === "generating" && streamingContent && (
          <div className="message assistant">
            <div className="message-content">
              <MarkdownContent content={streamingContent} />
            </div>
          </div>
        )}

        {status === "generating" && !streamingContent && (
          <div className="message assistant">
            <div className="message-content thinking">Thinking...</div>
          </div>
        )}

        <div ref={messagesEndRef} />
      </main>

      <footer className="input-area">
        <textarea
          ref={inputRef}
          className="input"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Type a message..."
          disabled={status === "generating"}
          rows={1}
        />
        <button
          className="send-button"
          onClick={handleSend}
          disabled={status === "generating" || !input.trim()}
        >
          Send
        </button>
      </footer>
    </div>
  );
}
