// Docs terminal blocks, drawn as the docs sites we compared draw theirs (Vercel, Next.js, uv, Bun): the window named
// in its header ("Terminal", the copy button beside it, styled in starlight.css) rather than three grey dots, and a
// block that shows a session told apart from one that only lists commands. A session is a block where some line starts
// with "$ ": those lines are commands, the prompt shown muted and never copied; every other line is what they print,
// plain text, not coloured as shell. The copy button copies the commands alone.
import { addClassName, definePlugin, isInlineStyleAnnotation, LanguageGroups } from "@astrojs/starlight/expressive-code";
import { h, select } from "@astrojs/starlight/expressive-code/hast";

/** The header's name for the window, by the page's language (Starlight's lang for each docs locale). */
export const TERMINAL_LABEL = { en: "Terminal", "zh-CN": "终端" };
/** A PowerShell block names its shell instead, so a Windows tab needs no "(PowerShell)" (getting-started/web.mdx). */
const POWERSHELL = ["powershell", "ps", "ps1"];
const PROMPT = "$ ";

/** The line's part in a session, from the moment its prompt is taken out until it is drawn. */
const roles = new WeakMap();

// Expressive Code's own rule (plugin-frames): a terminal frame unless the block asks for another, and a shell
// language unless it starts with a shebang, which the frames plugin has already turned into a code frame.
const isTerminal = ({ props, language }) =>
  props.frame === "terminal" || ((props.frame ?? "auto") === "auto" && LanguageGroups.terminal.includes(language));

export function pluginTerminal() {
  return definePlugin({
    name: "HarnessKit terminal",
    hooks: {
      preprocessCode: ({ codeBlock, locale }) => {
        if (!isTerminal(codeBlock)) return;
        // A visible name, which also stands in for the frames plugin's screen-reader-only "Terminal window".
        codeBlock.props.title ??= POWERSHELL.includes(codeBlock.language) ? "PowerShell" : (TERMINAL_LABEL[locale] ?? TERMINAL_LABEL.en);
        const lines = codeBlock.getLines();
        if (!lines.some((line) => line.text.startsWith(PROMPT))) return;
        for (const line of lines) {
          if (line.text.startsWith(PROMPT)) {
            line.editText(0, PROMPT.length, "");
            roles.set(line, "command");
          } else if (line.text.trim()) {
            roles.set(line, "output");
          }
        }
      },
      // Output keeps no syntax colours: the shell grammar would read "HarnessKit Web UI …" as a command.
      postprocessAnnotations: ({ codeBlock }) => {
        for (const line of codeBlock.getLines()) {
          if (roles.get(line) !== "output") continue;
          for (const annotation of line.getAnnotations()) {
            if (isInlineStyleAnnotation(annotation)) line.deleteAnnotation(annotation);
          }
        }
      },
      postprocessRenderedLine: ({ line, renderData }) => {
        const role = roles.get(line);
        if (!role) return;
        addClassName(renderData.lineAst, role);
        if (role !== "command") return;
        // The prompt as its own element: muted, skipped by text selection and screen readers.
        const code = select(".code", renderData.lineAst);
        code?.children.unshift(h("span", { className: "prompt", "aria-hidden": "true" }, PROMPT));
      },
      postprocessRenderedBlock: ({ codeBlock, renderData }) => {
        const commands = codeBlock.getLines().filter((line) => roles.get(line) === "command");
        if (!commands.length) return;
        const button = select("button[data-code]", renderData.blockAst);
        // The frames plugin joins lines with U+007F, which its script turns back into newlines.
        if (button) button.properties.dataCode = commands.map((line) => line.text).join("\x7F");
      },
    },
  });
}
