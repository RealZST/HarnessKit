// Where each App mascot draws inside its square, as a fraction of its size from the square's middle to the ink's
// (negative: above). The App's mascots leave room under the drawing for their moves, so on a line beside text they
// ride high by a tenth of their size. Measured by scripts/measure-mascot-ink.mjs; re-run it when the App's mascots
// change. Keyed by agent id (src/data/agents.ts).
export const MASCOT_INK_OFFSET: Readonly<Record<string, number>> = {
  claude: -0.1,
  codex: -0.031,
  gemini: -0.097,
  cursor: -0.114,
  antigravity: -0.117,
  copilot: -0.097,
  windsurf: -0.1,
  opencode: -0.11,
  hermes: -0.1,
  kiro: -0.058,
  omp: -0.097,
  dsh: -0.103,
  grok: -0.1,
  "qoder-cn": -0.093,
  openclaw: -0.076,
  penguin: -0.107,
  pi: -0.093,
};

/** The CSS `translate` that puts a `size`px mascot's drawing on the middle of its square, for one on a line of text. */
export function centerInk(agent: string, size: number): string {
  return `0 ${(-(MASCOT_INK_OFFSET[agent] ?? 0) * size).toFixed(1)}px`;
}
