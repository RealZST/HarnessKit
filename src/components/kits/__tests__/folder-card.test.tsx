import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { FolderCard } from "../folder-card";

const baseProps = {
  name: "my-kit",
  kindCounts: { skill: 8, mcp: 3, plugin: 0, hook: 0, cli: 0 },
  configCount: 1,
  selected: false,
  panelOpen: false,
  onOpenDetail: vi.fn(),
  onToggleSelect: vi.fn(),
};

describe("FolderCard", () => {
  it("renders name and per-kind pills on the folder body", () => {
    render(<FolderCard {...baseProps} />);
    expect(screen.getByText("my-kit")).toBeInTheDocument();
    // baseProps: 8 skill + 3 mcp + 1 config (cli=0). One pill per non-zero kind.
    expect(screen.getByText("SKILL")).toBeInTheDocument();
    expect(screen.getByText("8")).toBeInTheDocument();
    expect(screen.getByText("MCP")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
    expect(screen.getByText("FILE")).toBeInTheDocument();
    expect(screen.queryByText("CLI")).not.toBeInTheDocument();
  });

  it("renders one paper per non-zero kind plus grey for configs", () => {
    const { container } = render(<FolderCard {...baseProps} />);
    // 2 colored (skill + mcp) + 1 grey config = 3 papers
    expect(container.querySelectorAll("[data-paper]").length).toBe(3);
  });

  it("renders zero papers when kit is empty", () => {
    const empty = {
      ...baseProps,
      kindCounts: { skill: 0, mcp: 0, plugin: 0, hook: 0, cli: 0 },
      configCount: 0,
    };
    const { container } = render(<FolderCard {...empty} />);
    expect(container.querySelectorAll("[data-paper]").length).toBe(0);
  });

  it("shows checkbox only when selected (hover state not testable here)", () => {
    const { rerender } = render(<FolderCard {...baseProps} />);
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
    rerender(<FolderCard {...baseProps} selected={true} />);
    expect(screen.getByRole("checkbox")).toBeChecked();
  });

  it("calls onOpenDetail when folder body is clicked", async () => {
    const onOpenDetail = vi.fn();
    const user = (await import("@testing-library/user-event")).default.setup();
    render(<FolderCard {...baseProps} onOpenDetail={onOpenDetail} />);
    await user.click(screen.getByRole("button", { name: /my-kit/ }));
    expect(onOpenDetail).toHaveBeenCalledTimes(1);
  });

  it("calls onToggleSelect when checkbox is clicked without opening detail", async () => {
    const onOpenDetail = vi.fn();
    const onToggleSelect = vi.fn();
    const user = (await import("@testing-library/user-event")).default.setup();
    render(
      <FolderCard
        {...baseProps}
        selected={true}
        onOpenDetail={onOpenDetail}
        onToggleSelect={onToggleSelect}
      />,
    );
    await user.click(screen.getByRole("checkbox"));
    expect(onToggleSelect).toHaveBeenCalledTimes(1);
    expect(onOpenDetail).not.toHaveBeenCalled();
  });
});
