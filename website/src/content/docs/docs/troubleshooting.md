---
title: Troubleshooting
description: Common problems with HarnessKit, why they happen, and what to do about each one.
---

Each entry starts from what you see, then says why it happens and what to do. For questions that aren't problems, see the [FAQ](/docs/faq/). If your problem isn't here, [open an issue](https://github.com/RealZST/HarnessKit/issues/new) with what you did, what happened and the version you use.

## Installing and starting

### `hk: command not found`

On Windows, PowerShell says instead that the term `hk` is not recognized.

**Why.** The install script puts `hk` in `~/.local/bin`, or `%USERPROFILE%\.local\bin` on Windows. If that folder isn't on your PATH, the script adds it: to `~/.zshrc`, `~/.bashrc` or `~/.profile`, depending on your shell, or to your Windows user Path. A terminal that was already open doesn't see the change.

**What to do.** Open a new terminal, or on a server log out and back in, then run `hk --version`. If `hk` is still not found, check that `~/.local/bin/hk` exists, and add the folder to the startup file your shell reads. Bash in the macOS Terminal, for example, reads `~/.bash_profile`, not `~/.bashrc`. If you downloaded `hk` yourself, add the folder as in [Manual download](/docs/getting-started/web/#manual-download).

### hk.exe opens a window that closes at once

**Why.** `hk.exe`, named `hk-windows-x64.exe` on the release page, is the command-line version. Started by a double-click, it prints its help and exits, and its window closes with it.

**What to do.** For the desktop app, download and run `HarnessKit_x.x.x_x64-setup.exe` instead. For the command line, open PowerShell or Windows Terminal and run `hk` there, for example `hk serve`. [Which version should I download?](/docs/faq/#which-version-should-i-download) compares the two.

### Windows warns before it runs the installer

Opening `HarnessKit_x.x.x_x64-setup.exe` shows "Windows protected your PC".

**Why.** The warning is SmartScreen's. The installer isn't code-signed yet, so SmartScreen doesn't recognize it.

**What to do.** Click **More info**, then **Run anyway**. [Install the desktop app](/docs/getting-started/desktop/#windows) has the rest of the steps.

## Web mode

### The web UI opens but shows no data

The page loads, but the lists stay empty and actions fail.

**Why.** `hk serve` requires its access token, and the browser has none saved for this address. The browser saves the token when you open the full link, and only for that address: `http://localhost:7070` and `http://127.0.0.1:7070` are different addresses to it, and so is every port. Another browser, a private window or cleared site data starts without the token. There is no login page: the server turns away every request that lacks the token, with "401 Unauthorized".

**What to do.** Open the full link that `hk serve` prints, the one that ends in `?token=…`. The token stays the same each time you start `hk serve`. [The access token](/docs/getting-started/web/#the-access-token) explains more.

### `hk serve` stops with "Address already in use"

`hk serve` prints its link, then exits with an error such as `Error: Address already in use`. On Windows, the message says that only one usage of each socket address is permitted.

**Why.** Another program already listens on that port, 7070 unless you chose another. Often it is an `hk serve` you started earlier.

**What to do.** Stop the other `hk serve` with Ctrl+C in its terminal, or start this one on another port, such as `hk serve --port 8080`. Over SSH, forward the port you chose: `ssh -L 8080:localhost:8080 user@your-server`. [Port and address](/docs/getting-started/web/#port-and-address) has the options.

When the port is taken on your own computer instead, `ssh -L` prints a warning such as `bind [127.0.0.1]:7070: Address already in use` and logs you in without the tunnel. The link then reaches whatever runs on that port on your computer. Forward another local port, such as `ssh -L 7071:localhost:7070 user@your-server`, and change `7070` to `7071` in the link.

## Agents

### An agent isn't detected

**Why.** HarnessKit counts an agent as installed when its home folder exists, such as `~/.claude` for Claude Code. An agent you have never started may not have created that folder yet. An agent installed somewhere else, or moved by an environment variable, keeps it in another place. The desktop app doesn't read environment variables set in your shell.

**What to do.** Start the agent once, then switch back to HarnessKit, or reload the page in web mode. If the agent's folder is somewhere else, open **Settings → Agent Paths**, select the edit button next to the agent, choose the folder and select **Save**. [Supported agents and file locations](/docs/reference/agents/#all-agents) lists the folder HarnessKit looks for, and the environment variables each agent reads.

### An agent you don't use shows as installed

**Why.** Detection only checks that the agent's home folder exists. A folder that another tool created counts too.

**What to do.** Open **Settings → Agent Paths** and click the agent's **Enabled** button, so it reads **Disabled**. HarnessKit then leaves the agent out of its lists and install targets.

### The desktop app doesn't show a CLI that `hk list` shows

**Why.** On macOS, the desktop app starts without your shell's startup files, so its PATH holds only the system folders. HarnessKit also looks in `~/.local/bin`, `~/.cargo/bin`, `~/go/bin`, `~/.bun/bin`, `/opt/homebrew/bin` and `/usr/local/bin`. A CLI installed with `npm install -g` under nvm, Volta or fnm is in none of them. `hk` started from a terminal uses that terminal's PATH, so it finds the CLI.

**What to do.** Run `hk serve` in a terminal and use the web UI. It sees the same CLIs as `hk list`.

## Extensions

### A change doesn't take effect in the agent

**Why.** An agent loads its skills, MCP servers, hooks and plugins when a session starts. A session that is already running keeps what it loaded. That is why HarnessKit's messages end with "Takes effect in new sessions".

**What to do.** Start a new session in the agent, or quit the agent and open it again. Codex hooks and Grok Build project settings need one more step, described below.

### A change made outside HarnessKit doesn't show up

**Why.** HarnessKit shows what it found at its last scan. The desktop app scans at startup and each time its window comes back into focus. Web mode scans only when the page loads.

**What to do.** In web mode, reload the page. In the desktop app, switch to another window and back.

### A Codex hook doesn't run

**Why.** Codex runs a hook only after you trust it in Codex. HarnessKit writes the hook into Codex's file, but it can't trust the hook for you. The hook's detail panel says so: "Codex hooks must be trusted via `codex /hooks` before they run."

**What to do.** Start Codex, run its `/hooks` command and trust the hook. [Codex](/docs/reference/agents/#codex) lists its hook files.

### A Grok Build hook, MCP server or plugin in a project doesn't load

**Why.** Grok Build loads a project's hooks, MCP servers and plugins only after you trust the project folder in Grok Build. Skills load either way. The detail panel shows a note on these rows.

**What to do.** Open a terminal in the project folder, run `grok`, and answer yes when it asks whether you trust the folder. [Grok Build](/docs/reference/agents/#grok-build) lists its project files.

### A hook installed on another agent never runs

**Why.** HarnessKit translates a hook's event name, but copies its matcher as it is, and agents name their tools differently. A Gemini CLI hook on `run_shell_command` lands unchanged in Claude Code, where no tool has that name. A Claude Code prompt hook arrives as a shell command, so the agent tries to run the prompt's text. On Cursor and Copilot it goes the other way: HarnessKit drops the matcher, so the hook runs on every tool call.

**What to do.** Open the hook in the target agent's hook file and change the matcher to that agent's name for the tool. [Supported agents and file locations](/docs/reference/agents/) lists each agent's hook file. [Installing a hook](/docs/how-it-works/installing-a-hook/#limitations) lists what else changes when a hook moves to another agent.

### Installing fails when the agent's config file has comments

The detail panel says only "Failed to install to Copilot", or whichever agent it was. The MCP servers or hooks in that file are missing from HarnessKit too.

**Why.** HarnessKit reads and writes most agents' JSON config files as strict JSON. A file with comments or trailing commas, such as VS Code's `mcp.json` for Copilot, doesn't parse. HarnessKit then lists nothing from that file, and it refuses to write the file rather than lose your comments. OpenCode's and OpenClaw's config files are the exceptions: HarnessKit keeps their comments.

**What to do.** Take the comments and trailing commas out of the file, then install again. To see why an install failed, tick the row's checkbox and use **Install to agents**: its summary lists each failure with its error. [Installing an MCP server](/docs/how-it-works/installing-an-mcp-server/#limitations) says which files keep their comments when HarnessKit writes them.

## Overview

### Tip of the Day doesn't show

**Why.** HarnessKit downloads the tips from `raw.githubusercontent.com` each time the **Overview** opens, and keeps the last copy for when the download fails. If the first download fails, there is no copy and no tip. The tip card also stays hidden when no tip matches the agents HarnessKit detects. Some agents have no tips yet.

**What to do.** Check that your browser, or the desktop app, can reach `raw.githubusercontent.com`, then open the **Overview** again. If your agents have no tips, [Contributing tips and CLI listings](/docs/contributing/tips-and-cli-listings/#contribute-a-tip) shows how to add one. [Tip of the Day](/docs/how-it-works/tips/) explains how the tip is chosen.
