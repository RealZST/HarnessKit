import { type CSSProperties, type FocusEvent, type KeyboardEvent, type PointerEvent, useEffect, useId, useRef, useState } from "react";
import { AGENTS } from "../../data/agents";
import { CARD_MASCOT_SIZE, CLIPPED_CARDS, MASCOT_CLIPS, cardName, clickDuration } from "../../data/agent-card";
import type { CopyShape } from "../../data/copy/en";
import { Mascot } from "../shared/Mascot";

// Where each card starts before the gather, in px and degrees: the App's onboarding welcome
// (src/components/onboarding/onboarding.tsx, StepWelcome), as the hero mascot row had it.
export const SCATTER: Record<string, { x: number; y: number; r: number }> = {
  claude: { x: -140, y: -80, r: -15 }, codex: { x: 100, y: -90, r: 12 }, gemini: { x: 160, y: 50, r: -8 },
  cursor: { x: -120, y: 70, r: 10 }, antigravity: { x: -180, y: 10, r: -20 }, copilot: { x: 150, y: 80, r: 15 },
  windsurf: { x: 0, y: 108, r: -6 }, opencode: { x: 210, y: -10, r: 8 }, hermes: { x: -60, y: -115, r: 18 },
  kiro: { x: 40, y: -126, r: -10 }, omp: { x: -200, y: 60, r: 22 }, dsh: { x: 90, y: 135, r: 7 },
  grok: { x: -30, y: 145, r: -12 }, "qoder-cn": { x: 180, y: 100, r: -14 }, openclaw: { x: -170, y: -110, r: 16 },
  penguin: { x: -130, y: 145, r: 16 }, pi: { x: -80, y: 160, r: -18 },
};

interface Props { t: CopyShape["overview"] }

// Arrow keys move along the cards in AGENTS order (Left, Right), to the nearest card in the row above or below (Up,
// Down: rows depend on the screen, so it goes by where the cards are), or to the first or last (Home, End).
function step(key: string, from: number, cards: readonly HTMLElement[]): number | undefined {
  if (key === "ArrowLeft") return Math.max(from - 1, 0);
  if (key === "ArrowRight") return Math.min(from + 1, cards.length - 1);
  if (key === "Home") return 0;
  if (key === "End") return cards.length - 1;
  if (key !== "ArrowUp" && key !== "ArrowDown") return undefined;
  const dir = key === "ArrowDown" ? 1 : -1;
  const rects = cards.map((card) => card.getBoundingClientRect());
  const here = rects[from];
  const centre = (r: DOMRect) => r.left + r.width / 2;
  // Same row: tops within half a card, as the card in view sits 2px higher than its row.
  const sameRow = (a: DOMRect, b: DOMRect) => Math.abs(a.top - b.top) < here.height / 2;
  let row: DOMRect | undefined;
  let best: number | undefined;
  for (let i = from + dir; i >= 0 && i < cards.length; i += dir) {
    if (sameRow(rects[i], here)) continue;
    row ??= rects[i];
    if (!sameRow(rects[i], row)) break;
    if (best === undefined || Math.abs(centre(rects[i]) - centre(here)) < Math.abs(centre(rects[best]) - centre(here))) best = i;
  }
  return best ?? from;
}

