import { describe, expect, it } from "vitest";
import type {
  AgentInfo,
  ConfigScope,
  Extension,
  GroupedExtension,
} from "@/lib/types";
import { buildSyncPairs } from "../sync-pairs";

const GLOBAL: ConfigScope = { type: "global" };

function ext(over: Partial<Extension> & { id: string }): Extension {
  return {
    kind: "skill",
    name: "x",
    description: "",
    source: { origin: "local", url: null, version: null, commit_hash: null },
    agents: ["claude"],
    tags: [],
    pack: null,
    permissions: [],
    enabled: true,
    trust_score: null,
    installed_at: "2026-01-01",
    updated_at: "2026-01-01",
    source_path: null,
    cli_parent_id: null,
    cli_meta: null,
    install_meta: null,
    scope: GLOBAL,
    ...over,
  };
}

function agent(over: Partial<AgentInfo> & { name: string }): AgentInfo {
  return {
    detected: true,
    extension_count: 0,
    path: "",
    enabled: true,
    capabilities: {
      project_install: { skill: true, mcp: true, hook: true, cli: true },
      hooks_supported: true,
      global_hook_install: true,
      mcp_remote: { http: true, sse: true },
    },
    ...over,
  };
}

function group(
  over: Partial<GroupedExtension> & { groupKey: string },
): GroupedExtension {
  const instances = over.instances ?? [ext({ id: `${over.groupKey}-i` })];
  return {
    name: "x",
    kind: "skill",
    description: "",
    source: { origin: "local", url: null, version: null, commit_hash: null },
    agents: ["claude"],
    tags: [],
    pack: null,
    permissions: [],
    enabled: true,
    trust_score: null,
    installed_at: "2026-01-01",
    updated_at: "2026-01-01",
    instances,
    ...over,
  };
}

