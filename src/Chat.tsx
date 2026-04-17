import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import MarkdownContent from "./MarkdownContent";
import type { ModelInfo, ModelRef } from "./types/model";
import { formatLoadingModelName } from "./utils/format";

interface ChatMessage {
  id: number;
  role: "user" | "assistant";
  content: string;
}

type ChatStatus = "loading" | "ready" | "generating";

interface ChatProps {
  model: ModelInfo | null;
  loadingModel: ModelRef | null;
}

export default function Chat({ model, loadingModel }: ChatProps) {
  const [status, setStatus] = useState<ChatStatus>(model ? "ready" : "loading");
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [streamingContent, setStreamingContent] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const streamBufferRef = useRef("");
  const rafRef = useRef(0);
  const nextMessageIdRef = useRef(1);

  useEffect(() => {
    if (model) {
      setStatus("ready");
    }
  }, [model]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({
      behavior: status === "generating" ? "instant" : "smooth",
    });
  }, [messages, streamingContent, status]);

  function appendMessage(role: "user" | "assistant", content: string) {
    setMessages((prev) => [
      ...prev,
      { id: nextMessageIdRef.current++, role, content },
    ]);
  }

  async function handleSend() {
    const content = input.trim();
    if (!content || status !== "ready") return;

    appendMessage("user", content);
    setInput("");
    if (inputRef.current) inputRef.current.style.height = "auto";
    setStatus("generating");
    setStreamingContent("");

    const unlisteners: UnlistenFn[] = [];

    try {
      const unlistenToken = await listen<string>(
        "inference:token",
        (event) => {
          streamBufferRef.current += event.payload;
          if (!rafRef.current) {
            rafRef.current = requestAnimationFrame(() => {
              setStreamingContent(streamBufferRef.current);
              rafRef.current = 0;
            });
          }
        },
      );
      unlisteners.push(unlistenToken);

      const fullResponse = await invoke<string>("send_message", { content });

      appendMessage("assistant", fullResponse);
    } catch (err) {
      appendMessage("assistant", `Error: ${err}`);
    } finally {
      unlisteners.forEach((unlisten) => unlisten());
      if (rafRef.current) cancelAnimationFrame(rafRef.current);
      streamBufferRef.current = "";
      rafRef.current = 0;
      setStreamingContent("");
      setStatus("ready");
      inputRef.current?.focus();
    }
  }

  const adjustHeight = useCallback(() => {
    const ta = inputRef.current;
    if (!ta) return;
    ta.style.height = "auto";
    ta.style.height = `${ta.scrollHeight}px`;
  }, []);

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  }

  const loadingName = formatLoadingModelName(loadingModel);
  const isGenerating = status === "generating";

  return (
    <>
      {status === "loading" ? (
        <main className="messages" aria-busy="true" aria-label="Chat">
          <div className="loading-state">
            <p>Loading {loadingName}...</p>
          </div>
        </main>
      ) : (
        <main
          className="messages"
          aria-busy={isGenerating}
          aria-label="Chat messages"
        >
          {messages.length === 0 && status === "ready" && (
            <div className="empty-state">
              <p>Send a message to start chatting.</p>
            </div>
          )}

          {messages.map((msg) => (
            <div key={msg.id} className={`message ${msg.role}`}>
              <div className="message-content">
                {msg.role === "assistant" ? (
                  <MarkdownContent content={msg.content} />
                ) : (
                  msg.content
                )}
              </div>
            </div>
          ))}

          {isGenerating && streamingContent && (
            <div className="message assistant">
              <div className="message-content">
                <MarkdownContent content={streamingContent} />
              </div>
            </div>
          )}

          {isGenerating && !streamingContent && (
            <div className="message assistant">
              <div className="message-content thinking">Thinking...</div>
            </div>
          )}

          <div ref={messagesEndRef} />
        </main>
      )}

      <footer className="input-area">
        <textarea
          ref={inputRef}
          className="input"
          value={input}
          onChange={(e) => {
            setInput(e.target.value);
            adjustHeight();
          }}
          onKeyDown={handleKeyDown}
          placeholder="Type a message..."
          aria-label="Message input"
          disabled={status !== "ready"}
          rows={1}
        />
        <button
          className="send-button"
          onClick={handleSend}
          disabled={status !== "ready" || !input.trim()}
        >
          Send
        </button>
      </footer>
    </>
  );
}
