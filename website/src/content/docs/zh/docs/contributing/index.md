---
title: 参与贡献
description: 在本地搭建 HarnessKit，以 Web 模式或桌面应用运行它，运行测试，再提交 PR。
sidebar:
  order: 0
---
感谢你参与贡献！你可以从以下几个方面提供帮助：

- **报告 bug 或申请支持某个 agent**。[提交 issue](https://github.com/RealZST/HarnessKit/issues/new)，写明你的操作、结果以及所用的版本。
- **修复 bug 或添加功能**。按下面的步骤在本地搭建项目，再提交 PR。
- **支持一个新的 agent**。[编写 adapter](/zh/docs/contributing/writing-an-adapter/) 列出了需要修改的文件。
- **贡献一条每日一帖，或推荐收录一个 CLI**。[贡献每日一帖和 CLI 条目](/zh/docs/contributing/tips-and-cli-listings/)说明了各自的提交位置。
- **改进这套文档**。[编写文档](/zh/docs/contributing/writing-docs/) 介绍了文档的约定，也说明了发现页面错误时如何反馈。

HarnessKit 是一个 Cargo workspace，包含 4 个 Rust crate，前端是 `src/` 下的 React + Vite。桌面应用使用 [Tauri](https://tauri.app/) 打包。`hk-web` crate 用 [rust-embed](https://crates.io/crates/rust-embed) 内嵌构建好的前端（`dist/`），`hk-cli` 把 `hk-web` 作为普通 crate 链接进来，在 Web 模式下提供界面。

## 环境要求

- **Node.js** ≥ 20（Vitest 4 要求 Node ≥ 20，Node 18 已停止维护）
- **Rust** 1.88+（edition 2024），用 [rustup](https://rustup.rs/) 安装
- **Tauri CLI**（仅开发桌面应用时需要）：`cargo install tauri-cli --version "^2.0.0"`
- **Xcode Command Line Tools**（仅 macOS）：`xcode-select --install`

本项目使用 **npm**，不使用 pnpm 或 yarn。

## 本地搭建

```bash
git clone https://github.com/RealZST/HarnessKit.git
cd HarnessKit
npm ci
```

请使用 `npm ci`，不要使用 `npm install`：`npm ci` 严格按照 `package-lock.json` 锁定的版本安装，`@tauri-apps/*` 包才能与对应的 Rust crate 保持一致。每次 `git pull` / `git rebase` 之后都请重新运行一次，以安装新增的依赖。若遇到 Tauri 版本不匹配的报错，请参阅[问题排查](#问题排查)。

### Web 模式开发（macOS / Linux / Windows）

打开两个终端，分别运行 Vite 开发服务器和 Rust 后端：

```bash
# Terminal A
npm run dev                                  # http://localhost:1420 (HMR)

# Terminal B
cargo run -p hk-cli -- serve                 # http://127.0.0.1:7070
```

在浏览器中打开 `http://localhost:1420`。Vite 会把 `/api/*` 请求转发给 `:7070` 上的后端。

### 桌面应用开发（macOS / Windows）

```bash
cargo tauri dev
```

Tauri 会自动先运行 `npm run dev` 作为 before-dev 命令，再打开原生窗口。在 Windows 上，唯一需要额外安装的是 rustup 提示安装的 Visual Studio Build Tools（「使用 C++ 的桌面开发」）。`cargo tauri build` 会在 `target/release/bundle/` 下生成 NSIS 和 MSI 安装包。由于启用了 `createUpdaterArtifacts`，环境中还需要设置 `TAURI_SIGNING_PRIVATE_KEY`；若本地构建不需要 updater 产物，也可以改为传入 `--config '{"bundle":{"createUpdaterArtifacts":false}}'`。

## 构建发布版本

### macOS（两种架构，外加 CLI）

```bash
./build.sh
```

生成 Apple Silicon 和 Intel 两个 `.dmg` 安装包，以及 `hk` CLI 二进制文件。

### Windows（桌面安装包）

发布工作流（`release.yml` 里的 `release-desktop-windows`）在 Windows runner 上构建 NSIS 安装包，连同它的 updater 签名一起附到 GitHub release 上。如需在 Windows 本机构建，请参阅[桌面应用开发](#桌面应用开发macos--windows)。

### 只构建 CLI（任意平台）

```bash
npm run build                          # produce dist/ for rust-embed
cargo build --release -p hk-cli        # produces target/release/hk
```

## 项目结构

```text
crates/
├── hk-core/         Shared core: scanning, models, DB, agent adapters
├── hk-desktop/      Tauri desktop app (wraps hk-core + frontend)
├── hk-cli/          CLI binary (hk); includes `hk serve` for web mode
└── hk-web/          HTTP layer for web mode; embeds dist/ with rust-embed, linked into hk-cli as a crate

src/                 React frontend (shared by desktop app and web mode)
├── pages/           Route pages (Overview, Kits, Agents, Extensions, Marketplace, Audit, Settings)
├── components/      Shared UI components
├── stores/          Zustand stores
├── hooks/           Custom React hooks
├── lib/             Utils, API client, and the main types (types.ts)
└── types/           Kit types and i18next typings

public/              Static assets
```

## 测试

```bash
npm test                    # frontend tests (vitest)
npm run build               # type-checks (tsc) and builds the frontend (vite build)
cargo test --workspace      # Rust tests
cargo fmt --all             # Rust formatting (CI rejects unformatted code)
npm run lint                # optional: Biome lint for src/ (not run in CI)
```

CI 会在每个 PR 上运行前四项，其中格式化改为用 `cargo fmt --all -- --check` 检查。

需要独立主目录的测试会设置 `HARNESSKIT_HOME`，`hk_core::home_dir()` 会优先使用它，而非真实的主目录。它替换的是 HarnessKit 看到的整个主目录，而不仅是它的数据文件夹：`~/.harnesskit/` 和主目录下的每个 agent 文件夹都会随之改变。该变量仅用于测试隔离，因为在 Windows 上无法用 `HOME` 更改主目录。它不是迁移 HarnessKit 数据的正式方法。

## 问题排查

### `Found version mismatched Tauri packages`

Tauri CLI 要求每个 npm 包 `@tauri-apps/*` 与对应的 Rust crate 主版本号、次版本号都相同（补丁号可以不同）。最核心的一对是 `@tauri-apps/api` ↔ `tauri` crate，此外每个 `@tauri-apps/plugin-<name>` 都与各自的 `tauri-plugin-<name>` crate 成对。若使用了 `npm install` 而不是 `npm ci`，某个包可能在 `^` 范围内升级了版本，而 `Cargo.lock` 仍锁定在原来的版本，两边便不再匹配。版本不匹配时，`cargo tauri build`（以及移动端的 build/run 命令）会报错退出；`cargo tauri dev` 会记录一条错误日志，但仍能启动。若确定是误报，可传入 `--ignore-version-mismatches` 继续构建。

- **意外漂移**。运行 `npm install` 后，两个 lockfile 不再匹配。请恢复受版本控制的 lockfile 并重新安装：`git restore Cargo.lock package-lock.json && npm ci`。
- **有意升级 plugin**。先在 `package.json` 里升级 npm 包，再用 `cargo update -p tauri-plugin-<name>` 只同步对应的 crate（例如 `cargo update -p tauri-plugin-dialog`）。只有在打算刷新所有依赖时，才使用不带参数的 `cargo update`。

### `Failed to resolve import`（Vite）

通常是上游新增了前端依赖（例如 `react-i18next`）。运行 `npm ci` 安装即可。

## 提交 PR

- 从 `main` 创建一个功能分支（例如 `fix/marketplace-loading` 或 `feat/new-agent`）
- 提交信息遵循 Conventional Commits：`feat:`、`fix:`、`chore:`、`docs:`、`refactor:`
- 创建 PR 前，确认 `npm test`、`npm run build` 和 `cargo test --workspace` 都能通过，且运行 `cargo fmt --all` 后没有改动
- 在 PR 描述中写明：解决了什么问题，以及如何解决
- 改动涉及界面时，附上截图或短视频
- 小而专注的 PR 比大型 PR 更易于审阅，请尽量拆分

## 许可证

本项目以 [Apache-2.0](https://github.com/RealZST/HarnessKit/blob/main/LICENSE) 许可证发布。

美术资源（`public/icons/` 和 `src/components/shared/agent-mascot/`）**保留所有权利**（All Rights Reserved），不在 Apache-2.0 许可证的覆盖范围内。

所有产品名称、Logo 和商标均归各自所有者所有。HarnessKit 是独立项目，与任何 agent 厂商都没有关联，也未获得它们的背书。