describe("buildSyncPairs", () => {
  it("returns empty for empty inputs without throwing", () => {
    expect(buildSyncPairs([], [agent({ name: "a" })], [])).toEqual({
      pairs: [],
      skipped: [],
    });
    expect(buildSyncPairs([group({ groupKey: "g" })], [], [])).toEqual({
      pairs: [],
      skipped: [],
    });
  });

  it("skips already-installed agents", () => {
    const g = group({
      groupKey: "skill-a",
      kind: "skill",
      instances: [ext({ id: "i1", agents: ["codex"] })],
    });
    const { pairs, skipped } = buildSyncPairs(
      [g],
      [agent({ name: "codex" }), agent({ name: "claude" })],
      g.instances,
    );
    expect(skipped).toContainEqual({
      groupKey: "skill-a",
      targetAgent: "codex",
      reason: "already-installed",
    });
    expect(pairs).toHaveLength(1);
    expect(pairs[0]).toMatchObject({
      groupKey: "skill-a",
      sourceId: "i1",
      targetAgent: "claude",
    });
  });

  it("reports hook-unsupported before global-hook-blocked", () => {
    const g = group({
      groupKey: "hook-a",
      kind: "hook",
      instances: [ext({ id: "i1", kind: "hook", agents: [] })],
    });
    const noHooks = agent({
      name: "nohooks",
      capabilities: {
        project_install: { skill: true, mcp: true, hook: true, cli: true },
        hooks_supported: false,
        global_hook_install: false,
      },
    });
    const { skipped } = buildSyncPairs([g], [noHooks], g.instances);
    expect(skipped).toEqual([
      {
        groupKey: "hook-a",
        targetAgent: "nohooks",
        reason: "hook-unsupported",
      },
    ]);
  });

  it("reports global-hook-blocked when hooks supported but global install off", () => {
    const g = group({
      groupKey: "hook-b",
      kind: "hook",
      instances: [ext({ id: "i1", kind: "hook", agents: [] })],
    });
    const kiroLike = agent({
      name: "kiro",
      capabilities: {
        project_install: { skill: true, mcp: true, hook: true, cli: true },
        hooks_supported: true,
        global_hook_install: false,
      },
    });
    const { skipped, pairs } = buildSyncPairs([g], [kiroLike], g.instances);
    expect(pairs).toEqual([]);
    expect(skipped).toEqual([
      {
        groupKey: "hook-b",
        targetAgent: "kiro",
        reason: "global-hook-blocked",
      },
    ]);
  });

  it("skips remote MCP transports the agent cannot express", () => {
    const g = group({
      groupKey: "mcp-a",
      kind: "mcp",
      instances: [
        ext({ id: "i1", kind: "mcp", agents: [], mcp_transport: "sse" }),
      ],
    });
    const httpOnly = agent({
      name: "codex",
      capabilities: {
        project_install: { skill: true, mcp: true, hook: true, cli: true },
        hooks_supported: true,
        global_hook_install: true,
        mcp_remote: { http: true, sse: false },
      },
    });
    const { skipped, pairs } = buildSyncPairs([g], [httpOnly], g.instances);
    expect(pairs).toEqual([]);
    expect(skipped).toEqual([
      {
        groupKey: "mcp-a",
        targetAgent: "codex",
        reason: "transport-unsupported",
      },
    ]);
  });

  it("dedupes CLI children by name+kind and skips unsupported transports", () => {
    const cliInst = ext({
      id: "cli-1",
      kind: "cli",
      name: "bundle",
      pack: "p",
    });
    const g = group({
      groupKey: "cli-bundle",
      kind: "cli",
      name: "bundle",
      pack: "p",
      instances: [cliInst],
    });
    const children = [
      ext({
        id: "c1",
        kind: "skill",
        name: "s",
        pack: "p",
        agents: ["claude"],
      }),
      // Same name+kind: merged group sibling — only one pair.
      ext({ id: "c2", kind: "skill", name: "s", pack: "p", agents: ["codex"] }),
      ext({
        id: "c3",
        kind: "mcp",
        name: "m",
        pack: "p",
        agents: ["claude"],
        mcp_transport: "sse",
      }),
    ];
    const httpOnly = agent({
      name: "codex",
      capabilities: {
        project_install: { skill: true, mcp: true, hook: true, cli: true },
        hooks_supported: true,
        global_hook_install: true,
        mcp_remote: { http: true, sse: false },
      },
    });
    const { pairs, skipped } = buildSyncPairs(
      [g],
      [httpOnly],
      [cliInst, ...children],
    );
    expect(pairs).toHaveLength(1);
    expect(pairs[0]).toMatchObject({
      groupKey: "cli-bundle",
      sourceId: "c1",
      targetAgent: "codex",
    });
    expect(skipped).toContainEqual({
      groupKey: "cli-bundle",
      targetAgent: "codex",
      reason: "transport-unsupported",
    });
  });

  it("prefers the global copy as install source", () => {
    const g = group({
      groupKey: "skill-b",
      kind: "skill",
      instances: [
        ext({
          id: "proj-copy",
          agents: ["claude"],
          scope: { type: "project", name: "p", path: "/p" },
        }),
        ext({ id: "global-copy", agents: ["claude"], scope: GLOBAL }),
      ],
    });
    const { pairs } = buildSyncPairs(
      [g],
      [agent({ name: "codex" })],
      g.instances,
    );
    expect(pairs).toEqual([
      {
        groupKey: "skill-b",
        sourceId: "global-copy",
        targetAgent: "codex",
        isHermesSkill: false,
      },
    ]);
  });

  it("marks hermes skill pairs", () => {
    const g = group({
      groupKey: "skill-c",
      kind: "skill",
      instances: [ext({ id: "i1", agents: ["claude"] })],
    });
    const { pairs } = buildSyncPairs(
      [g],
      [agent({ name: "hermes" })],
      g.instances,
    );
    expect(pairs[0].isHermesSkill).toBe(true);
  });
});
