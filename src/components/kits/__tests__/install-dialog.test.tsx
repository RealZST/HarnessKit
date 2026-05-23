import { fireEvent, render } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  type Mock,
  vi,
} from "vitest";
import i18n from "@/lib/i18n";
import { useKitStore } from "@/stores/kit-store";
import { toast } from "@/stores/toast-store";
import { InstallDialog } from "../install-dialog";

vi.mock("@/stores/toast-store", () => ({
  toast: {
    error: vi.fn(),
    success: vi.fn(),
    warning: vi.fn(),
    info: vi.fn(),
  },
}));

vi.mock("@/lib/invoke", () => ({
  api: {
    listProjects: vi.fn().mockResolvedValue([]),
    listAgents: vi.fn().mockResolvedValue([
      { name: "claude", display_name: "Claude" },
      { name: "cursor", display_name: "Cursor" },
    ]),
    listKits: vi.fn().mockResolvedValue([]),
    previewKitProjectConflicts: vi
      .fn()
      .mockResolvedValue({ extension_conflicts: [], config_conflicts: [] }),
    syncKitToProject: vi.fn().mockResolvedValue({
      installed_count: 0,
      skipped_conflict_count: 0,
      skipped_paths: [],
      written_paths: [],
    }),
    listProjectInstallRecords: vi.fn().mockResolvedValue([]),
  },
}));

function renderWithI18n(ui: React.ReactElement) {
  return render(<I18nextProvider i18n={i18n}>{ui}</I18nextProvider>);
}

const setStore = (
  overrides: Partial<ReturnType<typeof useKitStore.getState>> = {},
) => {
  useKitStore.setState({
    ...useKitStore.getState(),
    kits: [
      {
        id: "k1",
        name: "Frontend",
      } as never,
    ],
    installRecords: [],
    previewConflicts: vi.fn().mockResolvedValue({
      extension_conflicts: [],
      config_conflicts: [],
    }) as never,
    syncKit: vi.fn().mockResolvedValue({
      installed_count: 0,
      skipped_conflict_count: 0,
      skipped_paths: [],
      written_paths: [],
    }) as never,
    ...overrides,
  });
};

describe("InstallDialog v2 entry-point paths", () => {
  beforeEach(() => {
    setStore();
  });

  it("with pre-filled kit + project: lands on combined Configure step (project + agent picker)", () => {
    const { getByText } = renderWithI18n(
      <InstallDialog
        preFilledKitIds={["k1"]}
        preFilledProjectPath="/Users/me/myapp"
        onClose={() => {}}
      />,
    );
    expect(getByText(/Add to Project/i)).toBeTruthy();
  });

  it("with pre-filled kit only: lands on Configure step (project picker + agents combined)", () => {
    const { getByText } = renderWithI18n(
      <InstallDialog preFilledKitIds={["k1"]} onClose={() => {}} />,
    );
    expect(getByText(/Add to Project/i)).toBeTruthy();
  });

  it("with nothing pre-filled: first step is Kit pick (then advance to Configure)", () => {
    const { getByText } = renderWithI18n(<InstallDialog onClose={() => {}} />);
    expect(getByText(/Pick Kit/i)).toBeTruthy();
  });

  it("with pre-filled project only: first step is Kit pick", () => {
    const { getByText } = renderWithI18n(
      <InstallDialog
        preFilledProjectPath="/Users/me/myapp"
        onClose={() => {}}
      />,
    );
    expect(getByText(/Pick Kit/i)).toBeTruthy();
  });

  it("forceOverwriteMode=true skips preview entirely, jumps to install", async () => {
    const previewMock = vi.fn().mockResolvedValue({
      extension_conflicts: [
        {
          extension_id: "e1",
          asset_name: "skill-a",
          target_path: "/a",
          conflict_reason: "Exists",
        },
      ],
      config_conflicts: [],
    });
    const syncMock = vi.fn().mockResolvedValue({
      installed_count: 1,
      skipped_conflict_count: 0,
      skipped_paths: [],
      written_paths: [],
    });
    setStore({
      previewConflicts: previewMock as never,
      syncKit: syncMock as never,
    });

    const { queryByText, findByTestId } = renderWithI18n(
      <InstallDialog
        preFilledKitIds={["k1"]}
        preFilledProjectPath="/Users/me/myapp"
        preFilledAgents={["claude"]}
        forceOverwriteMode={true}
        onClose={() => {}}
      />,
    );

    // The agent step is also skipped when agents are pre-filled
    expect(queryByText(/Pick agent/i)).toBeNull();
    // Preview step is skipped in forceOverwriteMode even when there ARE conflicts
    expect(queryByText(/Review conflicts/i)).toBeNull();
    // We are on the install step (identified by its body's data-testid).
    expect(await findByTestId("install-step-body")).toBeTruthy();

    // sync should be called eventually with the conflicting extension id auto-forced
    await vi.waitFor(() => {
      expect(syncMock).toHaveBeenCalled();
    });
    const callArg = (syncMock as Mock).mock.calls[0]?.[0] as {
      force_overwrite_extension_ids?: string[];
    };
    expect(callArg.force_overwrite_extension_ids).toContain("e1");
  });
});

