import { describe, expect, it } from "vitest";
import type { Extension } from "@/lib/types";
import { buildGroups, expandGroupKeys } from "../extension-helpers";

const baseExt: Extension = {
  id: "test-1",
  kind: "skill",
  name: "my-skill",
  description: "A test skill",
  source: {
    origin: "git",
    url: "https://github.com/alice/repo.git",
    version: null,
    commit_hash: null,
  },
  agents: ["claude"],
  tags: ["utils"],
  pack: null,
  permissions: [],
  enabled: true,
  trust_score: 80,
  installed_at: "2025-01-01T00:00:00Z",
  updated_at: "2025-01-01T00:00:00Z",
  source_path: null,
  cli_parent_id: null,
  cli_meta: null,
  install_meta: null,
};

// ---------------------------------------------------------------------------
// buildGroups
// ---------------------------------------------------------------------------

describe("buildGroups", () => {
  it("groups extensions with same name and source into one group", () => {
    const a = { ...baseExt, id: "a", agents: ["claude"] };
    const b = { ...baseExt, id: "b", agents: ["cursor"] };
    const groups = buildGroups([a, b]);

    expect(groups).toHaveLength(1);
    expect(groups[0].instances).toHaveLength(2);
    expect(groups[0].agents).toContain("claude");
    expect(groups[0].agents).toContain("cursor");
  });

  it("separates extensions with different names", () => {
    const a = { ...baseExt, id: "a", name: "skill-a" };
    const b = { ...baseExt, id: "b", name: "skill-b" };
    const groups = buildGroups([a, b]);

    expect(groups).toHaveLength(2);
    expect(groups.map((g) => g.name).sort()).toEqual(["skill-a", "skill-b"]);
  });

  it("includes extensions with cli_parent_id as separate groups", () => {
    const parent = {
      ...baseExt,
      id: "parent",
      name: "my-cli",
      kind: "cli" as const,
    };
    const child = {
      ...baseExt,
      id: "child",
      name: "my-skill",
      cli_parent_id: "parent",
    };
    const groups = buildGroups([parent, child]);

    expect(groups).toHaveLength(2);
    expect(groups.map((g) => g.name).sort()).toEqual(["my-cli", "my-skill"]);
  });

  it("merges tags from all instances (deduped)", () => {
    const a = { ...baseExt, id: "a", tags: ["utils", "git"] };
    const b = { ...baseExt, id: "b", tags: ["git", "deploy"] };
    const groups = buildGroups([a, b]);

    expect(groups).toHaveLength(1);
    const tags = groups[0].tags;
    expect(tags).toHaveLength(3);
    expect(tags).toContain("utils");
    expect(tags).toContain("git");
    expect(tags).toContain("deploy");
  });

  it("uses minimum trust_score across instances", () => {
    const a = { ...baseExt, id: "a", trust_score: 90 };
    const b = { ...baseExt, id: "b", trust_score: 60 };
    const groups = buildGroups([a, b]);

    expect(groups).toHaveLength(1);
    expect(groups[0].trust_score).toBe(60);
  });

  it("returns null trust_score when all instances have null", () => {
    const a = { ...baseExt, id: "a", trust_score: null };
    const b = { ...baseExt, id: "b", trust_score: null };
    const groups = buildGroups([a, b]);

    expect(groups).toHaveLength(1);
    expect(groups[0].trust_score).toBeNull();
  });

  it("returns empty array for empty input", () => {
    expect(buildGroups([])).toEqual([]);
  });

  it("enabled is true if any instance is enabled", () => {
    const a = { ...baseExt, id: "a", enabled: false };
    const b = { ...baseExt, id: "b", enabled: true };
    const groups = buildGroups([a, b]);

    expect(groups).toHaveLength(1);
    expect(groups[0].enabled).toBe(true);
  });

  it("enabled is false when all instances are disabled", () => {
    const a = { ...baseExt, id: "a", enabled: false };
    const b = { ...baseExt, id: "b", enabled: false };
    const groups = buildGroups([a, b]);

    expect(groups).toHaveLength(1);
    expect(groups[0].enabled).toBe(false);
  });

  // -- permission merging (tests deduplicatePermissions indirectly) --

  it("merges permissions of same type (paths deduped and sorted)", () => {
    const a: Extension = {
      ...baseExt,
      id: "a",
      permissions: [{ type: "filesystem", paths: ["/tmp", "/home"] }],
    };
    const b: Extension = {
      ...baseExt,
      id: "b",
      permissions: [{ type: "filesystem", paths: ["/home", "/var"] }],
    };
    const groups = buildGroups([a, b]);
    const perms = groups[0].permissions;

    expect(perms).toHaveLength(1);
    expect(perms[0].type).toBe("filesystem");
    expect((perms[0] as { type: "filesystem"; paths: string[] }).paths).toEqual(
      ["/home", "/tmp", "/var"],
    );
  });

  it("keeps different permission types separate", () => {
    const a: Extension = {
      ...baseExt,
      id: "a",
      permissions: [{ type: "filesystem", paths: ["/tmp"] }],
    };
    const b: Extension = {
      ...baseExt,
      id: "b",
      permissions: [{ type: "network", domains: ["example.com"] }],
    };
    const groups = buildGroups([a, b]);
    const perms = groups[0].permissions;

    expect(perms).toHaveLength(2);
    const types = perms.map((p) => p.type).sort();
    expect(types).toEqual(["filesystem", "network"]);
  });

  it("returns empty permissions when no instances have permissions", () => {
    const a = {
      ...baseExt,
      id: "a",
      permissions: [] as Extension["permissions"],
    };
    const b = {
      ...baseExt,
      id: "b",
      permissions: [] as Extension["permissions"],
    };
    const groups = buildGroups([a, b]);

    expect(groups[0].permissions).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// expandGroupKeys
// ---------------------------------------------------------------------------

describe("expandGroupKeys", () => {
  it("expands group keys into instance IDs", () => {
    const a = { ...baseExt, id: "ext-1", agents: ["claude"] };
    const b = { ...baseExt, id: "ext-2", agents: ["cursor"] };
    const groups = buildGroups([a, b]);
    const key = groups[0].groupKey;

    const ids = expandGroupKeys(groups, new Set([key]));
    expect(ids.sort()).toEqual(["ext-1", "ext-2"]);
  });

  it("ignores unselected groups", () => {
    const a = { ...baseExt, id: "ext-1", name: "skill-a" };
    const b = { ...baseExt, id: "ext-2", name: "skill-b" };
    const groups = buildGroups([a, b]);
    const keyA = groups.find((g) => g.name === "skill-a")!.groupKey;

    const ids = expandGroupKeys(groups, new Set([keyA]));
    expect(ids).toEqual(["ext-1"]);
  });

  it("returns empty array when no keys are selected", () => {
    const groups = buildGroups([baseExt]);
    expect(expandGroupKeys(groups, new Set())).toEqual([]);
  });
});
