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

  it("produces a stable key from kind, name, origin, and developer", () => {
    const key = extensionGroupKey(baseExt);
    expect(key).toBe("skill\0my-skill\0git\0alice/repo");
  });

  it("strips .git suffix from GitHub URLs", () => {
    const key = extensionGroupKey(baseExt);
    expect(key).not.toContain(".git");
  });

  it("handles null source URL", () => {
    const ext = { ...baseExt, source: { ...baseExt.source, url: null } };
    const key = extensionGroupKey(ext);
    expect(key).toBe("skill\0my-skill\0git\0");
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
