import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "@/lib/invoke";
import type {
  RepoSyncResult,
  Repository,
  RepositoryWithItems,
} from "@/lib/types";
import { useRepositoryStore } from "../repository-store";

vi.mock("@/lib/invoke");

const sampleRepo: Repository = {
  id: "repo-1",
  url: "https://github.com/anthropics/skills.git",
  name: "anthropics/skills",
  branch: "main",
  last_synced_at: "2026-05-20T00:00:00Z",
  last_commit_hash: "abc1234",
  last_commit_message: "feat: add skill",
  last_sync_error: null,
  created_at: "2026-05-20T00:00:00Z",
};

const sampleRepoData: RepositoryWithItems = {
  repository: sampleRepo,
  items: [
    {
      repo_id: "repo-1",
      kind: "skill",
      item_id: "weather",
      name: "Weather",
      description: "Weather lookup",
      subpath: "skills/weather",
      status: "available",
      installed_agents: [],
    },
  ],
};

describe("repository-store", () => {
  beforeEach(() => {
    useRepositoryStore.setState({
      repositories: [],
      loading: false,
      syncingRepoIds: new Set(),
      isSyncingAll: false,
      updatingRepoIds: new Set(),
      isUpdatingAll: false,
    });
    vi.resetAllMocks();
  });

  it("fetch populates repositories from api", async () => {
    vi.mocked(api.listRepositories).mockResolvedValue([sampleRepoData]);
    await useRepositoryStore.getState().fetch();
    expect(useRepositoryStore.getState().repositories).toEqual([
      sampleRepoData,
    ]);
  });

  it("deleteRepo removes repo from state on success", async () => {
    useRepositoryStore.setState({ repositories: [sampleRepoData] });
    vi.mocked(api.deleteRepository).mockResolvedValue(true);
    const result = await useRepositoryStore.getState().deleteRepo("repo-1");
    expect(result).toBe(true);
    expect(useRepositoryStore.getState().repositories).toEqual([]);
  });

  it("setItemStatus updates local item status optimistically", async () => {
    useRepositoryStore.setState({ repositories: [sampleRepoData] });
    vi.mocked(api.setRepositoryItemStatus).mockResolvedValue(undefined);
    await useRepositoryStore
      .getState()
      .setItemStatus("repo-1", "weather", "ignored", "skill");
    const items = useRepositoryStore.getState().repositories[0].items;
    expect(items[0].status).toBe("ignored");
  });

  it("syncRepo manages syncingRepoIds set", async () => {
    const syncRes: RepoSyncResult = {
      repo_id: "repo-1",
      commit_hash: "newhash",
      commit_message: "updated",
      items_count: 1,
      synced_at: "2026-05-21T00:00:00Z",
    };
    vi.mocked(api.syncRepository).mockResolvedValue(syncRes);
    vi.mocked(api.listRepositories).mockResolvedValue([
      {
        ...sampleRepoData,
        repository: { ...sampleRepo, last_commit_hash: "newhash" },
      },
    ]);

    await useRepositoryStore.getState().syncRepo("repo-1");
    expect(useRepositoryStore.getState().syncingRepoIds.has("repo-1")).toBe(
      false,
    );
    expect(
      useRepositoryStore.getState().repositories[0].repository.last_commit_hash,
    ).toBe("newhash");
  });

  it("syncRepo refreshes repositories in finally even when syncRepository rejects", async () => {
    vi.mocked(api.syncRepository).mockRejectedValue(
      new Error("git clone failed"),
    );
    vi.mocked(api.listRepositories).mockResolvedValue([
      {
        ...sampleRepoData,
        repository: {
          ...sampleRepo,
          last_sync_error: "git clone failed: repository not found",
        },
      },
    ]);

    await expect(
      useRepositoryStore.getState().syncRepo("repo-1"),
    ).rejects.toThrow("git clone failed");

    expect(api.listRepositories).toHaveBeenCalledTimes(1);
    expect(useRepositoryStore.getState().syncingRepoIds.has("repo-1")).toBe(
      false,
    );
    expect(
      useRepositoryStore.getState().repositories[0].repository.last_sync_error,
    ).toBe("git clone failed: repository not found");
  });

  it("updateAll manages isUpdatingAll and updatingRepoIds and refreshes stores", async () => {
    vi.mocked(api.updateRepositoryItems).mockResolvedValue(3);
    vi.mocked(api.listRepositories).mockResolvedValue([sampleRepoData]);
    vi.mocked(api.listExtensions).mockResolvedValue([]);
    vi.mocked(api.getAllTags).mockResolvedValue([]);
    vi.mocked(api.getAllPacks).mockResolvedValue([]);
    vi.mocked(api.getCachedUpdateStatuses).mockResolvedValue([]);

    const countGlobal = await useRepositoryStore.getState().updateAll();
    expect(countGlobal).toBe(3);
    expect(api.updateRepositoryItems).toHaveBeenCalledWith(undefined);
    expect(useRepositoryStore.getState().isUpdatingAll).toBe(false);

    vi.mocked(api.updateRepositoryItems).mockResolvedValue(1);
    const countRepo = await useRepositoryStore.getState().updateAll("repo-1");
    expect(countRepo).toBe(1);
    expect(api.updateRepositoryItems).toHaveBeenCalledWith("repo-1");
    expect(useRepositoryStore.getState().updatingRepoIds.has("repo-1")).toBe(
      false,
    );
  });
});
