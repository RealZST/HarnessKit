import { useCallback, useState } from "react";
import { type AgentInfo, agentDisplayName } from "@/lib/types";
import { AgentMascot } from "./agent-mascot/agent-mascot";

interface AgentCardProps {
  agent: AgentInfo;
}

const CLICK_DURATIONS: Partial<Record<AgentInfo["name"], number>> = {
  claude: 4200,
  windsurf: 1800,
  opencode: 4000,
  antigravity: 800,
  kiro: 1100,
  omp: 900,
  dsh: 1200,
  // the longest of grok's three click tracks is the 2.05s upright-and-unwind;
  // anything shorter pulls the class before the mark has turned back to its
  // brand angle and it snaps the last 45deg
  grok: 2100,
};

export function AgentCard({ agent }: AgentCardProps) {
  const [isHovered, setIsHovered] = useState(false);
  const [isClicked, setIsClicked] = useState(false);

  const handleClick = useCallback(() => {
    setIsClicked(true);
    const duration = CLICK_DURATIONS[agent.name] ?? 600;
    setTimeout(() => setIsClicked(false), duration);
  }, [agent.name]);

  return (
    <button
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      onClick={handleClick}
      className={`group flex w-[110px] flex-col items-center gap-1.5 rounded-lg border border-border/60 bg-card/50 px-3 py-2.5 text-center transition-all duration-200 hover:border-border hover:bg-card hover:shadow-sm hover:-translate-y-0.5 ${agent.name === "codex" || agent.name === "antigravity" || agent.name === "claude" || agent.name === "opencode" || agent.name === "dsh" ? "overflow-hidden" : "overflow-visible"}`}
    >
      {/* dsh dives below its box on hover; clip around the mascot (with
          top headroom for the leap and the spout) so the whale vanishes
          above the label instead of sliding over the text */}
      <div
        className={
          agent.name === "dsh"
            ? "-mx-3 -mt-4 overflow-hidden px-3 pt-4"
            : undefined
        }
      >
        <AgentMascot
          name={agent.name}
          size={36}
          animated={isHovered}
          clicked={isClicked}
        />
      </div>
      <div className="min-w-0">
        <span className="block whitespace-nowrap text-sm font-medium text-foreground">
          {agentDisplayName(agent.name)}
        </span>
        <span className="block text-xs text-muted-foreground">
          <span className="font-semibold">{agent.extension_count}</span> ext.
        </span>
      </div>
    </button>
  );
}
