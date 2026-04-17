import { useCallback, useEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import MarkdownContent from "./MarkdownContent";
import type { ModelInfo, ModelRef } from "./types/model";
import { formatLoadingModelName } from "./utils/format";
import { useInferenceStream } from "./hooks/useInferenceStream";

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
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const nextMessageIdRef = useRef(1);

  const isGenerating = status === "generating";
  const streamingContent = useInferenceStream(isGenerating);

  useEffect(() => {
    if (model) {
      setStatus("ready");
      setMessages([]);
      nextMessageIdRef.current = 1;
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

    try {
      const fullResponse = await invoke<string>("send_message", { content });
      appendMessage("assistant", fullResponse);
    } catch (err) {
      appendMessage("assistant", `Error: ${err}`);
    } finally {
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

  function handleKeyDown(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  }

  const loadingName = formatLoadingModelName(loadingModel);

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
