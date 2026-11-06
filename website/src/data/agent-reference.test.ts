// @vitest-environment node
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { AGENTS } from "./agents";
import { AGENT_REFERENCE, type AgentReference, agentNameFor, anchorFor, SHARED_SKILLS_DIR } from "./agent-reference";
import { AGENT_REFERENCE_ZH } from "./agent-reference-zh";

const ADAPTER_DIR = new URL("../../../crates/hk-core/src/adapter/", import.meta.url);
const ADAPTER_MOD = new URL("mod.rs", ADAPTER_DIR);
const REFERENCE_PAGES = [
  new URL("../content/docs/docs/reference/agents.mdx", import.meta.url),
  new URL("../content/docs/zh/docs/reference/agents.mdx", import.meta.url),
].filter((page) => existsSync(page));

/** The body of one Rust test function in adapter/mod.rs, so a parse can't wander into the next test. */
function rustTestBody(name: string): string {
  const source = readFileSync(ADAPTER_MOD, "utf8");
  const start = source.indexOf(`fn ${name}()`);
  if (start === -1) throw new Error(`${name} not found in adapter/mod.rs; update this test's parser`);
  const next = source.indexOf("#[test]", start);
  return source.slice(start, next === -1 ? undefined : next);
}

/** test_agent_capabilities_matrix rows: (name, project skill, project mcp, project hook, hooks_supported, global hook). */
function capabilityMatrix(): Map<string, { projectMcp: boolean; projectHook: boolean; hooks: boolean }> {
  const rows = [...rustTestBody("test_agent_capabilities_matrix").matchAll(
    /\("([a-z-]+)",\s*(true|false),\s*(true|false),\s*(true|false),\s*(true|false),\s*(true|false)\)/g,
  )];
  return new Map(rows.map((m) => [m[1], { projectMcp: m[3] === "true", projectHook: m[4] === "true", hooks: m[5] === "true" }]));
}

/** Agents test_supports_native_mcp_toggle_only_native_agents expects to toggle MCP natively. */
function nativeMcpAgents(): string[] {
  const body = rustTestBody("test_supports_native_mcp_toggle_only_native_agents");
  const list = body.match(/matches!\(\s*a\.name\(\),([\s\S]*?)\)/);
  if (!list) throw new Error("native MCP toggle list not found; update this test's parser");
  return [...list[1].matchAll(/"([a-z-]+)"/g)].map((m) => m[1]);
}

/** Agents test_supports_native_skill_toggle_only_hermes expects to toggle skills in their own config. */
function nativeSkillAgents(): string[] {
  const body = rustTestBody("test_supports_native_skill_toggle_only_hermes");
  const list = body.match(/matches!\(\s*a\.name\(\),([\s\S]*?)\)/)?.[1] ?? body.match(/a\.name\(\)\s*==\s*"[a-z-]+"/g)?.join(" ");
  if (!list) throw new Error("native skill toggle list not found; update this test's parser");
  return [...list.matchAll(/"([a-z-]+)"/g)].map((m) => m[1]);
}

/**
 * Environment variables an adapter file reads outside its test module, as `var_os("X")`, `env::var("X")`
 * or OpenClaw's `var("X")` helper. HARNESSKIT_HOME moves every agent at once and is read in lib.rs, not here.
 */
