import { clsx } from "clsx";
import type { ExtensionKind } from "@/lib/types";

const kindStyles: Record<ExtensionKind, string> = {
  skill: "bg-kind-skill/15 text-kind-skill ring-kind-skill/25",
  mcp: "bg-kind-mcp/15 text-kind-mcp ring-kind-mcp/25",
  plugin: "bg-kind-plugin/15 text-kind-plugin ring-kind-plugin/25",
  hook: "bg-kind-hook/15 text-kind-hook ring-kind-hook/25",
  cli: "bg-kind-cli/15 text-kind-cli ring-kind-cli/25",
};

const kindLabel: Record<ExtensionKind, string> = {
  skill: "skill",
  mcp: "MCP",
  plugin: "plugin",
  hook: "hook",
  cli: "CLI",
};

const kindTitle: Record<ExtensionKind, string> = {
  skill: "Reusable prompt instructions for AI agents",
  mcp: "Model Context Protocol server — extends agent capabilities",
  plugin: "Agent-specific plugin extension",
  hook: "Shell command triggered by agent events",
  cli: "Agent-oriented CLI tool — binary + skills bundle",
};

export function KindBadge({
  kind,
  noTooltip = false,
}: {
  kind: ExtensionKind;
  /** Suppress the native browser title tooltip. */
  noTooltip?: boolean;
}) {
  return (
    <span
      title={noTooltip ? undefined : kindTitle[kind]}
      className={clsx(
        "rounded-full px-2.5 py-0.5 text-xs font-medium ring-1 ring-inset transition-colors duration-150",
        kindStyles[kind],
      )}
    >
      {kindLabel[kind]}
    </span>
  );
}
