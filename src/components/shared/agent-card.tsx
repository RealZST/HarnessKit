import { useState, useCallback } from "react";
import { AgentMascot } from "./agent-mascot/agent-mascot";
import { agentDisplayName, type AgentInfo } from "@/lib/types";

interface AgentCardProps {
  agent: AgentInfo;
}

export function AgentCard({ agent }: AgentCardProps) {
  const [isHovered, setIsHovered] = useState(false);
  const [isClicked, setIsClicked] = useState(false);

  const handleClick = useCallback(() => {
    setIsClicked(true);
    setTimeout(() => setIsClicked(false), 600);
  }, []);

  return (
    <button
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      onClick={handleClick}
      className="group flex w-[110px] flex-col items-center gap-1.5 overflow-visible rounded-lg border border-border/60 bg-card/50 px-3 py-2.5 text-center transition-all duration-200 hover:border-border hover:bg-card hover:shadow-sm"
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
          {agent.extension_count} ext.
        </span>
      </div>
    </button>
  );
}
