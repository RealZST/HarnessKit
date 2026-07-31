import { describe, expect, it } from "vitest";
import {
  canInstallAtScope,
  canReceiveMcpTransport,
} from "@/lib/agent-capabilities";
import type { AgentCapabilities, AgentInfo } from "@/lib/types";
import type { ScopeValue } from "@/stores/scope-store";

const GLOBAL: ScopeValue = { type: "global" };
const ALL: ScopeValue = { type: "all" };
const PROJECT: ScopeValue = {
  type: "project",
  name: "demo",
  path: "/tmp/demo",
};

function agent(name: string, capabilities: AgentCapabilities): AgentInfo {
  return {
    name,
    detected: true,
    extension_count: 0,
    path: `/home/user/.${name}`,
    enabled: true,
    capabilities,
  };
}

// Shapes mirror AgentCapabilities::from_adapter output (backend matrix
// test: test_agent_capabilities_matrix in crates/hk-core/src/adapter/mod.rs).
const CLAUDE = agent("claude", {
  project_install: { skill: true, mcp: true, hook: true, cli: true },
  hooks_supported: true,
  global_hook_install: true,
});
const HERMES = agent("hermes", {
  project_install: { skill: false, mcp: false, hook: false, cli: false },
  hooks_supported: true,
  global_hook_install: true,
});
const WINDSURF = agent("windsurf", {
  project_install: { skill: true, mcp: false, hook: true, cli: true },
  hooks_supported: true,
  global_hook_install: true,
});

describe("canInstallAtScope", () => {
  it("returns true for any agent/kind at global and all scopes", () => {
    expect(canInstallAtScope(HERMES, "skill", GLOBAL)).toBe(true);
    expect(canInstallAtScope(CLAUDE, "mcp", GLOBAL)).toBe(true);
    expect(canInstallAtScope(HERMES, "skill", ALL)).toBe(true);
    // Even an unknown agent is unrestricted outside project scope.
    expect(canInstallAtScope(undefined, "skill", GLOBAL)).toBe(true);
  });

  it("reads the per-kind project flags at project scope", () => {
    expect(canInstallAtScope(CLAUDE, "skill", PROJECT)).toBe(true);
    expect(canInstallAtScope(CLAUDE, "hook", PROJECT)).toBe(true);
    expect(canInstallAtScope(WINDSURF, "skill", PROJECT)).toBe(true);
    // Windsurf MCP is global-only upstream.
    expect(canInstallAtScope(WINDSURF, "mcp", PROJECT)).toBe(false);
  });

  it("returns false at project scope for Hermes (global-only, hermes-agent#4667)", () => {
    expect(canInstallAtScope(HERMES, "skill", PROJECT)).toBe(false);
    expect(canInstallAtScope(HERMES, "cli", PROJECT)).toBe(false);
  });

  it("returns false at project scope when the agent is unknown or unloaded", () => {
    expect(canInstallAtScope(undefined, "skill", PROJECT)).toBe(false);
  });

  it("returns false at project scope for kinds without a project flag", () => {
    expect(canInstallAtScope(CLAUDE, "plugin", PROJECT)).toBe(false);
  });
});

describe("canReceiveMcpTransport", () => {
  const codex = agent("codex", {
    project_install: { skill: true, mcp: true, hook: true, cli: true },
    hooks_supported: true,
    global_hook_install: true,
    mcp_remote: { http: true, sse: false },
  });
  const claudeRemote = agent("claude", {
    ...CLAUDE.capabilities,
    mcp_remote: { http: true, sse: true },
  });

  it("always allows stdio (including absent transport on legacy rows)", () => {
    expect(canReceiveMcpTransport(codex, "stdio")).toBe(true);
    expect(canReceiveMcpTransport(codex, undefined)).toBe(true);
    expect(canReceiveMcpTransport(undefined, undefined)).toBe(true);
  });

  it("gates remote transports by the backend-derived flags", () => {
    expect(canReceiveMcpTransport(claudeRemote, "http")).toBe(true);
    expect(canReceiveMcpTransport(claudeRemote, "sse")).toBe(true);
    // Codex speaks Streamable HTTP only.
    expect(canReceiveMcpTransport(codex, "http")).toBe(true);
    expect(canReceiveMcpTransport(codex, "sse")).toBe(false);
  });

  it("gates remote transports off when capabilities are absent (old backend / unknown agent)", () => {
    expect(canReceiveMcpTransport(CLAUDE, "http")).toBe(false);
    expect(canReceiveMcpTransport(undefined, "http")).toBe(false);
  });
});
