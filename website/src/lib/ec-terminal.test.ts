// @vitest-environment node
import { ExpressiveCode } from "@astrojs/starlight/expressive-code";
import { selectAll, select, toText } from "@astrojs/starlight/expressive-code/hast";
import { describe, expect, it } from "vitest";
import { pluginTerminal } from "./ec-terminal.mjs";

const engine = new ExpressiveCode({ plugins: [pluginTerminal()] });

async function render(code: string, language = "bash", locale = "en") {
  const { renderedGroupAst } = await engine.render({ code, language, locale });
  const lines = selectAll(".ec-line", renderedGroupAst);
  return {
    title: toText(select(".title", renderedGroupAst) ?? { type: "root", children: [] }),
    screenReaderTitle: select(".sr-only", renderedGroupAst),
    copied: String(select("button[data-code]", renderedGroupAst)?.properties.dataCode ?? "").replace(/\x7f/g, "\n"),
    lines: lines.map((line) => ({
      text: toText(line),
      classes: line.properties.className as string[],
      prompt: select(".prompt", line),
      coloured: selectAll("span[style]", line).length > 0,
    })),
  };
}

describe("docs terminal blocks", () => {
  it("names the window in its header, in the page's language, or names PowerShell", async () => {
    expect((await render("hk status")).title).toBe("Terminal");
    expect((await render("hk status", "bash", "zh-CN")).title).toBe("终端");
    expect((await render("irm https://example.com/install.ps1 | iex", "powershell", "zh-CN")).title).toBe("PowerShell");
    // The visible name replaces the frames plugin's hidden "Terminal window".
    expect((await render("hk status")).screenReaderTitle).toBeUndefined();
  });

  it("keeps a title the block gives itself, and leaves other code alone", async () => {
    const titled = await engine.render({ code: "hk serve", language: "bash", meta: 'title="Server"' });
    expect(toText(select(".title", titled.renderedGroupAst)!)).toBe("Server");
    expect((await render('{ "a": 1 }', "json")).title).toBe("");
  });

  it("shows a session's prompt apart from its command, and its output as plain text", async () => {
    const { lines } = await render("$ hk serve\nHarnessKit Web UI [my-host] running at http://127.0.0.1:7070/");
    expect(lines[0].prompt).toBeDefined();
    expect(lines[0].classes).toContain("command");
    expect(lines[0].text).toBe("$ hk serve");
    expect(lines[1].classes).toContain("output");
    expect(lines[1].prompt).toBeUndefined();
    expect(lines[1].coloured).toBe(false);
  });

  it("copies a session's commands without the prompt or the output", async () => {
    const { copied } = await render("$ hk status\n  Agents        7 enabled\n\n$ hk list --kind skill    # skills only");
    expect(copied).toBe("hk status\nhk list --kind skill    # skills only");
  });

  it("leaves a block of commands without prompts as it is", async () => {
    const { lines, copied } = await render("hk enable <NAME>\nhk disable <NAME>");
    expect(lines.every((line) => !line.prompt && !line.classes.includes("output"))).toBe(true);
    expect(copied).toBe("hk enable <NAME>\nhk disable <NAME>");
  });
});
