import { fireEvent, render, screen } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import i18n from "@/lib/i18n";
import { useRepositoryStore } from "@/stores/repository-store";
import { RepositoriesView } from "../repositories-view";

vi.mock("@/lib/invoke", () => ({
  api: {
    listRepositories: vi.fn().mockResolvedValue([]),
    syncRepository: vi.fn().mockResolvedValue({}),
    deleteRepository: vi.fn().mockResolvedValue(undefined),
  },
}));

function renderWithI18n(ui: React.ReactElement) {
  return render(<I18nextProvider i18n={i18n}>{ui}</I18nextProvider>);
}

describe("RepositoriesView search clear button", () => {
  beforeEach(() => {
    useRepositoryStore.setState({
      repositories: [
        {
          repository: {
            id: "repo-1",
            name: "test-repo",
            url: "https://github.com/test/repo",
            branch: "main",
            last_synced_at: null,
            last_commit_hash: null,
            last_commit_message: null,
            last_sync_error: null,
            created_at: "2026-01-01",
          },
          items: [],
        },
      ],
      loading: false,
      isSyncingAll: false,
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("shows clear button when text is entered and clears input when clicked", () => {
    renderWithI18n(<RepositoriesView />);

    const searchInput = screen.getByPlaceholderText(
      /Search repositories, skills, descriptions.../i,
    ) as HTMLInputElement;

    // Initially clear button should not be present
    expect(screen.queryByLabelText("Clear search")).toBeNull();

    // Type query
    fireEvent.change(searchInput, { target: { value: "test query" } });
    expect(searchInput.value).toBe("test query");

    // Clear button should appear
    const clearBtn = screen.getByLabelText("Clear search");
    expect(clearBtn).toBeInTheDocument();

    // Click clear button
    fireEvent.click(clearBtn);
    expect(searchInput.value).toBe("");
    expect(screen.queryByLabelText("Clear search")).toBeNull();

    // Type query again and press Escape
    fireEvent.change(searchInput, { target: { value: "another query" } });
    expect(searchInput.value).toBe("another query");
    fireEvent.keyDown(searchInput, { key: "Escape" });
    expect(searchInput.value).toBe("");
  });

  it("filters repositories during search and shows empty state when nothing matches", () => {
    useRepositoryStore.setState({
      repositories: [
        {
          repository: {
            id: "repo-1",
            name: "ponytail-repo",
            url: "https://github.com/foo/ponytail",
            branch: "main",
            last_synced_at: null,
            last_commit_hash: null,
            last_commit_message: null,
            last_sync_error: null,
            created_at: "2026-01-01",
          },
          items: [
            {
              repo_id: "repo-1",
              kind: "skill",
              item_id: "ponytail",
              name: "ponytail",
              description: "Lazy senior dev mode",
              subpath: "skills/ponytail",
              status: "available",
              installed_agents: [],
            },
          ],
        },
        {
          repository: {
            id: "repo-2",
            name: "other-repo",
            url: "https://github.com/bar/other",
            branch: "main",
            last_synced_at: null,
            last_commit_hash: null,
            last_commit_message: null,
            last_sync_error: null,
            created_at: "2026-01-01",
          },
          items: [
            {
              repo_id: "repo-2",
              kind: "skill",
              item_id: "other-skill",
              name: "other-skill",
              description: "Another skill",
              subpath: "skills/other",
              status: "available",
              installed_agents: [],
            },
          ],
        },
      ],
      loading: false,
      isSyncingAll: false,
    });

    renderWithI18n(<RepositoriesView />);

    expect(screen.getByText("ponytail-repo")).toBeInTheDocument();
    expect(screen.getByText("other-repo")).toBeInTheDocument();

    const searchInput = screen.getByPlaceholderText(
      /Search repositories, skills, descriptions.../i,
    );

    // Search for ponytail: other-repo should be filtered out
    fireEvent.change(searchInput, { target: { value: "ponytail" } });
    expect(screen.getByText("ponytail-repo")).toBeInTheDocument();
    expect(screen.queryByText("other-repo")).toBeNull();

    // Search for non-existent query: empty search state should be shown
    fireEvent.change(searchInput, { target: { value: "nonexistent-query" } });
    expect(screen.queryByText("ponytail-repo")).toBeNull();
    expect(screen.queryByText("other-repo")).toBeNull();
    expect(
      screen.getByText(/No repositories or skills match your search/i),
    ).toBeInTheDocument();

    // Click "Clear search" in empty state
    const clearButtons = screen.getAllByRole("button", {
      name: /Clear search/i,
    });
    fireEvent.click(clearButtons[clearButtons.length - 1]);
    expect(screen.getByText("ponytail-repo")).toBeInTheDocument();
    expect(screen.getByText("other-repo")).toBeInTheDocument();
  });

  it("highlights specific skill when highlightItemId is passed", () => {
    useRepositoryStore.setState({
      repositories: [
        {
          repository: {
            id: "repo-1",
            name: "test-repo",
            url: "https://github.com/foo/test",
            branch: "main",
            last_synced_at: null,
            last_commit_hash: null,
            last_commit_message: null,
            last_sync_error: null,
            created_at: "2026-01-01",
          },
          items: [
            {
              repo_id: "repo-1",
              kind: "skill",
              item_id: "target-skill",
              name: "Target Skill",
              description: "Target skill description",
              subpath: "skills/target",
              status: "available",
              installed_agents: [],
            },
            {
              repo_id: "repo-1",
              kind: "skill",
              item_id: "other-skill",
              name: "Other Skill",
              description: "Other skill description",
              subpath: "skills/other",
              status: "available",
              installed_agents: [],
            },
          ],
        },
      ],
      loading: false,
      isSyncingAll: false,
    });

    renderWithI18n(<RepositoriesView highlightItemId="target-skill" />);

    const targetItemEl = document.getElementById("repo-item-target-skill");
    expect(targetItemEl).toBeInTheDocument();
    expect(targetItemEl?.className).toContain("ring-primary");

    const otherItemEl = document.getElementById("repo-item-other-skill");
    expect(otherItemEl).toBeInTheDocument();
    expect(otherItemEl?.className).not.toContain("ring-primary");
  });

  it("renders Update button and not Install button when item has an update", () => {
    useRepositoryStore.setState({
      repositories: [
        {
          repository: {
            id: "repo-1",
            name: "test-repo",
            url: "https://github.com/foo/test",
            branch: "main",
            last_synced_at: "2026-01-01",
            last_commit_hash: "newcommit123",
            last_commit_message: "update skill",
            last_sync_error: null,
            created_at: "2026-01-01",
          },
          items: [
            {
              repo_id: "repo-1",
              kind: "skill",
              item_id: "outdated-skill",
              name: "Outdated Skill",
              description: "Needs update",
              subpath: "skills/outdated",
              status: "available",
              installed_agents: [
                {
                  agent: "claude",
                  scope: { type: "global" },
                  extension_id: "ext-1",
                  revision: "oldcommit000",
                },
              ],
            },
          ],
        },
      ],
      loading: false,
      isSyncingAll: false,
    });

    renderWithI18n(<RepositoriesView />);

    expect(screen.getByRole("button", { name: /Update/i })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Install$/i })).toBeNull();
  });
});
