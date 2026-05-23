import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { AgentConfigFile } from "@/lib/types";
import { EditorConfigsTab } from "../editor-configs-tab";

const mkCfg = (
  agent: string,
  category: string,
  path: string,
): AgentConfigFile => {
  const file_name = path.split("/").pop() ?? "";
  return {
    agent,
    category: category as never,
    path,
    file_name,
    scope: { type: "global" } as never,
    size_bytes: 0,
    modified_at: null,
    is_dir: false,
    exists: true,
  } as unknown as AgentConfigFile;
};

describe("EditorConfigsTab", () => {
  it("renders flat list with agent + category badges + filename + path", () => {
    const cands = [
      mkCfg("claude", "rules", "/u/CLAUDE.md"),
      mkCfg("cursor", "rules", "/u/.cursorrules"),
      mkCfg("claude", "memory", "/u/MEMORY.md"),
    ];
    render(
      <EditorConfigsTab
        selected={[]}
        onSelectionChange={vi.fn()}
        candidates={cands}
      />,
    );
    const list = screen.getByRole("list");
    expect(within(list).getByText("CLAUDE.md")).toBeInTheDocument();
    expect(within(list).getByText(".cursorrules")).toBeInTheDocument();
    expect(within(list).getByText("MEMORY.md")).toBeInTheDocument();
    // Agent is rendered as an icon (AgentMascot) with display name in
    // aria-label/title — saves horizontal space vs a text badge.
    expect(within(list).getAllByLabelText("Claude Code")).toHaveLength(2);
    expect(within(list).getByLabelText("Cursor")).toBeInTheDocument();
    // Category badge stays as visible lowercase text.
    expect(within(list).getAllByText("rules")).toHaveLength(2);
    expect(within(list).getByText("memory")).toBeInTheDocument();
  });

  it("toggles selection on click", () => {
    const cands = [mkCfg("claude", "rules", "/u/CLAUDE.md")];
    const onChange = vi.fn();
    render(
      <EditorConfigsTab
        selected={[]}
        onSelectionChange={onChange}
        candidates={cands}
      />,
    );
    fireEvent.click(screen.getByText("CLAUDE.md"));
    expect(onChange).toHaveBeenCalledWith([
      expect.objectContaining({ agent: "claude", category: "rules" }),
    ]);
  });

  it("search matches agent + category + file name", () => {
    const cands = [
      mkCfg("claude", "rules", "/u/CLAUDE.md"),
      mkCfg("cursor", "memory", "/u/.cursor/notes.md"),
    ];
    render(
      <EditorConfigsTab
        selected={[]}
        onSelectionChange={vi.fn()}
        candidates={cands}
      />,
    );
    const search = screen.getByPlaceholderText(/search/i) as HTMLInputElement;
    fireEvent.change(search, { target: { value: "cursor" } });
    const list = screen.getByRole("list");
    expect(within(list).queryByText("CLAUDE.md")).not.toBeInTheDocument();
    expect(within(list).getByText("notes.md")).toBeInTheDocument();
  });

  it("selected row is marked role=checkbox aria-checked=true (per-file dedup uses source_path)", () => {
    // Tab no longer owns a chip rail — that's hoisted to KitEditorDialog.
    // Also: dedup key includes source_path, so two files under the same
    // (agent, category) — e.g. two CLAUDE.md from different paths — are
    // independently selectable.
    const cands = [
      mkCfg("claude", "rules", "/u/CLAUDE.md"),
      mkCfg("claude", "rules", "/v/CLAUDE.md"),
    ];
    render(
      <EditorConfigsTab
        selected={[
          {
            agent: "claude",
            category: "rules" as never,
            source_path: "/u/CLAUDE.md",
            source_file_name: "CLAUDE.md",
          },
        ]}
        onSelectionChange={vi.fn()}
        candidates={cands}
      />,
    );
    const list = screen.getByRole("list");
    const rows = within(list).getAllByRole("checkbox");
    expect(rows).toHaveLength(2);
    // Only the row whose path matches /u/CLAUDE.md is selected — the other
    // CLAUDE.md from /v stays unselected (confirms per-file dedup).
    const checkedRow = rows.find(
      (r) => r.getAttribute("aria-checked") === "true",
    );
    expect(checkedRow).toBeDefined();
    expect(checkedRow).toHaveTextContent("/u/CLAUDE.md");
  });
});
