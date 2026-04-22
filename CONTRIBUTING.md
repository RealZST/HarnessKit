# Contributing to HarnessKit

Thanks for your interest in contributing!

HarnessKit is a Cargo workspace containing four Rust crates and a React + Vite frontend in `src/`. The desktop app is packaged with [Tauri](https://tauri.app/); the CLI embeds the built frontend via [rust-embed](https://crates.io/crates/rust-embed) to serve it in web mode.

## Prerequisites

- **Node.js** ≥ 18
- **Rust** 1.85+ (edition 2024) — install via [rustup](https://rustup.rs/)
- **Tauri CLI** (only for desktop development): `cargo install tauri-cli --version "^2.0.0"`
- **Xcode Command Line Tools** (macOS only): `xcode-select --install`

This project uses **npm**, not pnpm or yarn.

## Getting Started

```bash
git clone https://github.com/RealZST/HarnessKit.git
cd HarnessKit
npm install
```

### Web Mode Development (macOS / Linux / Windows)

Two terminals — Vite dev server + Rust backend:

```bash
# Terminal A
npm run dev                                  # http://localhost:1420 (HMR)

# Terminal B
cargo run -p hk-cli -- serve --no-open       # http://127.0.0.1:7070
```

Open `http://localhost:1420` in your browser. Vite proxies `/api/*` requests to the backend at `:7070`.

### Desktop App Development (macOS only)

```bash
cargo tauri dev
```

Tauri automatically runs `npm run dev` as a before-dev command and launches the native window.

## Building Releases

### macOS (both architectures + CLI)

```bash
./build.sh
```

Produces `.dmg` bundles for Apple Silicon and Intel, plus `hk` CLI binaries.

### CLI only (any platform)

```bash
npm run build                          # produce dist/ for rust-embed
cargo build --release -p hk-cli        # produces target/release/hk
```

## Project Layout

```
crates/
├── hk-core/         Shared core: scanning, models, DB, agent adapters
├── hk-desktop/      Tauri desktop app (wraps hk-core + frontend)
├── hk-cli/          CLI binary (hk); includes `hk serve` for web mode
└── hk-web/          HTTP layer for web mode (embedded into hk-cli via rust-embed)

src/                 React frontend (shared by desktop app and web mode)
├── pages/           Route pages (Overview, Agents, Extensions, Marketplace, Audit, Settings)
├── components/      Shared UI components
├── stores/          Zustand stores
├── hooks/           Custom React hooks
└── lib/             Utils, API client, type definitions

public/              Static assets
```

## Tests

```bash
npm test                    # frontend tests (vitest)
cargo test --workspace      # Rust tests
```

## Pull Requests

- Create a feature branch from `main` (e.g. `fix/marketplace-loading` or `feat/new-agent`)
- Use Conventional Commits in commit messages — `feat:`, `fix:`, `chore:`, `docs:`, `refactor:`
- Ensure `npm test` and `cargo test --workspace` pass before opening a PR
- Write a clear PR description: what problem it solves and how
- For UI changes, include a screenshot or short video
- Small, focused PRs are easier to review than large ones — prefer splitting when possible
