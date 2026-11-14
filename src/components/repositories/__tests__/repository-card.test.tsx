import { fireEvent, render, screen } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import i18n from "@/lib/i18n";
import type { RepositoryWithItems } from "@/lib/types";
import { useRepositoryStore } from "@/stores/repository-store";
import { RepositoryCard } from "../repository-card";

vi.mock("@/lib/invoke", () => ({
  api: {
    listRepositories: vi.fn().mockResolvedValue([]),
    syncRepository: vi.fn().mockResolvedValue({}),
    deleteRepository: vi.fn().mockResolvedValue(undefined),
    updateRepositoryItems: vi.fn().mockResolvedValue(1),
  },
}));

function renderWithI18n(ui: React.ReactElement) {
  return render(<I18nextProvider i18n={i18n}>{ui}</I18nextProvider>);
}

const repoWithUpdate: RepositoryWithItems = {
  repository: {
    id: "repo-card-1",
    name: "card-repo",
    url: "https://github.com/foo/card-repo",
    branch: "main",
    last_synced_at: "2026-01-01T00:00:00Z",
    last_commit_hash: "abcdef999999",
    last_commit_message: "feat: new skill version",
    last_sync_error: null,
    created_at: "2026-01-01T00:00:00Z",
  },
  items: [
    {
      repo_id: "repo-card-1",
      kind: "skill",
      item_id: "outdated-item",
      name: "Outdated Item",
      description: "Needs update",
      subpath: "skills/outdated-item",
      status: "available",
      installed_agents: [
        {
          agent: "claude",
          scope: { type: "global" },
          extension_id: "ext-1",
          revision: "111111100000",
        },
      ],
    },
    {
      repo_id: "repo-card-1",
      kind: "skill",
      item_id: "uptodate-item",
      name: "UpToDate Item",
      description: "Already current",
      subpath: "skills/uptodate-item",
      status: "available",
      installed_agents: [
        {
          agent: "claude",
          scope: { type: "global" },
          extension_id: "ext-2",
          revision: "abcdef999999",
        },
      ],
    },
  ],
};

describe("RepositoryCard Update All button", () => {
  beforeEach(() => {
    useRepositoryStore.setState({
      repositories: [repoWithUpdate],
      loading: false,
      isSyncingAll: false,
      isUpdatingAll: false,
      syncingRepoIds: new Set(),
      updatingRepoIds: new Set(),
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("renders per-repo Update All (1) button in card header and calls updateAll(repo.id)", () => {
    const updateAllSpy = vi.fn().mockResolvedValue(1);
    useRepositoryStore.setState({ updateAll: updateAllSpy });

    renderWithI18n(
      <RepositoryCard repoData={repoWithUpdate} searchFilter="" />,
    );

    const updateBtn = screen.getByTitle(
      /Update all 1 items in this repository/i,
    );
    expect(updateBtn).toBeInTheDocument();
    expect(updateBtn).toHaveTextContent(/Update All \(1\)/i);

    fireEvent.click(updateBtn);
    expect(updateAllSpy).toHaveBeenCalledWith("repo-card-1");
  });

  it("hides per-repo Update All button when all items are up to date", () => {
    const upToDateRepo: RepositoryWithItems = {
      ...repoWithUpdate,
      items: [repoWithUpdate.items[1]],
    };

    renderWithI18n(<RepositoryCard repoData={upToDateRepo} searchFilter="" />);

    expect(
      screen.queryByTitle(/Update all .* items in this repository/i),
    ).toBeNull();
  });

  it("disables per-repo Update All button when repo is in updatingRepoIds", () => {
    useRepositoryStore.setState({
      updatingRepoIds: new Set(["repo-card-1"]),
    });

    renderWithI18n(
      <RepositoryCard repoData={repoWithUpdate} searchFilter="" />,
    );

    const updateBtn = screen.getByTitle(
      /Update all 1 items in this repository/i,
    );
    expect(updateBtn).toBeDisabled();
    expect(updateBtn).toHaveTextContent(/Updating\.\.\./i);
  });
});
