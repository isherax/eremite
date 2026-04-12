# Eremite Design Decisions

This document captures the rationale behind Eremite's technology choices, organized by the project's core value propositions.

## 1. Lightweight and Performant

### Inference: llama.cpp via Rust bindings

llama.cpp is a battle-tested C++ inference engine with excellent quantization support (Q4, Q5, Q8, etc.), Metal acceleration on macOS, and active development. Rust bindings add minimal overhead while giving the surrounding application code memory safety.

**Alternatives considered:**

- **Candle** (Hugging Face's pure Rust ML framework): Keeps the entire stack in Rust, but has narrower model coverage and is less mature than llama.cpp.
- **Burn** (Rust ML framework): Similar trade-offs to Candle. Less ecosystem support for LLM-specific workloads.

llama.cpp was chosen for its broad model compatibility, proven Metal acceleration, and active community.

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

## 2. Private

### Offline by design

Privacy in Eremite is not an added feature -- it is a structural property of the codebase. The `eremite-inference` and `eremite-core` crates have zero network dependencies. Once a model is downloaded, the application never contacts the internet. There is no telemetry, no account system, and no server dependency.

The only crate with network access is `eremite-models`, which downloads models from the public Hugging Face Hub over HTTPS. This is the sole network interaction in the entire application.

### Open source as proof

The MIT license and open codebase mean anyone can audit the privacy claims. The structural guarantee is verifiable: inspect the `Cargo.toml` files for `eremite-inference` and `eremite-core` to confirm that no networking crates (`reqwest`, `hyper`, etc.) appear in their dependency trees.

## 3. Free and Open Source

- **MIT licensed**: Permissive license with no restrictions on use, modification, or distribution.
- **No telemetry**: The application collects no usage data.
- **No accounts**: No sign-up, no login, no server interaction beyond model downloads.
- **Public model source**: Models are downloaded from the public Hugging Face Hub. No proprietary model registries or gated access.
- **No vendor lock-in**: Every layer of the stack uses open standards and open source tooling.
