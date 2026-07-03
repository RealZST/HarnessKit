import { describe, expect, it } from "vitest";
import type { AgentConfigFile } from "@/lib/types";
import { groupMemoryFiles } from "../memory-grouping";

function file(
  path: string,
  size: number,
  scope: AgentConfigFile["scope"],
): AgentConfigFile {
  return {
    path,
    agent: "claude",
    category: "memory",
    scope,
    file_name: path.slice(
      Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\")) + 1,
    ),
    size_bytes: size,
    modified_at: null,
    is_dir: false,
    exists: true,
  };
}
const GLOBAL = { type: "global" } as const;

describe("groupMemoryFiles", () => {
  it("groups by storage directory, sums bytes, sorts files by name", () => {
    const groups = groupMemoryFiles([
      file("/h/.claude/projects/-a/memory/two.md", 50, GLOBAL),
      file("/h/.claude/projects/-a/memory/one.md", 100, GLOBAL),
      file("/h/.claude/projects/-b/memory/three.md", 200, GLOBAL),
    ]);
    expect(groups).toHaveLength(2);
    const a = groups.find(
      (g) => g.storePath === "/h/.claude/projects/-a/memory",
    )!;
    expect(a.files.map((f) => f.file_name)).toEqual(["one.md", "two.md"]);
    expect(a.totalBytes).toBe(150);
    expect(a.projectName).toBeNull();
    expect(a.storePath).toBe("/h/.claude/projects/-a/memory");
  });

  it("puts project groups first with their project name", () => {
    const proj = { type: "project", name: "CS", path: "/real/CS" } as const;
    const groups = groupMemoryFiles([
      file("/h/.claude/projects/-z/memory/g.md", 10, GLOBAL),
      file("/h/.claude/projects/-cs/memory/p.md", 10, proj),
    ]);
    expect(groups[0].projectName).toBe("CS");
    expect(groups[1].projectName).toBeNull();
  });

  it("groups by directory on Windows-style backslash paths", () => {
    const groups = groupMemoryFiles([
      file("C:\\Users\\z\\.claude\\projects\\-a\\memory\\one.md", 100, GLOBAL),
      file("C:\\Users\\z\\.claude\\projects\\-a\\memory\\two.md", 50, GLOBAL),
    ]);
    expect(groups).toHaveLength(1);
    expect(groups[0].storePath).toBe(
      "C:\\Users\\z\\.claude\\projects\\-a\\memory",
    );
    expect(groups[0].files.map((f) => f.file_name)).toEqual([
      "one.md",
      "two.md",
    ]);
  });

  it("returns [] for empty input", () => {
    expect(groupMemoryFiles([])).toEqual([]);
  });
});
