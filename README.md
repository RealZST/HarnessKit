<p align="center">
  <img src="public/icons/harnesskit-icons.png" alt="HarnessKit" width="160" />
</p>

<h1 align="center">HarnessKit</h1>

<p align="center">
  <strong>A free, open-source app to manage all your AI coding agents.</strong><br/>
  Extensions, configs, memory, rules, security — across every agent, from one place.
</p>

<p align="center">
  <a href="https://github.com/RealZST/HarnessKit/releases/latest"><img src="https://img.shields.io/github/v/release/RealZST/HarnessKit?style=flat-square&color=brightgreen" alt="Latest Release" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue?style=flat-square" alt="License" /></a>
  <img src="https://img.shields.io/badge/platform-macOS-lightgrey?style=flat-square" alt="Platform" />
</p>

<p align="center">
  <a href="#why-harnesskit">Why</a>&nbsp;&nbsp;&bull;&nbsp;&nbsp;<a href="#key-features">Features</a>&nbsp;&nbsp;&bull;&nbsp;&nbsp;<a href="#getting-started">Get Started</a>&nbsp;&nbsp;&bull;&nbsp;&nbsp;<a href="#roadmap">Roadmap</a>
</p>

<br/>

<p align="center">
  <img src="docs/demo.gif" alt="HarnessKit Demo" width="800" />
</p>

<br/>

## Why HarnessKit?

You use multiple AI coding agents (Claude Code, Codex, Gemini CLI, Cursor...) — maybe two, maybe five. Each one scatters its skills, MCP servers, plugins, hooks, agent-first CLIs, configs, memory, and rules across different directories, in different formats, with different conventions.

**You need one place to manage** — all types of extensions across agents, and every agent's configs, memory, and rules.

**HarnessKit is the control center for all of it.** One native desktop app to see, secure, and manage everything across every agent.

---

## Key Features

### 🧩 Full Suite Extension Management

Most tools only handle skills. HarnessKit manages **all five extension types** from a unified interface — **Skills**, **MCP Servers**, **Plugins**, **Hooks**, and **Agent-first CLIs**.

| Agent | Skills | MCP | Plugins | Hooks | Agent-first CLIs |
|:---|:---:|:---:|:---:|:---:|:---:|
| **Claude Code** | ✓ | ✓ | ✓ | ✓ | ✓ |
| **Codex** | ✓ | ✓ | ✓ | ✓ | ✓ |
| **Gemini CLI** | ✓ | ✓ | ✓ | ✓ | ✓ |
| **Cursor** | ✓ | ✓ | ✓ | ✓ | ✓ |
| **Antigravity** | ✓ | ✓ | — | — | ✓ |
| **Copilot** | ✓ | ✓ | ✓ | ✓ | ✓ |

- **Smart organization** — Filter by type, agent, or source, and search by name. Extensions from the same repo are automatically grouped into **Packs** for batch management.
- **Full visibility** — Every extension shows its agents, permissions, trust score, and status at a glance. Open the detail panel for per-agent file paths, directory structure, and audit findings.
- **Effortless management** — Enable or disable right from the list. Check for updates across all extensions with one click.
- **Cross-agent deployment** — See which agents have the extension and which don't — deploy to any missing agent with one click. HarnessKit handles the format differences between agents (JSON, TOML, hook conventions, MCP schemas) automatically.

---

### 🤖 Agent Configs, Memory & Rules

HarnessKit manages every agent's **Configs**, **Memory**, **Rules**, and **Ignore** files from one place. Currently supporting **6 agents**: **Claude Code**, **Codex**, **Gemini CLI**, **Cursor**, **Antigravity**, and **Copilot**.

- **Config file tracking** — Automatically discovers every agent's config files — both global and per-project. Add your project directories or custom paths and HarnessKit picks them up alongside the global ones.
- **Per-agent dashboard** — Each agent gets its own page with all files organized by category, showing scope, path, file size, and a summary of installed extensions. Expand any file to preview its content right in the app.
- **Custom paths** — Add any file or folder to an agent's dashboard for tracking. Useful for custom configs or scripts that HarnessKit doesn't auto-discover — they show up alongside everything else with the same live preview.
- **Real-time detection** — The moment a config file is modified, the dashboard reflects it.


---

### 🛡️ Security Audit & Permission Transparency

Every extension is scanned by a built-in security engine with 17 static analysis rules and receives a **Trust Score** (0–100), grouped into three tiers — **Safe** (80+), **Low Risk** (60–79), and **Needs Review** (below 60). A dedicated Audit page lets you search, filter by tier, and drill into every finding.

