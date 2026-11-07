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
  });
});
