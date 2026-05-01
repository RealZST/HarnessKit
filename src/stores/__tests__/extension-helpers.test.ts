import { describe, expect, it } from "vitest";
import type { Extension } from "@/lib/types";
import {
  buildGroups,
  expandGroupKeys,
  getCachedFiltered,
  getCachedGroups,
} from "../extension-helpers";

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
  scope: { type: "global" },
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

  it("merges a sourceless row into a URL-based sibling sharing kind+name+scope", () => {
    // When some instances of the same logical extension carry pack/url
    // metadata and others don't (e.g. a later scan finds a copy without
    // marketplace provenance), they should still group into one row.
    const shared: Extension = {
      ...baseExt,
      source: { origin: "agent", url: null, version: null, commit_hash: null },
      install_meta: null,
    };
    const withPack = { ...shared, pack: "owner/repo" };
    const a = { ...withPack, id: "a", agents: ["x"] };
    const b = { ...withPack, id: "b", agents: ["y"] };
    const c = { ...shared, id: "c", agents: ["z"], pack: null };

    const groups = buildGroups([a, b, c]);

    expect(groups).toHaveLength(1);
    expect(groups[0].instances).toHaveLength(3);
  });

  it("does NOT merge a sourceless row when there are multiple URL-based siblings (ambiguous)", () => {
    const shared: Extension = {
      ...baseExt,
      source: { origin: "agent", url: null, version: null, commit_hash: null },
      install_meta: null,
    };
    const a = { ...shared, id: "a", agents: ["x"], pack: "owner-1/repo" };
    const b = { ...shared, id: "b", agents: ["y"], pack: "owner-2/repo" };
    const c = { ...shared, id: "c", agents: ["z"], pack: null };

    const groups = buildGroups([a, b, c]);

    // Two distinct URL-based developers → can't decide where `c` belongs;
    // it stays as its own group rather than getting attached arbitrarily.
    expect(groups).toHaveLength(3);
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

// ---------------------------------------------------------------------------
// Issue #16 reproduction: toggle status not updating
// ---------------------------------------------------------------------------

describe("Issue #16: single-instance toggle", () => {
  it("single instance enabled toggles correctly in buildGroups", () => {
    // Single agent → single instance → group enabled directly reflects instance
    const ext = { ...baseExt, id: "a", enabled: false, agents: ["claude"] };
    const groups = buildGroups([ext]);
    expect(groups).toHaveLength(1);
    expect(groups[0].enabled).toBe(false);

    // Simulate optimistic update: create new extension with enabled: true
    const toggled = { ...ext, enabled: true };
    const groupsAfter = buildGroups([toggled]);
    expect(groupsAfter).toHaveLength(1);
    expect(groupsAfter[0].enabled).toBe(true);
  });

  it("getCachedGroups invalidates when extensions array ref changes", () => {
    const ext = { ...baseExt, id: "a", enabled: false };
    const exts1 = [ext];
    const groups1 = getCachedGroups(exts1);
    expect(groups1[0].enabled).toBe(false);

    // Simulate optimistic update: .map() creates new array with new objects
    const exts2 = exts1.map((e) => ({ ...e, enabled: true }));
    const groups2 = getCachedGroups(exts2);

    // Cache should be invalidated — new array ref → rebuild
    expect(groups2[0].enabled).toBe(true);
    // Must be different references (new group objects)
    expect(groups2).not.toBe(groups1);
    expect(groups2[0]).not.toBe(groups1[0]);
  });

  it("getCachedGroups returns same ref when extensions array is identical", () => {
    const exts = [{ ...baseExt, id: "a" }];
    const g1 = getCachedGroups(exts);
    const g2 = getCachedGroups(exts);
    // Same input ref → same output ref (cache hit)
    expect(g2).toBe(g1);
  });

  it("optimistic update pattern produces new group with updated enabled", () => {
    // Simulates the exact flow in extension-store.ts toggle():
    // 1. Start with disabled extension
    const original: Extension[] = [
      {
        ...baseExt,
        id: "plugin-1",
        kind: "plugin",
        enabled: false,
        agents: ["claude"],
      },
    ];
    const groupsBefore = getCachedGroups(original);
    expect(groupsBefore[0].enabled).toBe(false);

    // 2. Optimistic update: set(() => ({ extensions: s.extensions.map(...) }))
    const ids = new Set(["plugin-1"]);
    const updated = original.map((e) =>
      ids.has(e.id) ? { ...e, enabled: true } : e,
    );
    const groupsAfter = getCachedGroups(updated);

    // 3. New groups should reflect the toggle
    expect(groupsAfter[0].enabled).toBe(true);
    // 4. Different references — Zustand selector would detect change
    expect(groupsAfter).not.toBe(groupsBefore);
  });
});

// ---------------------------------------------------------------------------
// getCachedFiltered with scope
// ---------------------------------------------------------------------------

describe("getCachedFiltered with scope", () => {
  const globalExt: Extension = {
    ...baseExt,
    id: "g",
    scope: { type: "global" },
  };
  const projectExt: Extension = {
    ...baseExt,
    id: "p",
    name: "proj-skill",
    scope: { type: "project", name: "alpha", path: "/p/alpha" },
  };
  const groups = buildGroups([globalExt, projectExt]);

  it("returns only global rows when scope = global", () => {
    const result = getCachedFiltered(groups, null, null, null, null, "", {
      type: "global",
    });
    expect(result.map((g) => g.instances[0].id)).toEqual(["g"]);
  });

  it("returns only project rows when scope = project", () => {
    const result = getCachedFiltered(groups, null, null, null, null, "", {
      type: "project",
      name: "alpha",
      path: "/p/alpha",
    });
    expect(result.map((g) => g.instances[0].id)).toEqual(["p"]);
  });

  it("returns all rows when scope = all", () => {
    const result = getCachedFiltered(groups, null, null, null, null, "", {
      type: "all",
    });
    expect(result.length).toBe(2);
  });
});