- **One-click audit** — Run a full security scan across all extensions with a single click. The dashboard shows how many extensions were scanned and when the last audit ran.
- **Precise tracing** — Every finding pinpoints the exact file and line number, so you can trace the issue immediately.
- **Per-agent scanning** — Even if multiple agents share the same extension, each agent's copy is audited independently — because versions can drift, and a safe copy on one agent doesn't guarantee safety on another.
- **Unicode deobfuscation** — Strips hidden characters (zero-width spaces, directional marks, variation selectors) that could evade detection.
- **Permission transparency** — Every extension's permissions are surfaced across five dimensions — filesystem paths, network domains, shell commands, database engines, and environment variables. You see exactly what each extension can reach before you decide to keep it.

---

### 🏪 Marketplace Ecosystem

Discover, evaluate, and install — three marketplaces in one, each with trending lists and search:

- **Skills** — Browse and install from the [skills.sh](https://skills.sh) registry. Also supports install from **Git URL** or **local directory**.
- **MCP Servers** — Browse the [Smithery](https://smithery.ai) registry of Model Context Protocol servers.
- **Agent-first CLI** — Discover CLI tools built specifically for agents — the newest frontier of the agent extension ecosystem.

Every listing shows its description, install count, and source. For skills, you can preview the documentation, check third-party security audit scores before installing, and install to any agent with one click — HarnessKit tracks the source so you always know where each extension came from.

---

### 📂 In-Place Management

Unlike tools that copy your files into a managed folder, HarnessKit works directly with your agents' native directories — no shadow copies, no sync conflicts.

- **Native directories** — Reads and writes directly to each agent's own config directory. Your files stay exactly where they are.
- **Non-destructive operations** — Enabling or disabling an extension is a simple file rename in place. Nothing is moved or duplicated.
- **Zero lock-in** — Uninstall HarnessKit and everything is exactly where it was. No migration, no cleanup needed.

---

### ⌨️ CLI Support

HarnessKit also ships a standalone command-line interface (`hk`) for terminal-first workflows, so you can manage extensions without opening the desktop app:

```shell
$ hk status
  Agents        6 detected (claude · codex · gemini · cursor · antigravity · copilot)
  Extensions    136 total (124 skills · 2 mcp · 8 plugins · 1 hooks · 1 clis)

$ hk list --kind skill --agent claude    # filter by type and agent
$ hk audit                               # security audit with trust scores
$ hk enable my-skill                     # enable by name
$ hk disable --pack owner/repo           # batch disable by source
```

---

### ✨ Thoughtful & Interactive UX

- 💡 **Tip of the Day** — The Overview dashboard surfaces contextual tips for each detected agent from a community-maintained library. Learn shortcuts and best practices as you work.
- 📊 **Dynamic Activity Feed** — Agent Activity and Recently Installed timelines capture every config change, extension install, and agent event in real time.
- ⚡ **Quick Actions** — One-click View Agents, Run Audit, Check Updates, and Marketplace access right from the dashboard.
- 🎨 **Themes** — Multiple themes with Light, Dark, and System mode support.
- 🎯 **Playful Touches** — Smooth animations and micro-interactions throughout the app make daily use feel alive.

---

## Getting Started

**Requirements:** macOS 12+, at least one supported AI coding agent installed.

### Desktop App

1. Download the DMG for your architecture (Apple Silicon or Intel) from the [latest release](https://github.com/RealZST/HarnessKit/releases).
2. Open the DMG and drag **HarnessKit** to the Applications folder.
3. Launch HarnessKit. It will automatically detect your installed agents and scan their extensions.

> [!NOTE]
> On first launch, macOS may show a security warning because the app is signed but not notarized. Go to **System Settings → Privacy & Security** and click **Open Anyway**.

### CLI

```bash
curl -fsSL https://raw.githubusercontent.com/RealZST/HarnessKit/main/install.sh | sh
```

After installation, restart your terminal and verify with `hk status`.

---

## Roadmap

- 🤖 **More Agents** — OpenClaw, Kiro, Cline, Roo Code, Continue, and 20+ additional agents
- 🖥️ **Windows & Linux** — Cross-platform desktop support
- ⌨️ **CLI Enhancements** — More commands and richer functionality for `hk`

---

## License

This project is licensed under [Apache-2.0](LICENSE).

Artwork (`public/icons/` and `src/components/shared/agent-mascot/`) is **All Rights Reserved** and is not covered by the Apache-2.0 license.

All product names, logos, and trademarks are property of their respective owners. HarnessKit is an independent project, not affiliated with or endorsed by any agent vendor.
