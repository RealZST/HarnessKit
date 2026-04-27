import { useCallback, useState } from "react";
import { type AgentInfo, agentDisplayName } from "@/lib/types";
import { AgentMascot } from "./agent-mascot/agent-mascot";

interface AgentCardProps {
  agent: AgentInfo;
}

export function AgentCard({ agent }: AgentCardProps) {
  const [isHovered, setIsHovered] = useState(false);
  const [isClicked, setIsClicked] = useState(false);

  const handleClick = useCallback(() => {
    setIsClicked(true);
    const duration =
      agent.name === "claude"
        ? 4200
        : agent.name === "windsurf"
          ? 1800
          : agent.name === "antigravity"
            ? 800
            : 600;
    setTimeout(() => setIsClicked(false), duration);
  }, [agent.name]);

  return (
    <button
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      onClick={handleClick}
      className={`group flex w-[110px] flex-col items-center gap-1.5 rounded-lg border border-border/60 bg-card/50 px-3 py-2.5 text-center transition-all duration-200 hover:border-border hover:bg-card hover:shadow-sm hover:-translate-y-0.5 ${agent.name === "codex" || agent.name === "antigravity" || agent.name === "claude" ? "overflow-hidden" : "overflow-visible"}`}
    >
      <AgentMascot
        name={agent.name}
        size={36}
        animated={isHovered}
        clicked={isClicked}
      />
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
