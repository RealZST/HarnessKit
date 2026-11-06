import { useRef, useState, type CSSProperties } from "react";
import { Check, ChevronRight, RefreshCw, Shield } from "lucide-react";
import type { CopyShape } from "../../../data/copy/en";
import { MOCK_AUDIT, SEVERITY_TONE, type MockAuditItem } from "./mock-data";
import MockCursor, { HIDDEN_CURSOR, cursorPointAt, type CursorState } from "./MockCursor";
import MockWindow from "./MockWindow";
import Score from "./Score";
import { useMockCycle } from "./use-mock-cycle";

type Strings = CopyShape["mockLabels"]["audit"];
/** The App's page before its first audit, while it runs (`loading`), and with results. */
type Phase = "ready" | "running" | "done";
/** One run and its rest, as the App's onboarding windows loop. */
export const PERIOD = 7400;

// A first audit, drawn as the App's onboarding draws its Security Audit page: the cursor clicks Run Audit, the scores
// land lowest first, and it opens the lowest to show what lowered it: each rule with its severity and where it was
// found, the hand-drawn note drawing itself in beside them. The run takes under 5 s; the window rests there, the
// results fade out and it starts over, for as long as it is on-screen. A hidden copy of the finished list, row 0 open,
// shares one grid cell with the live one: the window always has the finished state's height at any width or font
// size, so nothing in the run moves the page.
export default function AuditMock({ t }: { t: Strings }) {
  const winRef = useRef<HTMLDivElement>(null);
  const runRef = useRef<HTMLSpanElement>(null);
  const firstRowRef = useRef<HTMLDivElement>(null);
  const [phase, setPhase] = useState<Phase>("ready");
  const [expanded, setExpanded] = useState(-1);
  const [leaving, setLeaving] = useState(false);
  const [cursor, setCursor] = useState<CursorState>(HIDDEN_CURSOR);
  const press = (clicking: boolean) => setCursor((c) => ({ ...c, clicking }));

  const reduced = useMockCycle([
    [0, () => { setPhase("ready"); setExpanded(-1); setLeaving(false); setCursor(HIDDEN_CURSOR); }],
    [300, () => setCursor({ ...cursorPointAt(winRef.current, runRef.current, 0.5, 0.6), visible: true, clicking: false })],
    [900, () => press(true)],
    [1050, () => { press(false); setPhase("running"); }],
    [2050, () => setPhase("done")],
    [2650, () => setCursor({ ...cursorPointAt(winRef.current, firstRowRef.current, 0.3, 0.6), visible: true, clicking: false })],
    [3200, () => press(true)],
    [3350, () => { press(false); setExpanded(0); }],
    [4200, () => setCursor((c) => ({ ...c, visible: false }))],
    [PERIOD - 400, () => setLeaving(true)],
  ], winRef, PERIOD);
  // Reduced motion shows where the run ends: the scores in, the first row open, no cursor.
  const shown: Phase = reduced ? "done" : phase;
  const open = reduced ? 0 : expanded;

  return (
    <MockWindow ref={winRef} title={t.title} label={t.label} className={leaving ? "mockwin-app mockwin-audit is-leaving" : "mockwin-app mockwin-audit"} focusable={!reduced}>
      <div className="mock-audit-head">
        {/* Both labels share one cell, so the pill keeps its width and the line never rewraps. */}
        <span className={shown === "running" ? "mock-run is-running" : "mock-run"}>
          <RefreshCw className="mock-run-icon" aria-hidden="true" />
          <span className="mock-run-labels"><span>{t.run}</span><span>{t.auditing}</span></span>
        </span>
        {/* Only once there are results, as in the App; it keeps its room before then. */}
        <span className={shown === "done" ? "mock-meta mock-results" : "mock-meta mock-results mock-unseen"}>{t.scanned.replace("{count}", String(MOCK_AUDIT.length))} · {t.lastRun}</span>
      </div>
      <div className="mock-audit-stack">
        <div className={shown === "done" ? "mock-audit-list mock-audit-live mock-results" : "mock-audit-list mock-audit-live mock-results is-empty"}>
          {shown === "ready" && (
            <div className="mock-audit-state">
              <span className="mock-state-title">{t.readyTitle}</span>
              <span className="mock-meta">{t.readyBody}</span>
              <span ref={runRef} className="mock-pill mock-run-primary"><Shield size={11} aria-hidden="true" />{t.run}</span>
            </div>
          )}
          {shown === "running" && (
            <div className="mock-audit-state">
              <span className="mock-state-title">{t.running}</span>
              <span className="mock-meta">{t.scanning}</span>
              <span className="mock-bar"><span /></span>
            </div>
          )}
          {shown === "done" && MOCK_AUDIT.map((item, i) => (
            <AuditRow key={item.name} t={t} item={item} index={i} open={i === open} note={i === 0} rowRef={i === 0 ? firstRowRef : undefined} />
          ))}
        </div>
        <div className="mock-audit-list mock-audit-reserve" aria-hidden="true">
          {MOCK_AUDIT.map((item, i) => <AuditRow key={item.name} t={t} item={item} index={i} open={i === 0} note={i === 0} />)}
        </div>
      </div>
      {!reduced && <MockCursor {...cursor} />}
    </MockWindow>
  );
}

