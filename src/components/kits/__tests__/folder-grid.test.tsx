import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { KitSummary } from "@/types/kits";
import { FolderGrid } from "../folder-grid";

const makeKit = (
  id: string,
  name: string,
  overrides: Partial<KitSummary> = {},
): KitSummary => ({
  id,
  name,
  description: "",
  extension_count: 5,
  config_file_count: 1,
  sync_count: 0,
  kind_counts: { skill: 3, mcp: 1, plugin: 0, hook: 0, cli: 1 },
  created_at: "2026-01-01T00:00:00Z",
  updated_at: "2026-01-01T00:00:00Z",
  corrupt: false,
  search_keywords: name.toLowerCase(),
  ...overrides,
});

describe("FolderGrid", () => {
  it("renders one FolderCard per kit", () => {
    render(
      <FolderGrid
        kits={[makeKit("a", "alpha"), makeKit("b", "bravo")]}
        activeKitId={null}
        selectedKitIds={[]}
        onOpenDetail={vi.fn()}
        onSelectionChange={vi.fn()}
      />,
    );
    expect(screen.getByText("alpha")).toBeInTheDocument();
    expect(screen.getByText("bravo")).toBeInTheDocument();
  });

  it("clicking a folder body emits onOpenDetail with its id", async () => {
    const onOpenDetail = vi.fn();
    const user = userEvent.setup();
    render(
      <FolderGrid
        kits={[makeKit("a", "alpha")]}
        activeKitId={null}
        selectedKitIds={[]}
        onOpenDetail={onOpenDetail}
        onSelectionChange={vi.fn()}
      />,
    );
    await user.click(screen.getByRole("button", { name: /alpha/ }));
    expect(onOpenDetail).toHaveBeenCalledWith("a");
  });

  it("clicking a checkbox emits onSelectionChange and does NOT open detail", async () => {
    const onOpenDetail = vi.fn();
    const onSelectionChange = vi.fn();
    const user = userEvent.setup();
    render(
      <FolderGrid
        kits={[makeKit("a", "alpha")]}
        activeKitId={null}
        selectedKitIds={["a"]}
        onOpenDetail={onOpenDetail}
        onSelectionChange={onSelectionChange}
      />,
    );
    await user.click(screen.getByRole("checkbox", { name: /Select alpha/ }));
    expect(onSelectionChange).toHaveBeenCalledWith([]);
    expect(onOpenDetail).not.toHaveBeenCalled();
  });

  it("toggling unselected kit adds it to selectedKitIds", async () => {
    const onSelectionChange = vi.fn();
    const user = userEvent.setup();
    render(
      <FolderGrid
        kits={[makeKit("a", "alpha"), makeKit("b", "bravo")]}
        activeKitId={null}
        selectedKitIds={["a"]}
        onOpenDetail={vi.fn()}
        onSelectionChange={onSelectionChange}
      />,
    );
    // bravo isn't selected, so its checkbox is hover-only; fire mouseEnter
    // directly on the wrapper to reveal it (React's onMouseEnter only fires
    // on the bound element, not via child hover under jsdom).
    const bravoBody = screen.getByRole("button", { name: /bravo/ });
    const bravoCard = bravoBody.parentElement;
    if (!bravoCard) throw new Error("bravo card wrapper not found");
    fireEvent.mouseEnter(bravoCard);
    await user.click(screen.getByRole("checkbox", { name: /Select bravo/ }));
    expect(onSelectionChange).toHaveBeenCalledWith(["a", "b"]);
  });
});
