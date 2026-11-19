import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { AgentInfo, ConfigScope, RepositoryItem } from "@/lib/types";
import { useAgentStore } from "@/stores/agent-store";
import { useScopeStore } from "@/stores/scope-store";
import { InstallItemDialog } from "../install-item-dialog";

vi.mock("@/lib/invoke", () => ({ api: {} }));

const project: ConfigScope = { type: "project", name: "proj", path: "/proj" };
const agent = (name: string) =>
  ({ name, detected: true, enabled: true }) as AgentInfo;

const item: RepositoryItem = {
  item_id: "pdf",
  name: "pdf",
  description: "",
  ignored: false,
  is_new: false,
  installed: [
    { extension_id: "e1", agent: "claude", scope: { type: "global" } },
    { extension_id: "e2", agent: "codex", scope: project },
  ],
};

describe("InstallItemDialog", () => {
  it("locks only the agents that have the skill in the target scope", () => {
    useAgentStore.setState({ agents: [agent("claude"), agent("codex")] });
    useScopeStore.setState({ current: { type: "global" }, hydrated: true });
    render(<InstallItemDialog repoId="r1" item={item} onClose={() => {}} />);

    const claude = screen.getByRole("checkbox", { name: /Claude Code/ });
    expect(claude).toBeChecked();
    expect(claude).toBeDisabled();
    expect(screen.getByRole("checkbox", { name: "Codex" })).toBeEnabled();
  });
});
