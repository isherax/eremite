# Eremite Design Decisions

This document captures the rationale behind Eremite's technology choices, organized by the project's core value propositions.

## 1. Lightweight and Performant

### Inference: llama.cpp via `llama-cpp-2` Rust bindings

llama.cpp is a battle-tested C++ inference engine with excellent quantization support (Q4, Q5, Q8, etc.), Metal acceleration on macOS, and active development. Rust bindings add minimal overhead while giving the surrounding application code memory safety.

The `llama-cpp-2` crate (from [`utilityai/llama-cpp-rs`](https://github.com/utilityai/llama-cpp-rs)) provides safe wrappers around nearly direct bindings to llama.cpp. It compiles llama.cpp from source via its `llama-cpp-sys-2` sys crate, so Metal GPU support on macOS is automatic with no feature flag required.

**Alternatives considered:**

- **Candle** (Hugging Face's pure Rust ML framework): Keeps the entire stack in Rust, but has narrower model coverage and is less mature than llama.cpp.
- **Burn** (Rust ML framework): Similar trade-offs to Candle. Less ecosystem support for LLM-specific workloads.
- **`llama_cpp-rs`** (edgenai): Higher-level Rust bindings for llama.cpp, but last updated in June 2024. The `llama-cpp-2` crate is actively maintained with frequent releases tracking upstream llama.cpp changes.

llama.cpp was chosen for its broad model compatibility, proven Metal acceleration, and active community. `llama-cpp-2` was chosen over `llama_cpp-rs` for active maintenance and closer alignment with upstream.

### Token streaming: synchronous callbacks

`eremite-inference` streams generated tokens to callers via a synchronous `FnMut(InferenceEvent)` callback. This matches the progress callback pattern already used in `eremite-models` for download progress.

**Alternatives considered:**

- **`tokio::sync::mpsc` channel**: More natural for async consumers, but would force `eremite-inference` to depend on `tokio`. The inference crate is a pure synchronous compute crate -- adding an async runtime dependency contradicts the "no unnecessary dependencies" principle and couples it to a specific executor.
- **Returning a `Stream` / `Iterator`**: Clean API but harder to implement efficiently with the llama.cpp decode loop, which is inherently push-based.

Callbacks keep the crate dependency-free with respect to async runtimes. Callers that need async (e.g., `eremite-core` bridging to Tauri events) capture a channel sender in the callback closure.

### Chat template application in the inference crate

`eremite-inference` exposes both a raw `generate(prompt)` method and a `generate_chat(messages)` method. The chat method uses the model's embedded chat template (read by llama.cpp from the GGUF metadata) to format conversations.

**Alternatives considered:**

- **Formatting in `eremite-core`**: Would separate conversation logic from inference, but requires core to reimplement per-model chat template parsing -- a fragile process since templates vary significantly across model families (ChatML, Llama, Mistral, etc.).
- **Exposing only the raw template string**: Flexible but pushes complexity to the caller and duplicates work that llama.cpp already handles correctly.

The model knows its own template best. Exposing both raw and chat-aware APIs gives `eremite-core` full flexibility without forcing it to reimplement template logic.

### Model format: GGUF (quantized)

GGUF is the standard format for quantized models used by llama.cpp. 4-bit quantized 7B-parameter models run comfortably in ~4-6GB of RAM on consumer hardware, making local inference practical without specialized equipment.

### Application shell: Tauri v2

Tauri v2 uses the operating system's native webview (WebKit on macOS) rather than bundling a browser engine like Electron does. This keeps the binary size small (~10MB) and memory usage low. Tauri also provides first-class support for macOS, Windows, Linux, iOS, and Android.

**Alternatives considered:**

- **Electron**: Large binary size (~150MB+), high memory overhead from bundled Chromium. Contradicts the lightweight goal.
- **Dioxus** (Rust-native UI): Nearly all-Rust stack, but the ecosystem is smaller and mobile support is experimental.
- **SwiftUI + Rust FFI**: Best native macOS performance, but zero cross-platform code reuse. Would require separate UI implementations for every platform.

Tauri was chosen for its small footprint, Rust backend integration, and cross-platform support.

### Frontend: React with TypeScript

React provides the largest frontend ecosystem, with ready-made solutions for markdown rendering, syntax highlighting, and chat UI patterns. The ~40KB runtime is negligible in a desktop app loaded from local disk. Vite provides fast hot module replacement during development.

**Alternatives considered:**

- **Svelte**: Compiles away to vanilla JS with zero runtime overhead. Theoretically lighter, but the performance difference is imperceptible for a chat-style UI where the bottleneck is inference speed, not rendering. Smaller ecosystem and contributor pool.
- **SolidJS**: Excellent performance (no virtual DOM), React-like syntax. Smallest ecosystem of the three, which means fewer off-the-shelf components.

React was chosen for ecosystem depth and contributor accessibility. The performance difference between frameworks is not meaningful for this application's UI patterns.

### Cargo workspace

The Rust code is split into separate crates (`eremite-core`, `eremite-inference`, `eremite-models`). This keeps dependency trees isolated -- the inference crate doesn't pull in UI dependencies and vice versa -- and allows independent compilation and testing.

### Crate dependency boundaries: core does not depend on models

`eremite-core` depends only on `eremite-inference`. It does **not** depend on `eremite-models`. This keeps the entire dependency tree for core and inference free of networking crates (`reqwest`, `hyper`, `tokio`, etc.) -- a structural guarantee, not just a convention.

Core accepts model file paths (`&Path`) directly. The `src-tauri` layer is the integration point that wires `eremite-models` (for model discovery and downloads) with `eremite-core` (for inference orchestration), passing model paths from one to the other.

**Alternatives considered:**

- **Core depends on `eremite-models` for local queries only (list, get, model_path)**: Simpler API surface for model loading, but `reqwest` and `tokio` would appear in core's transitive dependency tree via `eremite-models`. The "no networking" claim would rely on code-level discipline rather than structural enforcement.
- **Feature-gated dependency**: `eremite-models` could expose a `no-network` feature that excludes `reqwest`/`tokio`, but feature flags add complexity and are easy to misconfigure.

Keeping the dependency boundary at the crate level makes the privacy guarantee trivially auditable: inspect `eremite-core/Cargo.toml` and confirm `eremite-models` is absent.

## 2. Private

### Offline by design

Privacy in Eremite is not an added feature -- it is a structural property of the codebase. The `eremite-inference` and `eremite-core` crates have zero network dependencies -- not even transitively. `eremite-core` depends only on `eremite-inference`, and neither crate depends on `eremite-models`. Once a model is downloaded, the application never contacts the internet. There is no telemetry, no account system, and no server dependency.

The only crate with network access is `eremite-models`, which downloads models from the public Hugging Face Hub over HTTPS. This is the sole network interaction in the entire application. The `src-tauri` layer is the only place that depends on both `eremite-models` and `eremite-core`, bridging model paths from one to the other.

### Open source as proof

The MIT license and open codebase mean anyone can audit the privacy claims. The structural guarantee is verifiable: inspect the `Cargo.toml` files for `eremite-inference` and `eremite-core` to confirm that neither depends on `eremite-models` and no networking crates (`reqwest`, `hyper`, etc.) appear anywhere in their dependency trees.

## 3. Free and Open Source

- **MIT licensed**: Permissive license with no restrictions on use, modification, or distribution.
- **No telemetry**: The application collects no usage data.
- **No accounts**: No sign-up, no login, no server interaction beyond model downloads.
- **Public model source**: Models are downloaded from the public Hugging Face Hub. No proprietary model registries or gated access.
- **No vendor lock-in**: Every layer of the stack uses open standards and open source tooling.
