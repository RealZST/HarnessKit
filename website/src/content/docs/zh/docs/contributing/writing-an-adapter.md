---
title: "编写 adapter"
description: "让 HarnessKit 支持一个新的 agent。"
sidebar:
  order: 1
---

adapter 告诉 HarnessKit 某个 agent 把文件放在哪里，以及如何读写这些文件。每个 agent 在 `crates/hk-core/src/adapter/` 下有一个 Rust 文件。新增一个 agent，就是编写这个文件并注册它，再在应用中为它设置名称、颜色和吉祥物。

## 开始之前

- **先创建 issue**。说明要添加哪个 agent，附上它的文档或源码链接，在编写代码之前先商定要覆盖的范围。
- **用真实安装测试**。agent 的文档与实际行为常常不一致。请将每种扩展都安装一次，并执行启用、禁用和删除，再新建一个会话，确认 agent 识别到了改动。
- **写明测试内容**。在 PR 中注明测试所用的 agent 版本和平台。

## adapter 是什么

adapter 实现 `crates/hk-core/src/adapter/mod.rs` 中的 `AgentAdapter` trait。每个 adapter 都必须实现以下方法：

- `name()`：agent 的 id，例如 `pi`。
- `base_dir()` 和 `detect()`：agent 的安装位置，以及是否已安装。
- `skill_dirs()`：全局 skill 文件夹。HarnessKit 会安装到第一个。
- `mcp_config_path()`、`read_mcp_servers()`、`hook_config_path()`、`read_hooks()` 和 `plugin_dirs()`。

其余方法都有默认实现，而大多数默认实现的含义是「这个 agent 没有这类内容」。遗漏覆盖时，代码照样能编译，测试也能通过，只是相应功能不显示任何内容。最常需要覆盖的方法有：

- `mcp_format()` 和 `remote_mcp_schema()`：用于 MCP 文件不是 Claude Code 那种 `mcpServers` JSON，或 agent 支持远程 server 的情况。
- `hook_format()`：默认为 Claude Code 的文件结构。没有 shell 命令 hook 的 agent 返回 `HookFormat::None`。
- `translate_hook_event()`：默认将 Claude Code 的事件名原样传递。见第 4 步。
- `read_plugins()`：列出全局 plugin。默认不列出任何内容，因此只实现 `plugin_dirs()` 不会显示任何 plugin。
- `project_skill_dirs()`、`project_mcp_config_relpath()`、`project_hook_config_relpath()` 和 `project_plugin_dirs()`：用于项目范围。除 skill 外，每个项目路径都必须配备对应的读取方法：`read_mcp_servers_from()`、`read_hooks_from()` 和 `read_plugins_from()`。它们默认不返回任何内容，因此若只声明路径而不实现读取方法，项目中将无法列出任何内容。MCP 和 hook 的路径仍会作为安装目标出现，于是 HarnessKit 会向其中写入，却不显示写入的内容。
- `project_markers()`：哪些文件或文件夹表明一个目录是该 agent 的项目。若 adapter 未声明，`scanner.rs` 中的一个测试会失败，除非它在该测试中被列为仅有全局范围。
- `global_rules_files()` 和 `project_rules_patterns()`：供**智能体**页使用。agent 的其他文件也有对应的方法：`global_memory_files()` 和 `project_memory_patterns()`、`global_settings_files()` 和 `project_settings_patterns()`、`global_subagent_files()` 和 `project_subagent_patterns()`、`project_ignore_patterns()`，以及 `global_workflow_files()` 和 `project_workflow_patterns()`。

应用提供哪些安装目标，完全由这些声明决定，没有单独的能力清单。

## 检查清单

在 `crates/hk-core/src/` 中：

