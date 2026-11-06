---
title: FAQ
description: Common questions about HarnessKit, its data, and where it runs.
# source: the first eight questions, and the first paragraph under each, are copy/en.ts faq.items (the landing's
# plain answers); the paragraphs after them are docs-only detail. src/data/copy/faq-sync.test.ts checks the shared part.
---

Short answers to the questions people ask first, with links to the pages that go deeper. When something doesn't work as expected, [Troubleshooting](/docs/troubleshooting/) starts from what you see and says what to do.

## Which version should I download?

If you use your agents on your own Mac or Windows computer, download the desktop app. If they run on Linux or a remote server, install the command-line version: it prints a link that opens the same interface in your browser. The Windows installer isn't code-signed yet, so Windows may show a warning the first time you run it: select More info, then Run anyway.

The [desktop app](/docs/getting-started/desktop/) is built for macOS (Apple Silicon and Intel) and 64-bit Windows 10 and 11; there is no Linux build. The command line installs the `hk` CLI on macOS, Linux or Windows. `hk serve` starts [web mode](/docs/getting-started/web/) and prints a link to the same UI, which you open in your browser yourself.

The warning is SmartScreen's, because the installer isn't code-signed. SmartScreen stops warning once a file has enough downloads.

## Is HarnessKit free?

Yes. HarnessKit is free and open source under the Apache-2.0 license. It requires no account and has no paid plan.

## What languages is HarnessKit available in?

The interface is available in English, Simplified Chinese and Traditional Chinese. It follows your system or browser language by default, and you can change it in Settings → Language, in the desktop app or in your browser.

The `hk` command prints English. These docs are in English and Simplified Chinese: switch with EN / 中文 at the top of the page.

## Will HarnessKit change my existing setup?

Only when you ask it to. HarnessKit reads your agents' files to show what's installed, and writes to them only when you install, update, enable, disable or delete something, or add a Kit to a project. Changes go into each agent's own files, which stay where they are. HarnessKit doesn't move them into its own storage or replace them with links, and anything you disable can be re-enabled. [See what changes on disk](/docs/how-it-works/in-place/).

