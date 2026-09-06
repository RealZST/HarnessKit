import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "@/lib/invoke";
import type { Extension } from "@/lib/types";
import { useExtensionStore } from "../extension-store";
import { toast } from "../toast-store";

vi.mock("@/lib/invoke");

const dshPlugin: Extension = {
  id: "p1",
  kind: "plugin",
  name: "timer",
  description: "Plugin from profile web, bundle @deepseek-ai/dsh-base",
  source: { origin: "agent", url: null, version: null, commit_hash: null },
  agents: ["dsh"],
  tags: [],
  pack: "@deepseek-ai/dsh-base",
  permissions: [],
  enabled: true,
  trust_score: null,
  installed_at: "2026-08-01T00:00:00Z",
  updated_at: "2026-08-01T00:00:00Z",
  source_path: null,
  cli_parent_id: null,
  cli_meta: null,
  install_meta: null,
  scope: { type: "global" },
};

const linearMcp: Extension = {
  id: "m1",
  kind: "mcp",
  name: "linear",
  description: "Remote MCP server",
  source: { origin: "agent", url: null, version: null, commit_hash: null },
  agents: ["claude"],
  tags: [],
  pack: null,
  permissions: [],
  enabled: false,
  trust_score: null,
  installed_at: "2026-08-01T00:00:00Z",
  updated_at: "2026-08-01T00:00:00Z",
  source_path: null,
  cli_parent_id: null,
  cli_meta: null,
  install_meta: null,
  scope: { type: "global" },
};

describe("extension-store toggle", () => {
  beforeEach(() => {
    useExtensionStore.setState({ extensions: [], pendingDelete: null });
    vi.resetAllMocks();
  });

  // Legacy snapshots only: disable is lossless now, but a server disabled by
  // an older version has `<redacted>` where its secrets were. Re-enabling one
  // succeeds, yet the server cannot start until the user restores the real
  // values. The backend only wrote that to stderr, so desktop and web users
  // never saw it.
  it("toggle surfaces a warning toast when re-enable reports redacted secrets", async () => {
    useExtensionStore.setState({ extensions: [linearMcp] });
    const groupKey = useExtensionStore.getState().grouped()[0].groupKey;
    vi.mocked(api.toggleExtension).mockResolvedValue({
      redacted_secret_keys: ["API_KEY"],
    });
    const warnSpy = vi.spyOn(toast, "warning").mockImplementation(() => {});

    await useExtensionStore.getState().toggle(groupKey, true);

    expect(warnSpy).toHaveBeenCalledTimes(1);
    expect(warnSpy.mock.calls[0][0]).toContain("API_KEY");
    expect(warnSpy.mock.calls[0][0]).toContain("linear");
  });

  it("stays quiet when no secrets came back redacted", async () => {
    useExtensionStore.setState({ extensions: [linearMcp] });
    const groupKey = useExtensionStore.getState().grouped()[0].groupKey;
    vi.mocked(api.toggleExtension).mockResolvedValue({
      redacted_secret_keys: [],
    });
    const warnSpy = vi.spyOn(toast, "warning").mockImplementation(() => {});

    await useExtensionStore.getState().toggle(groupKey, true);

    expect(warnSpy).not.toHaveBeenCalled();
  });
});

describe("extension-store confirmDelete", () => {
  beforeEach(() => {
    useExtensionStore.setState({ extensions: [], pendingDelete: null });
    vi.resetAllMocks();
  });

  // Deletion is optimistic: the row leaves the list and the success toast
  // fires five seconds BEFORE the request goes out. A refusal that only got
  // logged left the UI asserting a deletion that never happened — the row
  // stayed gone until a manual reload, and nothing told the user why.
  it("puts the rows back and reports why when the backend refuses", async () => {
    const errorToast = vi.spyOn(toast, "error").mockImplementation(() => {});
    vi.mocked(api.deleteExtension).mockRejectedValue(
      '{"kind":"Validation","message":"\'timer\' is a dsh plugin row"}',
    );
    // State right after the optimistic removal: gone from the list, parked
    // in pendingDelete.
    useExtensionStore.setState({
      extensions: [],
      pendingDelete: {
        ids: new Set(["p1"]),
        extensions: [dshPlugin],
        timer: 0 as unknown as ReturnType<typeof setTimeout>,
      },
    });

    await expect(
      useExtensionStore.getState().confirmDelete(),
    ).resolves.toBeUndefined();

    expect(useExtensionStore.getState().extensions).toEqual([dshPlugin]);
    expect(errorToast).toHaveBeenCalledTimes(1);
    // The backend's reason has to reach the toast, not a generic failure.
    expect(errorToast.mock.calls[0][0]).toContain("dsh plugin row");
    // A failed delete must not leave a rescan running over a stale list.
    expect(api.scanAndSync).not.toHaveBeenCalled();
  });

  it("does not restore anything when the delete succeeds", async () => {
    vi.mocked(api.deleteExtension).mockResolvedValue(undefined);
    vi.mocked(api.scanAndSync).mockResolvedValue(0);
    vi.mocked(api.listExtensions).mockResolvedValue([]);
    useExtensionStore.setState({
      extensions: [],
      pendingDelete: {
        ids: new Set(["p1"]),
        extensions: [dshPlugin],
        timer: 0 as unknown as ReturnType<typeof setTimeout>,
      },
    });

    await useExtensionStore.getState().confirmDelete();

    expect(useExtensionStore.getState().extensions).toEqual([]);
  });
});
