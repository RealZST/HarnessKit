import type { Agent } from "./agents";

// The App's Overview card (src/components/shared/agent-card.tsx), mirrored by the landing's live Overview
// (components/landing/OverviewLive.tsx). agent-card.test.ts reads the App's source, so a change there fails here.

export const CARD_WIDTH = 110;
export const CARD_MASCOT_SIZE = 36;
export const CARD_PADDING = { x: 12, y: 10 };

// How long a click keeps `clicked` on: each mascot's click animation runs to its end before the class goes.
export const CLICK_DURATIONS: Readonly<Record<string, number>> = {
  claude: 4200, windsurf: 1800, opencode: 4000, antigravity: 800, kiro: 1100, omp: 900, dsh: 1200,
  grok: 2100, "qoder-cn": 2850, openclaw: 3050, penguin: 1650, pi: 3200,
};
export const DEFAULT_CLICK_MS = 600;
export const clickDuration = (id: string) => CLICK_DURATIONS[id] ?? DEFAULT_CLICK_MS;

// Cards that clip to their own box: these mascots' hover motion leaves it (codex rolls sideways, and so on).
export const CLIPPED_CARDS: ReadonlySet<string> = new Set(["codex", "antigravity", "claude", "opencode", "dsh"]);

// A clip around the mascot alone, out to the card's sides and `top` px above the mascot. The dsh whale dives
// below its box and must vanish above the label; pi's tetris pieces fall in from the card's top edge.
export const MASCOT_CLIPS: Readonly<Record<string, { top: number }>> = { dsh: { top: 16 }, pi: { top: 10 } };

// The App's card label (AGENT_DISPLAY_NAMES in src/lib/types.ts) is shorter than the full name for two agents.
const SHORT_NAMES: Readonly<Record<string, string>> = { dsh: "DeepSeek", penguin: "Penguin" };
export const cardName = (agent: Agent) => SHORT_NAMES[agent.id] ?? agent.name;
