---
title: "Writing an adapter"
description: "Add support for a new agent to HarnessKit."
sidebar:
  order: 1
---

An adapter tells HarnessKit where one agent keeps its files and how to read and write them. Each agent has one Rust file in `crates/hk-core/src/adapter/`. Adding an agent means writing that file, registering it, and giving the app a name, a colour and a mascot.

## Before you start

- **Open an issue first.** Say which agent you want to add and link its docs or source, so we can agree on what it covers before you write code.
- **Test against a real install.** An agent's docs and its behaviour often differ. Install, enable, disable and delete each kind of extension, and check that the agent picks up the change in a new session.
- **Write down what you tested.** In the pull request, give the agent's version and the platform you tested it on.

## What an adapter is

An adapter implements the `AgentAdapter` trait from `crates/hk-core/src/adapter/mod.rs`. Every adapter writes these methods:

- `name()`, the agent's id, such as `pi`.
- `base_dir()` and `detect()`: where the agent lives, and whether it is installed.
- `skill_dirs()`: the global skill folders. HarnessKit installs into the first one.
- `mcp_config_path()`, `read_mcp_servers()`, `hook_config_path()`, `read_hooks()` and `plugin_dirs()`.

The other methods have defaults, and most defaults mean "this agent has none". A missing override still compiles and passes the tests, but the feature shows nothing. The ones you most often override:

- `mcp_format()` and `remote_mcp_schema()`, when the MCP file is not Claude Code's `mcpServers` JSON or takes remote servers.
- `hook_format()`, which defaults to Claude Code's layout. An agent without shell-command hooks returns `HookFormat::None`.
- `translate_hook_event()`, which defaults to passing Claude Code's event name through unchanged. See step 4.
- `read_plugins()`, which lists the global plugins. The default lists none, so `plugin_dirs()` alone shows nothing.
- `project_skill_dirs()`, `project_mcp_config_relpath()`, `project_hook_config_relpath()` and `project_plugin_dirs()`, for project scope. Each project path except skills needs its reader: `read_mcp_servers_from()`, `read_hooks_from()` and `read_plugins_from()`. Their defaults return nothing, so a project path without its reader lists nothing. An MCP or hook path is still offered as an install target, so HarnessKit writes there but never shows what it wrote.
- `project_markers()`: the files or folders that make a directory a project for this agent. A test in `scanner.rs` fails for an adapter without markers, unless it is listed there as global-only.
- `global_rules_files()` and `project_rules_patterns()`, for the Agents page, with their siblings for the agent's other files: `global_memory_files()` and `project_memory_patterns()`, `global_settings_files()` and `project_settings_patterns()`, `global_subagent_files()` and `project_subagent_patterns()`, `project_ignore_patterns()`, and `global_workflow_files()` and `project_workflow_patterns()`.

The app offers install targets from these declarations alone. There is no separate capability list.

## Checklist

In `crates/hk-core/src/`:

