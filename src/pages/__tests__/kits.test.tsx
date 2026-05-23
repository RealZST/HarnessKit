import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useKitStore } from "@/stores/kit-store";
import KitsPage from "../kits";

// Mock react-i18next with a small interpolator so plural-style assertions
// (e.g. "1 selected") can land without pulling in the full i18next stack.
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) => {
      if (opts && typeof opts === "object") {
        if ("count" in opts) {
          return `${(opts as { count: number }).count} selected`;
        }
        if ("name" in opts && key === "actions.selectKit") {
          return `Select ${(opts as { name: string }).name}`;
        }
      }
      return key;
    },
  }),
}));

// The page never opens these dialogs in this test suite; stub the dialog
// helpers anyway so any inadvertent invocation can't actually pop a Tauri
// dialog under jsdom.
vi.mock("@/lib/dialog", () => ({
  openFilePicker: vi.fn().mockResolvedValue(null),
  openDirectoryPicker: vi.fn().mockResolvedValue(null),
  saveFilePicker: vi.fn().mockResolvedValue(null),
}));

const ONBOARDING_KEY = "hk:kits-v4:onboarding-toast-shown";

beforeEach(() => {
  localStorage.removeItem(ONBOARDING_KEY);
  useKitStore.setState({
    kits: [
      {
        id: "a",
        name: "alpha",
        description: "",
        extension_count: 1,
        config_file_count: 0,
        sync_count: 0,
        kind_counts: { skill: 1, mcp: 0, plugin: 0, hook: 0, cli: 0 },
        created_at: "2026-01-01T00:00:00Z",
        updated_at: "2026-01-01T00:00:00Z",
        corrupt: false,
      },
      {
        id: "b",
        name: "bravo",
        description: "",
        extension_count: 2,
        config_file_count: 1,
        sync_count: 0,
        kind_counts: { skill: 1, mcp: 1, plugin: 0, hook: 0, cli: 0 },
        created_at: "2026-01-01T00:00:00Z",
        updated_at: "2026-01-01T00:00:00Z",
        corrupt: false,
      },
    ],
    installRecords: [],
    fetchKits: vi.fn().mockResolvedValue(undefined),
    fetchInstallRecords: vi.fn().mockResolvedValue(undefined),
  } as never);
});

// Helper: reveal the per-card checkbox (it's hover-only) by firing mouseEnter
// on its wrapper, then return it. Mirrors the pattern used in
// folder-grid.test.tsx.
async function selectKit(
  user: ReturnType<typeof userEvent.setup>,
  name: string,
) {
  const body = screen.getByRole("button", { name: new RegExp(name) });
  const card = body.parentElement;
  if (!card) throw new Error(`${name} card wrapper not found`);
  fireEvent.mouseEnter(card);
  await user.click(
    screen.getByRole("checkbox", { name: new RegExp(`Select ${name}`) }),
  );
}

describe("KitsPage", () => {
  it("renders FolderGrid with all kits and header entry points", () => {
    render(<KitsPage />);
    expect(screen.getByText("alpha")).toBeInTheDocument();
    expect(screen.getByText("bravo")).toBeInTheDocument();
    // New Kit + Import Kit live in the header, always visible (replaces the
    // old grid-trailing GhostTile pattern).
    expect(
      screen.getByRole("button", { name: /page\.newKit/ }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /exportImport\.import/ }),
    ).toBeInTheDocument();
  });

  it("selecting one kit shows inline action bar; header entry points stay", async () => {
    // The selection toolbar has a single primary CTA (Add to Project) — the
    // old "New Project with these" entry was merged into the install dialog's
    // project-mode radio (existing / new folder). Header New Kit / Import Kit
    // stay visible during select mode (header is the persistent surface,
    // separate from the bottom selection bar).
    const user = userEvent.setup();
    render(<KitsPage />);
    await selectKit(user, "alpha");
    expect(screen.getByText(/1 selected/i)).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /actions\.applySelected/ }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /actions\.newProjectSelected/ }),
    ).not.toBeInTheDocument();
    // Header buttons remain available in select mode.
    expect(
      screen.getByRole("button", { name: /page\.newKit/ }),
    ).toBeInTheDocument();
  });

  it("Cancel hides selection bar; header entry points still present", async () => {
    const user = userEvent.setup();
    render(<KitsPage />);
    await selectKit(user, "alpha");
    await user.click(screen.getByRole("button", { name: /common:cancel/ }));
    expect(
      screen.getByRole("button", { name: /page\.newKit/ }),
    ).toBeInTheDocument();
  });
});
