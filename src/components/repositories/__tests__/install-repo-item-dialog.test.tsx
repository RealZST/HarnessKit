import { fireEvent, render, screen } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import i18n from "@/lib/i18n";
import type { RepositoryItemWithInstallStatus } from "@/lib/types";
import { useAgentStore } from "@/stores/agent-store";
import { useScopeStore } from "@/stores/scope-store";
import { InstallRepoItemDialog } from "../install-repo-item-dialog";

vi.mock("@/stores/toast-store", () => ({
  toast: {
    error: vi.fn(),
    success: vi.fn(),
    warning: vi.fn(),
    info: vi.fn(),
  },
}));

function renderWithI18n(ui: React.ReactElement) {
  return render(<I18nextProvider i18n={i18n}>{ui}</I18nextProvider>);
}

const mockAgents = [
  {
    name: "claude",
    detected: true,
    enabled: true,
    extension_count: 0,
    path: "/test/claude",
    capabilities: {
      project_install: { skill: true, mcp: true, hook: false, cli: false },
      hooks_supported: false,
      global_hook_install: false,
    },
  },
  {
    name: "cursor",
    detected: true,
    enabled: true,
    extension_count: 0,
    path: "/test/cursor",
    capabilities: {
      project_install: { skill: true, mcp: true, hook: false, cli: false },
      hooks_supported: false,
      global_hook_install: false,
    },
  },
];

describe("InstallRepoItemDialog", () => {
  beforeEach(() => {
    useAgentStore.setState({
      agents: mockAgents,
      agentOrder: ["claude", "cursor"],
      loading: false,
    });
    useScopeStore.setState({
      current: { type: "global" },
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("preselects the first detected agent on fresh install", () => {
    const item: RepositoryItemWithInstallStatus = {
      repo_id: "repo-1",
      item_id: "skill-1",
      kind: "skill",
      name: "awesome-skill",
      description: "A great skill",
      subpath: "skills/awesome",
      status: "available",
      installed_agents: [],
    };

    renderWithI18n(
      <InstallRepoItemDialog
        open={true}
        onClose={vi.fn()}
        repoId="repo-1"
        item={item}
      />,
    );

    // Claude (first agent) should be selected (has border-primary / font-medium)
    const claudeBtn = screen.getByRole("button", { name: /Claude Code/i });
    expect(claudeBtn.className).toContain("border-primary");

    const cursorBtn = screen.getByRole("button", { name: /Cursor/i });
    expect(cursorBtn.className).not.toContain("border-primary");
  });

  it("preselects agents that already have the item installed on update", () => {
    const item: RepositoryItemWithInstallStatus = {
      repo_id: "repo-1",
      item_id: "skill-1",
      kind: "skill",
      name: "awesome-skill",
      description: "A great skill",
      subpath: "skills/awesome",
      status: "available",
      installed_agents: [
        {
          agent: "cursor",
          scope: { type: "global" },
          extension_id: "ext-1",
          revision: "abc1234",
        },
      ],
    };

    renderWithI18n(
      <InstallRepoItemDialog
        open={true}
        onClose={vi.fn()}
        repoId="repo-1"
        item={item}
      />,
    );

    const cursorBtn = screen.getByRole("button", { name: /Cursor/i });
    expect(cursorBtn.className).toContain("border-primary");

    const claudeBtn = screen.getByRole("button", { name: /Claude Code/i });
    expect(claudeBtn.className).not.toContain("border-primary");
  });

  it("renders localized 'Installed' badge when agent is unselected but has item installed", () => {
    const item: RepositoryItemWithInstallStatus = {
      repo_id: "repo-1",
      item_id: "skill-1",
      kind: "skill",
      name: "awesome-skill",
      description: "A great skill",
      subpath: "skills/awesome",
      status: "available",
      installed_agents: [
        {
          agent: "cursor",
          scope: { type: "global" },
          extension_id: "ext-1",
          revision: "abc1234",
        },
      ],
    };

    renderWithI18n(
      <InstallRepoItemDialog
        open={true}
        onClose={vi.fn()}
        repoId="repo-1"
        item={item}
      />,
    );

    // Unselect cursor
    const cursorBtn = screen.getByRole("button", { name: /Cursor/i });
    fireEvent.click(cursorBtn);

    // Should display localized "Installed" text next to unselected cursor
    expect(screen.getByText("Installed")).toBeInTheDocument();
  });
});