describe("InstallDialog v2 production-quality guards", () => {
  beforeEach(() => {
    setStore();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("preview error surfaces as a toast and closes the dialog (no hang)", async () => {
    const previewMock = vi
      .fn()
      .mockRejectedValue(new Error("preview-backend-exploded"));
    const syncMock = vi.fn().mockResolvedValue({
      installed_count: 0,
      skipped_conflict_count: 0,
      skipped_paths: [],
      written_paths: [],
    });
    setStore({
      previewConflicts: previewMock as never,
      syncKit: syncMock as never,
    });
    (toast.error as Mock).mockClear();
    const onClose = vi.fn();

    // forceOverwriteMode + pre-fills auto-triggers runPreviewThenInstall.
    // previewConflicts rejects → toast.error + onClose (no in-dialog hang).
    renderWithI18n(
      <InstallDialog
        preFilledKitIds={["k1"]}
        preFilledProjectPath="/Users/me/myapp"
        preFilledAgents={["claude"]}
        forceOverwriteMode={true}
        onClose={onClose}
      />,
    );

    await vi.waitFor(() => {
      expect(toast.error).toHaveBeenCalledWith("preview-backend-exploded");
    });
    expect(onClose).toHaveBeenCalled();
    // Sanity: syncKit was never reached because preview failed first.
    expect(syncMock).not.toHaveBeenCalled();
  });

  it("unmount during preview does not setState on unmounted component", async () => {
    // Never-resolving preview to simulate slow backend; we control resolution
    let _resolve: (v: unknown) => void = () => {};
    const slowPreview = vi.fn(
      () =>
        new Promise((res) => {
          _resolve = res;
        }),
    );
    const syncMock = vi.fn().mockResolvedValue({
      installed_count: 0,
      skipped_conflict_count: 0,
      skipped_paths: [],
      written_paths: [],
    });
    setStore({
      previewConflicts: slowPreview as never,
      syncKit: syncMock as never,
    });

    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    // forceOverwriteMode auto-triggers preview cascade from useEffect.
    const { unmount } = renderWithI18n(
      <InstallDialog
        preFilledKitIds={["k1"]}
        preFilledProjectPath="/Users/me/myapp"
        preFilledAgents={["claude"]}
        forceOverwriteMode={true}
        onClose={() => {}}
      />,
    );

    // Give the auto-trigger useEffect a tick to fire and call slowPreview
    await vi.waitFor(() => {
      expect(slowPreview).toHaveBeenCalled();
    });

    // Preview is in-flight — unmount the dialog mid-flight
    unmount();

    // Resolve the dangling preview AFTER unmount; the dialog must not
    // attempt setState on its unmounted self
    _resolve({ extension_conflicts: [], config_conflicts: [] });
    await new Promise((r) => setTimeout(r, 20));

    // No "state update on an unmounted component" warning fired
    const warned = errorSpy.mock.calls.some((args) =>
      args.some(
        (a) =>
          typeof a === "string" &&
          /unmounted|memory leak|state update on/i.test(a),
      ),
    );
    expect(warned).toBe(false);
    // And syncKit must NOT have been invoked after unmount
    expect(syncMock).not.toHaveBeenCalled();
  });

  it("runInstall continues to next pair after one rejects (best-effort)", async () => {
    // Three pairs (1 kit × 3 agents). Middle one rejects. Use
    // forceOverwriteMode=true so the install cascade auto-triggers from
    // the mount useEffect without the user clicking through preview.
    const successResult = {
      installed_count: 1,
      skipped_conflict_count: 0,
      skipped_paths: [],
      written_paths: [],
    };
    const syncMock = vi
      .fn()
      .mockResolvedValueOnce(successResult)
      .mockRejectedValueOnce(new Error("middle pair boom"))
      .mockResolvedValueOnce(successResult);
    const previewMock = vi.fn().mockResolvedValue({
      extension_conflicts: [],
      config_conflicts: [],
    });
    setStore({
      previewConflicts: previewMock as never,
      syncKit: syncMock as never,
    });

    renderWithI18n(
      <InstallDialog
        preFilledKitIds={["k1"]}
        preFilledProjectPath="/Users/me/myapp"
        preFilledAgents={["claude", "cursor", "codex"]}
        forceOverwriteMode={true}
        onClose={() => {}}
      />,
    );

    // Wait for all three sync calls (i.e. the loop didn't short-circuit
    // after the second's rejection — the early-exit bug would yield 2).
    await vi.waitFor(() => {
      expect(syncMock).toHaveBeenCalledTimes(3);
    });
  });

  it("double-click on Install does not run install twice (preview cascade guarded)", async () => {
    // Slow preview so the second click fires while the first cascade is
    // still mid-flight — this is the bug we're guarding against.
    let _resolvePreview: (v: unknown) => void = () => {};
    const previewMock = vi.fn(
      () =>
        new Promise((res) => {
          _resolvePreview = res;
        }),
    );
    const syncMock = vi.fn().mockResolvedValue({
      installed_count: 1,
      skipped_conflict_count: 0,
      skipped_paths: [],
      written_paths: [],
    });
    setStore({
      previewConflicts: previewMock as never,
      syncKit: syncMock as never,
    });

    const { findByRole } = renderWithI18n(
      <InstallDialog
        preFilledKitIds={["k1"]}
        preFilledProjectPath="/Users/me/myapp"
        onClose={() => {}}
      />,
    );

    // Wait for agents to populate from the async api.listAgents mock
    const claudeCheckbox = (await findByRole("checkbox", {
      name: /claude/i,
    })) as HTMLInputElement;
    fireEvent.click(claudeCheckbox);

    // The footer Add button enables once project + an agent is selected.
    // (Was "Install" — now "Add" since the dialog was reframed around
    // "Add to Project".)
    const installButton = (await findByRole("button", {
      name: /^Add$/i,
    })) as HTMLButtonElement;

    // Rapid double-click — both events fire before the first preview resolves
    fireEvent.click(installButton);
    fireEvent.click(installButton);

    // Now resolve the preview so the cascade can complete
    _resolvePreview({ extension_conflicts: [], config_conflicts: [] });

    await vi.waitFor(() => {
      expect(syncMock).toHaveBeenCalled();
    });
    // Let any extra cascades settle
    await new Promise((r) => setTimeout(r, 30));

    // Preview must have run exactly once (cascade guard), not twice
    expect(previewMock).toHaveBeenCalledTimes(1);
    // And syncKit exactly once (1 kit × 1 agent), not twice
    expect(syncMock).toHaveBeenCalledTimes(1);
  });
});
