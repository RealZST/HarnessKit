import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useKitStore } from "@/stores/kit-store";
import { KitEditorDialog } from "../kit-editor-dialog";

vi.mock("@/lib/invoke", () => ({
  api: {
    listKitAssetCandidates: vi
      .fn()
      .mockResolvedValue({ extensions: [], config_files: [] }),
  },
}));

describe("KitEditorDialog shell", () => {
  beforeEach(() => {
    useKitStore.setState({
      ...useKitStore.getState(),
      candidates: { extensions: [], config_files: [] },
    });
  });

  it("renders name + description + 3 tabs (no CLI)", () => {
    // Note: this file does NOT mock react-i18next, so t() returns real
    // translation values from the configured locale (defaults to English).
    render(<KitEditorDialog onClose={() => {}} />);
    expect(screen.getByText("Name")).toBeInTheDocument();
    expect(screen.getByText("Description")).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /skills/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /mcp/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /files/i })).toBeInTheDocument();
    // Kits intentionally exclude CLI / hook / plugin
    expect(
      screen.queryByRole("tab", { name: /^cli$/i }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("tab", { name: /^hook$/i }),
    ).not.toBeInTheDocument();
  });

  it("description is a single-line input matching name's tag (no textarea)", () => {
    const { container } = render(<KitEditorDialog onClose={() => {}} />);
    // Description was made single-line (was textarea rows=2 in v1) to match
    // the name field's height. No textarea should be rendered anywhere in
    // the dialog.
    expect(container.querySelector("textarea")).toBeNull();
  });
});
