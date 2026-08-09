import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { SkillFileSection } from "../skill-file-section";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// Web mode: no Open button, copy path available.
vi.mock("@/lib/transport", () => ({
  isDesktop: () => false,
}));

const copyPathToClipboard = vi.fn();
vi.mock("@/lib/copy-path", () => ({
  copyPathToClipboard: (path: string) => copyPathToClipboard(path),
}));

vi.mock("@/lib/invoke", () => ({
  api: {
    listSkillFiles: vi.fn(async () => [
      { name: "SKILL.md", path: "/skill/SKILL.md", is_dir: false },
      { name: "LICENSE.txt", path: "/skill/LICENSE.txt", is_dir: false },
      {
        name: "agents",
        path: "/skill/agents",
        is_dir: true,
        children: [
          {
            name: "openai.yaml",
            path: "/skill/agents/openai.yaml",
            is_dir: false,
          },
        ],
      },
      {
        name: "references",
        path: "/skill/references",
        is_dir: true,
        children: [
          { name: "cli.md", path: "/skill/references/cli.md", is_dir: false },
        ],
      },
    ]),
    readConfigFilePreview: vi.fn(async (path: string) => `CONTENT:${path}`),
    openInSystem: vi.fn(),
    revealInFileManager: vi.fn(),
  },
}));

function renderSection() {
  return render(<SkillFileSection dirPath="/skill" loading={false} />);
}

describe("SkillFileSection file previews", () => {
  it("expands a file preview on click and collapses it on second click", async () => {
    const user = userEvent.setup();
    renderSection();

    await user.click(await screen.findByText("SKILL.md"));
    expect(
      await screen.findByText("CONTENT:/skill/SKILL.md"),
    ).toBeInTheDocument();

    await user.click(screen.getByText("SKILL.md"));
    expect(
      screen.queryByText("CONTENT:/skill/SKILL.md"),
    ).not.toBeInTheDocument();
  });

  it("keeps at most one file preview open (accordion)", async () => {
    const user = userEvent.setup();
    renderSection();

    await user.click(await screen.findByText("SKILL.md"));
    await screen.findByText("CONTENT:/skill/SKILL.md");

    await user.click(screen.getByText("LICENSE.txt"));
    expect(
      await screen.findByText("CONTENT:/skill/LICENSE.txt"),
    ).toBeInTheDocument();
    expect(
      screen.queryByText("CONTENT:/skill/SKILL.md"),
    ).not.toBeInTheDocument();
  });

  it("keeps at most one sibling directory open (accordion)", async () => {
    const user = userEvent.setup();
    renderSection();

    await user.click(await screen.findByText("agents"));
    expect(await screen.findByText("openai.yaml")).toBeInTheDocument();

    await user.click(screen.getByText("references"));
    expect(await screen.findByText("cli.md")).toBeInTheDocument();
    expect(screen.queryByText("openai.yaml")).not.toBeInTheDocument();
  });

  it("collapsing a directory also collapses the preview inside it", async () => {
    const user = userEvent.setup();
    renderSection();

    await user.click(await screen.findByText("agents"));
    await user.click(await screen.findByText("openai.yaml"));
    await screen.findByText("CONTENT:/skill/agents/openai.yaml");

    await user.click(screen.getByText("agents")); // collapse
    await user.click(screen.getByText("agents")); // re-expand
    expect(await screen.findByText("openai.yaml")).toBeInTheDocument();
    expect(
      screen.queryByText("CONTENT:/skill/agents/openai.yaml"),
    ).not.toBeInTheDocument();
  });

  it("web mode offers Copy Path but no Open button in the preview block", async () => {
    const user = userEvent.setup();
    renderSection();

    await user.click(await screen.findByText("SKILL.md"));
    await screen.findByText("CONTENT:/skill/SKILL.md");

    expect(screen.queryByText("fileTree.open")).not.toBeInTheDocument();
    await user.click(screen.getByText("file.copyPath"));
    await waitFor(() =>
      expect(copyPathToClipboard).toHaveBeenCalledWith("/skill/SKILL.md"),
    );
  });
});
