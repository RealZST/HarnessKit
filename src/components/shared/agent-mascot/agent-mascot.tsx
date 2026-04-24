import "./mascot.css";
import { AntigravityMascot } from "./antigravity-mascot";
import { ClaudeMascot } from "./claude-mascot";
import { CodexMascot } from "./codex-mascot";
import { CopilotMascot } from "./copilot-mascot";
import { CursorMascot } from "./cursor-mascot";
import { FallbackMascot } from "./fallback-mascot";
import { GeminiMascot } from "./gemini-mascot";
import { WindsurfMascot } from "./windsurf-mascot";

interface AgentMascotProps {
  name: string;
  size?: number;
  animated?: boolean;
  clicked?: boolean;
}

const MASCOT_MAP: Record<
  string,
  {
    component: React.ComponentType<{ size: number; clicked?: boolean }>;
    className: string;
    scale: number;
    offsetY?: number;
    passClicked?: boolean;
  }
> = {
  claude: { component: ClaudeMascot, className: "mascot-claude", scale: 1 },
  cursor: { component: CursorMascot, className: "mascot-cursor", scale: 1.15 },
  codex: {
    component: CodexMascot,
    className: "mascot-codex",
    scale: 0.8,
    offsetY: -1,
  },
  gemini: { component: GeminiMascot, className: "mascot-gemini", scale: 1.2 },
  antigravity: {
    component: AntigravityMascot,
    className: "mascot-antigravity",
    scale: 0.85,
    passClicked: true,
  },
  copilot: {
    component: CopilotMascot,
    className: "mascot-copilot",
    scale: 0.95,
    passClicked: true,
  },
  windsurf: {
    component: WindsurfMascot,
    className: "mascot-windsurf",
    scale: 1,
  },
};

export function AgentMascot({
  name,
  size = 48,
  animated = false,
  clicked = false,
}: AgentMascotProps) {
  const entry = MASCOT_MAP[name];
  const Comp = entry?.component ?? FallbackMascot;
  const baseClass = entry?.className ?? "mascot-fallback";
  const renderSize = size * (entry?.scale ?? 1);

  const classes = [
    baseClass,
    animated && "is-animated",
    clicked && "is-clicked",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div
      className={classes}
      style={{
        width: size,
        height: size,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        overflow: "visible",
      }}
    >
      <div
        style={{
          flexShrink: 0,
          transform: entry?.offsetY
            ? `translateY(${entry.offsetY}px)`
            : undefined,
        }}
      >
        <Comp
          size={renderSize}
          clicked={entry?.passClicked ? clicked : undefined}
        />
      </div>
    </div>
  );
}
