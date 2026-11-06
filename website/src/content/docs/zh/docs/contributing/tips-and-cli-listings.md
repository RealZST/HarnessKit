---
title: "贡献每日一帖和 CLI 条目"
description: "添加一条每日一帖，或将一个 Agent-first CLI 收录到市场。"
sidebar:
  order: 3
---

每日一帖和市场的 CLI 标签页，内容都不在应用中，而是来自公开仓库 [RealZST/harnesskit-resources](https://github.com/RealZST/harnesskit-resources)。改动合并后，无需发版即可送达每个用户。两种改动都需要向该仓库提交 PR。

| 要添加的内容 | 修改的文件 | 规则 |
|---|---|---|
| 一条提示 | `tips/tips.json` | `tips/TIPS_GUIDELINES.md` |
| 一个 CLI 条目 | `cli-registry/registry.json` | `cli-registry/REGISTRY_GUIDELINES.md` |

应用如何读取这些文件，以及每周和每 3 天的抓取如何提出新条目，请参阅[每日一帖](/zh/docs/how-it-works/tips/)和[市场来源](/zh/docs/how-it-works/marketplace/#agent-first-cli)。

## 贡献一条提示

1. 在 agent 的官方文档、官方博客或官方仓库文档中找到这条提示。不接受个人博客、SEO 网站和未经核实的社区帖子。
2. 用一两句话告诉读者具体的操作方法，给出明确的命令、路径或键。措辞尽量贴近原文。不收录关于 HarnessKit 本身的提示。
3. 在 `tips/tips.json` 的数组中添加一条，包含下面三个字段。
4. 确认没有内容相同的已有提示。
5. 向 RealZST/harnesskit-resources 提交一个 PR。

```json wrap
{
  "agent": "claude",
  "tip": "Use @path/to/import syntax in CLAUDE.md to import additional files. Imported files are expanded and loaded into context at launch alongside the CLAUDE.md that references them.",
  "source": "https://code.claude.com/docs/en/memory"
}
```

`source` 填写你核对这条提示时所用的页面。`agent` 填写该 agent 在 HarnessKit 中的 id；适用于所有 agent 的提示填写 `general`。提示只显示给安装了对应 agent 的用户，因此 id 拼写错误的提示永远不会出现。`tips/TIPS_GUIDELINES.md` 尚未列出所有 id，请以下表为准：

| agent | id |
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

有些 agent 的提示很少，甚至一条都没有，最需要补充。如需了解各 agent 目前的提示数量，统计 [`tips.json`](https://github.com/RealZST/harnesskit-resources/blob/main/tips/tips.json) 中每个 `agent` 的条目即可。

合并后，每个用户下次打开**概览**页时都会获取新文件。安装了该 agent 的用户，其每日轮换的提示中就会新增这一条。

每周的抓取只读取 `tips/sources.json` 中列出的页面。若某个 agent 在其中没有条目，抓取就永远不会为它提出候选，因此把它的官方文档页面添加进去也很有帮助。

## 收录一个 Agent-first CLI

市场的 CLI 标签页列出的是 `cli-registry/registry.json` 中的条目。HarnessKit 从该仓库的 `main` 分支下载此文件，并缓存 5 分钟。

### 开始之前

- **确认它符合要求**。这份清单收录的是专为 agent 调用而设计的命令行工具。常见的拒绝理由是：它不是 CLI，或者是 skill 合集、编程 agent、框架、精选列表、桌面应用。
- **查看 `cli-registry/excluded.json`**。列在其中的工具此前已被拒绝，并附有理由，抓取也会跳过它们。
- **检查 README**。条目的**安装指南**显示的是 `skills_repo` 仓库 README 里的一节。HarnessKit 查找第一个 `#` 到 `###` 级、文字中含有「install」「setup」「getting started」「quick start」「usage」「安装」或「快速开始」的标题。若没有这样的标题，则显示 README 的前 2000 个字符。README 必须是 `main` 或 `master` 分支上的 `README.md`。

### 添加条目

1. 在 `cli-registry/registry.json` 的数组末尾添加一个对象。最简便的做法是复制一个已有条目再修改。
2. 按下表填写每个字段。空值用 `null` 或 `[]`，任何字段都不能省略。
3. 把 `verified` 设为 `false`。维护者确认 skill 仓库中的 skill 定义有效、安装命令可用、内容没有可疑之处后，才会将其改为 `true`。
4. 确认文件仍能正常解析，例如运行 `python3 -m json.tool cli-registry/registry.json`。
5. 提交一个 PR，说明这个 CLI 的用途，以及应当收录它的理由。

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

| 字段 | 类型 | 应用中的用途 |
|---|---|---|
| `binary_name` | 字符串 | 标识该条目，在文件中必须唯一。 |
| `display_name` | 字符串 | 条目的标题。搜索会匹配它。 |
| `description` | 字符串 | 一句话简介。搜索会匹配它。 |
| `install_command` | 字符串 | 未使用。HarnessKit 从不运行它。 |
| `skills_repo` | 字符串，GitHub 上的 `owner/repo` | star 数、**在 GitHub 查看**链接和 README 都来自该仓库。抓取会将其视为已收录。 |
| `skills_install_command` | 字符串或 `null` | 未使用。 |
| `icon_url` | 字符串或 `null` | 条目的图标。桌面应用只从 `raw.githubusercontent.com` 加载它。 |
| `categories` | 字符串列表，小写 kebab-case | 标签。列表中显示前两个，详情面板显示全部。 |
| `verified` | `true` 或 `false` | 名称旁的对勾图标。 |
| `api_domains` | 字符串列表 | 暂未使用。若没有则填写 `[]`。 |
| `credentials_path` | 字符串或 `null` | 暂未使用。主目录写作 `~`。 |

HarnessKit 会一次性解析整个文件。只要有一个条目缺少字段或值的类型错误，本次下载即视为失败，所有用户看到的都会变为应用中内置的那份简短清单。内置清单只随新版本更新，因此新条目无需添加到其中。

合并后，用户下次在应用中下载此文件时即可看到新条目。条目按 `skills_repo` 的 star 数排序。

## 相关源码

- `src/pages/overview.tsx` 下载提示文件并保留最近一份，再选出当天的提示。
- `crates/hk-core/src/marketplace.rs` 下载并解析 CLI 清单，查询 star 数和 README，并存放内置的备用清单。
- `src/pages/marketplace.tsx` 显示 CLI 条目，并从 README 中找出介绍安装的那一节。
- `RealZST/harnesskit-resources: tips/TIPS_GUIDELINES.md` 规定提示的审核规则。
- `RealZST/harnesskit-resources: cli-registry/REGISTRY_GUIDELINES.md` 说明清单的每个字段，以及「已验证」的含义。
