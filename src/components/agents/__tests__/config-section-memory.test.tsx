import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import type { AgentConfigFile } from "@/lib/types";
import { ConfigSection } from "../config-section";

function mem(path: string, scope: AgentConfigFile["scope"]): AgentConfigFile {
  return {
    path,
    agent: "claude",
    category: "memory",
    scope,
    file_name: path.slice(path.lastIndexOf("/") + 1),
    size_bytes: 100,
    modified_at: null,
    is_dir: false,
    exists: true,
  };
}
beforeEach(() => localStorage.clear());

describe("ConfigSection memory grouping", () => {
  it("renders one header per storage dir; project name for project scope, raw path for global", () => {
    render(
      <ConfigSection
        category="memory"
        files={[
          mem("/h/.claude/projects/-cs/memory/a.md", {
            type: "project",
            name: "CS",
            path: "/real/CS",
          }),
          mem("/h/.claude/projects/-dl/memory/b.md", { type: "global" }),
        ]}
        agentName="claude"
      />,
    );
    expect(screen.getByText("CS")).toBeInTheDocument();
    // Project group header now also shows its storage path (not just the name).
    expect(
      screen.getByText("/h/.claude/projects/-cs/memory"),
    ).toBeInTheDocument();
    expect(
      screen.getByText("/h/.claude/projects/-dl/memory"),
    ).toBeInTheDocument();
    // Never decoded into a fake project path.
    expect(screen.queryByText("/h/dl")).not.toBeInTheDocument();
    // Rows hide their per-row scope path (now shown once in the group header):
    // the project file's scope path (/real/CS) must not render on the row.
    expect(screen.queryByText("/real/CS")).not.toBeInTheDocument();
    // Badge now lives on the rows (not the header): each file row shows its scope badge.
    expect(screen.getByText("Project")).toBeInTheDocument();
    expect(screen.getByText("Global")).toBeInTheDocument();
  });
});
