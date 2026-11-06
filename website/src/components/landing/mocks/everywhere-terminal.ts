// The Everywhere picture's fourth way in: HarnessKit's `hk` command in a terminal. A short session types itself out,
// as `hk` prints it (crates/hk-cli/src/main.rs, with its colours): the status of the same four agents and 43
// extensions as the App's Overview beside it, the skills on one agent, then a skill switched off and another on. The
// machine, the skills and their sources are made up; the commands, flags and output layout are the CLI's own.

/** One run of text in a line, with the colour `hk` gives it. */
export interface Run {
  text: string;
  tone?: "dim" | "bold" | "ok" | "warn" | "bad";
}

export interface TermLine {
  /** A prompt with this command typed into it; otherwise a line of output. */
  command?: string;
  runs: readonly Run[];
  /** When the line shows, in ms from the terminal coming into view. */
  at: number;
  /** For a command: how long it takes to type. */
  typeMs?: number;
}

/** The terminal's width in columns, as a default terminal window's; the longest line fits it. */
export const TERM_COLS = 80;
/** How long the terminal stays on before the tour moves on (everywhere-tour.ts): the session, then a rest. */
export const CLI_STEP_MS = 9000;

// The same agents and counts as the App's Overview in the picture (EverywherePanel.astro): its status line reads
// "04 agents / 43 extensions" with these kinds.
export const AGENT_IDS = ["claude", "codex", "gemini", "cursor"] as const;
export const KIND_COUNTS = { skill: 24, mcp: 9, plugin: 5, hook: 3, cli: 2 } as const;

const t = (text: string, tone?: Run["tone"]): Run => (tone ? { text, tone } : { text });
const total = Object.values(KIND_COUNTS).reduce((sum, n) => sum + n, 0);

// `hk list` draws its table with comfy-table's UTF8_FULL_CONDENSED preset: a cell is padded by a space each side, the
// header is set off by a double rule, and the score and status are coloured (format_score; enabled green, disabled
// red). A Trust Score of 80 or more is Safe (green), 60 to 79 Low Risk (yellow).
type Cell = string | Run;
const SKILLS: readonly Cell[][] = [
  ["code-review", "skill", "claude, cursor", "anthropics/skills", t("100", "ok"), t("enabled", "ok")],
  ["pdf-tools", "skill", "codex, cursor", "—", t("77", "warn"), t("enabled", "ok")],
  ["web-search", "skill", "cursor", "—", t("92", "ok"), t("disabled", "bad")],
];
const HEADER: readonly Cell[] = ["Name", "Kind", "Agent", "Source", "Score", "Status"];
const textOf = (cell: Cell) => (typeof cell === "string" ? cell : cell.text);
const widths = HEADER.map((_, col) => Math.max(...[HEADER, ...SKILLS].map((row) => textOf(row[col]).length)));
const rule = (left: string, fill: string, cross: string, right: string): Run[] =>
  [t(left + widths.map((w) => fill.repeat(w + 2)).join(cross) + right)];
const row = (cells: readonly Cell[], header = false): Run[] => [
  t("│ "),
  ...cells.flatMap((cell, col) => {
    const run = typeof cell === "string" ? t(cell, header ? "bold" : undefined) : cell;
    const pad = " ".repeat(widths[col] - run.text.length);
    return [run, t(pad + (col < cells.length - 1 ? " ┆ " : " │"))];
  }),
];

/** The session, in order: each command, then what it prints. */
const SESSION: { command: string; output: Run[][] }[] = [
  {
    command: "hk status",
    output: [
      [],
      [t("  "), t("Agents", "dim"), t(`        ${AGENT_IDS.length} enabled (${AGENT_IDS.join(" · ")})`)],
      [t("  "), t("Extensions", "dim"), t(`    ${total} total (${KIND_COUNTS.skill} skills · ${KIND_COUNTS.mcp} mcp · ${KIND_COUNTS.plugin} plugins · ${KIND_COUNTS.hook} hooks · ${KIND_COUNTS.cli} clis)`)],
    ],
  },
  {
    command: "hk list --kind skill --agent cursor",
    output: [
      [],
      [t("  "), t(String(SKILLS.length), "bold"), t(" "), t("results", "dim")],
      rule("┌", "─", "┬", "┐"),
      row(HEADER, true),
      rule("╞", "═", "╪", "╡"),
      ...SKILLS.map((cells) => row(cells)),
      rule("└", "─", "┴", "┘"),
    ],
  },
  { command: "hk disable pdf-tools", output: [[t("Disabled", "ok"), t(" pdf-tools")]] },
  { command: "hk enable web-search", output: [[t("Enabled", "ok"), t(" web-search")]] },
];

// The pace: a person typing, the output a beat after Enter, each line of it a moment after the last, and a pause
// before the next command.
const TYPE_PER_CHAR_MS = 38;
const BEFORE_TYPING_MS = 250;
const AFTER_ENTER_MS = 200;
const PER_OUTPUT_LINE_MS = 45;
const BEFORE_NEXT_PROMPT_MS = 450;

/** The whole session as lines, each with the moment it shows; it ends on an empty prompt. */
export function terminalLines(): TermLine[] {
  const lines: TermLine[] = [];
  let now = 0;
  for (const { command, output } of SESSION) {
    const typeMs = command.length * TYPE_PER_CHAR_MS;
    lines.push({ command, runs: [], at: now, typeMs });
    now += BEFORE_TYPING_MS + typeMs + AFTER_ENTER_MS;
    for (const runs of output) {
      lines.push({ runs, at: now });
      now += PER_OUTPUT_LINE_MS;
    }
    now += BEFORE_NEXT_PROMPT_MS;
  }
  lines.push({ command: "", runs: [], at: now });
  return lines;
}

/** Typing starts this long after its prompt shows. */
export const TYPING_DELAY_MS = BEFORE_TYPING_MS;
/** A line's text, as a terminal shows it. */
export const lineText = (line: TermLine) => line.runs.map((run) => run.text).join("");
