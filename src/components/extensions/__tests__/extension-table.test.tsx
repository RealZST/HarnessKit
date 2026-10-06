import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import type { GroupedExtension } from "@/lib/types";
import { ExtensionTable } from "../extension-table";

function group(
  over: Partial<GroupedExtension> & Pick<GroupedExtension, "name" | "kind">,
): GroupedExtension {
  return {
    groupKey: over.groupKey ?? over.name,
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
    expect(screen.getByRole("table")).toHaveClass("table-fixed");
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