// `index` staggers the rows as the scores land.
interface RowProps { t: Strings; item: MockAuditItem; index: number; open: boolean; note: boolean; rowRef?: React.Ref<HTMLDivElement> }

// The onboarding's rows: a clean one is a check, its name and Clean; one with findings opens to its rules, each with
// its severity and, under it, where it was found.
function AuditRow({ t, item, index, open, note, rowRef }: RowProps) {
  const land = { "--i": index } as CSSProperties;
  if (item.findings.length === 0) {
    return (
      <div className="mock-row mock-landing" style={land}>
        <Check className="mock-check" aria-hidden="true" />
        <span className="mock-name mock-muted">{item.name}</span>
        <span className="mock-meta">{t.clean}</span>
      </div>
    );
  }
  const count = item.findings.length;
  return (
    <div className={open ? "mock-audit-item mock-landing open" : "mock-audit-item mock-landing"} style={land}>
      <div ref={rowRef} className="mock-row">
        <ChevronRight className="mock-chevron" aria-hidden="true" />
        <span className="mock-name">{item.name}</span>
        <span className="mock-meta">{(count === 1 ? t.finding : t.findings).replace("{count}", String(count))}</span>
        <Score score={item.score} />
      </div>
      {open && (
        <div className="mock-findings">
          <div className="mock-rules">
            {item.findings.map((finding) => (
              <div key={finding.rule} className="mock-rule">
                <span className="mock-rule-head">
                  <span>{t.rules[finding.rule]}</span>
                  <span className="mock-pill mock-tinted mock-sev" style={{ "--tone": SEVERITY_TONE[finding.severity] } as CSSProperties}>
                    <span className="mock-dot" />{t.severity[finding.severity]}
                  </span>
                </span>
                <span className="mock-loc">{finding.location}</span>
              </div>
            ))}
          </div>
          {note && <Note text={t.note} />}
        </div>
      )}
    </div>
  );
}

// The hand-drawn note on this screen: a pen arrow and a line in the accent, drawn in as the row opens. It sits
// beside the findings, or under them when the window is narrow; the arrow points left and up at them either way.
function Note({ text }: { text: string }) {
  return (
    <p className="mock-note">
      <svg className="mock-note-arrow" viewBox="0 0 40 24" fill="none" aria-hidden="true">
        <path d="M38 19.5c-8 1.6-17.5.6-25.2-5.2-2.2-1.7-4.4-3.9-6.6-6.6" />
        <path d="M6.2 7.7l.5 6.5M6.2 7.7l6.2 1.8" />
      </svg>
      <span className="mock-note-text">{text}</span>
    </p>
  );
}
