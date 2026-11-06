import { useRef, useState, type CSSProperties } from "react";
import { Check, ChevronDown, Database, Download, File, Globe, Loader2, ShieldCheck, Terminal, X } from "lucide-react";
import { AGENTS } from "../../../data/agents";
import type { CopyShape } from "../../../data/copy/en";
import { centerInk } from "../../../data/mascot-ink";
import { Mascot } from "../../shared/Mascot";
import { MOCK_EXTENSIONS, type MockExtension, type MockPerm } from "./mock-data";
import MockCursor, { HIDDEN_CURSOR, cursorPointAt, type CursorState } from "./MockCursor";
import MockWindow from "./MockWindow";
import Score from "./Score";
import { useMockCycle } from "./use-mock-cycle";

type Strings = CopyShape["mockLabels"]["extensions"];

// Kinds keep their English names in both languages, as the App's zh locale does (common.json kinds): the kind
// filter's chips after All, and the badges.
const FILTER_KINDS = ["skill", "MCP", "plugin", "hook", "CLI"];
const KIND_NAMES = { skill: "skill", mcp: "MCP", hook: "hook" } satisfies Record<MockExtension["kind"], string>;
const PERM_ICONS = { filesystem: File, network: Globe, shell: Terminal, database: Database } satisfies Record<MockPerm, typeof File>;
// In this order on every row, so the same icon lines up down the column.
const PERM_ORDER = Object.keys(PERM_ICONS) as MockPerm[];
const MASCOT = 18;
const TILE_MASCOT = 14;
/** One run and its rest, as the App's onboarding windows loop. */
export const PERIOD = 8000;

// Each mascot names its agent on hover. Agents go in the App's order (AGENT_ORDER), as it lists them.
const nameOf = (id: string) => AGENTS.find((a) => a.id === id)?.name ?? id;
const rank = (id: string) => AGENTS.findIndex((a) => a.id === id);
const inOrder = (ids: readonly string[]) => [...ids].sort((a, b) => rank(a) - rank(b));
/** The row the story opens, and the agent it installs it to. */
const STORY = MOCK_EXTENSIONS.find((ext) => ext.addTo)!;
const TARGET = STORY.addTo!;
/** Its agents once installed. */
const AFTER = inOrder([...STORY.agents, TARGET]);
/** The detail panel's tiles: every agent the list shows, as the App shows each agent it found. */
export const TILE_AGENTS = inOrder([...new Set(MOCK_EXTENSIONS.flatMap((ext) => [...ext.agents, ...(ext.addTo ? [ext.addTo] : [])]))]);

type Install = "idle" | "installing" | "done";
/** The detail panel: not there, sliding in and open, or sliding out. It always takes its room (mocks.css). */
type Panel = "closed" | "open" | "closing";