Disabling a skill adds `.disabled` to its file name, as in `SKILL.md.disabled`. Hermes is the exception: there HarnessKit adds the skill to the `skills.disabled` list in `~/.hermes/config.yaml`. Some edits rewrite a whole config file: Codex's and Grok Build's `config.toml`, for example, lose their comments. [Installing an MCP server](/docs/how-it-works/installing-an-mcp-server/#limitations) lists which files.

## Do my configs or API keys leave my computer?

No. HarnessKit runs on your computer, and so does its security audit: your files, configs and keys are never uploaded, and no usage data is sent. It goes online only to check for updates, load the Marketplace, download what you install and make a few small lookups. [See the full list](/docs/how-it-works/in-place/#what-goes-online).

There is no telemetry. The small lookups load the interface fonts and the Tip of the Day, and find where a newly found skill came from. That is a skills.sh search, which sends the skill's name, then `git ls-remote` on the matching repo, which reads its latest commit without downloading it. The audit's rules are text patterns that run on your machine, so audits work offline too.

## If I stop using HarnessKit, will my agents still work?

Yes. Your agents read their own files directly and never depend on HarnessKit. Remove it, and every agent keeps the setup it had. Only what you disabled needs attention: re-enable it before you uninstall.

On agents without an off switch of their own, a disabled MCP server or hook lives only in `~/.harnesskit/metadata.db`, so it is lost with that file. A disabled skill keeps its `.disabled` name until you rename it back, and so does a plugin that HarnessKit disabled by renaming its manifest or file. [Uninstall](/docs/getting-started/update-and-uninstall/#uninstall) says what to turn back on and what to delete.

## Can I use HarnessKit on a remote server?

Yes. Install it on the server and open it in the browser on your own computer. [The setup guide](/docs/getting-started/web/#start-on-a-remote-server) walks through the steps.

In short: install on the server, forward a port with `ssh -L 7070:localhost:7070`, run `hk serve`, and open the URL it prints in your local browser. The same steps work on a cluster. Web mode has the same features as the desktop app, except the few in [What web mode can't do](/docs/getting-started/web/#what-web-mode-cant-do).

## My agent isn't supported. What can I do?

[Open an issue](https://github.com/RealZST/HarnessKit/issues/new) and tell us which agent you use. You can also add support yourself: [the adapter guide](/docs/contributing/writing-an-adapter/) explains how.

Each agent is a Rust adapter.

## My agent keeps its files somewhere else. Can HarnessKit find them?

Yes. Open **Settings → Agent Paths**, select the edit button next to the agent, choose the folder it uses, and select **Save**. The desktop app doesn't read environment variables set in your shell, so set the path there too when a variable such as `GROK_HOME` moves an agent's folder. [Supported agents and file locations](/docs/reference/agents/) lists the folder HarnessKit looks in for each agent.

There is no way yet to add an agent HarnessKit has no adapter for. [Open an issue](https://github.com/RealZST/HarnessKit/issues/new) to ask for one.

## I double-clicked hk.exe, and a window flashed and closed. What happened?

You opened the command-line version. `hk.exe`, downloaded as `hk-windows-x64.exe`, is meant to run in a terminal. Started by a double-click, it prints its help and its window closes at once.

For the desktop app, download `HarnessKit_x.x.x_x64-setup.exe` instead. For the command-line version, open PowerShell or Windows Terminal, run `hk serve` and open the link it prints. If you downloaded the file yourself, [Manual download](/docs/getting-started/web/#manual-download) shows how to put it on your PATH first.

## Can one window manage several machines?

Not yet. Each machine runs its own `hk serve`, and you open each one in its own browser tab.

Each tab and sidebar shows the machine's name: its hostname, or the label you give with `hk serve --name`. Over SSH, give each machine its own local port, so the browser keeps a separate token for each. [Start on a remote server](/docs/getting-started/web/#start-on-a-remote-server) has the setup.

## Can I run HarnessKit in Docker or another container?

There is no official image yet. The Linux `hk` binaries are statically linked, so one runs in any Linux container without extra libraries. HarnessKit only sees the agent folders inside the container: mount the ones you want it to manage, and `~/.harnesskit` if its database should outlive the container. To reach the web UI from outside, start it with `hk serve --host 0.0.0.0` and publish the port. Installing skills from the Marketplace or Git, and checking or applying updates, needs `git` in the container.

## Where does HarnessKit keep its own data?

In `~/.harnesskit/` in your home folder:

- `metadata.db` is a SQLite database with what HarnessKit knows about your extensions: install sources, audit results, your projects and settings, and snapshots of MCP servers and hooks it has disabled.
- `metadata.db.backup-vN` files are full copies of the database, made before each schema upgrade.
- `kits/` holds the Kits you save.
- `web-token` holds the access token for [web mode](/docs/getting-started/web/).

Some preferences live in the app's browser storage, not in this folder: your language, theme and agent visibility, whether you have seen the welcome screens, and the cached Tip of the Day. The desktop app keeps its own, and in web mode each browser keeps its own.

Deleting the folder resets HarnessKit's data and leaves your agents' files alone, but it doesn't reset those preferences. Disabled MCP servers and hooks that live only in the database are lost with it, so re-enable them first. [Managing files in place](/docs/how-it-works/in-place/#what-harnesskit-stores) has the details.

## Can several people use HarnessKit on one machine?

Yes. Each user has their own `~/.harnesskit`, so their own database, Kits and web token, and HarnessKit manages the agents in the home directory of whoever runs it. On a shared server, each person runs their own `hk serve` on a different `--port` and keeps the token on: other users on the machine can reach `127.0.0.1`, and only the token keeps them out.
