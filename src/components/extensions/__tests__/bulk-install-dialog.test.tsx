import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentInfo, Extension, GroupedExtension } from "@/lib/types";
import { useAgentStore } from "@/stores/agent-store";
import { useExtensionStore } from "@/stores/extension-store";
import { useScopeStore } from "@/stores/scope-store";
import { BulkInstallDialog } from "../bulk-install-dialog";

const installToAgent = vi.fn();
vi.mock("@/lib/invoke", () => ({
  api: {
    installToAgent: (...args: unknown[]) => installToAgent(...args),
    listHermesCategories: vi.fn().mockResolvedValue([]),
  },
}));

function ext(over: Partial<Extension> & { id: string }): Extension {
  return {
    kind: "skill",
    name: "pdf",
    description: "",
    source: { origin: "local", url: null, version: null, commit_hash: null },
    agents: ["claude"],
    tags: [],
    pack: null,
    permissions: [],
    enabled: true,
    trust_score: null,
    installed_at: "2026-01-01",
    updated_at: "2026-01-01",
    source_path: null,
    cli_parent_id: null,
    cli_meta: null,
    install_meta: null,
    scope: { type: "global" },
    ...over,
  };
}

function agent(name: string): AgentInfo {
  return {
    name,
    detected: true,
    extension_count: 0,
    path: "",
    enabled: true,
    capabilities: {
      project_install: { skill: true, mcp: true, hook: true, cli: true },
      hooks_supported: true,
      global_hook_install: true,
    },
  };
}

function groupOf(instances: Extension[]): GroupedExtension {
  return {
    groupKey: "pdf",
    name: "pdf",
    kind: "skill",
    description: "",
    source: { origin: "local", url: null, version: null, commit_hash: null },
    agents: [...new Set(instances.flatMap((i) => i.agents))],
    tags: [],
    pack: null,
    permissions: [],
    enabled: true,
    trust_score: null,
    installed_at: "2026-01-01",
    updated_at: "2026-01-01",
    instances,
  };
}

const onClaude = ext({ id: "pdf-claude" });

beforeEach(() => {
  installToAgent.mockReset().mockResolvedValue("new-id");
  useAgentStore.setState({
    agents: [agent("claude"), agent("codex")],
    agentOrder: ["claude", "codex"],
  } as never);
  useExtensionStore.setState({
    extensions: [onClaude],
    // The real rescan would surface the new copy; the results list must
    // keep reporting it as installed, not re-plan it as already there.
    rescanAndFetch: vi.fn(async () => {
      useExtensionStore.setState({
        extensions: [onClaude, ext({ id: "pdf-codex", agents: ["codex"] })],
      });
    }),
  } as never);
});

describe("BulkInstallDialog", () => {
  it("runs the plan for the ticked agents and reports what landed and what failed", async () => {
    useScopeStore.setState({ current: { type: "global" }, hydrated: true });
    useAgentStore.setState({
      agents: [agent("claude"), agent("codex"), agent("gemini")],
      agentOrder: ["claude", "codex", "gemini"],
    } as never);
    installToAgent
      .mockResolvedValueOnce("pdf")
      .mockRejectedValueOnce('{"kind":"Io","message":"Permission denied"}');
    render(
      <BulkInstallDialog
        groups={[groupOf([onClaude])]}
        onClose={() => {}}
        onDone={() => {}}
      />,
    );

    // Defaults: the active scope, nothing ticked, nothing to install.
    expect(
      screen.getByRole("combobox", { name: "Install to scope" }),
    ).toHaveValue("global");
    expect(screen.getByRole("button", { name: "Install (0)" })).toBeDisabled();

    await userEvent.click(screen.getByRole("checkbox", { name: "Codex" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Gemini CLI" }));
    await userEvent.click(screen.getByRole("button", { name: "Install (2)" }));

    await screen.findByRole("button", { name: "Done" });
    expect(installToAgent).toHaveBeenCalledTimes(2);
    expect(installToAgent).toHaveBeenCalledWith(
      "pdf-claude",
      "codex",
      { type: "global" },
      undefined,
    );
    expect(screen.getByText("1 installed")).toBeInTheDocument();
    expect(screen.getByText(/1 failed/)).toBeInTheDocument();
    expect(screen.getByText("Permission denied")).toBeInTheDocument();
  });

  it("re-installs onto agents that already have a copy when overwrite is on", async () => {
    useScopeStore.setState({ current: { type: "global" }, hydrated: true });
    const onCodex = ext({
      id: "pdf-codex",
      agents: ["codex"],
      updated_at: "2025-01-01",
    });
    useExtensionStore.setState({ extensions: [onClaude, onCodex] } as never);
    render(
      <BulkInstallDialog
        groups={[groupOf([onClaude, onCodex])]}
        onClose={() => {}}
        onDone={() => {}}
      />,
    );
    const codex = screen.getByRole("checkbox", { name: "Codex" });
    await userEvent.click(codex);
    expect(screen.getByRole("button", { name: "Install (0)" })).toBeDisabled();
    await userEvent.click(
      screen.getByRole("checkbox", { name: /Overwrite existing copies/ }),
    );
    await userEvent.click(screen.getByRole("button", { name: "Install (1)" }));
    await screen.findByRole("button", { name: "Done" });
    // Copied from the newest copy (Claude's), never from Codex's own.
    expect(installToAgent).toHaveBeenCalledWith(
      "pdf-claude",
      "codex",
      { type: "global" },
      undefined,
    );
  });

  it("says where a bundle went when all its parts were selected on their own", async () => {
    useScopeStore.setState({ current: { type: "global" }, hydrated: true });
    const cli = ext({ id: "cli-1", kind: "cli", name: "gh", pack: "gh" });
    const part = ext({
      id: "gh-skill",
      name: "gh-skill",
      pack: "gh",
      cli_parent_id: "cli-1",
    });
    useExtensionStore.setState({ extensions: [cli, part] } as never);
    const bundle = {
      ...groupOf([cli]),
      groupKey: "cli-gh",
      name: "gh",
      kind: "cli" as const,
      pack: "gh",
    };
    const partRow = {
      ...groupOf([part]),
      groupKey: "skill-gh",
      name: "gh-skill",
      pack: "gh",
    };
    render(
      <BulkInstallDialog
        groups={[bundle, partRow]}
        onClose={() => {}}
        onDone={() => {}}
      />,
    );
    await userEvent.click(screen.getByRole("checkbox", { name: "Codex" }));
    await userEvent.click(screen.getByRole("button", { name: "Install (1)" }));
    await screen.findByRole("button", { name: "Done" });
    // No skips were counted, yet the bundle's line is there.
    expect(screen.queryByRole("button", { name: /skipped/ })).toBeNull();
    expect(screen.getByText("covered by gh-skill")).toBeInTheDocument();
  });

  it("keeps every tile disabled in All mode until a target is picked", () => {
    useScopeStore.setState({ current: { type: "all" }, hydrated: true });
    render(
      <BulkInstallDialog
        groups={[groupOf([onClaude])]}
        onClose={() => {}}
        onDone={() => {}}
      />,
    );
    for (const name of ["Claude Code", "Codex"]) {
      expect(screen.getByRole("checkbox", { name })).toBeDisabled();
    }
  });
});
