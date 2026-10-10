import { describe, expect, it } from "vitest";
import { buildInstallPlan } from "@/lib/install-plan";
import type {
  AgentInfo,
  ConfigScope,
  Extension,
  GroupedExtension,
} from "@/lib/types";

const GLOBAL: ConfigScope = { type: "global" };
const PROJECT: ConfigScope = { type: "project", name: "p", path: "/p" };

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

describe("buildInstallPlan", () => {
  it("skips agents that already hold a copy in the target scope", () => {
    const g = group({
      groupKey: "skill-a",
      kind: "skill",
      instances: [ext({ id: "i1", agents: ["codex"] })],
    });
    const { pairs, skipped } = buildInstallPlan(
      [g],
      [agent({ name: "codex" }), agent({ name: "claude" })],
      g.instances,
      GLOBAL,
    );
    expect(skipped).toEqual([
      {
        groupKey: "skill-a",
        targetAgent: "codex",
        reason: "already-installed",
      },
    ]);
    expect(pairs).toEqual([
      {
        groupKey: "skill-a",
        sourceId: "i1",
        targetAgent: "claude",
        isHermesSkill: false,
      },
    ]);
  });

  it("only counts copies in the target scope as installed", () => {
    // Codex has the skill globally but not in the project: a project
    // install still goes ahead, copied from the global instance.
    const g = group({
      groupKey: "skill-a",
      kind: "skill",
      instances: [ext({ id: "i1", agents: ["codex"], scope: GLOBAL })],
    });
    const { pairs, skipped } = buildInstallPlan(
      [g],
      [agent({ name: "codex" })],
      g.instances,
      PROJECT,
    );
    expect(skipped).toEqual([]);
    expect(pairs).toMatchObject([{ sourceId: "i1", targetAgent: "codex" }]);
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
    const { skipped, pairs } = buildInstallPlan(
      [g],
      [httpOnly],
      g.instances,
      GLOBAL,
    );
    expect(pairs).toEqual([]);
    expect(skipped).toEqual([
      {
        groupKey: "mcp-a",
        targetAgent: "codex",
        reason: "transport-unsupported",
      },
    ]);
  });

  it("expands a CLI bundle to its children, deduped by name+kind", () => {
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
      // Same name+kind on another agent: one item, and because that
      // other agent is the target it already has it.
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
    const { pairs, skipped } = buildInstallPlan(
      [g],
      [httpOnly],
      [cliInst, ...children],
      GLOBAL,
    );
    expect(pairs).toEqual([]);
    expect(skipped).toEqual([
      {
        groupKey: "cli-bundle",
        itemName: "s",
        targetAgent: "codex",
        reason: "already-installed",
      },
      {
        groupKey: "cli-bundle",
        itemName: "m",
        targetAgent: "codex",
        reason: "transport-unsupported",
      },
    ]);
  });

  it("never copies a shared folder onto itself under overwrite", () => {
    // Several agents share ~/.agents/skills: one row each, same path.
    const shared = "/home/u/.agents/skills/x";
    const gemini = ext({
      id: "x-gemini",
      agents: ["gemini"],
      source_path: shared,
      updated_at: "2026-02-01",
    });
    const codex = ext({
      id: "x-codex",
      agents: ["codex"],
      source_path: shared,
      updated_at: "2026-01-01",
    });
    const g = group({ groupKey: "skill-x", instances: [gemini, codex] });
    const { pairs, skipped } = buildInstallPlan(
      [g],
      [agent({ name: "codex" }), agent({ name: "cursor" })],
      g.instances,
      GLOBAL,
      { overwrite: true },
    );
    // Codex holds the newest file (through Gemini's row): it is the
    // source, not a target. Cursor gets a real copy.
    expect(skipped).toEqual([
      { groupKey: "skill-x", targetAgent: "codex", reason: "source" },
    ]);
    expect(pairs).toMatchObject([
      { sourceId: "x-gemini", targetAgent: "cursor" },
    ]);
  });

  it("never installs a CLI child onto the agent whose copy it would come from", () => {
    // CLI rows are always Global, so a project target can't be "already
    // installed" at group level; the child's own project copy is the
    // preferred source — and must not be written onto itself.
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
    const own = ext({
      id: "s-codex-p",
      kind: "skill",
      name: "s",
      pack: "p",
      agents: ["codex"],
      scope: PROJECT,
    });
    const { pairs, skipped } = buildInstallPlan(
      [g],
      [agent({ name: "codex" })],
      [cliInst, own],
      PROJECT,
    );
    expect(pairs).toEqual([]);
    expect(skipped).toEqual([
      {
        groupKey: "cli-bundle",
        itemName: "s",
        targetAgent: "codex",
        reason: "already-installed",
      },
    ]);
  });

  it("gates a CLI bundle as a whole before expanding its children", () => {
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
    const child = ext({ id: "c1", kind: "skill", name: "s", pack: "p" });
    const noProjectCli = agent({
      name: "hermes",
      capabilities: {
        project_install: { skill: true, mcp: true, hook: true, cli: false },
        hooks_supported: true,
        global_hook_install: true,
      },
    });
    const { pairs, skipped } = buildInstallPlan(
      [g],
      [noProjectCli],
      [cliInst, child],
      PROJECT,
    );
    expect(pairs).toEqual([]);
    // One skip for the bundle, not one per child.
    expect(skipped).toEqual([
      {
        groupKey: "cli-bundle",
        targetAgent: "hermes",
        reason: "scope-unsupported",
      },
    ]);
  });

  it("lets a child selected as its own row own the item; the bundle covers the rest", () => {
    const cliInst = ext({
      id: "cli-1",
      kind: "cli",
      name: "bundle",
      pack: "p",
    });
    const skillChild = ext({ id: "c1", kind: "skill", name: "s", pack: "p" });
    const mcpChild = ext({ id: "c2", kind: "mcp", name: "m", pack: "p" });
    const bundle = group({
      groupKey: "cli-bundle",
      kind: "cli",
      name: "bundle",
      pack: "p",
      instances: [cliInst],
    });
    const skillRow = group({
      groupKey: "skill-s",
      kind: "skill",
      name: "s",
      pack: "p",
      instances: [skillChild],
    });
    const codex = [agent({ name: "codex" })];
    const all = [cliInst, skillChild, mcpChild];
    const { pairs, skipped } = buildInstallPlan(
      [bundle, skillRow],
      codex,
      all,
      GLOBAL,
    );
    // The skill is planned under its own row; the bundle only brings the MCP.
    expect(pairs).toMatchObject([
      { groupKey: "cli-bundle", itemName: "m", sourceId: "c2" },
      { groupKey: "skill-s", sourceId: "c1" },
    ]);
    expect(skipped).toEqual([]);

    // Every child selected on its own: the bundle row has nothing left.
    const mcpRow = group({
      groupKey: "mcp-m",
      kind: "mcp",
      name: "m",
      pack: "p",
      instances: [mcpChild],
    });
    const full = buildInstallPlan(
      [bundle, skillRow, mcpRow],
      codex,
      all,
      GLOBAL,
    );
    expect(full.pairs.map((p) => p.groupKey)).toEqual(["skill-s", "mcp-m"]);
    expect(full.skipped).toEqual([
      {
        groupKey: "cli-bundle",
        targetAgent: "codex",
        reason: "duplicate",
        via: ["skill-s", "mcp-m"],
      },
    ]);
  });

  it("never pairs an agent with a file it holds through another group", () => {
    // Two rows over one shared folder that landed in different groups
    // (different source metadata): the holder must still be recognised.
    const shared = "/home/u/.agents/skills/x";
    const viaA = ext({ id: "x-gem", agents: ["gemini"], source_path: shared });
    const viaB = ext({ id: "x-codex", agents: ["codex"], source_path: shared });
    const groupA = group({ groupKey: "skill|x|a", instances: [viaA] });
    const { pairs, skipped } = buildInstallPlan(
      [groupA],
      [agent({ name: "codex" })],
      [viaA, viaB],
      GLOBAL,
    );
    expect(pairs).toEqual([]);
    expect(skipped).toEqual([
      {
        groupKey: "skill|x|a",
        targetAgent: "codex",
        reason: "already-installed",
      },
    ]);
  });

  it("counts a same-named copy in another group as installed, unless overwriting", () => {
    // A skill without a source URL is grouped per scope: the Global row and
    // codex's project copy are two groups, but the same skill to codex.
    const globalCopy = ext({ id: "g", agents: ["claude"] });
    const codexInP = ext({ id: "p", agents: ["codex"], scope: PROJECT });
    const g = group({ groupKey: "skill|x|global", instances: [globalCopy] });
    const all = [globalCopy, codexInP];
    const codex = [agent({ name: "codex" })];
    expect(buildInstallPlan([g], codex, all, PROJECT).skipped).toEqual([
      {
        groupKey: "skill|x|global",
        targetAgent: "codex",
        reason: "already-installed",
      },
    ]);
    expect(
      buildInstallPlan([g], codex, all, PROJECT, { overwrite: true }).pairs,
    ).toMatchObject([{ sourceId: "g", targetAgent: "codex" }]);
  });

  it("does not let an unrelated same-named row claim a bundle's child", () => {
    const cliInst = ext({
      id: "cli-1",
      kind: "cli",
      name: "bundle",
      pack: "p",
    });
    const child = ext({ id: "c1", kind: "skill", name: "review", pack: "p" });
    const other = ext({ id: "o1", name: "review", pack: "elsewhere" });
    const bundle = group({
      groupKey: "cli-bundle",
      kind: "cli",
      name: "bundle",
      pack: "p",
      instances: [cliInst],
    });
    const otherRow = group({
      groupKey: "skill|review|elsewhere",
      name: "review",
      instances: [other],
    });
    const { pairs, skipped } = buildInstallPlan(
      [bundle, otherRow],
      [agent({ name: "codex" })],
      [cliInst, child, other],
      GLOBAL,
    );
    // The bundle still brings its own child. The unrelated row would write
    // the same name into the same place, so it is the one left out.
    expect(pairs).toHaveLength(1);
    expect(pairs[0]).toMatchObject({
      groupKey: "cli-bundle",
      itemName: "review",
    });
    expect(skipped).toEqual([
      {
        groupKey: "skill|review|elsewhere",
        targetAgent: "codex",
        reason: "duplicate",
        via: ["cli-bundle"],
      },
    ]);
  });

  it("reports no-source for a CLI bundle with no children", () => {
    const cliInst = ext({ id: "cli-1", kind: "cli", name: "bare", pack: null });
    const g = group({
      groupKey: "cli-bare",
      kind: "cli",
      name: "bare",
      instances: [cliInst],
    });
    expect(
      buildInstallPlan([g], [agent({ name: "codex" })], [cliInst], GLOBAL),
    ).toEqual({
      pairs: [],
      skipped: [
        { groupKey: "cli-bare", targetAgent: "codex", reason: "no-source" },
      ],
    });
  });

  it("with overwrite, an older copy is re-installed from the newest one", () => {
    const onClaude = ext({
      id: "on-claude",
      agents: ["claude"],
      updated_at: "2026-03-01",
    });
    const onCodex = ext({
      id: "on-codex",
      agents: ["codex"],
      updated_at: "2026-01-01",
    });
    const g = group({ groupKey: "skill-a", instances: [onClaude, onCodex] });
    const { pairs, skipped } = buildInstallPlan(
      [g],
      [agent({ name: "codex" })],
      g.instances,
      GLOBAL,
      { overwrite: true },
    );
    expect(skipped).toEqual([]);
    expect(pairs).toMatchObject([
      { sourceId: "on-claude", targetAgent: "codex" },
    ]);
  });

  it("with overwrite, the newest copy is the source and its holder is left alone", () => {
    const older = ext({
      id: "on-claude",
      agents: ["claude"],
      updated_at: "2026-01-01",
    });
    const newer = ext({
      id: "on-codex",
      agents: ["codex"],
      updated_at: "2026-03-01",
    });
    const g = group({ groupKey: "skill-a", instances: [older, newer] });
    const { pairs, skipped } = buildInstallPlan(
      [g],
      [
        agent({ name: "claude" }),
        agent({ name: "codex" }),
        agent({ name: "gemini" }),
      ],
      g.instances,
      GLOBAL,
      { overwrite: true },
    );
    expect(skipped).toEqual([
      { groupKey: "skill-a", targetAgent: "codex", reason: "source" },
    ]);
    expect(pairs).toMatchObject([
      { sourceId: "on-codex", targetAgent: "claude" },
      { sourceId: "on-codex", targetAgent: "gemini" },
    ]);
  });

  it("reports an uninstallable kind the same way on the agent that has it", () => {
    const g = group({
      groupKey: "plugin-a",
      kind: "plugin",
      instances: [ext({ id: "i1", kind: "plugin", agents: ["cursor"] })],
    });
    const { skipped } = buildInstallPlan(
      [g],
      [agent({ name: "cursor" }), agent({ name: "codex" })],
      g.instances,
      GLOBAL,
    );
    expect(skipped.map((s) => s.reason)).toEqual([
      "kind-unsupported",
      "kind-unsupported",
    ]);
  });

  it("marks hermes skill pairs", () => {
    const g = group({
      groupKey: "skill-c",
      kind: "skill",
      instances: [ext({ id: "i1", agents: ["claude"] })],
    });
    const { pairs } = buildInstallPlan(
      [g],
      [agent({ name: "hermes" })],
      g.instances,
      GLOBAL,
    );
    expect(pairs[0].isHermesSkill).toBe(true);
  });
});
