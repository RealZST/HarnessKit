import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  ConfigScope,
  RepositoryItem,
  RepositoryWithItems,
} from "@/lib/types";
import { useRepositoryStore } from "@/stores/repository-store";
import { useScopeStore } from "@/stores/scope-store";
import { useToastStore } from "@/stores/toast-store";
import { RepositoriesView } from "../repositories-view";

const project: ConfigScope = { type: "project", name: "proj", path: "/proj" };

function item(
  name: string,
  over: Partial<RepositoryItem> = {},
): RepositoryItem {
  return {
    item_id: name,
    name,
    description: "",
    ignored: false,
    is_new: false,
    installed: [],
    ...over,
  };
}

const repo: RepositoryWithItems = {
  id: "r1",
  url: "https://github.com/acme/skills",
  name: "acme/skills",
  key: "github.com/acme/skills",
  last_synced_at: new Date().toISOString(),
  last_commit_hash: null,
  last_commit_message: null,
  last_sync_error: null,
  extension_ids: ["e1", "e2"],
  items: [
    item("pdf", {
      installed: [
        { extension_id: "e1", agent: "claude", scope: { type: "global" } },
      ],
    }),
    item("docx", {
      installed: [{ extension_id: "e2", agent: "codex", scope: project }],
    }),
    item("pptx"),
  ],
};

const api = vi.hoisted(() => ({
  listRepositories: vi.fn(),
  syncRepository: vi.fn(),
  getRepositoryItemContent: vi.fn(),
}));
vi.mock("@/lib/invoke", () => ({ api }));

const renderView = () =>
  render(
    <MemoryRouter>
      <RepositoriesView onSearchDiscover={() => {}} />
    </MemoryRouter>,
  );

const rowNames = () =>
  screen.getAllByText(/^(pdf|docx|pptx)$/).map((el) => el.textContent);

describe("RepositoriesView", () => {
  beforeEach(() => {
    api.syncRepository.mockReset();
    useRepositoryStore.setState(useRepositoryStore.getInitialState());
    useToastStore.setState({ toasts: [] });
    useScopeStore.setState({ current: { type: "all" }, hydrated: true });
  });

  it("counts a copy as installed only in the scope being viewed", async () => {
    useScopeStore.setState({ current: project, hydrated: true });
    api.listRepositories.mockResolvedValue([repo]);
    renderView();

    await userEvent.click(
      await screen.findByRole("button", { name: "Installed (1)" }),
    );
    expect(rowNames()).toEqual(["docx"]);

    await userEvent.click(
      screen.getByRole("button", { name: "Not installed (2)" }),
    );
    expect(rowNames()).toEqual(["pdf", "pptx"]);
  });

  it("shows a skill's SKILL.md when the skill is picked, until the panel is closed", async () => {
    api.listRepositories.mockResolvedValue([repo]);
    api.getRepositoryItemContent.mockResolvedValue("# Slides, step by step");
    renderView();

    await userEvent.click(
      await screen.findByRole("button", { name: "View details for pptx" }),
    );
    expect(await screen.findByText("# Slides, step by step")).toBeVisible();
    expect(api.getRepositoryItemContent).toHaveBeenCalledWith("r1", "pptx");

    await userEvent.click(
      screen.getByRole("button", { name: "Close details" }),
    );
    expect(screen.queryByText("# Slides, step by step")).toBeNull();
  });

  it("marks a skill new until it is installed, and Escape clears the search", async () => {
    const installed = [
      {
        extension_id: "e9",
        agent: "claude",
        scope: { type: "global" as const },
      },
    ];
    api.listRepositories.mockResolvedValue([
      {
        ...repo,
        items: [
          item("fresh", { is_new: true }),
          item("taken", { is_new: true, installed }),
        ],
      },
    ]);
    renderView();

    expect(await screen.findAllByText("New")).toHaveLength(1);

    const search = screen.getByRole("textbox");
    await userEvent.type(search, "zzz");
    expect(screen.queryByText("fresh")).not.toBeInTheDocument();
    await userEvent.type(search, "{Escape}");
    expect(screen.getByText("fresh")).toBeInTheDocument();
  });

  it("syncs a few repositories at a time and marks the rest as queued", async () => {
    const many = Array.from({ length: 8 }, (_, i) => ({
      ...repo,
      id: `r${i}`,
    }));
    api.listRepositories.mockResolvedValue(many);
    api.syncRepository.mockReturnValue(new Promise(() => {}));
    renderView();

    await userEvent.click(
      await screen.findByRole("button", { name: "Refresh all" }),
    );

    expect(api.syncRepository).toHaveBeenCalledTimes(4);
    expect(screen.getAllByText("Queued")).toHaveLength(4);
    expect(screen.getByText("Refreshing 0/8")).toBeInTheDocument();
  });

  it("refreshes what is a day old on opening, quietly, and reports a refresh of all", async () => {
    api.listRepositories.mockResolvedValue([
      repo,
      { ...repo, id: "old", last_synced_at: "2020-01-01T00:00:00Z" },
    ]);
    api.syncRepository.mockImplementation((id: string) =>
      id === "old" ? Promise.reject(new Error("offline")) : Promise.resolve(),
    );
    renderView();

    await waitFor(() =>
      expect(api.syncRepository.mock.calls).toEqual([["old"]]),
    );
    const refreshAll = await screen.findByRole("button", {
      name: "Refresh all",
    });
    expect(useToastStore.getState().toasts).toEqual([]);

    await userEvent.click(refreshAll);
    await waitFor(() =>
      expect(useToastStore.getState().toasts.map((t) => t.message)).toEqual([
        "Couldn't refresh 1 of 2",
      ]),
    );
    expect(api.syncRepository).toHaveBeenCalledTimes(3);
  });
});
