// @vitest-environment node
import { load as parseYaml } from "js-yaml";
import { parse as parseToml } from "smol-toml";
import { describe, expect, it } from "vitest";
import { referenceFor } from "./agent-reference";
import { MCP_FILE_COUNT, MCP_SNIPPETS, markedRuns, type McpSnippet } from "./mcp-snippets";

// js-yaml and smol-toml are Astro's own dependencies (it parses frontmatter with them), not the site's.

/** The snippet read as a whole file. A JSON snippet is the members of the file's top-level object. */
function parse(snippet: McpSnippet): Record<string, unknown> {
  switch (snippet.format) {
    case "JSON":
    case "JSONC":
      return JSON.parse(`{${snippet.code}}`);
    case "TOML":
      return parseToml(snippet.code);
    case "YAML":
      return parseYaml(snippet.code) as Record<string, unknown>;
    default:
      throw new Error(`No parser for ${snippet.format}; add one to this test`);
  }
}

/** The server's command line, from {command, args} or OpenCode's single `command` array. */
function commandLine(server: { command?: unknown; args?: unknown }): unknown[] {
  if (Array.isArray(server.command)) return server.command;
  return [server.command, ...(Array.isArray(server.args) ? server.args : [])];
}

describe("Problem block MCP snippets", () => {
  it.each(MCP_SNIPPETS)("$agent's snippet parses as $format, with the servers under its agent-reference key", (snippet) => {
    const parsed = parse(snippet);
    const key = referenceFor(snippet.agent).mcp?.key;
    expect(key, snippet.agent).toBeDefined();
    expect(Object.keys(parsed)).toEqual([key]);
    expect(Object.keys(parsed[key!] as object)).toEqual(["git"]);
  });

  it.each(MCP_SNIPPETS)("$agent's snippet is the same server, uvx mcp-server-git", (snippet) => {
    const server = (parse(snippet)[referenceFor(snippet.agent).mcp!.key!] as Record<string, never>).git;
    expect(commandLine(server)).toEqual(["uvx", "mcp-server-git"]);
  });

  // The caption counts these as files, so one agent twice would count one file twice.
  it("names each agent once", () => {
    expect(new Set(MCP_SNIPPETS.map((snippet) => snippet.agent)).size).toBe(MCP_FILE_COUNT);
  });

  // McpSheets.astro sizes a sheet for 40 columns: that fits a 320px phone at 11px.
  it.each(MCP_SNIPPETS)("$agent's snippet fits 40 columns, with spaces for indents", (snippet) => {
    for (const line of snippet.code.split("\n")) expect(line.length, line).toBeLessThanOrEqual(40);
    expect(snippet.code).not.toContain("\t");
  });

  it.each(MCP_SNIPPETS)("$agent's marks are in its snippet, and marking keeps the text", (snippet) => {
    for (const mark of snippet.marks) expect(snippet.code).toContain(mark);
    const runs = markedRuns(snippet.code, snippet.marks);
    expect(runs.map((run) => run.text).join("")).toBe(snippet.code);
    expect(runs.filter((run) => run.marked).map((run) => run.text)).toEqual(
      expect.arrayContaining([...snippet.marks]),
    );
  });
});