1. **Paths.** Write the adapter. Check every path against the agent's docs or source, and note where in a comment at the top of the file.
2. **Shared skills.** If the agent reads `~/.agents/skills/`, add it to `skill_dirs()` after the agent's own folder.
3. **MCP format.** If no `McpFormat` fits, add one, with a writer in `deployer.rs`.
4. **Hook events.** For shell-command hooks, add an event table to `adapter/hook_events.rs`, keyed by Claude Code's event names, and override `translate_hook_event()` to use it. Without the override, a hook installed from another agent keeps Claude Code's event name, and your agent never fires it.
5. **Off switches.** Decide how each kind of extension is disabled. See [Native off switches](#native-off-switches) below.
6. **Single-file plugins.** If plugins are single `.ts` or `.js` files, add the agent where `manager.rs` names OpenCode, Oh My Pi and Pi. Otherwise a disabled plugin can't be turned back on.
7. **Register.** Add `pub mod <id>;` at the top of `adapter/mod.rs`, the adapter to `all_adapters()`, and the agent to the tests there that list every agent, including the capability matrix.

In the app:

8. **Order and name.** Add the id to `AGENT_ORDER` in `src/lib/types.ts`, in the `all_adapters()` position, plus its display name in the same file. Update the count in `src/lib/__tests__/types.test.ts`.
9. **Colour.** In `src/index.css`, add `--agent-<id>` to each of the three themes (the default, `claude` and `tiesen`), once for light and once for dark. Then map it in the `@theme inline` block as `--color-agent-<id>`. Add the filter chip to `extension-filters.tsx`.
10. **Mascot.** Add `<id>-mascot.tsx` to `src/components/shared/agent-mascot/`, register it in `agent-mascot.tsx`, and animate it in `mascot.css`.
11. **Onboarding.** Add entries to `FLOAT_DELAYS` and `SCATTER_POSITIONS` in `onboarding.tsx`. TypeScript flags a missing one. A click animation longer than 600 ms also needs `CLICK_DURATIONS` in `agent-card.tsx`.

In the docs:

12. **README.** Add the agent to the support table and the agent list, in `README.md` and `README.zh-CN.md`.
13. **This site.** The website is the `website/` folder of this repository. The maintainers add the agent here after your pull request is merged, and the paths and the version you list in the pull request go into the [agents reference](/docs/reference/agents/). You can also add it yourself in the same pull request: see [Writing docs](/docs/contributing/writing-docs/).

Then run `cargo fmt --all`, `npm test`, `npm run build` and `cargo test --workspace`.

## Native off switches

By default, HarnessKit disables an extension its own way. It renames a skill's `SKILL.md`, removes an MCP server or hook from the config and keeps a snapshot in its database, and renames a plugin's manifest. When the agent has its own off switch, use that instead. Each kind is wired up in its own place:

- **Skills.** Return true from `supports_native_skill_toggle()`, read the agent's list back in `natively_disabled_skills()`, and add a branch to `toggle_skill` in `manager.rs`. The test `test_supports_native_skill_toggle_only_hermes` fails until you add the agent to it.
- **MCP servers.** Return true from `supports_native_mcp_toggle()` and add a branch to `toggle_mcp` in `manager.rs`. `read_mcp_servers()` reads the state back. Without the branch, turning a server off fails. Add the agent to `test_supports_native_mcp_toggle_only_native_agents`.
- **Hooks.** There is no flag for hooks. Add a branch to `toggle_hook` in `manager.rs`, ahead of the remove-and-snapshot path, as Grok Build and Kiro have.
- **Plugins.** Add a branch to `toggle_plugin` in `manager.rs`. Without one, the generic fallback renames the plugin's manifest to `.disabled`, which breaks agents that load plugins some other way. When a plugin is deleted, its entry in the agent's config must go too: add a branch to `delete_extension` in `service.rs`. If the agent has its own uninstall command, or ships plugins that can't be removed, answer `plugin_removal()` instead.

## A worked example

[PR #157](https://github.com/RealZST/HarnessKit/pull/157) added Pi:

| Files | What changed |
|---|---|
| `adapter/pi.rs` | The adapter: skill folders, `mcp.json`, extensions, context files. |
| `adapter/mod.rs`, `adapter/omp.rs` | Registered Pi and added it to the tests. Added a remote schema for Streamable HTTP without SSE. Moved Oh My Pi's `.ts` extension reader into `mod.rs` to share it. |
| `deployer.rs`, `manager.rs` | Shared Oh My Pi's `enabled` flag writer, refused SSE servers, and added Pi's toggle branches. |
| `src/lib/types.ts` and its test, `src/index.css`, `extension-filters.tsx`, `agent-card.tsx`, `onboarding.tsx` | Order, name, colour, filter chip, click duration, onboarding. |
| `pi-mascot.tsx`, `agent-mascot.tsx`, `mascot.css` | The mascot. |
| `README.md`, `README.zh-CN.md` | Table row and agent list. |

Pi has no shell-command hooks, so `hook_events.rs` was left alone.

## Under the hood

- `crates/hk-core/src/adapter/mod.rs` defines the trait, its defaults and `all_adapters()`.
- `crates/hk-core/src/adapter/pi.rs` is a short, recent adapter to start from.
- `crates/hk-core/src/adapter/hook_events.rs` holds the hook event tables.
- `crates/hk-core/src/manager.rs` sends each native off switch to its writer.
- `crates/hk-core/src/service.rs` deletes extensions, including each agent's plugin cleanup.
