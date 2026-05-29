# Contributing to HarnessKit

Thanks for your interest in contributing!

HarnessKit is a Cargo workspace containing four Rust crates and a React + Vite frontend in `src/`. The desktop app is packaged with [Tauri](https://tauri.app/); the CLI embeds the built frontend via [rust-embed](https://crates.io/crates/rust-embed) to serve it in web mode.

## Prerequisites

- **Node.js** ≥ 20 (Vitest 4 requires Node ≥ 20; Node 18 is end-of-life)
- **Rust** 1.85+ (edition 2024) — install via [rustup](https://rustup.rs/)
- **Tauri CLI** (only for desktop development): `cargo install tauri-cli --version "^2.0.0"`
- **Xcode Command Line Tools** (macOS only): `xcode-select --install`

This project uses **npm**, not pnpm or yarn.

## Getting Started

```bash
git clone https://github.com/RealZST/HarnessKit.git
cd HarnessKit
npm ci
```

Use `npm ci` rather than `npm install`: it installs exactly what `package-lock.json` pins, keeping the `@tauri-apps/*` packages aligned with their matching Rust crates. Re-run it after every `git pull` / `git rebase` to pick up newly added dependencies. If you hit a Tauri version-mismatch error, see [Troubleshooting](#troubleshooting).

### Web Mode Development (macOS / Linux / Windows)

Two terminals — Vite dev server + Rust backend:

```bash
# Terminal A
npm run dev                                  # http://localhost:1420 (HMR)

# Terminal B
cargo run -p hk-cli -- serve                 # http://127.0.0.1:7070
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
├── pages/           Route pages (Overview, Kits, Agents, Extensions, Marketplace, Audit, Settings)
├── components/      Shared UI components
├── stores/          Zustand stores
├── hooks/           Custom React hooks
├── lib/             Utils, API client, type definitions
└── types/           Shared TypeScript type definitions

public/              Static assets
```

## Tests

```bash
npm test                    # frontend tests (vitest)
cargo test --workspace      # Rust tests
```

## Troubleshooting

### `Found version mismatched Tauri packages`

Tauri's CLI requires each npm `@tauri-apps/*` package and its matching Rust crate to share the same major and minor version (patch may differ) — the core pair is `@tauri-apps/api` ↔ the `tauri` crate, and each `@tauri-apps/plugin-<name>` ↔ its `tauri-plugin-<name>` crate. Running `npm install` instead of `npm ci` can bump a package within its `^` range while `Cargo.lock` stays pinned, desyncing the two. A mismatch errors out `cargo tauri build` (and the mobile build/run commands) and is logged as an error on `cargo tauri dev` (which still starts); pass `--ignore-version-mismatches` to build anyway when it is a known false positive.

- **Accidental drift** — you ran `npm install` and the lockfiles diverged: restore the tracked lockfiles and reinstall with `git restore Cargo.lock package-lock.json && npm ci`.
- **Intentional plugin upgrade** — bump the npm package in `package.json`, then sync only the matching crate with `cargo update -p tauri-plugin-<name>` (e.g. `cargo update -p tauri-plugin-dialog`). Use a bare `cargo update` only when you intend to refresh every dependency.

### `Failed to resolve import` (Vite)

Upstream likely added a new frontend dependency (e.g. `react-i18next`). Run `npm ci` to install it.

## Pull Requests

- Create a feature branch from `main` (e.g. `fix/marketplace-loading` or `feat/new-agent`)
- Use Conventional Commits in commit messages — `feat:`, `fix:`, `chore:`, `docs:`, `refactor:`
- Ensure `npm test` and `cargo test --workspace` pass before opening a PR
- Write a clear PR description: what problem it solves and how
- For UI changes, include a screenshot or short video
- Small, focused PRs are easier to review than large ones — prefer splitting when possible