1. **路径**。编写 adapter。每个路径都要对照 agent 的文档或源码核实，并在文件顶部的注释中注明出处。
2. **共享 skill**。若 agent 会读取 `~/.agents/skills/`，请把该目录加入 `skill_dirs()`，排在 agent 自己的文件夹之后。
3. **MCP 格式**。若现有的 `McpFormat` 都不适用，请新增一个，并在 `deployer.rs` 中实现对应的写入逻辑。
4. **hook 事件**。若 agent 支持 shell 命令 hook，请在 `adapter/hook_events.rs` 中添加一张事件表，以 Claude Code 的事件名为键，再覆盖 `translate_hook_event()`，使其查询这张表。若不覆盖，从其他 agent 安装过来的 hook 会保留 Claude Code 的事件名，你的 agent 永远不会触发它。
5. **禁用开关**。确定每种扩展的禁用方式，见下文的[原生禁用开关](#原生禁用开关)。
6. **单文件 plugin**。若 plugin 是单个 `.ts` 或 `.js` 文件，请在 `manager.rs` 中列出 OpenCode、Oh My Pi 和 Pi 的位置也加上这个 agent。否则，禁用的 plugin 将无法重新启用。
7. **注册**。在 `adapter/mod.rs` 顶部添加 `pub mod <id>;`，把 adapter 加入 `all_adapters()`，并在同一文件中列出所有 agent 的测试里加上它，包括能力矩阵测试。

应用部分：

8. **顺序和名称**。把 id 加入 `src/lib/types.ts` 的 `AGENT_ORDER`，位置与 `all_adapters()` 中一致，并在同一文件中添加显示名。更新 `src/lib/__tests__/types.test.ts` 中的数量。
9. **颜色**。在 `src/index.css` 的三套主题（默认、`claude` 和 `tiesen`）中各添加一个 `--agent-<id>`，浅色和深色各一份。再在 `@theme inline` 块中把它映射为 `--color-agent-<id>`。在 `extension-filters.tsx` 中添加筛选标签。
10. **吉祥物**。在 `src/components/shared/agent-mascot/` 下新建 `<id>-mascot.tsx`，在 `agent-mascot.tsx` 中注册，在 `mascot.css` 中编写动画。
11. **新手引导**。在 `onboarding.tsx` 的 `FLOAT_DELAYS` 和 `SCATTER_POSITIONS` 中各添加一项，遗漏任何一项，TypeScript 都会报错。若点击动画超过 600 ms，还需要在 `agent-card.tsx` 的 `CLICK_DURATIONS` 中添加一项。

文档部分：

12. **README**。在 `README.md` 和 `README.zh-CN.md` 的支持表格和 agent 列表中添加这个 agent。
13. **本站**。网站是本仓库的 `website/` 目录。你的 PR 合并后，由维护者把这个 agent 添加到网站上，你在 PR 中注明的路径和版本会写入[支持的 agent 和文件位置](/zh/docs/reference/agents/)。也可以在同一个 PR 中自行添加，见[编写文档](/zh/docs/contributing/writing-docs/)。

最后运行 `cargo fmt --all`、`npm test`、`npm run build` 和 `cargo test --workspace`。

## 原生禁用开关

默认情况下，HarnessKit 用自己的方式禁用扩展：为 skill 的 `SKILL.md` 改名；把 MCP server 或 hook 从配置中移除，并在数据库中保留一份快照；为 plugin 的 manifest 改名。若 agent 自带禁用开关，则改用 agent 的开关。每种扩展的接入位置不同：

- **skill**。让 `supports_native_skill_toggle()` 返回 true，在 `natively_disabled_skills()` 中读回 agent 自己的列表，并在 `manager.rs` 的 `toggle_skill` 中添加一个分支。若不把该 agent 加入 `test_supports_native_skill_toggle_only_hermes`，该测试会失败。
- **MCP server**。让 `supports_native_mcp_toggle()` 返回 true，并在 `manager.rs` 的 `toggle_mcp` 中添加一个分支，状态由 `read_mcp_servers()` 读回。若遗漏这个分支，禁用 server 会失败。请把该 agent 加入 `test_supports_native_mcp_toggle_only_native_agents`。
- **hook**。hook 没有对应的开关方法。请在 `manager.rs` 的 `toggle_hook` 中添加一个分支，放在「移除并保留快照」那条路径之前，Grok Build 和 Kiro 就是这样实现的。
- **plugin**。在 `manager.rs` 的 `toggle_plugin` 中添加一个分支。若没有该分支，通用的兜底逻辑会把 plugin 的 manifest 改名为 `.disabled`，以其他方式加载 plugin 的 agent 会因此出现问题。删除 plugin 时，agent 配置中的对应条目也需要一并删除：请在 `service.rs` 的 `delete_extension` 中添加一个分支。若 agent 有自己的卸载命令，或自带无法删除的 plugin，则改为实现 `plugin_removal()`。

## 实例

[PR #157](https://github.com/RealZST/HarnessKit/pull/157) 加入了 Pi：

| 文件 | 改动内容 |
|---|---|
| `adapter/pi.rs` | adapter 本身：skill 文件夹、`mcp.json`、extension、上下文文件。 |
| `adapter/mod.rs`、`adapter/omp.rs` | 注册 Pi，并将其加入测试。为不含 SSE 的 Streamable HTTP 添加了远程 schema。把 Oh My Pi 读取 `.ts` extension 的代码移入 `mod.rs` 共用。 |
| `deployer.rs`、`manager.rs` | 复用 Oh My Pi 写入 `enabled` 标志的逻辑，拒绝 SSE server，并添加 Pi 的开关分支。 |
| `src/lib/types.ts` 及其测试、`src/index.css`、`extension-filters.tsx`、`agent-card.tsx`、`onboarding.tsx` | 顺序、名字、颜色、筛选标签、点击时长、新手引导。 |
| `pi-mascot.tsx`、`agent-mascot.tsx`、`mascot.css` | 吉祥物。 |
| `README.md`、`README.zh-CN.md` | 表格行和 agent 列表。 |

Pi 没有 shell 命令 hook，因此未修改 `hook_events.rs`。

## 相关源码

- `crates/hk-core/src/adapter/mod.rs` 定义 trait、它的默认实现和 `all_adapters()`。
- `crates/hk-core/src/adapter/pi.rs` 是一个较新、篇幅较短的 adapter，适合作为参考。
- `crates/hk-core/src/adapter/hook_events.rs` 存放 hook 事件表。
- `crates/hk-core/src/manager.rs` 把每个原生禁用开关交给对应的写入逻辑。
- `crates/hk-core/src/service.rs` 负责删除扩展，包括各 agent 删除 plugin 后的清理。
