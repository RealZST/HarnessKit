import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentInfo, NewRepoSkill } from "@/lib/types";
import { useAgentStore } from "@/stores/agent-store";
import { useExtensionStore } from "@/stores/extension-store";
import { useProjectStore } from "@/stores/project-store";
import { NewSkillsDialog } from "../new-skills-dialog";

const ignoreRepoSkill = vi.fn().mockResolvedValue(undefined);
vi.mock("@/lib/invoke", () => ({
  api: {
    ignoreRepoSkill: (...args: unknown[]) => ignoreRepoSkill(...args),
  },
}));

const mockSkills: NewRepoSkill[] = [
  {
    repo_url: "https://github.com/tazztone/skills-server",
    skill_id: "userscript-development",
    name: "userscript-development",
    description: "Develop, debug, and test userscripts for Tampermonkey",
    pack: "tazztone/skills-server",
  },
  {
    repo_url: "https://github.com/tazztone/skills-server",
    skill_id: "signal-stickers",
    name: "signal-stickers",
    description: "Create and prepare Signal sticker packs",
    pack: "tazztone/skills-server",
  },
  {
    repo_url: "https://github.com/comfyui/comfy-nodes",
    skill_id: "comfyui-node-packaging",
    name: "comfyui-node-packaging",
    description: "Packaging ComfyUI custom nodes",
    pack: "comfyui/comfy-nodes",
  },
];

const mockAgent: AgentInfo = {
  name: "antigravity",
  detected: true,
  extension_count: 5,
  path: "/home/user/.antigravity",
  enabled: true,
  capabilities: {
    project_install: { skill: true, mcp: true, hook: true, cli: true },
    hooks_supported: true,
    global_hook_install: true,
  },
};

describe("NewSkillsDialog", () => {
  const onInstall = vi.fn().mockResolvedValue(undefined);
  const onDismiss = vi.fn();
  const onClose = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    useAgentStore.setState({
      agents: [mockAgent],
      agentOrder: ["antigravity"],
    });
    useProjectStore.setState({ projects: [] });
    useExtensionStore.setState({ newRepoSkills: mockSkills });
  });

  it("renders skills grouped by repository with pinned header", () => {
    render(
      <NewSkillsDialog
        skills={mockSkills}
        onInstall={onInstall}
        onDismiss={onDismiss}
        onClose={onClose}
      />,
    );

    expect(screen.getByText("tazztone/skills-server")).toBeInTheDocument();
    expect(screen.getByText("comfyui/comfy-nodes")).toBeInTheDocument();
    expect(screen.getByText("userscript-development")).toBeInTheDocument();
    expect(screen.getByText("signal-stickers")).toBeInTheDocument();
    expect(screen.getByText("comfyui-node-packaging")).toBeInTheDocument();
  });

  it("filters skills dynamically when searching", async () => {
    const user = userEvent.setup();
    render(
      <NewSkillsDialog
        skills={mockSkills}
        onInstall={onInstall}
        onDismiss={onDismiss}
        onClose={onClose}
      />,
    );

    const searchInput = screen.getByPlaceholderText(
      "Search discovered skills...",
    );
    await user.type(searchInput, "signal");

    expect(screen.getByText("signal-stickers")).toBeInTheDocument();
    expect(
      screen.queryByText("userscript-development"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText("comfyui-node-packaging"),
    ).not.toBeInTheDocument();

    // Clear search
    await user.clear(searchInput);
    expect(screen.getByText("userscript-development")).toBeInTheDocument();
  });

  it("supports collapsing and expanding repository sections", async () => {
    const user = userEvent.setup();
    render(
      <NewSkillsDialog
        skills={mockSkills}
        onInstall={onInstall}
        onDismiss={onDismiss}
        onClose={onClose}
      />,
    );

    // Initial state: skills are visible
    expect(screen.getByText("comfyui-node-packaging")).toBeInTheDocument();

    // Click collapse on comfyui/comfy-nodes
    const collapseBtn = screen.getByLabelText("Collapse comfyui/comfy-nodes");
    await user.click(collapseBtn);

    // After collapsing, the skill is hidden
    expect(
      screen.queryByText("comfyui-node-packaging"),
    ).not.toBeInTheDocument();

    // Click expand
    const expandBtn = screen.getByLabelText("Expand comfyui/comfy-nodes");
    await user.click(expandBtn);
    expect(screen.getByText("comfyui-node-packaging")).toBeInTheDocument();
  });

  it("toggles all skills in a repo using the repo-level checkbox", async () => {
    const user = userEvent.setup();
    render(
      <NewSkillsDialog
        skills={mockSkills}
        onInstall={onInstall}
        onDismiss={onDismiss}
        onClose={onClose}
      />,
    );

    const repoCheckbox = screen.getByLabelText(
      "Select all skills in tazztone/skills-server",
    );
    // Initially checked because all skills are selected by default
    expect(repoCheckbox).toBeChecked();

    // Toggle off tazztone/skills-server
    await user.click(repoCheckbox);
    expect(repoCheckbox).not.toBeChecked();

    // Check individual items in that repo are unchecked
    const userscriptCheckbox = screen.getByLabelText(/userscript-development/i);
    expect(userscriptCheckbox).not.toBeChecked();

    // Skills in other repo remain selected
    const comfyCheckbox = screen.getByLabelText(/comfyui-node-packaging/i);
    expect(comfyCheckbox).toBeChecked();
  });

  it("handles ignore selected skills", async () => {
    const user = userEvent.setup();
    render(
      <NewSkillsDialog
        skills={mockSkills}
        onInstall={onInstall}
        onDismiss={onDismiss}
        onClose={onClose}
      />,
    );

    const ignoreBtn = screen.getByRole("button", {
      name: /ignore selected \(3\)/i,
    });
    await user.click(ignoreBtn);

    expect(ignoreRepoSkill).toHaveBeenCalledTimes(3);
    expect(ignoreRepoSkill).toHaveBeenCalledWith(
      "https://github.com/tazztone/skills-server",
      "userscript-development",
      "userscript-development",
    );
    expect(onClose).toHaveBeenCalled();
  });

  it("calls onDismiss when 'Dismiss for now' is clicked", async () => {
    const user = userEvent.setup();
    render(
      <NewSkillsDialog
        skills={mockSkills}
        onInstall={onInstall}
        onDismiss={onDismiss}
        onClose={onClose}
      />,
    );

    const dismissBtn = screen.getByRole("button", { name: "Dismiss for now" });
    await user.click(dismissBtn);
    expect(onDismiss).toHaveBeenCalled();
  });

  it("calls onClose when the X button is clicked", async () => {
    const user = userEvent.setup();
    render(
      <NewSkillsDialog
        skills={mockSkills}
        onInstall={onInstall}
        onDismiss={onDismiss}
        onClose={onClose}
      />,
    );

    const closeBtn = screen.getByLabelText("Cancel");
    await user.click(closeBtn);
    expect(onClose).toHaveBeenCalled();
  });
});
