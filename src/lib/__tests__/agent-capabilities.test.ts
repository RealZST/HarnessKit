import { describe, expect, it } from "vitest";
import { canInstallAtScope } from "@/lib/agent-capabilities";
import type { ScopeValue } from "@/stores/scope-store";

const GLOBAL: ScopeValue = { type: "global" };
const PROJECT: ScopeValue = {
  type: "project",
  name: "demo",
  path: "/tmp/demo",
};

describe("canInstallAtScope", () => {
  it("returns true for any agent/kind at global scope", () => {
    expect(canInstallAtScope("hermes", "skill", GLOBAL)).toBe(true);
    expect(canInstallAtScope("claude", "mcp", GLOBAL)).toBe(true);
    // Even an unknown agent is unrestricted outside project scope.
    expect(canInstallAtScope("totally-unknown", "skill", GLOBAL)).toBe(true);
  });

  it("returns true at project scope for an agent that supports project skills", () => {
    expect(canInstallAtScope("claude", "skill", PROJECT)).toBe(true);
  });

  it("returns false at project scope for Hermes (global-only, hermes-agent#4667)", () => {
    expect(canInstallAtScope("hermes", "skill", PROJECT)).toBe(false);
  });

  it("returns false at project scope for an unknown agent", () => {
    expect(canInstallAtScope("totally-unknown", "skill", PROJECT)).toBe(false);
  });
});
