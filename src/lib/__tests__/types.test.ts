import { describe, expect, it } from "vitest";
import type { Extension } from "../types";
import {
  agentDisplayName,
  extensionGroupKey,
  formatRelativeTime,
  severityColor,
  sortAgentNames,
  trustColor,
  trustTier,
} from "../types";

describe("extensionGroupKey", () => {
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
    tags: [],
    pack: null,
    permissions: [],
    enabled: true,
    trust_score: null,
    installed_at: "2025-01-01T00:00:00Z",
    updated_at: "2025-01-01T00:00:00Z",
    source_path: null,
    cli_parent_id: null,
    cli_meta: null,
    install_meta: null,
    scope: { type: "global" },
  };

  it("produces a stable key from kind, name, and developer", () => {
    const key = extensionGroupKey(baseExt);
    expect(key).toBe("skill\0my-skill\0alice/repo");
  });

  it("strips .git suffix from GitHub URLs", () => {
    const key = extensionGroupKey(baseExt);
    expect(key).not.toContain(".git");
  });

  it("handles null source URL", () => {
    const ext = { ...baseExt, source: { ...baseExt.source, url: null } };
    const key = extensionGroupKey(ext);
    expect(key).toBe("skill\0my-skill\0");
  });

  it("merges same-name same-developer skills regardless of origin", () => {
    // Same logical skill installed two ways: registry + local copy.
    // They should fold into the same group so the UI shows one row.
    const fromRegistry: Extension = {
      ...baseExt,
      source: { ...baseExt.source, origin: "registry" },
    };
    const fromLocal: Extension = {
      ...baseExt,
      source: { ...baseExt.source, origin: "local" },
    };
    expect(extensionGroupKey(fromRegistry)).toBe(extensionGroupKey(fromLocal));
  });

  it("keeps different developers' same-named skills separate", () => {
    // Two different lints both named "lint": shouldn't silently collapse.
    const aliceLint: Extension = {
      ...baseExt,
      name: "lint",
      source: {
        ...baseExt.source,
        url: "https://github.com/alice/lint.git",
      },
    };
    const bobLint: Extension = {
      ...baseExt,
      name: "lint",
      source: {
        ...baseExt.source,
        url: "https://github.com/bob/lint.git",
      },
    };
    expect(extensionGroupKey(aliceLint)).not.toBe(extensionGroupKey(bobLint));
  });

  it("falls back to install_meta.url when source.url is null", () => {
    // Marketplace-installed skills end up with source.url=null (scanner
    // re-discovers them as agent files), but install_meta.url carries the
    // authoritative origin. The 6 copies of pbakaus/impeccable/audit
    // deployed across agents should group together — and stay separate
    // from a same-named hand-written project skill that has neither field.
    const marketplaceCopy: Extension = {
      ...baseExt,
      name: "audit",
      source: { ...baseExt.source, origin: "agent", url: null },
      install_meta: {
        install_type: "marketplace",
        url: "https://github.com/pbakaus/impeccable/audit",
        url_resolved: null,
        branch: null,
        subpath: null,
        revision: null,
        remote_revision: null,
        checked_at: null,
        check_error: null,
      },
    };
    const handWrittenProject: Extension = {
      ...baseExt,
      name: "audit",
      source: { ...baseExt.source, origin: "agent", url: null },
      install_meta: null,
      scope: { type: "project", name: "test", path: "/tmp/test" },
    };
    expect(extensionGroupKey(marketplaceCopy)).toBe(
      "skill\0audit\0pbakaus/impeccable",
    );
    expect(extensionGroupKey(handWrittenProject)).toBe("skill\0audit\0");
    expect(extensionGroupKey(marketplaceCopy)).not.toBe(
      extensionGroupKey(handWrittenProject),
    );
  });

  it("uses pack as a user-driven tiebreaker for unlinked rows", () => {
    // Real-world case: arxiv-search was deployed to 4 agents but only the
    // agent that received the original `hk install` carries install_meta.
    // The other three rows had no source.url, no install_meta, no pack —
    // so they grouped together separately from the codex row. Letting the
    // user type "yorkeccak/scientific-skills" into the pack input on the
    // 3-row group should merge them with the codex row.
    const codexCopy: Extension = {
      ...baseExt,
      name: "arxiv-search",
      source: { ...baseExt.source, origin: "agent", url: null },
      install_meta: {
        install_type: "marketplace",
        url: "https://github.com/yorkeccak/scientific-skills",
        url_resolved: null,
        branch: null,
        subpath: null,
        revision: null,
        remote_revision: null,
        checked_at: null,
        check_error: null,
      },
    };
    const otherCopyAfterUserPack: Extension = {
      ...baseExt,
      name: "arxiv-search",
      source: { ...baseExt.source, origin: "agent", url: null },
      install_meta: null,
      pack: "yorkeccak/scientific-skills",
    };
    expect(extensionGroupKey(codexCopy)).toBe(
      extensionGroupKey(otherCopyAfterUserPack),
    );
  });

  it("merges sourceless same-named skills (documented edge)", () => {
    // Two bare skills (no source URL) with the same name will share a group
    // key. This is a deliberate trade-off: it lets the common case — a
    // marketplace skill that was originally registered without a URL
    // collapsing with a manually-installed copy of the same skill — work
    // without extra plumbing. The cost: two genuinely unrelated project
    // skills that happen to share a name will appear merged.
    //
    // The merged row keeps both Extensions in its `instances` array, and the
    // detail panel surfaces all scopes/paths, so no information is lost —
    // only the listing layout collapses. If this turns into a real UX
    // problem we'd add a per-instance discriminator (e.g. scopeKey) to the
    // empty-developer case; until then this test pins the current semantic.
    const projectFoo: Extension = {
      ...baseExt,
      name: "foo",
      source: { ...baseExt.source, url: null },
      scope: {
        type: "project",
        name: "alpha",
        path: "/Users/me/alpha",
      },
    };
    const otherProjectFoo: Extension = {
      ...projectFoo,
      scope: {
        type: "project",
        name: "beta",
        path: "/Users/me/beta",
      },
    };
    expect(extensionGroupKey(projectFoo)).toBe(
      extensionGroupKey(otherProjectFoo),
    );
  });
});

