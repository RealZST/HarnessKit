import { Check, Copy } from "lucide-react";
import { Fragment, useEffect, useRef, useState, useSyncExternalStore } from "react";
import { INSTALL_PS1, INSTALL_PS1_SCRIPT_URL, INSTALL_SH, INSTALL_SCRIPT_URL } from "../../data/agents";
import { renderInline } from "../../lib/inline-code";
import { announceWayUsed, onWayLeft } from "./way-used";

type Shell = "sh" | "ps";
type PerShell = Record<Shell, string>;
interface T {
  /** The row's heading, where nothing else names it: what it is, and what it's for. */
  kind?: string; kindDesc?: string;
  shells: PerShell; shellsLabel: string; next: PerShell; copy: string; copyDone: string;
  viewScript: string; copyKeys: { mac: string; other: string };
}

const SHELLS: readonly Shell[] = ["sh", "ps"];
const COMMAND: PerShell = { sh: INSTALL_SH, ps: INSTALL_PS1 };
const SCRIPT_URL: PerShell = { sh: INSTALL_SCRIPT_URL, ps: INSTALL_PS1_SCRIPT_URL };
// The check and "Copied" confirm the copy for a few seconds. The keys notice is only good until the keys are pressed.
const COPIED_MS = 6000;
const KEYS_MS = 6000;
// Split after each run of slashes, so a narrow box wraps on path separators but never inside "https://".
const AFTER_SLASHES = /(?<=\/)(?!\/)/;

// The shell lives on <html data-shell>: InstallOptions' inline script sets it before first paint (PowerShell
// for Windows visitors), the tabs flip it, and CSS shows the matching command and pressed tab. So hydration changes
// nothing visible, and the hero's and the final call to action's copies of this island follow each other.
function currentShell(): Shell {
  return document.documentElement.dataset.shell === "ps" ? "ps" : "sh";
}
function onShellChange(notify: () => void) {
  const observer = new MutationObserver(notify);
  observer.observe(document.documentElement, { attributeFilter: ["data-shell"] });
  return () => observer.disconnect();
}
const serverShell = (): Shell => "sh";
const isApple = () => /Mac|iPhone|iPad|iPod/.test(navigator.userAgent);
// A line's `code` spans (copy strings mark them with backticks) show as code.
const inline = (text: string) => renderInline(text).map((s, i) => (s.kind === "code" ? <code key={i}>{s.text}</code> : s.kind === "text" ? s.text : null));

function Command({ text }: { text: string }) {
  return <>{text.split(AFTER_SLASHES).map((part, i) => <Fragment key={i}>{i > 0 && <wbr />}{part}</Fragment>)}</>;
}

// The command-line way in: a tab per system (after what it is and what it's for, where the page doesn't already
// say); then the command, which copies on a click anywhere along it; under it, what to do with it.
export default function InstallCommand({ t }: { t: T }) {
  const shell = useSyncExternalStore(onShellChange, currentShell, serverShell);
  const [notice, setNotice] = useState<"" | "copied" | "keys">("");
  const tileRef = useRef<HTMLButtonElement>(null);
  const codeRefs = useRef<Partial<Record<Shell, HTMLElement>>>({});
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const clear = () => {
    clearTimeout(timer.current);
    setNotice("");
  };
  useEffect(() => {
    const stop = tileRef.current ? onWayLeft(tileRef.current, clear) : undefined;
    return () => {
      stop?.();
      clearTimeout(timer.current);
    };
  }, []);

  const pick = (k: Shell) => {
    document.documentElement.dataset.shell = k;
    // A check or keys notice would now speak of the other command.
    clear();
  };
  const copy = async () => {
    if (tileRef.current) announceWayUsed(tileRef.current);
    clearTimeout(timer.current);
    try {
      await navigator.clipboard.writeText(COMMAND[shell]);
    } catch {
      // No clipboard (insecure context, denied permission): select the command so ⌘C / Ctrl+C works.
      const code = codeRefs.current[shell];
      if (code) window.getSelection()?.selectAllChildren(code);
      setNotice("keys");
      timer.current = setTimeout(() => setNotice(""), KEYS_MS);
      return;
    }
    setNotice("copied");
    timer.current = setTimeout(() => setNotice(""), COPIED_MS);
  };

  const done = notice === "copied";
  const keys = isApple() ? t.copyKeys.mac : t.copyKeys.other;
  return (
    <div className="cli">
      <div className="cap">
        {t.kind && <p className="cap-text"><strong>{t.kind}</strong> <span>{inline(t.kindDesc ?? "")}</span></p>}
        {/* Without JavaScript the tabs can't swap commands: the sh command, its line and its script link stay. */}
        <span className="shells js-only" role="group" aria-label={t.shellsLabel}>
          {SHELLS.map((k) => (
            <button key={k} type="button" className="shell tap-target" data-shell-tab={k} aria-pressed={shell === k} onClick={() => pick(k)}>{t.shells[k]}</button>
          ))}
        </span>
      </div>
      <div className="cli-box">
        <button ref={tileRef} type="button" className="way" data-way="cli" onClick={copy}>
          {/* The name leads with the verb (WCAG 2.5.3); the word at the end is the same verb, drawn for the eye. */}
          <span className="sr-only">{t.copy}</span>{" "}
          {SHELLS.map((k) => (
            <code key={k} data-show={k} ref={(el) => { codeRefs.current[k] = el ?? undefined; }}><Command text={COMMAND[k]} /></code>
          ))}
          {/* Both words in one cell, so the button keeps its width when the word changes. */}
          <span className="cli-copy" aria-hidden="true">
            {done ? <Check size={14} /> : <Copy size={14} />}
            <span className="cli-words"><span className={done ? "off" : undefined}>{t.copy}</span><span className={done ? undefined : "off"}>{t.copyDone}</span></span>
          </span>
        </button>
      </div>
      <p className="meta" data-notice={notice === "keys" ? notice : undefined}>
        {SHELLS.map((k) => (
          <span key={k} data-show={k} className="facts">
            {inline(t.next[k])}{" · "}<a href={SCRIPT_URL[k]} className="script tap-target">{t.viewScript}&nbsp;<span aria-hidden="true">↗</span></a>
          </span>
        ))}
        {/* A copy is confirmed on the button and spoken here; the keys to press show, as the next thing to do. */}
        <span role="status" className={done ? "notice sr-only" : "notice"}>{done ? t.copyDone : notice === "keys" ? keys : ""}</span>
      </p>
    </div>
  );
}
