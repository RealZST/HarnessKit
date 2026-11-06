import type { AgentReference } from "./agent-reference";

// agent-reference.ts's sentences in Chinese, for the Chinese reference page (/zh/docs/reference/agents/). The facts
// live there; this file says the same ones in Chinese, sentence for sentence, in the docs' Chinese wording
// (docs/contributing/writing-docs.md). agent-reference.test.ts fails where an English sentence has no Chinese here.

type Sentences = Pick<AgentReference, "nativeDisable" | "notes">;

export const SKILL_DISABLE_ZH = "把 `SKILL.md` 改名为 `SKILL.md.disabled`。";
export const SNAPSHOT_DISABLE_ZH = "删除该条目，并在 HarnessKit 的数据库中保留一份快照。";

const ENABLED_FALSE = "为该 server 设置 `enabled: false`。";
const READS_HOOK_FOLDERS = (folders: string) => `HarnessKit 读取 ${folders} 中的所有 \`.json\` 文件，新添加的 hook 写入 \`harnesskit.json\`。`;

export const AGENT_REFERENCE_ZH: Readonly<Record<string, Sentences>> = {
  claude: {
    nativeDisable: {},
    notes: [
      "HarnessKit 只读取 `~/.claude.json` 顶层的 `mcpServers`。`claude mcp add` 默认保存到 local 范围，即同一文件的 `projects` 下，HarnessKit 暂不列出这些 server。",
    ],
  },
  codex: {
    nativeDisable: {},
    notes: [
      "只有 Codex 的全局 skill 安装在共享的 `~/.agents/skills/` 文件夹中。",
      "若 `config.toml` 设置了 `project_doc_fallback_filenames`，HarnessKit 会改为按这些文件名查找，不再使用上面 4 个规则文件名。",
      "HarnessKit 不读取 Codex 自带的 skill 开关，即 `config.toml` 中的 `[[skills.config]]`，因此在那里禁用的 skill 仍显示为已启用。",
    ],
  },
  gemini: { nativeDisable: {}, notes: ["项目中向下两层文件夹以内的 `GEMINI.md`，HarnessKit 也会列出。"] },
  cursor: {
    nativeDisable: {},
    notes: ["在 HarnessKit 中，Cursor 只有项目级规则。", "HarnessKit 会列出 `.cursor/rules/` 中的 `.mdc` 文件，包括子文件夹中的文件。"],
  },
  antigravity: {
    nativeDisable: {},
    notes: ["HarnessKit 也会在 `~/.gemini/antigravity/skills.txt` 列出的文件夹中查找 skill。", "Antigravity 和 Gemini CLI 都读取 `~/.gemini/GEMINI.md`。"],
  },
  copilot: {
    nativeDisable: {},
    notes: [
      "MCP 文件是 VS Code 用户级的 `mcp.json`。上面是 macOS 的路径；Linux 上为 `~/.config/Code/User/mcp.json`，Windows 上为 `~/AppData/Roaming/Code/User/mcp.json`。",
      "HarnessKit 读取 `~/.copilot/hooks/` 中的所有 `.json` 文件，新添加的 hook 写入 `hooks.json`。",
      "HarnessKit 会列出 `instructions/` 文件夹中以 `.instructions.md` 结尾的文件，包括子文件夹中的文件。",
      "HarnessKit 暂不读取 Copilot CLI 的 `~/.copilot/mcp-config.json`，也不读取项目根目录的 `.mcp.json`。",
    ],
  },
  windsurf: {
    nativeDisable: {},
    notes: [
      "Devin Desktop 原名 Windsurf，因此全局文件夹和部分项目文件仍沿用这一名称。",
      "`~/.codeium/windsurf/global_rules.md` 是旧位置，HarnessKit 仍会列出。",
      "HarnessKit 管理的是 Devin Desktop 中的 Cascade agent。Devin CLI 和 Devin Local 的设置在 `~/.config/devin/`，HarnessKit 暂不支持。",
    ],
  },
  opencode: {
    nativeDisable: {},
    notes: [
      "`opencode.json` 和 `opencode.jsonc` 同时存在时，HarnessKit 会从两个文件中读取 MCP server，同名的以 `.jsonc` 中的为准，与 OpenCode 的行为一致。写入时，若存在 `opencode.jsonc` 则写入它，否则写入 `opencode.json`，全局和项目中均是如此。",
    ],
  },
  hermes: {
    nativeDisable: { skill: "把该 skill 的名称加入 `~/.hermes/config.yaml` 的 `skills.disabled`。", mcp: ENABLED_FALSE },
    notes: [
      "HarnessKit 会列出 `~/.hermes/skills/` 下的每个 `SKILL.md`，包括子文件夹中的。其中的分类文件夹仅用于整理 skill。单独安装一个 skill 时，HarnessKit 会询问放入哪个分类；批量安装时，所有 skill 都放入按字母顺序排第一的分类，没有分类则放入 `local/`。",
      "HarnessKit 也会在 `config.yaml` 中 `skills.external_dirs` 列出的文件夹中查找 skill。",
      "在 HarnessKit 中，Hermes 没有项目范围。Hermes 本身还会在你信任的项目中读取 `<project>/.hermes/skills/` 和 `<project>/.agents/skills/` 中的 skill，HarnessKit 不列出这些 skill。",
    ],
  },
  kiro: {
    nativeDisable: { mcp: "为该 server 设置 `disabled: true`。", hook: "为该 hook 设置 `enabled: false`。" },
    notes: [READS_HOOK_FOLDERS("`~/.kiro/hooks/` 和 `<project>/.kiro/hooks/`")],
  },
  omp: {
    nativeDisable: { mcp: "为该 server 设置 `enabled: false`，并将其名称从 `~/.omp/agent/mcp.json` 的 `enabledServers` 中移除。" },
    notes: [
      "HarnessKit 只读取默认 profile，不读取 `~/.omp/profiles/` 下的其他 profile。",
      "Oh My Pi 本身也会读取 `~/.agents/skills/` 和 `~/.claude/skills/`，但 HarnessKit 不把这两个文件夹列在 Oh My Pi 下。",
    ],
  },
  dsh: {
    nativeDisable: { mcp: "在 `cordis.patch.yml` 末尾由 HarnessKit 管理的那一段中，为该 server 写入 `disabled: true`。" },
    notes: [
      "每个 MCP server 都是一行名为 `@deepseek-ai/dsh-mcp-client` 的 plugin。HarnessKit 只读取主目录下的 patch 文件，不读取各 profile 自己的 patch 文件。",
      "`DSH_HOME` 可以更改主目录的位置，`DSH_AGENTS_HOME` 可以更改共享的 `~/.agents/` 文件夹的位置。",
    ],
  },
  grok: {
    nativeDisable: {
      mcp: "把该 server 的名称加入 `~/.grok/config.toml` 的 `disabled_mcp_servers`。若 server 就定义在这个文件中，还会为它设置 `enabled = false`。",
      hook: "把该 hook 加入 `~/.grok/disabled-hooks`。",
    },
    notes: [
      "嵌套文件夹中的 skill 也会列出，不限于第一层。",
      READS_HOOK_FOLDERS("`~/.grok/hooks/` 和 `<project>/.grok/hooks/`"),
      "`GROK_HOME` 可以更改主目录的位置。",
    ],
  },
  "qoder-cn": {
    nativeDisable: {},
    notes: [
      "HarnessKit 把项目 skill 安装到共享的 `<project>/.agents/skills/` 文件夹，该文件夹也列在 Codex、Antigravity 和 Pi 下。你自行放入 `<project>/.qoder/skills/` 的 skill 不会列出。",
      "HarnessKit 不管理 Qoder CN 的项目级 MCP server。",
      "HarnessKit 不读取 `QODERCN_CONFIG_DIR`，只在 `~/.qoder-cn/` 中查找 Qoder CN。",
      "HarnessKit 不读取 Qoder CN 自带的 `skills.disabled` 列表（位于 `settings.json` 中），因此在那里禁用的 skill 仍显示为已启用。",
    ],
  },
  openclaw: {
    nativeDisable: { mcp: ENABLED_FALSE },
    notes: [
      "`openclaw.json` 中列出的每个 agent workspace 都有自己的 `skills/`、`.agents/skills/` 和规则文件。上面列出的是默认 workspace 的路径。",
      "只有 OpenClaw 使用默认的状态文件夹 `~/.openclaw/` 时，HarnessKit 才会为它读取 `~/.agents/skills/`。",
      "在 HarnessKit 中，OpenClaw 没有项目范围。",
      "`OPENCLAW_STATE_DIR` 可以更改状态文件夹 `~/.openclaw/` 的位置。`OPENCLAW_CONFIG_PATH` 可以更改 `openclaw.json` 的位置；未设置 `OPENCLAW_STATE_DIR` 时，HarnessKit 安装 skill 所用的 `skills/` 文件夹也会随之移动。`openclaw.json` 未指定默认 workspace 时，`OPENCLAW_WORKSPACE_DIR` 可以更改它的位置。HarnessKit 不读取 `OPENCLAW_PROFILE`。",
    ],
  },
  penguin: {
    nativeDisable: {},
    notes: [
      "PenguinHarness 的每个 agent 都有自己的 skill 文件夹和 `AGENTS.md`，HarnessKit 全部读取。",
      "HarnessKit 只把 skill 安装到一个 PenguinHarness agent 的文件夹中：先按项目、再按 agent 的字母顺序排第一的那个。尚无 PenguinHarness agent 时，则没有可安装的位置。",
      "HarnessKit 暂不读取 PenguinHarness 的 MCP server 和 hook。",
      "在 HarnessKit 中，PenguinHarness 没有项目范围。",
      "`PENGUIN_HOME` 可以更改数据文件夹的位置。",
    ],
  },
  pi: {
    nativeDisable: { mcp: ENABLED_FALSE },
    notes: [
      "`PI_CODING_AGENT_DIR` 可以更改 agent 文件夹（`~/.pi/agent/`）的位置。",
      "Pi 会读取嵌套文件夹中的 skill，HarnessKit 只列出直接位于各个 skill 文件夹中的 skill。",
      "HarnessKit 不读取 Pi 的 `settings.json` 中的 `skills` 过滤规则，因此在那里排除的 skill 仍显示为已启用。",
    ],
  },
};
