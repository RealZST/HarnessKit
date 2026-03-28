import "./mascot.css";
import { ClaudeMascot } from "./claude-mascot";
import { CursorMascot } from "./cursor-mascot";
import { CodexMascot } from "./codex-mascot";
import { GeminiMascot } from "./gemini-mascot";
import { FallbackMascot } from "./fallback-mascot";

interface AgentMascotProps {
  name: string;
  size?: number;
  animated?: boolean;
  clicked?: boolean;
}

const MASCOT_MAP: Record<string, { component: React.ComponentType<{ size: number }>; className: string; scale: number; offsetY?: number }> = {
  claude: { component: ClaudeMascot, className: "mascot-claude", scale: 1 },
  cursor: { component: CursorMascot, className: "mascot-cursor", scale: 1.15, offsetY: 2 },
  codex: { component: CodexMascot, className: "mascot-codex", scale: 1.6, offsetY: 3 },
  gemini: { component: GeminiMascot, className: "mascot-gemini", scale: 1.2 },
};

export function AgentMascot({ name, size = 48, animated = false, clicked = false }: AgentMascotProps) {
  const entry = MASCOT_MAP[name];
  const Comp = entry?.component ?? FallbackMascot;
  const baseClass = entry?.className ?? "mascot-fallback";
  const renderSize = size * (entry?.scale ?? 1);

  const classes = [baseClass, animated && "is-animated", clicked && "is-clicked"].filter(Boolean).join(" ");

  return (
    <div className={classes} style={{ width: size, height: size, display: "flex", alignItems: "center", justifyContent: "center", overflow: "visible" }}>
      <div style={{ flexShrink: 0, transform: entry?.offsetY ? `translateY(${entry.offsetY}px)` : undefined }}>
        <Comp size={renderSize} />
      </div>
    </div>
  );
}