function envVarsRead(source: string): string[] {
  const testModule = source.search(/#\[cfg\(test\)\]\s*mod\s+\w+/);
  const code = testModule === -1 ? source : source.slice(0, testModule);
  const names = [...code.matchAll(/\bvar(?:_os)?\(\s*"([A-Z][A-Z0-9_]*)"\s*\)/g)].map((m) => m[1]);
  return [...new Set(names)].filter((name) => name !== "HARNESSKIT_HOME");
}

const ADAPTER_FILES = readdirSync(ADAPTER_DIR).filter((file) => file.endsWith(".rs")).sort();

/** test_project_skill_dir_paths_match_upstream_conventions: agent → first project skill folder. */
function projectSkillDirs(): Map<string, string> {
  const pairs = rustTestBody("test_project_skill_dir_paths_match_upstream_conventions").matchAll(/\("([a-z-]+)",\s*"([^"]+)"\)/g);
  return new Map([...pairs].map((m) => [m[1], m[2]]));
}

function pathsOf(agent: AgentReference): string[] {
  return [
    agent.homeDir,
    ...agent.skillDirs.global,
    ...agent.skillDirs.project,
    ...agent.rulesFiles,
    ...(agent.mcp ? [agent.mcp.file, agent.mcp.projectFile] : []),
    ...(agent.hooks ? [agent.hooks.file, agent.hooks.projectFile] : []),
  ].filter((path) => path !== undefined);
}

function stringsIn(value: unknown): string[] {
  if (typeof value === "string") return [value];
  if (Array.isArray(value)) return value.flatMap(stringsIn);
  if (value && typeof value === "object") return Object.values(value).flatMap(stringsIn);
  return [];
}

const isProjectPath = (path: string) => path.startsWith("<project>/");
const isGlobalPath = (path: string) => path.startsWith("~/") || path.startsWith("/");
const supports = (id: string, kind: string) => AGENTS.find((a) => a.id === id)?.supports.includes(kind as never) ?? false;

describe("agent reference data", () => {
  it("has one record per agent, in AGENTS order", () => {
    expect(AGENT_REFERENCE.map((a) => a.id)).toEqual(AGENTS.map((a) => a.id));
  });

  it.each(AGENT_REFERENCE)("$id has hooks exactly when agents.ts says it supports hooks", (agent) => {
    expect(agent.hooks !== null).toBe(supports(agent.id, "hook"));
  });

  it.each(AGENT_REFERENCE)("$id has MCP servers exactly when agents.ts says it supports MCP", (agent) => {
    expect(agent.mcp !== null).toBe(supports(agent.id, "mcp"));
  });

  it.each(AGENT_REFERENCE)("$id has no empty strings", (agent) => {
    for (const value of stringsIn(agent)) expect(value.trim(), agent.id).not.toBe("");
  });

  it.each(AGENT_REFERENCE)("$id writes every path from home, a project or the root", (agent) => {
    for (const path of pathsOf(agent)) expect(isGlobalPath(path) || isProjectPath(path), path).toBe(true);
  });

  it.each(AGENT_REFERENCE)("$id keeps global and project paths in their own fields", (agent) => {
    for (const path of [agent.homeDir, ...agent.skillDirs.global]) expect(isGlobalPath(path), path).toBe(true);
    for (const path of agent.skillDirs.project) expect(isProjectPath(path), path).toBe(true);
    for (const location of [agent.mcp, agent.hooks]) {
      if (!location) continue;
      expect(isGlobalPath(location.file), location.file).toBe(true);
      if (location.projectFile) expect(isProjectPath(location.projectFile), location.projectFile).toBe(true);
    }
  });

  it.each(AGENT_REFERENCE)("$id marks folders with a trailing slash", (agent) => {
    for (const dir of [agent.homeDir, ...agent.skillDirs.global, ...agent.skillDirs.project]) expect(dir.endsWith("/"), dir).toBe(true);
  });

  it.each(AGENT_REFERENCE)("$id reads ~/.agents/skills/ exactly when it is one of its skill folders", (agent) => {
    expect(agent.readsSharedAgentsSkills).toBe(agent.skillDirs.global.includes(SHARED_SKILLS_DIR));
  });

  it.each(AGENT_REFERENCE)("$id names an off switch only for kinds it manages", (agent) => {
    if (agent.nativeDisable.mcp) expect(agent.mcp).not.toBeNull();
    if (agent.nativeDisable.hook) expect(agent.hooks).not.toBeNull();
  });
});

describe("agent reference data matches the adapters' own tests", () => {
  const matrix = capabilityMatrix();

  it("finds a capability row for every agent", () => {
    expect([...matrix.keys()].sort()).toEqual(AGENT_REFERENCE.map((a) => a.id).sort());
  });

  it.each(AGENT_REFERENCE)("$id has hooks, project MCP and project hooks as the capability matrix says", (agent) => {
    const row = matrix.get(agent.id);
    expect(agent.hooks !== null, "hooks_supported").toBe(row?.hooks);
    expect(agent.mcp?.projectFile !== undefined, "project mcp").toBe(row?.projectMcp);
    expect(agent.hooks?.projectFile !== undefined, "project hook").toBe(row?.projectHook);
  });

  it("names a native MCP off switch for exactly the agents that toggle MCP in place", () => {
    const withSwitch = AGENT_REFERENCE.filter((a) => a.nativeDisable.mcp).map((a) => a.id);
    expect(withSwitch.sort()).toEqual(nativeMcpAgents().sort());
  });

  it("names a native skill off switch for exactly the agents that toggle skills in their own config", () => {
    const withSwitch = AGENT_REFERENCE.filter((a) => a.nativeDisable.skill).map((a) => a.id);
    const native = nativeSkillAgents();
    expect(native.length, "parsed no agents from the Rust test").toBeGreaterThan(0);
    expect(withSwitch.sort()).toEqual(native.sort());
  });

  it("finds the environment variables the adapters read", () => {
    const all = ADAPTER_FILES.flatMap((file) => envVarsRead(readFileSync(new URL(file, ADAPTER_DIR), "utf8")));
    expect(all.length, "parsed no environment variables; update envVarsRead").toBeGreaterThan(0);
  });

  it.each(ADAPTER_FILES)("names every environment variable %s reads in that agent's notes", (file) => {
    const vars = envVarsRead(readFileSync(new URL(file, ADAPTER_DIR), "utf8"));
    if (vars.length === 0) return;
    const agent = AGENT_REFERENCE.find((a) => a.id === file.replace(/\.rs$/, "").replace(/_/g, "-"));
    expect(agent, `${file} reads ${vars.join(", ")} but is not an agent in agent-reference.ts`).toBeDefined();
    const notes = (agent?.notes ?? []).join("\n");
    for (const name of vars) expect(notes, `name \`${name}\` in ${agent?.id}'s notes`).toContain(`\`${name}\``);
  });

  it.each(AGENT_REFERENCE)("$id installs project skills where the adapter test pins it", (agent) => {
    const pinned = projectSkillDirs().get(agent.id);
    if (pinned === undefined) expect(agent.skillDirs.project).toEqual([]);
    else expect(agent.skillDirs.project[0]).toBe(`<project>/${pinned}/`);
  });
});

describe("the Chinese reference data", () => {
  // Chinese uses its own punctuation, so the check is on shape: a sentence for every English one, and the same code.
  const codeIn = (text: string) => [...text.matchAll(/`[^`]+`/g)].map((m) => m[0]).sort();

  it.each(AGENT_REFERENCE)("says each of $id's notes and off switches in Chinese", (agent) => {
    const zh = AGENT_REFERENCE_ZH[agent.id];
    expect(zh, `add ${agent.id} to agent-reference-zh.ts`).toBeDefined();
    expect(zh.notes?.length ?? 0).toBe(agent.notes?.length ?? 0);
    expect(Object.keys(zh.nativeDisable).sort()).toEqual(Object.keys(agent.nativeDisable).sort());
    const english = [...(agent.notes ?? []), ...Object.values(agent.nativeDisable)];
    const chinese = [...(zh.notes ?? []), ...Object.values(zh.nativeDisable)];
    chinese.forEach((sentence, i) => expect(codeIn(sentence ?? ""), english[i]).toEqual(codeIn(english[i] ?? "")));
  });
});

describe.each(REFERENCE_PAGES)("the agents reference page %s", (pageUrl) => {
  const page = readFileSync(pageUrl, "utf8");
  const headings = [...page.matchAll(/^## (.+)$/gm)].map((m) => m[1].trim());

  it("has one ## heading per agent, in AGENTS order, so each agent gets a stable anchor", () => {
    expect(headings.filter((h) => AGENTS.some((a) => a.name === h))).toEqual(AGENTS.map((a) => a.name));
  });

  it("renders each agent's facts under its own heading", () => {
    for (const agent of AGENTS) {
      const section = page.split(`## ${agent.name}\n`)[1]?.split(/^## /m)[0] ?? "";
      expect(section, agent.name).toContain(`<AgentFacts id="${agent.id}" />`);
    }
  });

  it("links the summary table to anchors made from display names", () => {
    expect(anchorFor(agentNameFor("claude"))).toBe("claude-code");
    expect(anchorFor(agentNameFor("qoder-cn"))).toBe("qoder-cn");
    expect(anchorFor(agentNameFor("penguin"))).toBe("penguinharness");
  });
});
