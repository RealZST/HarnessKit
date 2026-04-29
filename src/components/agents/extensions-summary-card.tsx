import { ArrowRight, Package } from "lucide-react";
import { useNavigate } from "react-router-dom";
import type { ExtensionCounts } from "@/lib/types";

export function ExtensionsSummaryCard({
  counts,
  agentName,
}: {
  counts: ExtensionCounts;
  agentName: string;
}) {
  const navigate = useNavigate();
  const total =
    counts.skill + counts.mcp + counts.plugin + counts.hook + counts.cli;
  if (total === 0) return null;

  return (
    <div className="mb-5" id="section-extensions">
      <div className="flex items-center gap-2 mb-2 px-1">
        <Package size={14} className="text-muted-foreground" />
        <span className="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
          Extensions
        </span>
        <span className="text-[10px] bg-muted px-1.5 py-0.5 rounded-full text-muted-foreground">
          {total}
        </span>
      </div>
      <button
        onClick={() => navigate(`/extensions?agent=${agentName}`)}
        className="w-full rounded-lg border border-border p-3.5 flex items-center justify-between transition-colors hover:bg-accent/30"
      >
        <div className="flex gap-4 text-[13px]">
          {counts.skill > 0 && (
            <span>
              <strong>{counts.skill}</strong>{" "}
              <span className="text-muted-foreground">Skills</span>
            </span>
          )}
          {counts.mcp > 0 && (
            <span>
              <strong>{counts.mcp}</strong>{" "}
              <span className="text-muted-foreground">MCP</span>
            </span>
          )}
          {counts.plugin > 0 && (
            <span>
              <strong>{counts.plugin}</strong>{" "}
              <span className="text-muted-foreground">Plugins</span>
            </span>
          )}
          {counts.hook > 0 && (
            <span>
              <strong>{counts.hook}</strong>{" "}
              <span className="text-muted-foreground">Hooks</span>
            </span>
          )}
          {counts.cli > 0 && (
            <span>
              <strong>{counts.cli}</strong>{" "}
              <span className="text-muted-foreground">CLIs</span>
            </span>
          )}
        </div>
        <span className="flex items-center gap-1 text-[12px] font-medium text-primary">
          View in Extensions <ArrowRight size={14} />
        </span>
      </button>
    </div>
  );
}
