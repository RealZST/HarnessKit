---
title: "Contributing tips and CLI listings"
description: "Add a Tip of the Day, or list an agent-first CLI in the Marketplace."
sidebar:
  order: 3
---

Tip of the Day and the Marketplace's CLI tab read their content from the public repository [RealZST/harnesskit-resources](https://github.com/RealZST/harnesskit-resources), not from the app. A change merged there reaches every user without a new release. Either kind of change is a pull request to that repository.

| To add | Edit | Rules |
|---|---|---|
| A tip | `tips/tips.json` | `tips/TIPS_GUIDELINES.md` |
| A CLI listing | `cli-registry/registry.json` | `cli-registry/REGISTRY_GUIDELINES.md` |

[Tip of the Day](/docs/how-it-works/tips/) and [Marketplace sources](/docs/how-it-works/marketplace/#agent-first-clis) explain how the app reads these files and how the weekly and three-day crawls propose new entries.

## Contribute a tip

1. Find the tip in the agent's official docs, official blog or official repository docs. Personal blogs, SEO sites and unverified community posts are not accepted.
2. Write one or two sentences that tell the reader what to do, with a concrete command, path or key. Keep the wording close to the source. Tips about HarnessKit itself are not accepted.
3. Add an entry to the array in `tips/tips.json`, with the three fields below.
4. Check that no existing tip says the same thing.
5. Open a pull request to RealZST/harnesskit-resources.

```json wrap
{
  "agent": "claude",
  "tip": "Use @path/to/import syntax in CLAUDE.md to import additional files. Imported files are expanded and loaded into context at launch alongside the CLAUDE.md that references them.",
  "source": "https://code.claude.com/docs/en/memory"
}
```

`source` is the page you checked the tip against. `agent` is the agent's id in HarnessKit, or `general` for a tip that applies to every agent. A tip shows only to users who have that agent, so a misspelled id never shows. `tips/TIPS_GUIDELINES.md` doesn't list every id yet, so take it from this table:

| Agent | id |
|---|---|
| Claude Code | `claude` |
| Codex | `codex` |
| Gemini CLI | `gemini` |
| Cursor | `cursor` |
| Antigravity | `antigravity` |
| Copilot | `copilot` |
| Devin Desktop | `windsurf` |
| OpenCode | `opencode` |
| Hermes | `hermes` |
| Kiro | `kiro` |
| Oh My Pi | `omp` |
| DeepSeek Harness | `dsh` |
| Grok Build | `grok` |
| Qoder CN | `qoder-cn` |
| OpenClaw | `openclaw` |
| PenguinHarness | `penguin` |
| Pi | `pi` |

Some agents have few tips or none, and those need contributions most. Count the entries for each `agent` in [`tips.json`](https://github.com/RealZST/harnesskit-resources/blob/main/tips/tips.json) to see where things stand.

After the merge, every user gets the new file the next time their Overview opens. The tip then joins the daily rotation for users with that agent.

The weekly crawl reads the pages listed in `tips/sources.json`. An agent with no entry there never gets proposed tips, so adding its official docs page there helps too.

## List an agent-first CLI

The Marketplace's CLI tab lists the entries in `cli-registry/registry.json`. HarnessKit downloads that file from the repository's `main` branch and keeps it for 5 minutes.

### Before you start

- **Check that it fits.** The list is for command-line tools built for agents to call. Common reasons for a rejection are that the tool is not a CLI, or is a skill pack, a coding agent, a framework, a curated list or a desktop app.
- **Check `cli-registry/excluded.json`.** A tool listed there was declined before, with its reason. The crawl skips it too.
- **Check the README.** A listing's **Installation Guide** shows one section of the README in the entry's `skills_repo`. HarnessKit takes the first heading, from `#` to `###`, whose text contains "install", "setup", "getting started", "quick start", "usage", 安装 or 快速开始. Without one, it shows the README's first 2,000 characters. The README must be `README.md` on `main` or `master`.

### Add the entry

1. Append an object to the array in `cli-registry/registry.json`. Copying an existing entry is the easiest start.
2. Fill in every field, as in the table below. Use `null` or `[]` for an empty value, and never leave a field out.
3. Set `verified` to `false`. A maintainer sets it to `true` after checking that the skills repository has valid skill definitions, that the install command works, and that nothing in it looks unsafe.
4. Check that the file still parses, for example with `python3 -m json.tool cli-registry/registry.json`.
5. Open a pull request that says what the CLI does and why it belongs in the list.

```json
{
  "binary_name": "wecom-cli",
  "display_name": "WeChat Work CLI",
  "description": "CLI for WeChat Work (WeCom) — team messaging, approvals, and bots",
  "install_command": "npm install -g @wecom/cli",
  "skills_repo": "WecomTeam/wecom-cli",
  "skills_install_command": null,
  "icon_url": null,
  "categories": ["collaboration", "messaging"],
  "verified": true,
  "api_domains": ["qyapi.weixin.qq.com"],
  "credentials_path": "~/.wecom/credentials.json"
}
```

| Field | Type | What the app does with it |
|---|---|---|
| `binary_name` | string | Identifies the listing. It must be unique in the file. |
| `display_name` | string | The listing's title. Search matches it. |
| `description` | string | The one-line summary. Search matches it. |
| `install_command` | string | Nothing. HarnessKit never runs it. |
| `skills_repo` | string, `owner/repo` on GitHub | The star count, the **View on GitHub** link and the README. The crawl treats this repository as listed. |
| `skills_install_command` | string or `null` | Nothing. |
| `icon_url` | string or `null` | The listing's icon. The desktop app loads it only from `raw.githubusercontent.com`. |
| `categories` | list of strings, lowercase kebab-case | Tags. The list shows the first two, and the detail panel shows all. |
| `verified` | `true` or `false` | A check mark beside the name. |
| `api_domains` | list of strings | Nothing yet. Use `[]` if there are none. |
| `credentials_path` | string or `null` | Nothing yet. Use `~` for the home directory. |

HarnessKit reads the whole file at once. If any entry has a missing field or a value of the wrong type, the download counts as failed, and every user sees the short list built into the app instead. That list changes only with a new release, so a new entry never needs to go there.

After the merge, users see the entry the next time the app downloads the file. Listings are sorted by the star count of `skills_repo`.

## Under the hood

- `src/pages/overview.tsx` downloads the tips file, keeps the last copy and picks the day's tip.
- `crates/hk-core/src/marketplace.rs` downloads and parses the CLI registry, reads star counts and READMEs, and holds the built-in fallback list.
- `src/pages/marketplace.tsx` shows a CLI listing and finds the install section of its README.
- `RealZST/harnesskit-resources: tips/TIPS_GUIDELINES.md` sets the review rules for tips.
- `RealZST/harnesskit-resources: cli-registry/REGISTRY_GUIDELINES.md` describes each registry field and what verified means.
