import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "@/lib/invoke";
import type { KitSummary } from "@/types/kits";
import { useKitStore } from "../kit-store";
import { useProjectStore } from "../project-store";

vi.mock("@/lib/invoke");

const sampleKit: KitSummary = {
  id: "k1",
  name: "Frontend",
  description: "",
  extension_count: 2,
  config_file_count: 1,
  sync_count: 0,
  kind_counts: { skill: 0, mcp: 0, plugin: 0, hook: 0, cli: 0 },
  created_at: "2026-05-21T00:00:00Z",
  updated_at: "2026-05-21T00:00:00Z",
  corrupt: false,
  search_keywords: "frontend",
};

describe("kit-store", () => {
  beforeEach(() => {
    useKitStore.setState({
      kits: [],
      details: null,
      installRecords: [],
    });
    vi.resetAllMocks();
  });

  it("fetchKits populates kits from api", async () => {
    vi.mocked(api.listKits).mockResolvedValue([sampleKit]);
    await useKitStore.getState().fetchKits();
    expect(useKitStore.getState().kits).toEqual([sampleKit]);
  });

  it("deleteKit removes the kit from state on success", async () => {
    useKitStore.setState({ kits: [sampleKit] });
    vi.mocked(api.deleteKit).mockResolvedValue(undefined);
    vi.mocked(api.listKits).mockResolvedValue([]);
    vi.mocked(api.listProjectInstallRecords).mockResolvedValue([]);
    await useKitStore.getState().deleteKit("k1");
    expect(useKitStore.getState().kits).toEqual([]);
  });
});

describe("kit-store — syncKit refreshes useProjectStore", () => {
  beforeEach(() => {
    useKitStore.setState({
      kits: [],
      details: null,
      installRecords: [],
    });
    vi.resetAllMocks();
  });

  it("calls useProjectStore.loadProjects after a successful syncKit", async () => {
    // Mock all api calls fanned-out by syncKit (sync + fetchKits + fetchInstallRecords).
    vi.mocked(api.syncKitToProject).mockResolvedValue({
      installed_count: 1,
      skipped_conflict_count: 0,
      skipped_paths: [],
      written_paths: [],
    });
    vi.mocked(api.listKits).mockResolvedValue([]);
    vi.mocked(api.listProjectInstallRecords).mockResolvedValue([]);

    // Spy on loadProjects via the store action itself; do not need to mock
    // api.listProjects because we replace the action with a vi.fn.
    const loadSpy = vi.fn().mockResolvedValue(undefined);
    useProjectStore.setState({ loadProjects: loadSpy });

    await useKitStore.getState().syncKit({
      kit_id: "k1",
      project_path: "/tmp/new-proj",
      agent_name: "claude",
      force_overwrite_extension_ids: [],
      force_overwrite_config_keys: [],
    });

    expect(loadSpy).toHaveBeenCalled();
  });
});
