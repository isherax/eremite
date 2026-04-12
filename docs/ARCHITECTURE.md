# Eremite Architecture

Eremite is a local LLM application that lets users download open source models and run them entirely on their own hardware. This document describes the system architecture.

## Overview

```mermaid
graph TD
    UI["React Frontend"] -->|"Tauri IPC (commands + events)"| TauriShell["src-tauri"]
    TauriShell --> Core["eremite-core"]
    TauriShell --> ModelMgr["eremite-models"]
    Core --> Inference["eremite-inference (llama.cpp)"]
    ModelMgr -->|"HTTPS downloads only"| HFHub["Hugging Face Hub"]
    Inference --> Metal["Metal GPU"]
```

## Components

### eremite-core

Core engine library. Manages conversation state, configuration, and inference orchestration. All application logic lives here -- not in the frontend or the Tauri layer.

`eremite-core` depends only on `eremite-inference` -- it does **not** depend on `eremite-models`. This keeps networking crates (`reqwest`, `tokio`, etc.) completely out of core's dependency tree. Core accepts model file paths directly (`&Path`); the Tauri layer resolves those paths via `eremite-models` before passing them to core.

The crate defines an `InferenceProvider` trait that abstracts the inference boundary, allowing core to be tested with a mock implementation that requires no GPU or GGUF model. The production implementation (`LlamaInference`) wraps `eremite-inference::InferenceEngine`.

### eremite-inference

