import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { useKitStore } from "@/stores/kit-store";
import { RemoveFromProjectDialog } from "../remove-from-project-dialog";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

const targets = [
  {
    project_path: "/p/alpha",
    agent_name: "claude",
    synced_at: "2026-05-01T00:00:00Z",
    file_count: 4,
    shared_with: [],
  },
  {
    project_path: "/p/alpha",
    agent_name: "codex",
    synced_at: "2026-05-02T00:00:00Z",
    file_count: 2,
    shared_with: [],
  },
  {
    project_path: "/p/bravo",
    agent_name: "claude",
    synced_at: "2026-05-03T00:00:00Z",
    file_count: 3,
    shared_with: [],
  },
];

describe("RemoveFromProjectDialog", () => {
  it("lists each sync target with project + agent + file count", () => {
    render(
      <RemoveFromProjectDialog
        kitId="k1"
        syncTargets={targets}
        onClose={() => {}}
      />,
    );
    expect(screen.getAllByText(/\/p\/alpha/).length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText(/claude/i).length).toBeGreaterThanOrEqual(1);
  });

  it("ticking a target and clicking Remove calls unsyncKit and closes", async () => {
    const onClose = vi.fn();
    const unsyncKit = vi.fn().mockResolvedValue(undefined);
    useKitStore.setState({ unsyncKit } as never);
    const user = userEvent.setup();

    render(
      <RemoveFromProjectDialog
        kitId="k1"
        syncTargets={targets}
        onClose={onClose}
      />,
    );

    // Checkboxes: [0] = "All installs" select-all, [1..N] = per-target.
    // Tick the first per-target checkbox (= targets[0] = /p/alpha + claude).
    const checkboxes = screen.getAllByRole("checkbox");
    await user.click(checkboxes[1]);

    // Confirm button uses i18n key `detail.removeCount` (identity mock returns
    // the key as accessible name).
    await user.click(
      screen.getByRole("button", { name: /detail\.removeCount/ }),
    );

    expect(unsyncKit).toHaveBeenCalledWith({
      kit_id: "k1",
      project_path: "/p/alpha",
      agent_name: "claude",
    });
    expect(onClose).toHaveBeenCalled();
  });
});