// The App's Overview page, one card per agent, with the App's own mascot interactions: hover or keyboard focus
// plays a card's loop, a click (Enter, Space, tap) its one-shot. The cards are one Tab stop, the arrow keys move
// between them (roving tabindex). While the keyboard drives the cards, a mouse resting on another card stays out of
// it until it moves: the focused card is the one in view.
export default function OverviewLive({ t }: Props) {
  const [hovered, setHovered] = useState<string | null>(null);
  const [focused, setFocused] = useState<string | null>(null);
  // The card that Tab reaches: the first until another takes focus.
  const [current, setCurrent] = useState(AGENTS[0].id);
  // The keyboard moved focus last; a moving pointer takes over again (onPointerMove).
  const [keys, setKeys] = useState(false);
  const [clicked, setClicked] = useState<ReadonlySet<string>>(new Set());
  const [gathered, setGathered] = useState(false);
  const clickTimers = useRef(new Map<string, ReturnType<typeof setTimeout>>());
  const gridRef = useRef<HTMLUListElement>(null);
  const idPrefix = useId();
  const keysId = `${idPrefix}-keys`;
  const shown = keys ? focused ?? hovered : hovered ?? focused;

  // The section's inline script marks it data-gather before the cards first paint, unless the visitor asked for
  // reduced motion; the cards then wait, scattered, until the window is in view.
  useEffect(() => {
    const grid = gridRef.current;
    if (!grid?.closest("[data-gather]")) return;
    const io = new IntersectionObserver((entries) => {
      if (!entries.some((e) => e.isIntersecting)) return;
      setGathered(true);
      io.disconnect();
    // As soon as the grid's top is 40px into view: on a phone the grid is taller than the screen, and a share of it
    // (the old 0.2) left an empty window at the fold.
    }, { threshold: 0, rootMargin: "0px 0px -40px 0px" });
    io.observe(grid);
    return () => io.disconnect();
  }, []);

  useEffect(() => {
    const timers = clickTimers.current;
    return () => timers.forEach(clearTimeout);
  }, []);

  // A click while the animation still runs changes nothing, as in the App: the class stays on until its track ends.
  const click = (id: string) => {
    if (clickTimers.current.has(id)) return;
    setClicked((ids) => new Set(ids).add(id));
    clickTimers.current.set(id, setTimeout(() => {
      clickTimers.current.delete(id);
      setClicked((ids) => new Set([...ids].filter((other) => other !== id)));
    }, clickDuration(id)));
  };
  // Keyboard focus only: a mouse click focuses the button too, and its loop would then outlast the pointer. An arrow
  // key's move counts as keyboard focus whatever the browser makes of a scripted focus().
  const arrowed = useRef(false);
  const focus = (id: string, el: HTMLElement) => {
    const byKeys = arrowed.current || el.matches(":focus-visible");
    setCurrent(id);
    setFocused(byKeys ? id : null);
    if (byKeys) setKeys(true);
    arrowed.current = false;
  };
  // A real move only: a browser may send a still pointer's events when the page scrolls under it, as a focused card
  // scrolls into view.
  const pointerMoved = (e: PointerEvent<HTMLUListElement>) => {
    if (keys && (e.movementX || e.movementY)) setKeys(false);
  };
  const onKey = (e: KeyboardEvent<HTMLUListElement>) => {
    if (e.key === "Escape") {
      setHovered(null);
      setFocused(null);
      return;
    }
    const cards = [...e.currentTarget.querySelectorAll<HTMLElement>(".ov-card")];
    const from = cards.indexOf(document.activeElement as HTMLElement);
    const to = from < 0 ? undefined : step(e.key, from, cards);
    if (to === undefined) return;
    e.preventDefault();
    arrowed.current = to !== from;
    cards[to].focus();
  };
  const clearWhenFocusLeaves = (e: FocusEvent<HTMLUListElement>) => {
    if (e.currentTarget.contains(e.relatedTarget as Node | null)) return;
    setFocused(null);
    setKeys(false);
  };

  return (
    <div className="ov" onMouseLeave={() => setHovered(null)}>
      <div className="ov-wall">
        {/* role="list": Safari drops list semantics from a list-style: none list. */}
        {/* data-keys: CSS drops the hover look while the keyboard drives the cards. */}
        <ul ref={gridRef} className={gathered ? "ov-grid is-gathered" : "ov-grid"} role="list" aria-label={t.listLabel}
          aria-describedby={keysId} data-keys={keys || undefined} onKeyDown={onKey} onBlur={clearWhenFocusLeaves}
          onPointerMove={pointerMoved} onPointerDown={() => setKeys(false)}>
          {AGENTS.map((a, i) => {
            const s = SCATTER[a.id] ?? { x: 0, y: 0, r: 0 };
            const clip = MASCOT_CLIPS[a.id];
            const on = shown === a.id;
            const cardClass = ["ov-card", on && "on", CLIPPED_CARDS.has(a.id) && "clip"].filter(Boolean).join(" ");
            return (
              <li key={a.id} className="ov-cell" style={{ "--sx": `${s.x}px`, "--sy": `${s.y}px`, "--sr": `${s.r}deg`, "--i": i } as CSSProperties}>
                <button type="button" className={cardClass} tabIndex={a.id === current ? 0 : -1}
                  onMouseEnter={() => setHovered(a.id)} onFocus={(e) => focus(a.id, e.currentTarget)} onClick={() => click(a.id)}>
                  <span className={clip ? "ov-mascot clip" : "ov-mascot"} style={clip ? { "--clip-top": `${clip.top}px` } as CSSProperties : undefined}>
                    <Mascot name={a.id} size={CARD_MASCOT_SIZE} animated={on} clicked={clicked.has(a.id)} />
                  </span>
                  {/* The App's card also counts the agent's extensions; a sample count would mean nothing to a visitor. */}
                  <span className="ov-name">{cardName(a)}</span>
                </button>
              </li>
            );
          })}
        </ul>
      </div>
      {/* The hint that fits the device; while a card has keyboard focus, the arrow-key hint in its place, which describes
          the list for screen readers either way. */}
      <p className="ov-caption">
        <span className="ov-hint-pointer">{t.hint}</span><span className="ov-hint-touch">{t.hintTouch}</span><span id={keysId} className="ov-keys">{t.hintKeys}</span>
      </p>
    </div>
  );
}
