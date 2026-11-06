import { type ConfigFormat, referenceFor } from "./agent-reference";

// The figure in "The problem" on docs/how-it-works/installing-an-mcp-server (src/components/docs/McpSheets.astro):
// one MCP server, `git` (run as `uvx mcp-server-git`), in five agents' user-level MCP files. File and format come from agent-reference.ts. `code` is the part of the file that holds the server, in
// the shape the agent's reader accepts (crates/hk-core/src/adapter/<agent>.rs read_mcp_servers_from). HarnessKit's
// writers in crates/hk-core/src/deployer.rs write the same keys, plus an empty `env` (JSON) or `enabled: true` (YAML).
// mcp-snippets.test.ts parses each `code` and checks its top-level key against agent-reference.ts.

export interface McpSnippet {
  /** agents.ts id. */
  agent: string;
  file: string;
  format: ConfigFormat;
  code: string;
  /** What changes from file to file, highlighted on the sheet. Every occurrence in `code` is marked. */
  marks: readonly string[];
}

const SNIPPETS: readonly Omit<McpSnippet, "file" | "format">[] = [
  // claude.rs read_mcp_servers_from: "mcpServers" → {command, args}; deploy_mcp_server_json(…, "mcpServers").
  {
    agent: "claude",
    code: `"mcpServers": {
  "git": {
    "command": "uvx",
    "args": ["mcp-server-git"]
  }
}`,
    marks: [`"mcpServers"`],
  },
  // codex.rs read_mcp_servers_from: one [mcp_servers.<name>] table per server; deploy_mcp_server_toml.
  {
    agent: "codex",
    code: `[mcp_servers.git]
command = "uvx"
args = ["mcp-server-git"]`,
    marks: ["mcp_servers"],
  },
  // gemini.rs read_mcp_servers_from: "mcpServers" inside Gemini CLI's settings.json; deploy_mcp_server_json(…, "mcpServers").
  {
    agent: "gemini",
    code: `"mcpServers": {
  "git": {
    "command": "uvx",
    "args": ["mcp-server-git"]
  }
}`,
    marks: [`"mcpServers"`],
  },
  // opencode.rs parse_mcp_entry: "mcp" → {type: "local", command: [bin, ...args]}; build_opencode_mcp_value.
  {
    agent: "opencode",
    code: `"mcp": {
  "git": {
    "type": "local",
    "command": ["uvx", "mcp-server-git"]
  }
}`,
    marks: [`"mcp"`, `"type": "local"`],
  },
  // hermes.rs read_mcp_servers_from: "mcp_servers" mapping → {command, args}; deploy_mcp_server_hermes_yaml.
  {
    agent: "hermes",
    code: `mcp_servers:
  git:
    command: uvx
    args: [mcp-server-git]`,
    marks: ["mcp_servers"],
  },
];

export const MCP_SNIPPETS: readonly McpSnippet[] = SNIPPETS.map((snippet) => {
  const mcp = referenceFor(snippet.agent).mcp;
  if (!mcp) throw new Error(`mcp-snippets: ${snippet.agent} has no MCP file in agent-reference.ts`);
  return { ...snippet, file: mcp.file, format: mcp.format };
});

/** The figure's caption counts these. */
export const MCP_FILE_COUNT = MCP_SNIPPETS.length;
export const MCP_FORMAT_COUNT = new Set(MCP_SNIPPETS.map((snippet) => snippet.format)).size;

/** `code` cut into runs, each marked or not, in order. */
export function markedRuns(code: string, marks: readonly string[]): { text: string; marked: boolean }[] {
  if (marks.length === 0) return [{ text: code, marked: false }];
  const escaped = marks.map((mark) => mark.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  // A capture group keeps the marks in split()'s output.
  return code.split(new RegExp(`(${escaped.join("|")})`)).filter(Boolean).map((text) => ({ text, marked: marks.includes(text) }));
}
