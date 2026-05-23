import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { Extension } from "@/lib/types";
import { EditorAssetTab } from "../editor-asset-tab";

const mkExt = (
  id: string,
  name: string,
  kind: "skill" | "mcp" | "cli",
  agents = ["claude"],
  pack: string | null = null,
  description = "",
): Extension =>
  ({
    id,
    kind,
    name,
    description,
    source: { origin: "registry", url: null, version: null, commit_hash: null },
    agents,
    tags: [],
    pack,
    permissions: [],
    enabled: true,
    trust_score: null,
    installed_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
    source_path: null,
    cli_parent_id: null,
    cli_meta: null,
    install_meta: null,
    scope: { type: "global" },
  }) as unknown as Extension;

describe("EditorAssetTab", () => {
  it("filters to the requested kind", () => {
    const cands = [
      mkExt("e1", "ace-skill", "skill"),
      mkExt("e2", "ace-mcp", "mcp"),
      mkExt("e3", "ace-cli", "cli"),
    ];
    render(
      <EditorAssetTab
        kindFilter="skill"
        selectedIds={[]}
        onSelectionChange={vi.fn()}
        candidates={cands}
      />,
    );
    const list = screen.getByRole("list");
    expect(within(list).getByText("ace-skill")).toBeInTheDocument();
    expect(within(list).queryByText("ace-mcp")).not.toBeInTheDocument();
    expect(within(list).queryByText("ace-cli")).not.toBeInTheDocument();
  });

  it("search matches name, description, and kind", () => {
    const cands = [
      mkExt("e1", "frontend", "skill", ["claude"], null, "design tool"),
      mkExt("e2", "backend", "skill", ["claude"], null, "no match here"),
    ];
    render(
      <EditorAssetTab
        kindFilter="skill"
        selectedIds={[]}
        onSelectionChange={vi.fn()}
        candidates={cands}
      />,
    );
    const search = screen.getByPlaceholderText(/search/i) as HTMLInputElement;
    fireEvent.change(search, { target: { value: "design" } });
    const list = screen.getByRole("list");
    expect(within(list).getByText("frontend")).toBeInTheDocument();
    expect(within(list).queryByText("backend")).not.toBeInTheDocument();
  });

  it("clicking a row adds the id; clicking again removes", () => {
    const cands = [mkExt("e1", "ace", "skill")];
    const onChange = vi.fn();
    const { rerender } = render(
      <EditorAssetTab
        kindFilter="skill"
        selectedIds={[]}
        onSelectionChange={onChange}
        candidates={cands}
      />,
    );
    // Initially nothing selected — list shows "ace", chip rail is empty.
    const list = screen.getByRole("list");
    fireEvent.click(within(list).getByText("ace"));
    expect(onChange).toHaveBeenLastCalledWith(["e1"]);

    rerender(
      <EditorAssetTab
        kindFilter="skill"
        selectedIds={["e1"]}
        onSelectionChange={onChange}
        candidates={cands}
      />,
    );
    // Now "ace" appears in BOTH list and chip rail. Click the list row to deselect.
    const listAfter = screen.getByRole("list");
    fireEvent.click(within(listAfter).getByText("ace"));
    expect(onChange).toHaveBeenLastCalledWith([]);
  });

  it("selected row is marked role=checkbox aria-checked=true", () => {
    // Tab no longer owns a chip rail — that's hoisted to KitEditorDialog. The
    // tab only renders the row list; selection state is communicated via
    // aria-checked on each row's button.
    const cands = [mkExt("e1", "ace", "skill"), mkExt("e2", "bob", "skill")];
    render(
      <EditorAssetTab
        kindFilter="skill"
        selectedIds={["e1"]}
        onSelectionChange={vi.fn()}
        candidates={cands}
      />,
    );
    const list = screen.getByRole("list");
    expect(within(list).getByText("ace")).toBeInTheDocument();
    expect(within(list).getByText("bob")).toBeInTheDocument();
    const aceRow = within(list).getByText("ace").closest("button");
    const bobRow = within(list).getByText("bob").closest("button");
    expect(aceRow).toHaveAttribute("role", "checkbox");
    expect(aceRow).toHaveAttribute("aria-checked", "true");
    expect(bobRow).toHaveAttribute("aria-checked", "false");
  });
});
