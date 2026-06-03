import { describe, expect, it } from "vitest";
import type { Extension, GroupedExtension } from "../types";
import {
  agentDisplayName,
  extensionGroupKey,
  formatRelativeTime,
  groupOwnerRepo,
  isValidPackFormat,
  normalizePack,
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

  it("falls back to scope key when no URL is available", () => {
    // Truly sourceless extensions (no source.url, no install_meta, no
    // pack) use scopeKey as the developer slot so two unrelated same-
    // named skills in different scopes don't accidentally merge.
    const ext = { ...baseExt, source: { ...baseExt.source, url: null } };
    const key = extensionGroupKey(ext);
    expect(key).toBe("skill\0my-skill\0(global)");
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
    expect(extensionGroupKey(handWrittenProject)).toBe(
      "skill\0audit\0(/tmp/test)",
    );
    expect(extensionGroupKey(marketplaceCopy)).not.toBe(
      extensionGroupKey(handWrittenProject),
    );
  });

  it("prefers install_meta.url over a polluted enclosing-repo source.url", () => {
    // Regression: an agent home (e.g. ~/.claude) is kept inside the user's own
    // dotfiles git repo, so the scanner walks up and stamps that copy with a
    // *wrong* source.url (the enclosing backup repo). The other agents' copies
    // sit in non-git dirs and stay sourceless. All copies were installed from
    // tw93/waza, so they must group into one row. Pre-fix, source.url won and
    // the git-backed copy forked into its own dotfiles-repo group.
    const wazaInstallMeta = {
      install_type: "marketplace",
      url: "tw93/waza",
      url_resolved: null,
      branch: null,
      subpath: null,
      revision: "51222bf",
      remote_revision: null,
      checked_at: null,
      check_error: null,
    };
    const claudeCopyPolluted: Extension = {
      ...baseExt,
      name: "check",
      agents: ["claude"],
      source: {
        ...baseExt.source,
        origin: "git",
        url: "https://github.com/octo-user/dotfiles.git",
      },
      install_meta: wazaInstallMeta,
    };
    const codexCopySourceless: Extension = {
      ...baseExt,
      name: "check",
      agents: ["codex"],
      source: { ...baseExt.source, origin: "agent", url: null },
      install_meta: wazaInstallMeta,
    };
    // The polluted Claude copy now resolves to its true origin…
    expect(extensionGroupKey(claudeCopyPolluted)).toBe(
      "skill\0check\0tw93/waza",
    );
    // …and groups with the sourceless siblings instead of forking off.
    expect(extensionGroupKey(claudeCopyPolluted)).toBe(
      extensionGroupKey(codexCopySourceless),
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

  it("splits sourceless same-named skills across scopes", () => {
    // Concrete reproducer from a real DB: a hand-written
    // `code-review` skill in a project + an unrelated agent-bundled
    // `code-review` skill at copilot's global skill dir. Both have no
    // source.url, no install_meta, no pack — pre-fix they collapsed
    // into a single group row even though they're independent skills.
    // With scopeKey as the sourceless tiebreaker they stay separate.
    const projectCodeReview: Extension = {
      ...baseExt,
      name: "code-review",
      source: { ...baseExt.source, url: null },
      install_meta: null,
      pack: null,
      scope: {
        type: "project",
        name: "hk-scope-test",
        path: "/Users/zoe/Downloads/hk-scope-test",
      },
    };
    const globalCodeReview: Extension = {
      ...projectCodeReview,
      agents: ["copilot"],
      scope: { type: "global" },
    };
    expect(extensionGroupKey(projectCodeReview)).not.toBe(
      extensionGroupKey(globalCodeReview),
    );
  });

  it("splits sourceless same-named skills across different projects", () => {
    // Two unrelated projects each with a hand-written `foo` skill —
    // scopeKey includes the project path so they remain in separate
    // groups (instead of merging just because both lack a URL).
    const fooInAlpha: Extension = {
      ...baseExt,
      name: "foo",
      source: { ...baseExt.source, url: null },
      install_meta: null,
      pack: null,
      scope: { type: "project", name: "alpha", path: "/Users/me/alpha" },
    };
    const fooInBeta: Extension = {
      ...fooInAlpha,
      scope: { type: "project", name: "beta", path: "/Users/me/beta" },
    };
    expect(extensionGroupKey(fooInAlpha)).not.toBe(
      extensionGroupKey(fooInBeta),
    );
  });

  it("merges same-name MCPs across scopes (name is identity)", () => {
    // An MCP server's name IS its config-file key (mcpServers / [mcp_servers]),
    // so the same name across Global + Project always denotes the same
    // logical server. They should land in one group rather than two.
    const globalMcp: Extension = {
      ...baseExt,
      kind: "mcp",
      name: "memory",
      source: { ...baseExt.source, url: null },
      scope: { type: "global" },
    };
    const projectMcp: Extension = {
      ...baseExt,
      kind: "mcp",
      name: "memory",
      source: { ...baseExt.source, url: null },
      scope: { type: "project", name: "x", path: "/Users/me/x" },
    };
    expect(extensionGroupKey(globalMcp)).toBe(extensionGroupKey(projectMcp));
  });

  it("still separates different-name MCPs", () => {
    // Sanity check the MCP merge isn't over-eager — distinct server names
    // remain distinct groups.
    const memory: Extension = {
      ...baseExt,
      kind: "mcp",
      name: "memory",
      source: { ...baseExt.source, url: null },
    };
    const pencil: Extension = {
      ...baseExt,
      kind: "mcp",
      name: "pencil",
      source: { ...baseExt.source, url: null },
    };
    expect(extensionGroupKey(memory)).not.toBe(extensionGroupKey(pencil));
  });
});

describe("sortAgentNames", () => {
  it("sorts agents in canonical order", () => {
    const result = sortAgentNames([
      "opencode",
      "windsurf",
      "cursor",
      "claude",
      "gemini",
    ]);
    expect(result).toEqual([
      "claude",
      "gemini",
      "cursor",
      "windsurf",
      "opencode",
    ]);
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
    expect(agentDisplayName("opencode")).toBe("OpenCode");
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
    expect(formatRelativeTime(now, "en")).toBe("Just now");
  });

  it("returns minutes ago", () => {
    const fiveMinAgo = new Date(Date.now() - 5 * 60_000).toISOString();
    expect(formatRelativeTime(fiveMinAgo, "en")).toBe("5m ago");
  });

  it("returns hours ago", () => {
    const twoHoursAgo = new Date(Date.now() - 2 * 3600_000).toISOString();
    expect(formatRelativeTime(twoHoursAgo, "en")).toBe("2h ago");
  });

  it("returns days ago", () => {
    const threeDaysAgo = new Date(Date.now() - 3 * 86400_000).toISOString();
    expect(formatRelativeTime(threeDaysAgo, "en")).toBe("3d ago");
  });

  it("returns months ago for old dates", () => {
    const ninetyDaysAgo = new Date(Date.now() - 90 * 86400_000).toISOString();
    expect(formatRelativeTime(ninetyDaysAgo, "en")).toBe("3mo ago");
  });

  it("returns Chinese output when locale is zh", () => {
    const fiveMinAgo = new Date(Date.now() - 5 * 60_000).toISOString();
    expect(formatRelativeTime(fiveMinAgo, "zh")).toBe("5分钟前");
    const now = new Date().toISOString();
    expect(formatRelativeTime(now, "zh")).toBe("刚刚");
  });
});

describe("normalizePack", () => {
  // Mirrors `normalize_pack` in service.rs; both halves must accept the same
  // input shapes so the user sees the same result whichever client they hit.
  it("preserves canonical owner/repo input", () => {
    expect(normalizePack("anthropics/skills")).toBe("anthropics/skills");
    expect(normalizePack("  baoyu/foo  ")).toBe("baoyu/foo");
  });

  it("strips GitHub URL scheme, .git suffix, and trailing slash", () => {
    expect(normalizePack("https://github.com/anthropics/skills")).toBe(
      "anthropics/skills",
    );
    expect(normalizePack("http://github.com/anthropics/skills.git")).toBe(
      "anthropics/skills",
    );
    expect(normalizePack("github.com/anthropics/skills/")).toBe(
      "anthropics/skills",
    );
  });

  it("discards extra path segments (tree/main, issues/…)", () => {
    expect(
      normalizePack("https://github.com/anthropics/skills/tree/main"),
    ).toBe("anthropics/skills");
    expect(
      normalizePack("https://github.com/anthropics/skills/issues/42"),
    ).toBe("anthropics/skills");
  });

  it("handles SSH clone URLs", () => {
    expect(normalizePack("git@github.com:anthropics/skills.git")).toBe(
      "anthropics/skills",
    );
    expect(normalizePack("git@github.com:anthropics/skills")).toBe(
      "anthropics/skills",
    );
  });

  it("passes unknown input through (trimmed) for the validator to reject", () => {
    expect(normalizePack("not-a-pack")).toBe("not-a-pack");
    expect(normalizePack("https://example.com/foo/bar")).toBe(
      "https://example.com/foo/bar",
    );
  });
});

describe("isValidPackFormat", () => {
  // Frontend gate before calling updatePack; must accept exactly what the
  // backend `is_valid_pack_format` in service.rs accepts so the synthesized
  // install_meta URL doesn't get rejected server-side.
  it("accepts standard owner/repo", () => {
    expect(isValidPackFormat("anthropics/skills")).toBe(true);
  });

  it("accepts the GitHub character set: alnum / - / _ / .", () => {
    expect(isValidPackFormat("user-name/repo.name")).toBe(true);
    expect(isValidPackFormat("a_b/c.d-e")).toBe(true);
  });

  it("rejects empty / single segment / extra slashes", () => {
    expect(isValidPackFormat("")).toBe(false);
    expect(isValidPackFormat("noslash")).toBe(false);
    expect(isValidPackFormat("a/b/c")).toBe(false);
    expect(isValidPackFormat("/repo")).toBe(false);
    expect(isValidPackFormat("owner/")).toBe(false);
  });

  it("rejects URL-shaped input and whitespace", () => {
    expect(isValidPackFormat("https://github.com/a/b")).toBe(false);
    expect(isValidPackFormat("a b/c")).toBe(false);
    expect(isValidPackFormat("a/b c")).toBe(false);
  });

  it("rejects host-shaped owner so non-github paste forms fail validation", () => {
    // `gitlab.com/foo` would normalize to itself (non-github gate in
    // normalize_pack) and used to slip past the validator because `.` was
    // allowed in the owner half. bind_pack would then synthesize a wrong
    // `https://github.com/gitlab.com/foo.git` URL.
    expect(isValidPackFormat("gitlab.com/foo")).toBe(false);
    expect(isValidPackFormat("google.com/foo")).toBe(false);
    // Repo half still allows '.' (legitimate GitHub repo name pattern).
    expect(isValidPackFormat("user/repo.name")).toBe(true);
  });
});

describe("groupOwnerRepo", () => {
  // Source pill in the detail panel reads through this — has to consult the
  // three places source info can live (pack, group.source.url, any instance's
  // install_meta.url) and return `null` only when truly nothing is known.
  const baseInst: Extension = {
    id: "i1",
    kind: "skill",
    name: "x",
    description: "",
    source: { origin: "agent", url: null, version: null, commit_hash: null },
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

  const baseGroup: GroupedExtension = {
    groupKey: "g",
    name: "x",
    kind: "skill",
    description: "",
    source: { origin: "agent", url: null, version: null, commit_hash: null },
    agents: ["claude"],
    tags: [],
    pack: null,
    permissions: [],
    enabled: true,
    trust_score: null,
    installed_at: "2025-01-01T00:00:00Z",
    updated_at: "2025-01-01T00:00:00Z",
    instances: [baseInst],
  };

  it("returns pack directly when set", () => {
    expect(groupOwnerRepo({ ...baseGroup, pack: "alice/repo" }, [])).toBe(
      "alice/repo",
    );
  });

  it("extracts owner/repo from group.source.url when pack is null", () => {
    const g = {
      ...baseGroup,
      source: {
        ...baseGroup.source,
        url: "https://github.com/bob/tools.git",
      },
    };
    expect(groupOwnerRepo(g, [])).toBe("bob/tools");
  });

  it("falls back to any instance's install_meta.url", () => {
    const inst: Extension = {
      ...baseInst,
      install_meta: {
        install_type: "manual",
        url: "https://github.com/carol/widgets.git",
        url_resolved: null,
        branch: null,
        subpath: null,
        revision: null,
        remote_revision: null,
        checked_at: null,
        check_error: null,
      },
    };
    expect(groupOwnerRepo({ ...baseGroup, instances: [inst] }, [])).toBe(
      "carol/widgets",
    );
  });

  it("walks CLI child extensions for install_meta when parent has none", () => {
    // CLI parent rows usually carry no install_meta of their own — the URL
    // lives on the child skill that was installed via marketplace.
    const cliParent: Extension = { ...baseInst, id: "cli-1", kind: "cli" };
    const cliGroup: GroupedExtension = {
      ...baseGroup,
      kind: "cli",
      instances: [cliParent],
    };
    const childSkill: Extension = {
      ...baseInst,
      id: "child-1",
      kind: "skill",
      cli_parent_id: "cli-1",
      install_meta: {
        install_type: "marketplace",
        url: "https://github.com/dave/cli-tool.git",
        url_resolved: null,
        branch: null,
        subpath: null,
        revision: null,
        remote_revision: null,
        checked_at: null,
        check_error: null,
      },
    };
    expect(groupOwnerRepo(cliGroup, [cliParent, childSkill])).toBe(
      "dave/cli-tool",
    );
  });

  it("returns null when source info is missing everywhere", () => {
    expect(groupOwnerRepo(baseGroup, [])).toBeNull();
  });
});