describe("sortAgentNames", () => {
  it("sorts agents in canonical order", () => {
    const result = sortAgentNames(["windsurf", "cursor", "claude", "gemini"]);
    expect(result).toEqual(["claude", "gemini", "cursor", "windsurf"]);
  });

  it("puts unknown agents at the end", () => {
    const result = sortAgentNames(["unknown-agent", "claude"]);
    expect(result[0]).toBe("claude");
    expect(result[1]).toBe("unknown-agent");
  });
});

describe("agentDisplayName", () => {
  it("returns display name for known agents", () => {
    expect(agentDisplayName("claude")).toBe("Claude Code");
    expect(agentDisplayName("codex")).toBe("Codex");
    expect(agentDisplayName("cursor")).toBe("Cursor");
    expect(agentDisplayName("windsurf")).toBe("Windsurf");
  });

  it("capitalizes first letter for unknown agents", () => {
    expect(agentDisplayName("myagent")).toBe("Myagent");
  });
});

describe("trustTier", () => {
  it("returns Safe for scores >= 80", () => {
    expect(trustTier(80)).toBe("Safe");
    expect(trustTier(100)).toBe("Safe");
  });

  it("returns LowRisk for scores 60-79", () => {
    expect(trustTier(60)).toBe("LowRisk");
    expect(trustTier(79)).toBe("LowRisk");
  });

  it("returns NeedsReview for scores < 60", () => {
    expect(trustTier(59)).toBe("NeedsReview");
    expect(trustTier(0)).toBe("NeedsReview");
  });
});

describe("trustColor", () => {
  it("returns correct CSS class per tier", () => {
    expect(trustColor(90)).toBe("text-trust-safe");
    expect(trustColor(70)).toBe("text-trust-low-risk");
    expect(trustColor(30)).toBe("text-trust-high-risk");
  });
});

describe("severityColor", () => {
  it("maps each severity to a CSS class", () => {
    expect(severityColor("Critical")).toBe("text-trust-critical");
    expect(severityColor("High")).toBe("text-trust-high-risk");
    expect(severityColor("Medium")).toBe("text-trust-low-risk");
    expect(severityColor("Low")).toBe("text-muted-foreground");
  });
});

describe("formatRelativeTime", () => {
  it("returns 'Just now' for very recent timestamps", () => {
    const now = new Date().toISOString();
    expect(formatRelativeTime(now)).toBe("Just now");
  });

  it("returns minutes ago", () => {
    const fiveMinAgo = new Date(Date.now() - 5 * 60_000).toISOString();
    expect(formatRelativeTime(fiveMinAgo)).toBe("5m ago");
  });

  it("returns hours ago", () => {
    const twoHoursAgo = new Date(Date.now() - 2 * 3600_000).toISOString();
    expect(formatRelativeTime(twoHoursAgo)).toBe("2h ago");
  });

  it("returns days ago", () => {
    const threeDaysAgo = new Date(Date.now() - 3 * 86400_000).toISOString();
    expect(formatRelativeTime(threeDaysAgo)).toBe("3d ago");
  });

  it("returns months ago for old dates", () => {
    const ninetyDaysAgo = new Date(Date.now() - 90 * 86400_000).toISOString();
    expect(formatRelativeTime(ninetyDaysAgo)).toBe("3mo ago");
  });
});
