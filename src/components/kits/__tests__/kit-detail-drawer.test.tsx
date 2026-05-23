import { render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useKitStore } from "@/stores/kit-store";
import type { KitDetails } from "@/types/kits";
import { KitDetailDrawer } from "../kit-detail-drawer";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

function makeDetails(over: Partial<KitDetails> = {}): KitDetails {
  return {
    summary: {
      id: "k1",
      name: "test-kit",
      description: "a description",
      extension_count: 2,
      config_file_count: 1,
      sync_count: 1,
      kind_counts: { skill: 1, mcp: 1, plugin: 0, hook: 0, cli: 0 },
      created_at: "2026-05-22T10:00:00Z",
      updated_at: "2026-05-22T10:00:00Z",
      corrupt: false,
      search_keywords: "test-kit a description",
    },
    extensions: [
      {
        extension_id: "e1",
        asset_name: "ext-a",
        kind: "skill",
        content_hash: "h",
        secrets_stripped: false,
      },
    ],
    config_files: [
      {
        agent: "claude",
        category: "rules",
        source_path: "/u/CLAUDE.md",
        source_file_name: "CLAUDE.md",
      },
    ],
    sync_targets: [
      {
        project_path: "/projects/p1",
        agent_name: "claude",
        synced_at: "2026-05-22T09:00:00Z",
        file_count: 3,
        shared_with: [],
      },
    ],
    ...over,
  };
}

describe("KitDetailPanel (rewritten)", () => {
  beforeEach(() => {
    useKitStore.setState({ details: makeDetails() } as never);
  });

  it("renders primary action row with Add to (single primary CTA); Remove sits in secondary row", () => {
    // "New Project with Kit" was merged into Add to Project's install
    // dialog (radio: existing project / new folder), so detail panel now
    // has just one primary CTA.
    render(<KitDetailDrawer kitId="k1" onClose={() => {}} />);
    expect(
      screen.getByRole("button", { name: /actions\.install/ }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /actions\.newProject/ }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /actions\.removeFrom/ }),
    ).toBeInTheDocument();
  });

  it("Remove from is disabled when sync_targets is empty", () => {
    useKitStore.setState({
      details: makeDetails({ sync_targets: [] }),
    } as never);
    render(<KitDetailDrawer kitId="k1" onClose={() => {}} />);
    expect(
      screen.getByRole("button", { name: /actions\.removeFrom/ }),
    ).toBeDisabled();
  });

  it("Installed-In section is read-only (no per-row Uninstall button)", () => {
    render(<KitDetailDrawer kitId="k1" onClose={() => {}} />);
    const installedSection = screen.getByTestId("section-installed-in");
    expect(
      within(installedSection).queryByRole("button", { name: /uninstall/i }),
    ).not.toBeInTheDocument();
    expect(
      within(installedSection).getByText("/projects/p1"),
    ).toBeInTheDocument();
  });

  it("renders kind_counts-aware extension list", () => {
    render(<KitDetailDrawer kitId="k1" onClose={() => {}} />);
    expect(screen.getByText("ext-a")).toBeInTheDocument();
  });

  it("does not render any refresh/update-all UI (Kits are immutable snapshots)", () => {
    render(<KitDetailDrawer kitId="k1" onClose={() => {}} />);
    expect(
      screen.queryByRole("button", { name: /refresh/i }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /update.?all/i }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText(/staleInstalls/)).not.toBeInTheDocument();
  });
});