Wraps [llama.cpp](https://github.com/ggerganov/llama.cpp) via the [`llama-cpp-2`](https://github.com/utilityai/llama-cpp-rs) Rust bindings. Handles model loading, tokenization, sampling, and inference. Targets the GGUF model format with Metal GPU acceleration on macOS.

The public API centers on `InferenceEngine`, which loads a GGUF model and exposes two generation methods:

- `generate(prompt, params, callback)` -- raw text completion from a prompt string.
- `generate_chat(messages, params, callback)` -- applies the model's embedded chat template to a list of `ChatMessage` structs, then generates. Using the model's own template avoids the fragile and error-prone task of reimplementing per-model formatting outside the inference layer.

Both methods stream tokens to the caller via a synchronous `FnMut(InferenceEvent)` callback. This keeps the crate free of async runtime dependencies (`tokio`, etc.) and matches the callback pattern used in `eremite-models` for download progress. Callers that need async (e.g., `eremite-core` bridging to Tauri events) wrap the callback with a channel sender.

This crate has **no network or async runtime dependencies**. It is fully offline by design.

### eremite-models

Downloads GGUF models from Hugging Face Hub, manages local model storage (`~/.eremite/models/`), and tracks model metadata.

This is the **only crate with network access** in the entire project.

### src-tauri

The Tauri v2 application shell. Wires `eremite-core`'s Rust API to Tauri commands and events for the frontend to consume. This is the integration layer that connects `eremite-models` (model discovery and download) with `eremite-core` (inference orchestration) -- for example, resolving a model's on-disk path via `ModelManager::model_path()` and passing it to `CoreEngine::load_model()`.

### React Frontend

Presentation layer inside Tauri's system webview (WebKit on macOS). Lives in `src/` following Tauri conventions. Handles UI rendering and user interaction only.

## Data Flows

### Model Download

1. User browses or searches for models in the UI.
2. UI issues a Tauri command to `src-tauri`.
3. `eremite-models` fetches the GGUF file from Hugging Face Hub over HTTPS.
4. The model is stored locally in `~/.eremite/models/`.
5. Model metadata is registered in a local manifest.

### Inference

1. User sends a prompt via the UI.
2. UI issues a Tauri command to `src-tauri`.
3. `src-tauri` calls into `eremite-core`.
4. `eremite-core` delegates to `eremite-inference`, which runs llama.cpp.
5. Tokens are streamed back to the UI via Tauri events.
6. The UI renders tokens incrementally as they arrive.

**Zero network access is involved during inference.**

## Repository Structure

```
eremite/
  src/                         # React + TypeScript frontend (Tauri default location)
  src-tauri/                   # Tauri app entry point, commands, config
    src/
      main.rs
      lib.rs                   # Tauri command handlers
    Cargo.toml                 # Depends on eremite-core, eremite-inference, eremite-models
    tauri.conf.json
  crates/
    eremite-core/              # Core engine library (all application logic lives here)
      src/
        lib.rs                 # Public API re-exports
        config.rs              # CoreConfig: inference defaults, system prompt
        conversation.rs        # Conversation, Message, ConversationId
        inference.rs           # InferenceProvider trait + LlamaInference impl
        engine.rs              # CoreEngine: orchestration, conversation CRUD, send_message
      tests/
        engine.rs              # Integration tests with MockInference (no GPU required)
    eremite-inference/         # llama.cpp bindings, inference logic (offline only)
      src/
        lib.rs                 # Public API re-exports
        engine.rs              # InferenceEngine: load, generate, generate_chat
        params.rs              # InferenceParams, ChatMessage
        event.rs               # InferenceEvent enum for callbacks
      tests/
        inference.rs           # Integration tests (require a real GGUF model, #[ignore])
    eremite-models/            # Model download and management (only crate with network)
      src/
        lib.rs                 # Public API: ModelManager
        download.rs            # HTTP download, SHA-256 hashing, progress callback
        manifest.rs            # Manifest persistence (JSON), ModelEntry
      tests/
        manager.rs             # Integration tests with wiremock (no real network)
  docs/                        # Architecture and design docs
  index.html                   # Vite entry point
  package.json                 # Frontend dependencies
  vite.config.ts
  tsconfig.json
  Cargo.toml                   # Workspace root: members = ["src-tauri", "crates/*"]
  LICENSE
  README.md
```

This follows standard Tauri conventions (`src/`, `src-tauri/`, root-level frontend config) with an added `crates/` directory for the Rust library code. Tauri CLI commands (`npx tauri dev`, `npx tauri build`) work without extra configuration.

The Cargo workspace keeps crates isolated. `eremite-core` depends only on `eremite-inference`, and `eremite-inference` has no network or async runtime dependencies. Neither crate depends on `eremite-models`, so networking crates (`reqwest`, `hyper`, `tokio`, etc.) never appear in their dependency trees -- this is the structural privacy guarantee, verifiable by inspecting their `Cargo.toml` files.

## Technology Stack

| Layer | Technology | Role |
|---|---|---|
| Inference | llama.cpp (via `llama-cpp-2` Rust bindings) | Model loading, tokenization, inference, Metal GPU |
| Core | Rust | Application logic, state management, orchestration |
| App shell | Tauri v2 | Native window, IPC, system integration |
| Frontend | React + TypeScript + Vite | UI rendering, user interaction |
| Models | GGUF format | Quantized model storage and loading |
| Model source | Hugging Face Hub | Public model downloads |

## Testing

### Principles

- **Configurable, not hardcoded.** Structs accept paths and URLs as constructor parameters rather than hardcoding values like `~/.eremite/` or `https://huggingface.co`. Tests pass temp directories and mock server URLs.
- **No network or GPU in default `cargo test`.** Tests requiring real network or hardware use `#[ignore]`. Default `cargo test` is fast and runs anywhere, including CI without a GPU.
- **Mock at the network level.** Use `wiremock` to spin up a real HTTP server on localhost rather than introducing traits/generics just for testing. The production HTTP client (`reqwest`) is used in tests -- only the URL changes.
- **Traits at crate boundaries when needed.** When one crate depends on another's behavior (e.g., `eremite-core` using inference), define a trait for that boundary so the dependent crate can be tested with a mock implementation. Introduce these traits when the crates are built, not upfront.
- **Isolate for parallel execution.** `cargo test` runs tests in parallel by default. Each test that touches the filesystem uses its own `tempfile::TempDir` so concurrent tests never interfere with each other.

### Test Locations

- **Unit tests** live inline in each module as `#[cfg(test)] mod tests` with `use super::*;` to access the parent module's items. These test internal logic: serialization, path construction, hashing, state management, etc. They can test private functions directly.
- **Integration tests** live in each crate's `tests/` directory. Each file compiles as a separate crate with access to the **public API only**. These verify end-to-end workflows within a crate using mocked externals.
- **Doc tests** live in `///` doc comments on public types and functions. `cargo test` runs code examples in doc comments automatically, keeping documentation accurate. Add these to key public API entry points.
- **Ignored tests** (`#[ignore]`) cover scenarios that need real external resources (network, GPU). Run manually or on a schedule, not on every PR.
- **Shared test utilities** start as local `#[cfg(test)]` helpers within each crate. If multiple crates need the same helpers, extract them into a `crates/eremite-test-utils/` crate and add it as a `[dev-dependency]` in each consuming crate's `Cargo.toml`.

### CI

- `cargo test --workspace` -- runs all non-ignored tests (unit, integration, doc). PR gate.
- `cargo test --workspace -- --ignored` -- runs network/GPU tests. Scheduled or manual.

## Platform Support

The initial target is **macOS** with Metal GPU acceleration. The architecture supports future expansion to:

- Windows (Tauri + Vulkan/CUDA for inference)
- Linux (Tauri + Vulkan/CUDA for inference)
- iOS and Android (Tauri v2 mobile support)