// A landing-only Extensions page, drawn like the App's onboarding: the kind filter, then a row per extension with its
// kind, the agents that have it, what it may touch, its score, on or off. The story is the App's install to another
// agent (src/pages/extensions.tsx, components/extensions/extension-detail.tsx): the cursor opens a hook (mock-data.ts says
// which, and why that one), its detail
// panel slides in over the table's right side, the cursor clicks Codex's tile under Install to Agent, it installs, and
// Codex joins the panel's Agents and the row behind, with the App's toast. The window rests, the panel slides out to
// show the row with Codex, the changes fade and it starts over, for as long as it is on-screen.
export default function ExtensionsMock({ t }: { t: Strings }) {
  const winRef = useRef<HTMLDivElement>(null);
  const rowRef = useRef<HTMLSpanElement>(null);
  const tileRef = useRef<HTMLSpanElement>(null);
  const [selected, setSelected] = useState(false);
  const [panel, setPanel] = useState<Panel>("closed");
  const [install, setInstall] = useState<Install>("idle");
  const [flash, setFlash] = useState(false);
  const [leaving, setLeaving] = useState(false);
  const [cursor, setCursor] = useState<CursorState>(HIDDEN_CURSOR);
  const press = (clicking: boolean) => setCursor((c) => ({ ...c, clicking }));
  const point = (target: HTMLElement | null, fx: number) => setCursor({ ...cursorPointAt(winRef.current, target, fx, 0.6), visible: true, clicking: false });

  const reduced = useMockCycle([
    [0, () => { setSelected(false); setPanel("closed"); setInstall("idle"); setFlash(false); setLeaving(false); setCursor(HIDDEN_CURSOR); }],
    [700, () => point(rowRef.current, 0.5)],
    [1300, () => press(true)],
    [1400, () => { press(false); setSelected(true); setPanel("open"); }],
    [2100, () => point(tileRef.current, 0.45)],
    [2700, () => press(true)],
    [2800, () => { press(false); setInstall("installing"); }],
    // The App flashes the tile it just installed to.
    [3500, () => { setInstall("done"); setFlash(true); }],
    [4300, () => { setFlash(false); setCursor((c) => ({ ...c, visible: false })); }],
    [6500, () => { setPanel("closing"); setSelected(false); }],
    [PERIOD - 400, () => setLeaving(true)],
  ], winRef, PERIOD);
  // Reduced motion shows where the run ends: the panel open on PDF tools, Codex installed, no cursor.
  const open: Panel = reduced ? "open" : panel;
  const installed: Install = reduced ? "done" : install;
  const toast = installed === "done";
  const targetName = nameOf(TARGET);
  const panelAgents = installed === "done" ? AFTER : inOrder(STORY.agents);

  return (
    <MockWindow ref={winRef} title={t.title} label={t.label} className={leaving ? "mockwin-app mockwin-ext is-leaving" : "mockwin-app mockwin-ext"} focusable={!reduced}>
      {toast && <span className="mock-toast"><Check size={12} strokeWidth={2.5} aria-hidden="true" />{t.installed.replace("{agent}", targetName)}</span>}
      <div className="mock-chips">
        <span className="mock-chip on">{t.all}</span>
        {FILTER_KINDS.map((kind) => <span key={kind} className="mock-chip">{kind}</span>)}
      </div>
      {/* The table and the panel share one cell: the window is always as tall as the taller, so opening the panel
          moves nothing. */}
      <div className="mock-ext-stage">
        {/* Narrow windows drop Permissions, then Score, then give Kind's badge to the name (mocks.css). */}
        <table className="mock-table">
          <thead>
            <tr>
              <th>{t.name}</th><th className="mock-col-kind">{t.kind}</th><th>{t.agents}</th>
              <th className="mock-col-perms">{t.perms}</th><th className="mock-col-score">{t.score}</th><th>{t.status}</th>
            </tr>
          </thead>
          <tbody>
            {MOCK_EXTENSIONS.map((ext) => {
              const story = ext === STORY;
              const kind = <KindBadge kind={ext.kind} />;
              return (
                <tr key={ext.name} className={story && (reduced || selected) ? "is-selected" : undefined}>
                  <td><span ref={story ? rowRef : undefined} className="mock-ext-name">{ext.name}</span><span className="mock-kind-under">{kind}</span></td>
                  <td className="mock-col-kind">{kind}</td>
                  <td>
                    <span className="mock-agents">
                      {inOrder(story && installed === "done" ? [...ext.agents, TARGET] : ext.agents).map((a) => (
                        // Each drawing on the row's middle line, as the words beside it are (data/mascot-ink.ts).
                        <span key={a} className={story && a === TARGET ? "mock-new" : undefined} title={nameOf(a)} style={{ translate: centerInk(a, MASCOT) }}><Mascot name={a} size={MASCOT} /></span>
                      ))}
                    </span>
                  </td>
                  <td className="mock-col-perms">
                    <span className="mock-perms">
                      {PERM_ORDER.filter((p) => ext.perms.includes(p)).map((p) => {
                        const Icon = PERM_ICONS[p];
                        return <span key={p} title={t.permNames[p]}><Icon aria-hidden="true" /></span>;
                      })}
                    </span>
                  </td>
                  <td className="mock-col-score"><Score score={ext.score} /></td>
                  <td><span className={ext.enabled ? "mock-pill mock-status on" : "mock-pill mock-status"}>{ext.enabled ? t.enabled : t.disabled}</span></td>
                </tr>
              );
            })}
          </tbody>
        </table>
        <div className={`mock-detail is-${open}`}>
          <div className="mock-detail-head">
            <span className="mock-detail-name">{STORY.name}</span>
            <KindBadge kind={STORY.kind} />
            <X className="mock-detail-close" aria-hidden="true" />
          </div>
          {/* The description, as the onboarding draws text it doesn't spell out. */}
          <span className="mock-skel" /><span className="mock-skel short" />
          <span className="mock-on">{t.on}</span>
          <span className="mock-heading">{t.agentsHeading}</span>
          {/* With a hidden copy of the chips after the install under them, so a chip that wraps moves nothing. */}
          <span className="mock-agent-chips">
            <span>{panelAgents.map((a) => <span key={a} className={a === TARGET ? "mock-agent-chip mock-new-chip" : "mock-agent-chip"}>{nameOf(a)}</span>)}</span>
            <span aria-hidden="true">{AFTER.map((a) => <span key={a} className="mock-agent-chip">{nameOf(a)}</span>)}</span>
          </span>
          <span className="mock-heading">{t.installTo}</span>
          <span className="mock-scope"><span>{t.scope}</span><span className="mock-select">{t.global}<ChevronDown aria-hidden="true" /></span></span>
          <span className="mock-tiles">
            {TILE_AGENTS.map((a) => {
              const target = a === TARGET;
              const has = STORY.agents.includes(a) || (target && installed === "done");
              const state = has ? (target && flash ? "flash" : "has") : target && installed === "installing" ? "busy" : "can";
              const Icon = has ? ShieldCheck : state === "busy" ? Loader2 : Download;
              return (
                <span key={a} ref={target ? tileRef : undefined} className={`mock-tile is-${state}`}>
                  <span className="mock-tile-mascot" style={{ translate: centerInk(a, TILE_MASCOT) }}><Mascot name={a} size={TILE_MASCOT} /></span>{nameOf(a)}<Icon className="mock-tile-icon" aria-hidden="true" />
                </span>
              );
            })}
          </span>
        </div>
      </div>
      {!reduced && <MockCursor {...cursor} />}
    </MockWindow>
  );
}

// The App's kind badge: a pill tinted with the kind's color.
function KindBadge({ kind }: { kind: MockExtension["kind"] }) {
  return <span className="mock-kind" style={{ "--tone": `var(--kind-${kind})` } as CSSProperties}>{KIND_NAMES[kind]}</span>;
}
