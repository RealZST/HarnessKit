import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it } from "vitest";
import type { GroupedExtension } from "@/lib/types";
import { useExtensionStore } from "@/stores/extension-store";
import { useScopeStore } from "@/stores/scope-store";
import { ExtensionTable } from "../extension-table";

// The stores are module-global; put sorting, selection and scope back to
// their defaults so no test depends on the one before it.
beforeEach(() => {
  useExtensionStore.setState(useExtensionStore.getInitialState(), true);
  useScopeStore.setState({ current: { type: "global" } });
});

function group(
  over: Partial<GroupedExtension> & Pick<GroupedExtension, "name" | "kind">,
): GroupedExtension {
  return {
    groupKey: over.groupKey ?? over.name,
    scope: { type: "global" },
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
    instances: [],
    ...over,
  };
}

function renderTable(data: GroupedExtension[]) {
  return render(
    <MemoryRouter>
      <ExtensionTable data={data} />
    </MemoryRouter>,
  );
}

describe("ExtensionTable scope labels", () => {
  const rows = [
    group({ kind: "skill", name: "lint", groupKey: "lint-global" }),
    group({
      kind: "skill",
      name: "lint",
      groupKey: "lint-demo",
      scope: { type: "project", name: "demo", path: "/tmp/demo" },
    }),
  ];

  it("tags project rows with the project when every scope is listed", () => {
    useScopeStore.setState({ current: { type: "all" } });
    renderTable(rows);

    expect(screen.getByTitle("/tmp/demo")).toHaveTextContent("demo");
    expect(screen.getByLabelText("Toggle lint (demo)")).toBeInTheDocument();
    expect(screen.getByLabelText("Toggle lint")).toBeInTheDocument();
  });

  it("leaves the tag out when a single scope is listed", () => {
    renderTable(rows);

    expect(screen.queryByTitle("/tmp/demo")).not.toBeInTheDocument();
  });
});

describe("ExtensionTable name overflow", () => {
  it("exposes the raw plugin name as title and accessible name", () => {
    renderTable([group({ kind: "plugin", name: "cursor" })]);
    const cell = screen.getByLabelText("cursor");
    expect(cell).toHaveAttribute("aria-label", "cursor");
    expect(screen.getByTitle("cursor")).toHaveTextContent("cursor");
  });

  it("shows a shortened hook label but keeps the raw name for AT", () => {
    const raw = "Stop:*:/usr/bin/afplay /System/Library/Sounds/Glass.aiff";
    renderTable([group({ kind: "hook", name: raw, groupKey: "hook-afplay" })]);
    const cell = screen.getByLabelText(raw);
    expect(cell).toHaveAttribute("aria-label", raw);
    expect(screen.getByTitle(raw)).toHaveTextContent("afplay Glass.aiff");
  });

  it("keeps a blob name on title/a11y and truncates the cell", () => {
    const raw = `gBDXELA${"A".repeat(200)}`;
    renderTable([group({ kind: "hook", name: raw, groupKey: "hook-blob" })]);
    const cell = screen.getByLabelText(raw);
    expect(cell).toHaveAttribute("aria-label", raw);
    expect(screen.getByTitle(raw).className).toMatch(/truncate/);
    // Widths follow content: a fixed layout or pinned Name width would
    // squeeze Agent or leave short names in an oversized column.
    expect(screen.getByRole("table")).not.toHaveClass("table-fixed");
    expect(
      screen.getByRole("columnheader", { name: /name/i }).style.width,
    ).toBe("");
  });

  it("sorts by raw name, not the display label", async () => {
    const user = userEvent.setup();
    renderTable([
      group({ kind: "plugin", name: "zeta-ext", groupKey: "z" }),
      group({ kind: "plugin", name: "alpha-ext", groupKey: "a" }),
    ]);
    const table = screen.getByRole("table");
    const firstBodyRow = () => table.querySelector("tbody tr");
    // Store default is name ascending.
    expect(firstBodyRow()?.textContent).toContain("alpha-ext");
    await user.click(screen.getByRole("columnheader", { name: /name/i }));
    expect(firstBodyRow()?.textContent).toContain("zeta-ext");
  });
});

describe("ExtensionTable shift+click range select", () => {
  // Row checkboxes only; the header's "Select all" is excluded.
  const rowCheckboxes = () =>
    screen.getAllByRole("checkbox").filter((cb) => {
      const label = cb.getAttribute("aria-label") ?? "";
      return label.startsWith("Select ") && !label.startsWith("Select all");
    });

  it("shift+click selects all rows between anchor and target", async () => {
    const user = userEvent.setup();
    renderTable([
      group({ kind: "skill", name: "aaa", groupKey: "a" }),
      group({ kind: "skill", name: "bbb", groupKey: "b" }),
      group({ kind: "skill", name: "ccc", groupKey: "c" }),
      group({ kind: "skill", name: "ddd", groupKey: "d" }),
    ]);
    const checkboxes = rowCheckboxes();
    expect(checkboxes).toHaveLength(4);

    // Normal click on first row sets the anchor.
    await user.click(checkboxes[0]);
    expect(checkboxes[0]).toBeChecked();
    expect(checkboxes[1]).not.toBeChecked();

    // Shift+click on third row selects rows 0–2.
    fireEvent.click(checkboxes[2], { shiftKey: true });

    expect(checkboxes[0]).toBeChecked();
    expect(checkboxes[1]).toBeChecked();
    expect(checkboxes[2]).toBeChecked();
    expect(checkboxes[3]).not.toBeChecked();
  });

  it("shift+click follows the sorted order on screen, not the data order", async () => {
    const user = userEvent.setup();
    renderTable([
      group({ kind: "skill", name: "aaa" }),
      group({ kind: "mcp", name: "bbb" }),
      group({ kind: "skill", name: "ccc" }),
      group({ kind: "mcp", name: "ddd" }),
    ]);
    // Sort by Kind: the two MCP rows now sit next to each other.
    await user.click(screen.getByText("Kind"));
    const checkboxes = rowCheckboxes();
    expect(checkboxes.map((cb) => cb.getAttribute("aria-label"))).toEqual([
      "Select bbb",
      "Select ddd",
      "Select aaa",
      "Select ccc",
    ]);

    // bbb to aaa on screen spans ddd; in data order it would span nothing
    // and bbb..ddd would have pulled in ccc.
    await user.click(checkboxes[0]);
    fireEvent.click(checkboxes[2], { shiftKey: true });

    expect(useExtensionStore.getState().selectedIds).toEqual(
      new Set(["bbb", "ddd", "aaa"]),
    );
  });
});
