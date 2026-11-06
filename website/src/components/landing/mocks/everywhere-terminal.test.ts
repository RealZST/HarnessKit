// @vitest-environment node
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { CLI_STEP_MS, lineText, TERM_COLS, terminalLines, TYPING_DELAY_MS } from "./everywhere-terminal";

// The prompt before each command, as the picture draws it (EverywherePanel.astro).
const PROMPT = "➜  ~ ";
// Through a variable: Vite would turn `new URL(…, import.meta.url)` into an asset import.
const here = import.meta.url;
const cliSource = readFileSync(new URL("../../../../../crates/hk-cli/src/main.rs", here), "utf8");
const lines = terminalLines();
const commands = lines.filter((line) => line.command).map((line) => line.command!);

describe("the terminal in the Everywhere picture", () => {
  it("fits every line in the terminal's width", () => {
    for (const line of lines) {
      const text = line.command !== undefined ? PROMPT + line.command : lineText(line);
      expect([...text].length, text).toBeLessThanOrEqual(TERM_COLS);
    }
  });

  it("types only commands and flags the hk CLI has", () => {
    expect(commands.length).toBeGreaterThan(1);
    for (const command of commands) {
      const [bin, sub, ...args] = command.split(" ");
      expect(bin).toBe("hk");
      // clap names each subcommand after its Commands variant: `hk list` is `List {`, `hk status` is `Status`.
      expect(cliSource, sub).toMatch(new RegExp(`^\\s+${sub[0].toUpperCase()}${sub.slice(1)}\\b`, "m"));
      for (const flag of args.filter((arg) => arg.startsWith("--"))) {
        expect(cliSource, flag).toMatch(new RegExp(`^\\s+${flag.slice(2)}: `, "m"));
      }
    }
  });

  it("draws the skills table with its columns lined up", () => {
    const table = lines.map(lineText).filter((text) => /^[┌│╞└]/.test(text));
    expect(table.length).toBeGreaterThan(4);
    const width = [...table[0]].length;
    for (const text of table) expect([...text].length, text).toBe(width);
  });

  it("shows each line after the one before, and rests before the tour moves on", () => {
    for (let i = 1; i < lines.length; i++) expect(lines[i].at).toBeGreaterThan(lines[i - 1].at);
    const last = lines[lines.length - 1];
    expect(last.command).toBe("");
    expect(CLI_STEP_MS - last.at).toBeGreaterThanOrEqual(1500);
  });

  it("finishes typing each command before its output shows", () => {
    lines.forEach((line, i) => {
      if (!line.command || i === lines.length - 1) return;
      expect(lines[i + 1].at).toBeGreaterThan(line.at + TYPING_DELAY_MS + line.typeMs!);
    });
  });
});
